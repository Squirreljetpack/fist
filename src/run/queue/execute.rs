//! Execution logic for queued items.
//!
//! [`perform`] runs one row: builtin transfers (`copy`, `move`) are
//! submitted to the global [`fist_copy`] scheduler inline — cheap, and it
//! keeps them on the dispatching thread where config lives — while
//! blocking work (Lua scripts, symlinks) is spawned. Transfers require a
//! navigation directory to resolve their destination; without one they are
//! left pending for a later dispatch. Engine-backed rows are finalized
//! asynchronously by the pump. Also provides [`QUEUE::check_validity`],
//! which marks pending items whose source paths no longer exist as
//! [`QueueItemState::PendingErr`].

use super::*;

use std::path::PathBuf;
use std::sync::atomic::Ordering;

use cba::bs::symlink;

use crate::{
    cli::paths::actions_dir,
    lua::{execute, load_script},
    run::{
        item::short_display,
        state::{GLOBAL, MENU_ACTIONS, TOAST, ToastStyle},
    },
};

/// Runs one row. `item.data` is task input, verbatim from enqueue time:
/// the destination (or destination hint) for transfers, the script
/// destination for custom kinds. Transfer rows carry exactly one source;
/// they resolve their destination from `data` + `nav`, and are a silent
/// no-op (row stays pending) when both are missing — [`QUEUE::dispatch`]
/// filters those before counting.
///
/// By value so ownership can move into spawned closures; the row in
/// `QUEUE_STATE` shares its status atomics, so every update here is
/// visible to the UI.
pub fn perform(
    item: QueueItem,
    nav: Option<AbsPath>,
) {
    log::debug!("Transferring: {item:?}");

    item.status.state.store(QueueItemState::Started);

    match item.kind.as_str() {
        // extraction rows never go through dispatch: [`QUEUE::start_extract`]
        // submits them directly
        "extract" => {
            log::error!("extract row reached perform; this is a bug: {item:?}");
            item.status.state.store(QueueItemState::CompleteErr);
        }
        "symlink" => {
            let Some(dest) = transfer_dest(&item, nav.as_ref()) else {
                return;
            };
            let source = item.src.first().cloned();
            TASKS::spawn_blocking("queue symlink", move || {
                let result = match source {
                    Some(path) => symlink(&path, &dest, true)
                        .map(|_| ())
                        .map_err(|e| e.to_string()),
                    None => Err("symlink row has no source".into()),
                };
                finish_inline(item, result);
            });
        }
        "none" => {
            // the kind is reserved; queue rows may still be enqueued
            // under it as an explicit no-op
            item.status.state.store(QueueItemState::CompleteOk);
            QUEUE_ACTION_HISTORY.lock().unwrap().push(item);
        }
        // builtin transfers: submission is cheap, so this stays inline on
        // the dispatching thread (config is main-thread TLS)
        "copy" | "move" => {
            let Some(dest) = transfer_dest(&item, nav.as_ref()) else {
                return;
            };
            let params = if item.kind == "copy" {
                fist_copy::JobKind::Copy(GLOBAL::cfg().queue.copy.clone())
            } else {
                fist_copy::JobKind::Move(GLOBAL::cfg().queue.r#move.clone())
            };

            let request = fist_copy::JobRequest {
                kind: params,
                source: item.src[0].to_path_buf(),
                dest,
            };
            match super::scheduler().submit(request) {
                Ok(handle) => {
                    // the status atomics are shared with the stored row,
                    // so the watcher discovers this task by id
                    item.status.set_task_id(handle.id);
                    super::ensure_watcher();
                }
                Err(e) => {
                    log::error!("Transfer submit failed for {item:?}: {e}");
                    item.status.state.store(QueueItemState::CompleteErr);
                    let display = short_display(item.src.first().expect("single source checked"));
                    TOAST::push(ToastStyle::Error, "Failed: ", [display]);
                    TOAST::notice(ToastStyle::Error, e.to_string());
                }
            }
            // completion (toasts + action history) is handled by the pump
        }
        // any other kind is a menu action key
        script => {
            let command = match MENU_ACTIONS.get().and_then(|m| m.get(script)) {
                Some(action) => action.command.clone(),
                None => {
                    log::error!("No menu action for queue kind {script:?}: {item:?}");
                    item.status.state.store(QueueItemState::CompleteErr);
                    TOAST::notice(
                        ToastStyle::Error,
                        format!("No menu action for kind {script}"),
                    );
                    return;
                }
            };
            item.status.progress.store(0, Ordering::Relaxed);
            let desc = format!("{}: {}", item.kind, item.display());
            TASKS::spawn_blocking(desc, move || {
                let result = load_script(&command, Some(actions_dir()))
                    .ok_or_else(|| anyhow::anyhow!("failed to load script"))
                    .and_then(|s| {
                        execute(
                            &s,
                            &item.src,
                            &item.data.to_string_lossy(),
                            nav.as_ref(),
                            Some(&item.status.progress),
                        )
                        .map(|_| ())
                        .map_err(anyhow::Error::msg)
                    });
                finish_inline(item, result.map_err(|e| e.to_string()));
            });
        }
    }
}

// todo: this makes sense but maybe has too many cases to be intuitive, at least needs documentation
/// Resolves a transfer's concrete destination. An explicit `data` is used
/// as-is (absolute) or resolved against `nav`; empty `data` means "into
/// `nav`" and requires it — without one the row stays pending. The source
/// file name is appended when the result names a directory.
fn transfer_dest(
    item: &QueueItem,
    nav: Option<&AbsPath>,
) -> Option<PathBuf> {
    let base: PathBuf = match (item.data.is_empty(), nav) {
        (true, Some(nav)) => {
            let mut d = nav.as_os_str().to_owned();
            d.push(std::path::MAIN_SEPARATOR_STR);
            d.into()
        }
        (_, Some(nav)) => item.data.abs(nav),
        (false, None) => item.data.clone().into(),
        (true, None) => return None,
    };
    Some(crate::utils::path::desired_path(
        &item.src[0],
        base.as_os_str(),
    ))
}

/// Blocking tail shared by kinds that finish on their own terms (symlink,
/// scripts): resolves terminal state and records history.
fn finish_inline(
    item: QueueItem,
    result: Result<(), String>,
) {
    match result {
        Ok(()) => {
            item.status.state.store(QueueItemState::CompleteOk);
            QUEUE_ACTION_HISTORY.lock().unwrap().push(item);
        }
        Err(e) => {
            log::error!("Queue task error for {item:?}: {e}");
            item.status.state.store(QueueItemState::CompleteErr);
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

    fn wait_terminal(
        watch: &QueueItemStatus,
        secs: u64,
    ) {
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(secs);
        while !watch.state.is_complete() {
            assert!(
                std::time::Instant::now() < deadline,
                "queue task did not finish in time"
            );
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
    }

    #[tokio::test]
    async fn test_perform_copy_and_symlink() {
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
            data: dst_folder.as_os_str().to_owned(),
            status: QueueItemStatus::new(&src_dir),
        };
        // the unified watcher resolves tasks through rows in the shared
        // queue, mirroring the production dispatch path
        QUEUE_STATE.lock().unwrap().shared.push(item.clone());
        let watch = item.status.clone();
        perform(item, Some(AbsPath::new_unchecked(&dst_dir)));
        wait_terminal(&watch, 30);
        assert_eq!(watch.state.load(), QueueItemState::CompleteOk);
        assert!(dst_folder.exists(), "dst_folder should exist after copy");
        assert!(
            dst_folder.join("file.txt").exists(),
            "file.txt should exist inside copied dst_folder"
        );

        // 2. Symlink
        let symlink_target = dst_dir.join("symlink_folder");
        let sym_item = QueueItem {
            kind: "symlink".into(),
            src: vec![AbsPath::new_unchecked(&src_dir)],
            data: symlink_target.as_os_str().to_owned(),
            status: QueueItemStatus::new(&src_dir),
        };
        QUEUE_STATE.lock().unwrap().shared.push(sym_item.clone());
        let watch = sym_item.status.clone();
        perform(sym_item, Some(AbsPath::new_unchecked(&dst_dir)));
        wait_terminal(&watch, 30);
        assert_eq!(watch.state.load(), QueueItemState::CompleteOk);
        assert!(symlink_target.exists(), "symlink_folder should exist");

        // remove only our rows: sibling tests own theirs
        QUEUE_STATE.lock().unwrap().shared.clear();
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

        wait_terminal(&status, 30);
        assert_eq!(status.state.load(), QueueItemState::CompleteOk);
        assert!(dest.join("hello.txt").exists());
        assert!(dest.join("nested/data.txt").exists());
    }

    /// Exercises the production dispatch path end to end: a pending row in
    /// the shared queue is executed via [`QUEUE::dispatch`] and finalized
    /// by the watcher.
    #[tokio::test]
    async fn dispatched_copy_runs_to_completion() {
        let _guard = SERIAL.lock().unwrap();
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
                data: Default::default(),
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
        wait_terminal(&watch, 30);
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
