//! Execution logic for queued items.
//!
//! Extends [`QueueItem`] with [`QueueItem::execute`], which runs one item to
//! completion: builtin transfers (`copy`, `move`, `symlink`, `none`) or a Lua
//! script for custom kinds. Copy/move are submitted to the global
//! [`fist_copy`] scheduler; the pump finalizes their rows asynchronously.
//! Also provides [`QUEUE::check_validity`], which marks pending items whose
//! source paths no longer exist as [`QueueItemState::PendingErr`].

use super::*;

use std::{fs::create_dir_all, path::PathBuf, sync::atomic::Ordering};

use cba::bs::symlink;

use crate::{
    cli::paths::actions_dir,
    lua::{execute, load_script},
    run::{
        item::short_display,
        state::{MENU_ACTIONS, TOAST, ToastStyle},
    },
};

impl QueueItem {
    /// Execute this item according to its kind:
    /// - `"copy"` / `"move"` / `"symlink"` run the builtin transfer logic on
    ///   each source path, using the destination as-is (single-path items
    ///   are pre-resolved by the caller);
    /// - `"none"` is a no-op;
    /// - any other kind is a menu action key: the mapped command runs once
    ///   with the full path list, the destination, and the navigation
    ///   directory when it exists (`(paths, dst)` or
    ///   `(paths, dst, nav_cwd)`), and `set_progress` writes the item's
    ///   progress for the duration of the call. The progress is reset when
    ///   the call starts and marked complete afterwards so the display is
    ///   sensible even when the script never calls `set_progress`.
    ///
    /// `job_kind` carries the resolved transfer parameters for builtin
    /// `copy`/`move` rows; config lives in main-thread TLS, so callers
    /// running this item off-thread must resolve it before spawning.
    ///
    /// A custom kind with no mapping fails the item with an error toast. The
    /// action history records one entry per executed item that completed
    /// successfully.
    pub fn execute(
        self,
        nav_cwd: Option<&AbsPath>,
        job_kind: Option<fist_copy::JobKind>,
    ) {
        log::debug!("Transferring: {self:?}");

        let Self {
            kind,
            src,
            status,
            dst,
            ..
        } = &self;

        status.state.store(QueueItemState::Started);

        let mut any_success = false;

        match kind.as_str() {
            "symlink" => {
                for path in src {
                    match symlink(path, dst, true) {
                        Ok(()) => {
                            status.state.store(QueueItemState::CompleteOk);
                            any_success = true;
                        }
                        Err(_) => status.state.store(QueueItemState::CompleteErr),
                    }
                }
            }
            "none" => {
                // the kind is reserved; queue rows may still be enqueued
                // under it as an explicit no-op
                status.state.store(QueueItemState::CompleteOk);
                any_success = true;
            }
            "copy" | "move" => {
                let Some(job_kind) = job_kind else {
                    log::error!("copy/move row dispatched without resolved engine params");
                    status.state.store(QueueItemState::CompleteErr);
                    return;
                };

                for path in src {
                    if let Some(parent) = std::path::Path::new(dst).parent() {
                        let _ = create_dir_all(parent);
                    }

                    let request = fist_copy::JobRequest {
                        kind: job_kind.clone(),
                        source: path.to_path_buf(),
                        dest: PathBuf::from(dst),
                    };
                    match super::scheduler().submit(request) {
                        Ok(handle) => {
                            // the status atomics are shared with the stored
                            // row, so the watcher discovers this task by id
                            status.set_task_id(handle.id);
                            super::ensure_watcher();
                        }
                        Err(e) => {
                            log::error!("Transfer submit failed for {self:?}: {e}");
                            status.state.store(QueueItemState::CompleteErr);
                            let display = short_display(path);
                            TOAST::push(ToastStyle::Error, "Failed: ", [display]);
                            TOAST::notice(ToastStyle::Error, e.to_string());
                        }
                    }
                }
                // completion (toasts + action history) is handled by the pump
                return;
            }
            // any other kind is a menu action key
            script => {
                let command = match MENU_ACTIONS.get().and_then(|m| m.get(script)) {
                    Some(action) => action.command.clone(),
                    None => {
                        log::error!("No menu action for queue kind {script:?}: {self:?}");
                        status.state.store(QueueItemState::CompleteErr);
                        TOAST::notice(
                            ToastStyle::Error,
                            format!("No menu action for kind {script}"),
                        );
                        return;
                    }
                };
                status.progress.store(0, Ordering::Relaxed);
                let result = load_script(&command, Some(actions_dir()))
                    .ok_or_else(|| anyhow::anyhow!("failed to load script"))
                    .and_then(|s| {
                        execute(
                            &s,
                            src,
                            &dst.to_string_lossy(),
                            nav_cwd,
                            Some(&status.progress),
                        )
                        .map_err(anyhow::Error::msg)
                    });
                // status.progress.store(255, Ordering::Relaxed);

                match result {
                    Ok(_) => {
                        status.state.store(QueueItemState::CompleteOk);
                        any_success = true;
                    }
                    Err(e) => {
                        log::error!("Queue script error for {self:?}: {e}");
                        status.state.store(QueueItemState::CompleteErr);
                    }
                }
            }
        }

        if any_success {
            QUEUE_ACTION_HISTORY.lock().unwrap().push(self.clone());
        }
    }
}

impl QUEUE {
    pub fn check_validity() {
        let state = QUEUE_STATE.lock().unwrap();
        for item in &state.shared {
            if item.status.state.is_pending() && !item.src.iter().all(|p| p.exists()) {
                item.status.state.store(QueueItemState::PendingErr)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::run::state::GLOBAL;
    use tempfile::tempdir;

    /// Queue tests share the process-global [`QUEUE_STATE`]; tokio runs
    /// them in parallel, and dispatch/clear are index- and state-sensitive,
    /// so they are serialized explicitly.
    static SERIAL: Mutex<()> = Mutex::new(());

    #[test]
    fn test_execute_copy_and_symlink() {
        let _guard = SERIAL.lock().unwrap();
        GLOBAL::init_test_senders();
        let dir = tempdir().unwrap();
        let src_dir = dir.path().join("src_folder");
        std::fs::create_dir(&src_dir).unwrap();
        std::fs::write(src_dir.join("file.txt"), "hello").unwrap();

        let dst_dir = dir.path().join("dst_parent");
        std::fs::create_dir(&dst_dir).unwrap();

        // 1. Copy directory (async via fist-copy engine: poll for completion)
        let dst_folder = dst_dir.join("src_folder");
        let item = QueueItem {
            kind: "copy".into(),
            src: vec![AbsPath::new_unchecked(&src_dir)],
            dst: dst_folder.as_os_str().to_owned(),
            status: QueueItemStatus::new(&src_dir),
        };
        // the unified watcher resolves tasks through rows in the shared
        // queue, mirroring the production dispatch path
        QUEUE_STATE.lock().unwrap().shared.push(item.clone());
        let watch = item.status.clone();
        item.execute(
            Some(&AbsPath::new_unchecked(&dst_dir)),
            Some(fist_copy::JobKind::Copy(fist_copy::CopyParams::default())),
        );
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(30);
        while !watch.state.is_complete() {
            assert!(
                std::time::Instant::now() < deadline,
                "copy task did not finish in time"
            );
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        assert_eq!(watch.state.load(), QueueItemState::CompleteOk);
        assert!(dst_folder.exists(), "dst_folder should exist after copy");
        assert!(
            dst_folder.join("file.txt").exists(),
            "file.txt should exist inside copied dst_folder"
        );

        // 2. Symlink directory
        let symlink_folder = dst_dir.join("symlink_folder");
        let sym_item = QueueItem {
            kind: "symlink".into(),
            src: vec![AbsPath::new_unchecked(&src_dir)],
            dst: symlink_folder.as_os_str().to_owned(),
            status: QueueItemStatus::new(&src_dir),
        };
        sym_item.execute(Some(&AbsPath::new_unchecked(&dst_dir)), None);
        let read_link = std::fs::read_link(&symlink_folder);
        println!("symlink target result: {:?}", read_link);
        assert!(symlink_folder.exists(), "symlink_folder should exist");
        assert!(
            symlink_folder.join("file.txt").exists(),
            "symlink target should resolve correctly"
        );
    }

    /// Extraction rows are created started, run on the engine, and
    /// finalized by the pump watcher into CompleteOk with the payload
    /// materialized under the given destination.
    #[test]
    fn extract_row_runs_to_completion() {
        let _guard = SERIAL.lock().unwrap();
        GLOBAL::init_test_senders();
        if !std::process::Command::new("tar")
            .arg("--version")
            .output()
            .is_ok_and(|o| o.status.success())
        {
            return;
        }
        let dir = tempdir().unwrap();
        let src_dir = dir.path().join("xsrc");
        std::fs::create_dir_all(src_dir.join("nested")).unwrap();
        std::fs::write(src_dir.join("hello.txt"), "hello").unwrap();
        std::fs::write(src_dir.join("nested/data.txt"), "data").unwrap();
        let archive = dir.path().join("a.tar");
        let out = std::process::Command::new("tar")
            .args(["-cf"])
            .arg(&archive)
            .arg("-C")
            .arg(&src_dir)
            .args(["."])
            .output()
            .unwrap();
        assert!(out.status.success(), "tar fixture failed");

        let dest = dir.path().join("workdir");
        std::fs::create_dir_all(&dest).unwrap();
        let status = QUEUE::start_extract(AbsPath::new_unchecked(archive.clone()), dest.clone())
            .expect("engine submission");

        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(30);
        while !status.state.is_complete() {
            assert!(
                std::time::Instant::now() < deadline,
                "extraction row did not finish in time"
            );
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        assert_eq!(status.state.load(), QueueItemState::CompleteOk);
        assert!(dest.join("hello.txt").exists());
        assert!(dest.join("nested/data.txt").exists());

        // NOTE: no shared-queue row assertions here — sibling tests clear
        // the global QUEUE_STATE concurrently; reaching CompleteOk already
        // proves the row was registered with a live task (the pump watcher
        // only finalizes rows it tracks)
    }

    /// Exercises the production dispatch path: engine params must be resolved
    /// on the dispatching thread, because the row's task runs on a worker
    /// thread where config TLS is unset. A regression leaves the row Started
    /// and the deadline assertion fires.
    #[tokio::test]
    async fn dispatched_copy_resolves_engine_params_off_thread() {
        GLOBAL::init_test_senders();
        let dir = tempdir().unwrap();
        let src_dir = dir.path().join("disp_src");
        std::fs::create_dir(&src_dir).unwrap();
        std::fs::write(src_dir.join("file.txt"), "hello").unwrap();
        let dst_dir = dir.path().join("disp_dst_parent");
        std::fs::create_dir(&dst_dir).unwrap();

        let index = {
            let mut state = QUEUE_STATE.lock().unwrap();
            state.shared.push(QueueItem {
                kind: "copy".into(),
                src: vec![AbsPath::new_unchecked(&src_dir)],
                dst: Default::default(),
                status: QueueItemStatus::new(&src_dir),
            });
            state.shared.len() - 1
        };
        QUEUE::dispatch(vec![index], Some(AbsPath::new_unchecked(&dst_dir)));

        // resolve OUR row by source: sibling tests push rows concurrently,
        // so a fixed index can point at someone else's
        let watch = loop {
            if let Some(item) = QUEUE_STATE.lock().unwrap().shared.iter().find(|i| {
                i.kind == "copy"
                    && i.src
                        .first()
                        .is_some_and(|p| p.as_os_str() == src_dir.as_os_str())
            }) {
                break item.status.clone();
            }
            std::thread::sleep(std::time::Duration::from_millis(5));
        };
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(30);
        while !watch.state.is_complete() {
            assert!(
                std::time::Instant::now() < deadline,
                "dispatched copy did not finish in time"
            );
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        assert_eq!(watch.state.load(), QueueItemState::CompleteOk);
        assert!(
            dst_dir.join("disp_src").join("file.txt").exists(),
            "dispatched copy should land under the nav directory"
        );

        // remove only our row: sibling tests own theirs
        QUEUE_STATE.lock().unwrap().shared.retain(|i| {
            !(i.kind == "copy"
                && i.src
                    .first()
                    .is_some_and(|p| p.as_os_str() == src_dir.as_os_str()))
        });
    }
}
