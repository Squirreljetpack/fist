//! The archive strategy: preview extraction into the per-process temp root.
//!
//! Advancing into an archive ([`mount`]) reserves a free-named directory under
//! [`root`] (`folder.zip` -> `folder/`, `folder_1`, ...), submits the full
//! extraction to the copy engine as a queue row, and lets the fs watcher
//! surface progress by keeping the workdir's reloads unthrottled (see
//! [`WatcherMessage::MustWatch`]). The pump watcher records the finished
//! extraction in [`crate::mount::MountRegistry`], which is what the mounts
//! picker lists; the whole root is removed on exit, taking partial and
//! finished previews with it.
//!
//! User-chosen extractions ([`extract_to`]) write into the chosen destination
//! instead and are the user's to keep.

use std::{
    path::{Path, PathBuf},
    sync::OnceLock,
};

use cba::blaim::{ClaimPolicy, reserve_dir};

use crate::{
    abspath::AbsPath,
    cli::paths::__unzip,
    run::{
        queue::{QUEUE, QUEUE_STATE, scheduler},
        state::GLOBAL,
    },
    watcher::WatcherMessage,
};
use fist_copy::extract;

// -------------------------- the extraction root --------------------------

/// Root of every preview extraction:
/// `<tmp>/fist/<pid>-<nanos>/unzip`.
pub fn root() -> PathBuf {
    __unzip().to_path_buf()
}

/// Arms the best-effort exit hook once, on first use: nothing needs cleaning
/// before the first extraction exists.
#[cfg(unix)]
fn arm_exit_hook() {
    static ARMED: OnceLock<()> = OnceLock::new();
    ARMED.get_or_init(|| unsafe {
        libc::atexit(cleanup_on_exit);
    });
}

#[cfg(not(unix))]
fn arm_exit_hook() {}

/// Removes the whole extraction root and its per-process parent (the latter
/// only when it holds nothing else). Registered with `atexit` so hard exits
/// also clean up; best effort, as extraction tasks may still be running.
#[cfg(unix)]
extern "C" fn cleanup_on_exit() {
    let _ = std::fs::remove_dir_all(root());
    remove_temp_parent();
}

/// Removes the extraction root synchronously. The scheduler's own shutdown has
/// already cancelled in-flight extractions by then.
pub fn shutdown() {
    match std::fs::remove_dir_all(root()) {
        Ok(()) => {}
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
        Err(e) => log::warn!("Failed to remove extraction root {:?}: {e}", root()),
    }
    remove_temp_parent();
}

/// Removes this process's temp directory when the extraction root was its
/// only content. `remove_dir` refuses a non-empty directory, so anything else
/// living there (or another process's root) keeps it alive.
fn remove_temp_parent() {
    if let Some(parent) = root().parent() {
        let _ = std::fs::remove_dir(parent);
    }
}

// -------------------------- the extract strategy --------------------------

/// Whether `path` is an archive the engine can extract.
pub fn supported(path: &Path) -> bool {
    extract::detect(path).is_some()
}

/// Make the archive at `source` navigable as a preview: reuse the extraction
/// already running for it, or reserve a free-named directory under [`root`]
/// and submit a fresh extraction. Submission is synchronous — it either starts
/// the work or fails — so there is always a directory to enter.
pub fn mount(source: &AbsPath) -> Result<Option<AbsPath>, String> {
    arm_exit_hook();
    let source = source
        .canonicalize()
        .map_err(|e| format!("cannot resolve {}: {e}", source.to_string_lossy()))?;

    // a running extraction wins: entering its workdir is also how the user
    // watches it fill in
    if let Some(workdir) = running_workdir(&source) {
        return Ok(Some(AbsPath::new_unchecked(workdir)));
    }

    let dest = reserve_workdir(&source)?;
    if QUEUE::start_extract(AbsPath::new_unchecked(source.clone()), dest.clone()).is_none() {
        let _ = std::fs::remove_dir(&dest);
        return Err("extraction engine is shutting down".to_string());
    }
    Ok(Some(AbsPath::new_unchecked(dest)))
}

/// Nothing to undo: preview directories are removed with the root on exit, and
/// user-chosen destinations belong to the user.
pub fn unmount(_dest: &Path) -> Result<(), String> {
    Ok(())
}

/// Whether `dest` still holds an extraction.
pub fn is_live(dest: &Path) -> bool {
    dest.is_dir()
}

/// Reserve the extraction directory for `source`: `<root>/<archive stem>`,
/// with the first free `_N` variant on collisions.
fn reserve_workdir(source: &Path) -> Result<PathBuf, String> {
    let Some(stem) = source.file_stem().filter(|stem| !stem.is_empty()) else {
        return Err(format!("no directory name for {}", source.display()));
    };
    let root = root();
    std::fs::create_dir_all(&root).map_err(|e| format!("cannot create {}: {e}", root.display()))?;
    let claim = reserve_dir(&root.join(stem), ClaimPolicy::default())
        .map_err(|e| format!("cannot reserve a directory for {}: {e}", source.display()))?;
    claim
        .into_path()
        .ok_or_else(|| format!("cannot reserve a directory for {}", source.display()))
}

// -------------------------- user extractions --------------------------

/// Submits a user-requested extraction of `source` into `dest`, creating the
/// destination directory first. Unlike [`mount`], no free name is chosen and
/// the engine writes straight into `dest`; the queue row carries it as its
/// destination.
///
/// The caller must have checked that `source` is extractable (the menu action
/// gates on [`supported`] before the prompt); a source the engine cannot
/// detect is reported as a failed task instead. Returns a human-readable error
/// when the destination cannot be prepared or the engine refuses the
/// submission.
pub fn extract_to(
    source: &AbsPath,
    dest: &Path,
) -> Result<(), String> {
    let canonical = source
        .canonicalize()
        .map_err(|e| format!("cannot resolve {}: {e}", source.to_string_lossy()))?;
    std::fs::create_dir_all(dest)
        .map_err(|e| format!("cannot create {}: {e}", dest.to_string_lossy()))?;
    let dest = dest
        .canonicalize()
        .map_err(|e| format!("cannot resolve {}: {e}", dest.to_string_lossy()))?;
    if QUEUE::start_extract(AbsPath::new_unchecked(canonical), dest).is_none() {
        return Err("extraction engine is shutting down".to_string());
    }
    Ok(())
}

/// Cancels the in-flight extraction of `path`, if one is running.
pub fn cancel(path: &Path) -> bool {
    let Ok(source) = path.canonicalize() else {
        return false;
    };
    let Some(id) = running_row(&source).and_then(|i| i.status.task_id()) else {
        return false;
    };
    scheduler().cancel(id);
    true
}

// -------------------------- queue queries --------------------------

/// The started extraction whose source / destination matches `probe`, if any.
fn running(
    probe: impl Fn(&crate::run::queue::QueueItem) -> bool
) -> Option<crate::run::queue::QueueItem> {
    let state = QUEUE_STATE.lock().unwrap();
    state
        .shared
        .iter()
        .find(|i| i.kind == "extract" && i.status.state.is_started() && probe(i))
        .cloned()
}

/// The shared-queue row of a started extraction of `source`, if any.
fn running_row(source: &Path) -> Option<crate::run::queue::QueueItem> {
    running(|item| {
        item.src
            .first()
            .is_some_and(|p| p.as_os_str() == source.as_os_str())
    })
}

/// The archive a started extraction writing into `dest` came from.
pub fn running_source(dest: &Path) -> Option<PathBuf> {
    running(|item| item.data.as_os_str() == dest.as_os_str())
        .and_then(|item| item.src.first().map(|p| p.inner()))
}

/// The workdir of a started extraction of `source`.
fn running_workdir(source: &Path) -> Option<PathBuf> {
    running_row(source).map(|i| PathBuf::from(i.data.as_os_str()))
}

/// Tell the watcher to keep reloading `dir` through event storms (the
/// extraction itself produces them) until it pauses or is switched away.
pub fn must_watch(dir: &Path) {
    GLOBAL::send_watcher(WatcherMessage::MustWatch(dir.to_path_buf()));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_mount_reserves_a_free_directory() {
        GLOBAL::init_test_senders();
        let tmp = tempfile::tempdir().unwrap();
        let src_dir = tmp.path().join("src");
        std::fs::create_dir_all(&src_dir).unwrap();
        std::fs::write(src_dir.join("hello.txt"), b"hello world").unwrap();

        let tar_path = tmp.path().join("folder.tar");
        let status = std::process::Command::new("tar")
            .arg("-cf")
            .arg(&tar_path)
            .arg("-C")
            .arg(&src_dir)
            .arg(".")
            .status()
            .unwrap();
        assert!(status.success());

        // a taken name under the extraction root takes the next free variant
        std::fs::create_dir_all(root()).unwrap();
        std::fs::create_dir(root().join("folder")).unwrap();

        let entered = mount(&AbsPath::new_unchecked(&tar_path))
            .unwrap()
            .expect("submitting an extraction yields its workdir");
        assert_eq!(entered.as_path(), root().join("folder_1"));
        assert!(entered.is_dir());

        // re-entry while it runs reuses the same directory
        let again = mount(&AbsPath::new_unchecked(&tar_path))
            .unwrap()
            .expect("a running extraction yields its workdir");
        assert_eq!(again.as_path(), entered.as_path());

        cancel(&tar_path);
        let _ = std::fs::remove_dir_all(root());
    }
}
