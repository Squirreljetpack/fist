//! Background archive extraction into temporary skeletons.
//!
//! Advancing into an archive synchronously creates a directory-only
//! preview of its contents ([`init`]), submits the full extraction to the
//! copy scheduler as a queue row, and lets the fs watcher surface progress
//! by keeping the workdir's reloads unthrottled (see
//! [`WatcherMessage::MustWatch`]).
//!
//! This module is stateless: everything worth knowing lives either on disk
//! or in the queue. Skeleton dir names encode the archive's canonical path
//! plus an allocation timestamp (`<percent-encoded path>--<unix millis>`,
//! see [`recover_archive`]), so re-entry reuse, staleness, and recovery
//! are pure reads; in-flight facts come from the queue row created by
//! [`QUEUE::start_extract`] (its status carries the engine task id).
//! Failures are recorded as a `.failed` marker in the skeleton dir by the
//! pump watcher, and cleaned up with the whole per-process root on exit.

use std::path::{Path, PathBuf};

use fist_copy::extract;
use matchmaker::nucleo::Span;

use crate::{
    abspath::AbsPath,
    cli::paths::__unzip,
    run::{
        queue::{EXTRACT_KIND, QUEUE, QUEUE_STATE, scheduler},
        state::{GLOBAL, TASKS, TOAST, ToastFlags, ToastStyle},
    },
    watcher::WatcherMessage,
};

// -------------------------- entry points --------------------------

/// Root of every extraction skeleton:
/// `<tmp>/fist/<process-id>/unzipped_storage_press_undo_to_go_back`.
pub fn root() -> PathBuf {
    __unzip().to_path_buf()
}

/// Registers the best-effort exit hook. No other setup: extraction runs on
/// the copy scheduler and its lifecycle lives on disk and in the queue.
pub fn start() {
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

/// Removes the skeleton root as a named background task so the shutdown
/// sequence can surface what it waits on. The scheduler's own shutdown has
/// already cancelled in-flight extractions by then.
pub fn shutdown() {
    TASKS::spawn_blocking(
        CLEANUP_TASK_DESC,
        || match std::fs::remove_dir_all(root()) {
            Ok(()) => {}
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => log::warn!("Failed to remove skeleton root {:?}: {e}", root()),
        },
    );
}

const CLEANUP_TASK_DESC: &str = "unzip skeleton cleanup";

/// Whether `path` is an archive the engine can extract.
pub fn supported(path: &Path) -> bool {
    extract::detect(path).is_some()
}

/// Synchronously create (or reuse) the extraction skeleton for `path`,
/// start its extraction via a queue row, and return the workdir to
/// navigate into.
///
/// Re-entry resolves everything from disk and the queue: a running
/// extraction for this source means cd into *that* row's workdir; a fresh,
/// populated skeleton is reused; stale or emptied ones are rebuilt. A
/// `.failed` marker reports the previous attempt and bails. `None` means
/// the caller should treat the path normally (the failure is toasted).
pub fn init(path: &Path) -> Option<AbsPath> {
    let source = path.canonicalize().ok()?;
    let format = extract::detect(&source)?;
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

    // a running extraction wins unconditionally: cd into its workdir, no
    // matter what the on-disk skeletons look like (also covers the
    // same-second reallocation edge — two skeletons cannot both be "the"
    // destination while one task is writing)
    if let Some(workdir) = running_workdir(&source) {
        must_watch(&workdir);
        return Some(AbsPath::new_unchecked(workdir));
    }

    let mtime = mtime_secs(&source);

    // freshest existing skeleton for this source, if any
    if let Some((skel, ts)) = freshest_skeleton(&source) {
        if skel.join(FAILED_MARKER).exists() {
            TOAST::notice(ToastStyle::Error, "Extraction failed");
            return None;
        }
        let workdir = workdir_of(&skel, &source);
        if ts > mtime && !workdir_is_empty(&workdir) {
            // fresh and populated: reuse
            must_watch(&workdir);
            return Some(AbsPath::new_unchecked(workdir));
        }
        // stale or emptied: fall through to rebuild; the sweep below
        // removes this and any older copies once allocation succeeded
    }

    let workdir = alloc_dir(&source)?;
    extract::skeleton(&workdir, &listing);
    sweep_others(&source);

    QUEUE::start_extract(AbsPath::new_unchecked(source.clone()), workdir.clone())?;

    must_watch(&workdir);
    Some(AbsPath::new_unchecked(workdir))
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

/// The shared-queue row of a started extraction of `source`, if any.
fn running_row(source: &Path) -> Option<crate::run::queue::QueueItem> {
    let state = QUEUE_STATE.lock().unwrap();
    state
        .shared
        .iter()
        .find(|i| {
            i.kind == EXTRACT_KIND
                && i.status.state.is_started()
                && i.src
                    .first()
                    .is_some_and(|p| p.as_os_str() == source.as_os_str())
        })
        .cloned()
}
/// The workdir of a started extraction of `source`.
fn running_workdir(source: &Path) -> Option<PathBuf> {
    running_row(source).map(|i| PathBuf::from(i.dst.as_os_str()))
}

/// Tell the watcher to keep reloading `temp` through event storms (the
/// extraction itself produces them) until it pauses or is switched away.
fn must_watch(temp: &Path) {
    GLOBAL::send_watcher(WatcherMessage::MustWatch(temp.to_path_buf()));
}

pub fn toast_entering(path: &Path) {
    let source = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
    let extracting = running_row(&source).is_some();

    if extracting {
        let name = short_display_span(path);
        TOAST::push_with_flag(
            ToastStyle::Info,
            "Extracting: ",
            [name],
            ToastFlags::PERSIST_CURSOR | ToastFlags::PERSIST_PANE,
        );
    } else {
        TOAST::msg(Span::styled("Entering archive", ToastStyle::Info), true);
    }
}

fn short_display_span(path: &Path) -> Span<'static> {
    crate::run::item::short_display(path)
}

// -------------------------- skeleton layout --------------------------

/// Marker file dropped into a skeleton dir when its extraction ends in
/// error (see the pump watcher); makes re-entry report the failure instead
/// of silently showing partial files.
const FAILED_MARKER: &str = ".failed";

/// Allocates the skeleton dir for `source` (named by [`skeleton_name`])
/// plus its archive-name workdir — the extraction destination and the dir
/// the user navigates into.
fn alloc_dir(source: &Path) -> Option<PathBuf> {
    let dir = root().join(skeleton_name(source));
    std::fs::create_dir_all(&dir).ok()?;
    let workdir = workdir_of(&dir, source);
    std::fs::create_dir_all(&workdir).ok()?;
    Some(workdir)
}

/// The workdir inside a skeleton: the archive file name, extension
/// included.
fn workdir_of(
    skel: &Path,
    source: &Path,
) -> PathBuf {
    skel.join(
        source
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| encode_path(source)),
    )
}

/// Skeleton dir name for `source`:
/// `<percent-encoded canonical path>--<unix millis>`. Milliseconds make
/// same-second re-allocation practically impossible; the encoded path
/// keeps the original archive path recoverable from the dir name alone
/// (see [`recover_archive`]).
fn skeleton_name(source: &Path) -> String {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0);
    format!("{}--{now}", encode_path(source))
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

/// Parses the `--<unix millis>` timestamp suffix of a skeleton dir name.
fn skeleton_timestamp(name: &str) -> Option<u64> {
    let (_, suffix) = name.rsplit_once("--")?;
    if suffix.is_empty() || !suffix.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    suffix.parse().ok()
}

/// Archive mtime in unix millis (0 when unknowable — treated as ancient).
fn mtime_secs(source: &Path) -> u64 {
    std::fs::metadata(source)
        .ok()
        .and_then(|m| m.modified().ok())
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

/// Whether an extraction workdir holds no entries. A missing directory
/// counts as empty.
fn workdir_is_empty(workdir: &Path) -> bool {
    std::fs::read_dir(workdir)
        .map(|mut it| it.next().is_none())
        .unwrap_or(true)
}

/// The freshest existing skeleton decoding back to `source`:
/// `(skeleton dir, timestamp)`.
fn freshest_skeleton(source: &Path) -> Option<(PathBuf, u64)> {
    let mut best: Option<(PathBuf, u64)> = None;
    for entry in std::fs::read_dir(root()).ok()?.flatten() {
        let name = entry.file_name();
        let Some(name) = name.to_str() else {
            continue;
        };
        if recover_archive(name).as_deref() != Some(source) {
            continue;
        }
        let Some(ts) = skeleton_timestamp(name) else {
            continue;
        };
        if best.as_ref().is_none_or(|(_, bts)| ts > *bts) {
            best = Some((entry.path(), ts));
        }
    }
    best
}

/// Removes every skeleton decoding back to `source` except the one just
/// allocated. Spawned on the blocking pool: deletion of large stale trees
/// must not stall navigation.
fn sweep_others(source: &Path) {
    let keep = skeleton_name(source);
    let source = source.to_path_buf();
    TASKS::spawn_blocking("unzip stale cleanup", move || {
        let Ok(entries) = std::fs::read_dir(root()) else {
            return;
        };
        for entry in entries.flatten() {
            let name = entry.file_name();
            let Some(name) = name.to_str() else {
                continue;
            };
            if name == keep {
                continue;
            }
            if recover_archive(name).as_deref() == Some(&source) {
                let _ = std::fs::remove_dir_all(entry.path());
            }
        }
    });
}
