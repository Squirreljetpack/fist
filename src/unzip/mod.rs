//! Background archive extraction into temporary skeletons.
//!
//! Advancing into an archive synchronously creates a directory-only
//! skeleton of its contents under the app temp dir ([`init`]), submits the
//! full extraction to the [`fist_copy`] scheduler as an [`extract`] job,
//! and lets the fs watcher surface progress by keeping the skeleton dir's
//! reloads unthrottled (see [`WatcherMessage::MustWatch`]).
//!
//! Format support lives in [`fist_copy::extract`]: per-format detection,
//! entry-wise listing, and cancellation-aware extraction with entry-count
//! progress on the copy worker pool.
//!
//! Every source path maps to one current skeleton for the lifetime of the
//! process: re-entering an archive reuses its skeleton unless the archive
//! changed after the skeleton was created, in which case the skeleton is
//! regenerated. The skeleton dir is named `<percent-encoded path>--<unix
//! seconds>` under the unzip root, and holds one subfolder named after the
//! archive (extension included) which is the extraction workdir — so the
//! archive's absolute path is recoverable from the skeleton dir name alone
//! (see [`recover_archive`]). Cleanup runs as a named background task on
//! exit ([`shutdown`]); a hard exit strands the per-process root under the
//! system temp dir, where the system tmp cleaner reclaims it.

use std::{
    collections::HashMap,
    path::{Component, Path, PathBuf},
    sync::{Mutex, OnceLock},
    thread,
    time::Duration,
};

use fist_copy::extract;
use matchmaker::nucleo::Span;

use crate::{
    abspath::AbsPath,
    cli::paths::__unzip,
    config::ArchiveConfig,
    run::{
        queue::{QUEUE, QueueItemState, QueueItemStatus},
        state::{GLOBAL, TASKS, TOAST, ToastFlags, ToastStyle},
    },
    watcher::WatcherMessage,
};

/// Name under which [`shutdown`] registers its cleanup task, so the
/// shutdown wait UI can surface it.
const CLEANUP_TASK_DESC: &str = "unzip skeleton cleanup";

// -------------------------- registry --------------------------

/// Root of every extraction skeleton:
/// `<tmp>/fist/<process-id>/unzip`.
pub fn root() -> PathBuf {
    __unzip().to_path_buf()
}

/// Lifecycle of a registered archive.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum EntryState {
    /// Extraction queued or in progress.
    Skeleton,
    Complete,
    Failed,
}

/// One registered archive: its skeleton dir, lifecycle state, and the
/// queue row driving the extraction.
struct Entry {
    /// Skeleton dir holding the archive-name workdir.
    temp: PathBuf,
    /// The extraction workdir (the archive-name subfolder) the user
    /// navigates into.
    workdir: PathBuf,
    /// Active-user counter, reserved for refcounted cleanup.
    #[allow(dead_code)]
    active: u32,
    state: EntryState,
    /// The extraction queue row's status, present between start and
    /// terminal row state.
    row: Option<QueueItemStatus>,
}

struct Registry {
    /// Source path → entry, consulted by [`init`] to avoid re-extracting.
    entries: Mutex<HashMap<PathBuf, Entry>>,
    /// Extraction settings.
    config: ArchiveConfig,
}

static REGISTRY: OnceLock<Registry> = OnceLock::new();

fn registry() -> &'static Registry {
    REGISTRY.get().expect("unzip::start not called")
}

/// Start the extraction subsystem: registers settings and arms the
/// best-effort exit hook. Extraction itself runs on the copy scheduler.
pub fn start(config: ArchiveConfig) {
    let _ = REGISTRY.set(Registry {
        entries: Mutex::new(HashMap::new()),
        config,
    });

    // best-effort cleanup for exit paths that skip [`shutdown`]
    // (e.g. `std::process::exit`) — leftovers are swept on the next start
    #[cfg(unix)]
    unsafe {
        libc::atexit(cleanup_on_exit);
    }
}

/// Removes the whole skeleton root. Registered with `atexit` so hard exits
/// also clean up; best effort, as extraction tasks may still be running.
#[cfg(unix)]
extern "C" fn cleanup_on_exit() {
    let _ = std::fs::remove_dir_all(root());
}

/// Whether `path` is an archive the engine can extract.
pub fn supported(path: &Path) -> bool {
    extract::detect(path).is_some()
}

/// Cancels the in-flight extraction of `path` through its queue row, if
/// one is running.
pub fn cancel(path: &Path) -> bool {
    let Ok(source) = path.canonicalize() else {
        return false;
    };
    let Ok(state) = crate::run::queue::QUEUE_STATE.lock() else {
        return false;
    };
    let Some(id) = state
        .shared
        .iter()
        .filter(|i| i.kind == crate::run::queue::EXTRACT_KIND)
        .filter(|i| i.src.len() == 1 && i.src[0].as_os_str() == source.as_os_str())
        .find(|i| i.status.state.is_started())
        .and_then(|i| i.task_id)
    else {
        return false;
    };
    drop(state);
    crate::run::queue::scheduler().cancel(id);
    true
}

/// Synchronously create (or reuse) the extraction skeleton for `path`,
/// queue its extraction on the copy scheduler, and re-arm must-watch on
/// the extraction workdir.
///
/// Returns the workdir to navigate into; `None` means the archive could
/// not be detected or listed (or previously failed) and the caller should
/// handle the path normally.
pub fn init(path: &Path) -> Option<AbsPath> {
    let source = path.canonicalize().ok()?;
    let format = extract::detect(&source)?;
    let r = registry();

    // Re-entering an archive reuses its skeleton — extraction may still be
    // running or already done — but only while the skeleton is newer than
    // the archive itself; an archive changed after its skeleton was
    // created makes the cached tree stale and it is regenerated. An
    // emptied workdir is likewise not reusable and is recreated.
    if let Some(entry) = r.entries.lock().ok()?.get(&source) {
        if matches!(entry.state, EntryState::Failed) {
            TOAST::notice(
                ToastStyle::Error,
                format!("Extraction previously failed for {}", entry.temp.display()),
            );
            return None;
        }
        // An emptied workdir is not reusable: remove the skeleton so the
        // recreate path below builds a fresh one. Only completed entries
        // are eligible — a Skeleton entry is an extraction that may still
        // be populating the workdir.
        if matches!(entry.state, EntryState::Complete) && workdir_is_empty(&entry.workdir) {
            log::info!("Empty skeleton for {}, re-extracting", source.display());
            let _ = std::fs::remove_dir_all(&entry.temp);
        } else if skeleton_is_fresh(&entry.temp, &source) {
            must_watch(&entry.workdir);
            return Some(AbsPath::new_unchecked(entry.workdir.clone()));
        } else {
            log::info!("Stale skeleton for {}, re-extracting", source.display());
            if r.config.cleanup_duplicates {
                let source = source.clone();
                TASKS::spawn_blocking("unzip stale cleanup", move || {
                    remove_stale_skeletons(&source);
                });
            }
        }
    }

    log::info!("Entering archive {} via {}", source.display(), format.id());
    let listing = match extract::list(&source, format) {
        Ok(listing) => listing,
        Err(e) => {
            log::error!(
                "Failed to list archive {}: {e} ({})",
                e.archive.display(),
                e.source
            );
            return None;
        }
    };

    let workdir = alloc_dir(&source)?;
    for entry in &listing {
        skeleton_dir(&workdir, entry);
    }

    // register before submitting so a re-entry can never double-extract
    r.entries.lock().ok()?.insert(
        source.clone(),
        Entry {
            temp: workdir
                .parent()
                .expect("the workdir sits inside its skeleton")
                .to_path_buf(),
            workdir: workdir.clone(),
            active: 0,
            state: EntryState::Skeleton,
            row: None,
        },
    );

    match QUEUE::start_extract(AbsPath::new_unchecked(source.clone()), workdir.clone()) {
        Some(row) => {
            if let Ok(mut entries) = r.entries.lock()
                && let Some(entry) = entries.get_mut(&source)
            {
                entry.row = Some(row.clone());
            }
            spawn_watch(row, source);
        }
        None => {
            // no row was created; the entry stays registered as failed so
            // re-entry reports the error instead of silently rebuilding
            if let Ok(mut entries) = r.entries.lock()
                && let Some(entry) = entries.get_mut(&source)
            {
                entry.state = EntryState::Failed;
            }
        }
    }

    must_watch(&workdir);
    Some(AbsPath::new_unchecked(workdir))
}

/// Watches the extraction queue row until terminal state, updating the
/// skeleton lifecycle and retiring the "Extracting" toast. Completion and
/// failure notifications are the queue pump's job.
fn spawn_watch(
    row: QueueItemStatus,
    source: PathBuf,
) {
    let _ = thread::Builder::new()
        .name("fist-unzip-watch".into())
        .spawn(move || {
            loop {
                thread::sleep(Duration::from_millis(100));
                let state = match row.state.load() {
                    QueueItemState::Pending
                    | QueueItemState::Started
                    | QueueItemState::PendingErr => {
                        continue;
                    }
                    QueueItemState::CompleteOk => EntryState::Complete,
                    QueueItemState::CompleteErr => EntryState::Failed,
                };
                if let Ok(mut entries) = registry().entries.lock()
                    && let Some(entry) = entries.get_mut(&source)
                {
                    entry.state = state;
                    entry.row = None;
                }
                toast_extract_done(&source);
                break;
            }
        });
}

/// Allocates the skeleton dir for `source` (named by [`skeleton_name`])
/// plus its archive-name workdir — the extraction destination and the dir
/// the user navigates into.
fn alloc_dir(source: &Path) -> Option<PathBuf> {
    let dir = root().join(skeleton_name(source));
    std::fs::create_dir_all(&dir).ok()?;
    let workdir = dir.join(archive_dir_name(source));
    std::fs::create_dir_all(&workdir).ok()?;
    Some(workdir)
}

/// Skeleton dir name for `source`:
/// `<percent-encoded canonical path>--<unix seconds>`. The timestamp makes
/// regenerated skeletons unique; the encoded path (see [`encode_path`])
/// keeps the original archive path recoverable from the dir name alone
/// (see [`recover_archive`]).
fn skeleton_name(source: &Path) -> String {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    format!("{}--{now}", encode_path(source))
}

/// The workdir name inside a skeleton: the archive file name, extension
/// included, so the archive's parent is recoverable from the path alone.
fn archive_dir_name(source: &Path) -> String {
    source
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| encode_path(source))
}

/// Percent-encodes `path` for use as a skeleton dir name: `/` becomes
/// `%2F` and each literal `%` becomes `%25` (escaped as it is met, so the
/// encoding is unambiguous); every other character is kept as-is.
fn encode_path(path: &Path) -> String {
    let mut out = String::with_capacity(path.as_os_str().len());
    for c in path.to_string_lossy().chars() {
        match c {
            '/' => out.push_str("%2F"),
            '%' => out.push_str("%25"),
            c => out.push(c),
        }
    }
    out
}

/// Recovers the canonical archive path a skeleton dir name was built from:
/// strips the trailing `--<timestamp>` and percent-decodes. `None` when
/// `name` does not look like a skeleton dir name.
pub fn recover_archive(name: &str) -> Option<PathBuf> {
    let encoded = strip_timestamp(name)?;
    let bytes = encoded.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%'
            && let Some(byte) = percent_byte(&bytes[i..])
        {
            out.push(byte);
            i += 3;
        } else {
            out.push(bytes[i]);
            i += 1;
        }
    }
    Some(PathBuf::from(String::from_utf8_lossy(&out).into_owned()))
}

/// Decodes the `%xx` escape at the start of `bytes`, or `None` when it is
/// not a valid escape.
fn percent_byte(bytes: &[u8]) -> Option<u8> {
    if bytes.len() < 3 || bytes[0] != b'%' {
        return None;
    }
    let hi = (bytes[1] as char).to_digit(16)?;
    let lo = (bytes[2] as char).to_digit(16)?;
    Some((hi * 16 + lo) as u8)
}

/// Strips the trailing `--<digits>` timestamp suffix from a skeleton dir
/// name. The suffix is always the last `--` in the name (the encoded path
/// is a strict prefix), which is what `rsplit_once` picks.
fn strip_timestamp(name: &str) -> Option<&str> {
    let (prefix, suffix) = name.rsplit_once("--")?;
    if suffix.is_empty() || !suffix.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    Some(prefix)
}

/// Parses the `--<unix seconds>` timestamp suffix of a skeleton dir name.
/// `None` when the name has no valid timestamp suffix.
fn skeleton_timestamp(name: &str) -> Option<u64> {
    let (_, suffix) = name.rsplit_once("--")?;
    if suffix.is_empty() || !suffix.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    suffix.parse().ok()
}

/// Whether `skeleton`'s cached extraction is still current for `source`.
/// The skeleton's allocation time is read from the `--<unix seconds>`
/// suffix of its dir name — no filesystem timestamps are consulted — and
/// compared, at second granularity, against the archive's mtime. A name
/// without a parseable suffix is not a skeleton and counts as stale, so
/// re-entry allocates a fresh workdir.
fn skeleton_is_fresh(
    skeleton: &Path,
    source: &Path,
) -> bool {
    let Some(ts) = skeleton
        .file_name()
        .and_then(|n| n.to_str())
        .and_then(skeleton_timestamp)
    else {
        return false;
    };
    let Ok(archive) = std::fs::metadata(source) else {
        return true;
    };
    let mtime_secs = archive
        .modified()
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_secs())
        .unwrap_or(0);
    ts > mtime_secs
}

/// Whether an extraction workdir holds no entries. A missing directory
/// counts as empty.
fn workdir_is_empty(workdir: &Path) -> bool {
    std::fs::read_dir(workdir)
        .map(|mut it| it.next().is_none())
        .unwrap_or(true)
}

/// Removes every skeleton under the unzip root that decodes back to
/// `source` (older timestamped copies included).
fn remove_stale_skeletons(source: &Path) {
    if let Ok(entries) = std::fs::read_dir(root()) {
        for entry in entries.flatten() {
            let name = entry.file_name();
            let Some(name) = name.to_str() else {
                continue;
            };
            if recover_archive(name).as_deref() == Some(source) {
                let _ = std::fs::remove_dir_all(entry.path());
            }
        }
    }
}

/// Creates the directories implied by one archive entry: the entry itself
/// when it is an explicit directory, else its parent chain.
fn skeleton_dir(
    root: &Path,
    entry: &extract::ArchiveEntry,
) {
    let path = entry.path.as_path();
    // defensive: listings should already be sanitized, but archive
    // metadata is never trusted to stay inside the skeleton
    if !is_safe(path) {
        log::warn!("Skipping unsafe archive entry {:?}", entry.path);
        return;
    }

    if entry.is_dir {
        let _ = std::fs::create_dir_all(root.join(path));
    } else if let Some(parent) = path.parent().filter(|p| !p.as_os_str().is_empty()) {
        // parents of every file form the directory tree, which also
        // covers listings that drop explicit dir entries (zip/tar)
        let _ = std::fs::create_dir_all(root.join(parent));
    }
}

/// True when `path` is relative and contains no `..` components.
fn is_safe(path: &Path) -> bool {
    !path.is_absolute()
        && path
            .components()
            .all(|c| matches!(c, Component::Normal(_) | Component::CurDir))
}

/// Tell the watcher to keep reloading `temp` through event storms (the
/// extraction itself produces them) until it pauses or is switched away.
fn must_watch(temp: &Path) {
    GLOBAL::send_watcher(WatcherMessage::MustWatch(temp.to_path_buf()));
}

pub fn toast_entering(path: &Path) {
    let source = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
    let is_complete = registry()
        .entries
        .lock()
        .ok()
        .and_then(|entries| {
            entries
                .get(&source)
                .map(|e| matches!(e.state, EntryState::Complete))
        })
        .unwrap_or(false);

    if is_complete {
        TOAST::msg(Span::styled("Entering archive", ToastStyle::Info), true);
    } else {
        let name = path
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| path.to_string_lossy().to_string());
        TOAST::push_with_flag(
            ToastStyle::Info,
            "Extracting: ",
            [Span::raw(name)],
            ToastFlags::PERSIST_CURSOR | ToastFlags::PERSIST_PANE,
        );
    }
}

/// Retires the persistent "Extracting" toast pushed by [`toast_entering`].
/// The queue pump owns completion and failure notifications.
fn toast_extract_done(source: &Path) {
    let name = source
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| source.to_string_lossy().to_string());
    TOAST::pop("Extracting: ", &Span::raw(name));
}

/// Schedules skeleton cleanup as a named background task so the shutdown
/// sequence can surface what it waits on ([`TASKS::shutdown`]); the task
/// removes the whole skeleton root. In-flight extractions are cancelled
/// beforehand by the scheduler's own shutdown.
pub fn shutdown() {
    if REGISTRY.get().is_none() {
        return;
    }
    TASKS::spawn_blocking(
        CLEANUP_TASK_DESC,
        || match std::fs::remove_dir_all(root()) {
            Ok(()) => {}
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => log::warn!("Failed to remove skeleton root {:?}: {e}", root()),
        },
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn safe_paths() {
        assert!(is_safe(Path::new("a/b/c.txt")));
        assert!(is_safe(Path::new("dir/")));
        assert!(is_safe(Path::new("./x")));
        assert!(!is_safe(Path::new("/etc/passwd")));
        assert!(!is_safe(Path::new("../evil")));
        assert!(!is_safe(Path::new("a/../../evil")));
    }

    #[test]
    fn detection_covers_enabled_formats() {
        let dir = std::env::temp_dir().join(format!("fist-unzip-detect-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();

        // content sniffing: zip and gzip magic
        let zip = dir.join("foo.zip");
        std::fs::write(&zip, b"PK\x03\x04").unwrap();
        assert_eq!(extract::detect(&zip), Some(extract::Format::Zip));

        let gz = dir.join("foo.tar.gz");
        std::fs::write(&gz, b"\x1f\x8b\x08").unwrap();
        assert_eq!(extract::detect(&gz), Some(extract::Format::Gz));

        // unrecognized content is not an archive
        let txt = dir.join("bar.txt");
        std::fs::write(&txt, b"plain text").unwrap();
        assert_eq!(extract::detect(&txt), None);

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn skeleton_names() {
        let path = Path::new("/tmp/fist-archives.x/archive.7z");
        let name = skeleton_name(path);

        // <percent-encoded path>--<unix seconds>, recovering the source
        let (encoded, ts) = name.rsplit_once("--").unwrap();
        assert_eq!(encoded, "%2Ftmp%2Ffist-archives.x%2Farchive.7z");
        assert!(!ts.is_empty() && ts.bytes().all(|b| b.is_ascii_digit()));
        assert_eq!(recover_archive(&name).as_deref(), Some(path));

        // a path at the root has no leading separator to trim
        let root_path = Path::new("/a.tar");
        let root_name = skeleton_name(root_path);
        let (encoded, ts) = root_name.rsplit_once("--").unwrap();
        assert_eq!(encoded, "%2Fa.tar");
        assert!(!ts.is_empty() && ts.bytes().all(|b| b.is_ascii_digit()));
        assert_eq!(recover_archive(&root_name).as_deref(), Some(root_path));
    }

    #[test]
    fn skeleton_roundtrip() {
        // % and / are escaped so the original path is losslessly recoverable
        let path = Path::new("/tmp/a%2Fb (1)/ar..zip");
        let name = skeleton_name(path);
        assert_eq!(recover_archive(&name).as_deref(), Some(path));

        // a path ending in --<digits> still recovers the full path
        let tricky = Path::new("/x/a--123");
        let name = skeleton_name(tricky);
        assert_eq!(recover_archive(&name).as_deref(), Some(tricky));

        // names without a timestamp suffix are not skeletons
        assert_eq!(recover_archive("whatever"), None);
    }

    #[test]
    fn skeleton_freshness() {
        let dir = std::env::temp_dir().join(format!("fist-unzip-fresh-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();

        let archive = dir.join("a.zip");
        std::fs::write(&archive, b"x").unwrap();
        let modified = std::time::UNIX_EPOCH + std::time::Duration::from_secs(1_700_000_000);
        std::fs::File::options()
            .write(true)
            .open(&archive)
            .unwrap()
            .set_modified(modified)
            .unwrap();

        // freshness comes from the --<unix seconds> suffix alone; the
        // skeleton dir itself is never stat-ed
        let skeleton = |ts: u64| dir.join(format!("whatever--{ts}"));

        // skeleton allocated after the archive's mtime -> fresh
        assert!(skeleton_is_fresh(&skeleton(1_800_000_000), &archive));
        // skeleton allocated before the archive's mtime -> stale
        assert!(!skeleton_is_fresh(&skeleton(1_500_000_000), &archive));
        // same-second allocation counts as stale (second granularity)
        assert!(!skeleton_is_fresh(&skeleton(1_700_000_000), &archive));
        // name without a parseable timestamp -> stale, a fresh workdir is
        // allocated on entry
        assert!(!skeleton_is_fresh(&dir.join("no-timestamp"), &archive));
        // an archive we cannot stat -> reuse whatever is cached
        assert!(skeleton_is_fresh(&skeleton(0), &dir.join("missing.zip")));

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn skeleton_timestamp_parsing() {
        let name = format!(
            "{}--{}",
            encode_path(Path::new("/a/b.zip")),
            1_727_000_000u64
        );
        assert_eq!(skeleton_timestamp(&name), Some(1_727_000_000));
        assert_eq!(
            recover_archive(&name).as_deref(),
            Some(Path::new("/a/b.zip"))
        );

        // missing or malformed suffixes parse to None (stale on entry)
        assert_eq!(skeleton_timestamp("whatever"), None);
        assert_eq!(skeleton_timestamp("a--"), None);
        assert_eq!(skeleton_timestamp("a--x"), None);
        assert_eq!(skeleton_timestamp("a--1x"), None);
    }

    #[test]
    fn workdir_emptiness() {
        let dir = std::env::temp_dir().join(format!("fist-unzip-empty-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();

        // an empty directory reports empty
        assert!(workdir_is_empty(&dir));
        // a missing directory counts as empty
        assert!(workdir_is_empty(&dir.join("missing")));
        // entries make it non-empty
        std::fs::write(dir.join("f.txt"), b"x").unwrap();
        assert!(!workdir_is_empty(&dir));

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn skeleton_contains_dirs_only() {
        let dir = std::env::temp_dir().join(format!("fist-unzip-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);

        skeleton_dir(
            &dir,
            &extract::ArchiveEntry {
                path: PathBuf::from("a/b/c.txt"),
                is_dir: false,
            },
        );
        skeleton_dir(
            &dir,
            &extract::ArchiveEntry {
                path: PathBuf::from("a/b/"),
                is_dir: true,
            },
        );
        skeleton_dir(
            &dir,
            &extract::ArchiveEntry {
                path: PathBuf::from("empty/"),
                is_dir: true,
            },
        );
        skeleton_dir(
            &dir,
            &extract::ArchiveEntry {
                path: PathBuf::from("flat"),
                is_dir: false,
            },
        );
        let evil = format!("../../fist-unzip-evil-{}", std::process::id());
        skeleton_dir(
            &dir,
            &extract::ArchiveEntry {
                path: PathBuf::from(evil),
                is_dir: true,
            },
        );
        skeleton_dir(
            &dir,
            &extract::ArchiveEntry {
                path: PathBuf::from("/abs"),
                is_dir: true,
            },
        );

        assert!(dir.join("a/b").is_dir());
        assert!(dir.join("empty").is_dir());
        // files are never created by the skeleton
        assert!(!dir.join("a/b/c.txt").exists());
        assert!(!dir.join("flat").exists());
        // traversal and absolute entries never touch the disk
        assert!(
            !std::env::temp_dir()
                .join(format!("fist-unzip-evil-{}", std::process::id()))
                .exists()
        );
        assert!(!dir.join("abs").exists());

        let _ = std::fs::remove_dir_all(&dir);
    }
}
