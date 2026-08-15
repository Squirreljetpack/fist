//! Background archive extraction into temporary skeletons.
//!
//! Advancing into an archive synchronously creates a directory-only
//! skeleton of its contents under the app temp dir ([`init`]), queues the
//! full extraction on a background worker pool, and lets the fs watcher
//! surface progress by keeping the skeleton dir's reloads unthrottled
//! (see [`WatcherMessage::MustWatch`]).
//!
//! Format support lives behind [`ArchiveBackend`]: the `decompress` crate
//! covers zip/tar family/ar/rar, and sevenz-rust2 covers 7z.
//!
//! Every source path maps to exactly one skeleton for the lifetime of the
//! process: re-entering an archive reuses its skeleton instead of
//! extracting again. Regenerating the contents requires exiting the app —
//! all skeletons are removed on exit. A hard exit strands the per-process
//! root under the system temp dir, where the system tmp cleaner reclaims it.

use std::{
    collections::HashMap,
    path::{Component, Path, PathBuf},
    sync::{Arc, Mutex, OnceLock, mpsc},
    thread,
};

use matchmaker::nucleo::{Color, Span, Style};

use crate::{
    abspath::AbsPath,
    cli::paths::__unzip,
    run::state::{GLOBAL, TOAST, ToastStyle},
    watcher::WatcherMessage,
};

mod detect;

// -------------------------- backends --------------------------

/// One entry in an archive listing.
struct EntryMeta {
    path: PathBuf,
    is_dir: bool,
}

/// A format backend: detection, cheap listing, and full extraction.
trait ArchiveBackend: Send + Sync {
    fn id(&self) -> &'static str;
    fn detect(
        &self,
        path: &Path,
    ) -> bool;
    fn list(
        &self,
        source: &Path,
    ) -> Result<Vec<EntryMeta>, String>;
    fn extract_all(
        &self,
        source: &Path,
        dest: &Path,
    ) -> Result<(), String>;
}

/// zip, tar family, ar, and rar via the `decompress` crate.
struct DecompressBackend;

impl ArchiveBackend for DecompressBackend {
    fn id(&self) -> &'static str {
        "decompress"
    }

    fn detect(
        &self,
        path: &Path,
    ) -> bool {
        detect::is_decompress_archive(path)
    }

    fn list(
        &self,
        source: &Path,
    ) -> Result<Vec<EntryMeta>, String> {
        // content detection so archives matched by sniffing (unknown
        // extension) list and extract through the same decompressor
        let opts = decompress::ExtractOptsBuilder::default()
            .detect_content(true)
            .build()
            .map_err(|e| e.to_string())?;
        let listing = decompress::list(source, &opts).map_err(|e| e.to_string())?;
        Ok(listing
            .entries
            .into_iter()
            .map(|entry| {
                let is_dir = entry.ends_with('/');
                EntryMeta {
                    path: PathBuf::from(entry),
                    is_dir,
                }
            })
            .collect())
    }

    fn extract_all(
        &self,
        source: &Path,
        dest: &Path,
    ) -> Result<(), String> {
        // the filter sees the joined destination path: keep everything
        // that stays inside the skeleton
        let captured = dest.to_path_buf();
        let opts = decompress::ExtractOptsBuilder::default()
            .detect_content(true)
            .filter(move |p| {
                p.starts_with(&captured) && is_safe(p.strip_prefix(&captured).unwrap_or(p))
            })
            .build()
            .map_err(|e| e.to_string())?;
        decompress::decompress(source, dest, &opts)
            .map(|_| ())
            .map_err(|e| e.to_string())
    }
}

/// 7z via sevenz-rust2 (pure Rust): header-only listing, whole-archive
/// extraction.
struct SevenZBackend;

impl ArchiveBackend for SevenZBackend {
    fn id(&self) -> &'static str {
        "sevenz"
    }

    fn detect(
        &self,
        path: &Path,
    ) -> bool {
        path.extension()
            .and_then(|e| e.to_str())
            .is_some_and(|e| e.eq_ignore_ascii_case("7z"))
    }

    fn list(
        &self,
        source: &Path,
    ) -> Result<Vec<EntryMeta>, String> {
        let archive = sevenz_rust2::Archive::open(source).map_err(|e| e.to_string())?;
        Ok(archive
            .files
            .iter()
            .map(|entry| EntryMeta {
                path: PathBuf::from(entry.name()),
                is_dir: entry.is_directory(),
            })
            .collect())
    }

    fn extract_all(
        &self,
        source: &Path,
        dest: &Path,
    ) -> Result<(), String> {
        sevenz_rust2::decompress_file(source, dest).map_err(|e| e.to_string())
    }
}

fn backends() -> &'static [Box<dyn ArchiveBackend>] {
    static BACKENDS: OnceLock<Vec<Box<dyn ArchiveBackend>>> = OnceLock::new();
    BACKENDS.get_or_init(|| vec![Box::new(DecompressBackend), Box::new(SevenZBackend)])
}

fn backend_for(path: &Path) -> Option<&'static dyn ArchiveBackend> {
    backends()
        .iter()
        .find(|backend| backend.detect(path))
        .map(|backend| backend.as_ref())
}

// -------------------------- worker --------------------------

/// Root of every extraction skeleton:
/// `<tmp>/fist/<process-id>/unzipped_storage_press_undo_to_go_back`.
fn root() -> PathBuf {
    __unzip().to_path_buf()
}

/// Extraction worker pool size.
const MAX_WORKERS: usize = 4;

/// Lifecycle of a registered archive.
enum EntryState {
    /// Extraction queued or in progress.
    Skeleton,
    Complete,
    Failed,
}

/// One registered archive: its skeleton dir and lifecycle state.
struct Entry {
    /// Skeleton dir the archive is extracted into.
    temp: PathBuf,
    /// Active-user counter, reserved for refcounted cleanup.
    #[allow(dead_code)]
    active: u32,
    state: EntryState,
}

/// One extraction job handed to a worker thread.
struct Job {
    /// Canonical archive path (also the map key).
    source: PathBuf,
    temp: PathBuf,
}

struct Worker {
    /// Job channel; dropping the sender stops the worker pool.
    jobs: Mutex<Option<mpsc::Sender<Job>>>,
    /// Source path → entry, consulted by [`init`] to avoid re-extracting.
    entries: Mutex<HashMap<PathBuf, Entry>>,
}

static WORKER: OnceLock<Worker> = OnceLock::new();

fn worker() -> &'static Worker {
    WORKER.get().expect("unzip::start not called")
}

/// Spawn the extraction worker pool.
pub fn start() {
    let (tx, rx) = mpsc::channel();
    let rx = Arc::new(Mutex::new(rx));
    let workers = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(1)
        .min(MAX_WORKERS);

    let mut spawned = 0;
    for _ in 0..workers {
        let rx = Arc::clone(&rx);
        let handle = thread::Builder::new()
            .name("fist-unzip".into())
            .spawn(move || {
                loop {
                    let job = {
                        let Ok(rx) = rx.lock() else {
                            return;
                        };
                        rx.recv()
                    };
                    match job {
                        Ok(job) => extract(job),
                        // channel closed: stop
                        Err(_) => return,
                    }
                }
            })
            .inspect_err(|e| log::error!("Failed to spawn unzip worker: {e}"));
        if handle.is_ok() {
            spawned += 1;
        }
        // workers are detached: exit must not block on an in-flight
        // extraction, and the process exit kills them anyway
    }

    let _ = WORKER.set(Worker {
        jobs: Mutex::new((spawned > 0).then_some(tx)),
        entries: Mutex::new(HashMap::new()),
    });

    // best-effort cleanup for exit paths that skip [`shutdown`]
    // (e.g. `std::process::exit`) — leftovers are swept on the next start
    #[cfg(unix)]
    unsafe {
        libc::atexit(cleanup_on_exit);
    }
}

/// Removes the whole skeleton root. Registered with `atexit` so hard exits
/// also clean up; best effort, as worker threads may still be
/// mid-extraction.
#[cfg(unix)]
extern "C" fn cleanup_on_exit() {
    let _ = std::fs::remove_dir_all(root());
}

/// Whether `path` is an archive a backend can extract.
pub fn supported(path: &Path) -> bool {
    backend_for(path).is_some()
}

/// Synchronously create (or reuse) the extraction skeleton for `path`,
/// queue its extraction on the worker pool, and re-arm must-watch on the
/// skeleton dir.
///
/// Returns the skeleton dir to navigate into; `None` means the archive
/// could not be listed (or previously failed) and the caller should handle
/// the path normally.
pub fn init(path: &Path) -> Option<AbsPath> {
    let source = path.canonicalize().ok()?;
    let backend = backend_for(&source)?;
    let w = worker();

    // re-entering an archive reuses its skeleton — extraction may still be
    // running or may already be done
    if let Some(entry) = w.entries.lock().ok()?.get(&source) {
        if matches!(entry.state, EntryState::Failed) {
            TOAST::notice(
                ToastStyle::Error,
                format!("Extraction previously failed for {}", entry.temp.display()),
            );
            return None;
        }
        must_watch(&entry.temp);
        toast_entering();
        return Some(AbsPath::new_unchecked(entry.temp.clone()));
    }

    log::info!(
        "Entering archive {} via backend {}",
        source.display(),
        backend.id()
    );
    let listing = match backend.list(&source) {
        Ok(listing) => listing,
        Err(e) => {
            log::error!("Failed to list archive {}: {e}", source.display());
            return None;
        }
    };

    let temp = alloc_dir(&source)?;
    for entry in &listing {
        skeleton_dir(&temp, entry);
    }

    // register before queuing so a re-entry can never double-extract
    w.entries.lock().ok()?.insert(
        source.clone(),
        Entry {
            temp: temp.clone(),
            active: 0,
            state: EntryState::Skeleton,
        },
    );

    if let Some(jobs) = w.jobs.lock().ok().and_then(|jobs| jobs.clone())
        && jobs
            .send(Job {
                source,
                temp: temp.clone(),
            })
            .is_err()
    {
        log::error!(
            "Unzip worker not running; extraction of {} was dropped",
            temp.display()
        );
    }

    must_watch(&temp);
    toast_entering();
    Some(AbsPath::new_unchecked(temp))
}

/// Allocates the skeleton dir for `source`, named by [`skeleton_name`].
fn alloc_dir(source: &Path) -> Option<PathBuf> {
    let dir = root().join(skeleton_name(source));
    std::fs::create_dir_all(&dir).ok()?;
    Some(dir)
}

/// Skeleton dir name for `source`: `ar--<escaped path>--<hash>` — the
/// canonical path with every slash replaced by `--` (leading dashes
/// trimmed), plus a hash of the canonical path, so distinct archives never
/// share a dir.
fn skeleton_name(source: &Path) -> String {
    let escaped = source
        .to_string_lossy()
        .replace('/', "--")
        .trim_start_matches('-')
        .to_string();
    format!("ar--{escaped}--{:x}", hash_path(source))
}

fn hash_path(path: &Path) -> u64 {
    use std::hash::{Hash, Hasher};

    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    path.hash(&mut hasher);
    hasher.finish()
}

/// Creates the directories implied by one archive entry: the entry itself
/// when it is an explicit directory, else its parent chain.
fn skeleton_dir(
    root: &Path,
    entry: &EntryMeta,
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

/// Runs on a worker thread.
fn extract(job: Job) {
    let backend = backend_for(&job.source);
    let result = match backend {
        Some(backend) => backend.extract_all(&job.source, &job.temp),
        None => Err("no backend recognizes this archive".into()),
    };

    // update lifecycle state
    if let Ok(mut entries) = worker().entries.lock()
        && let Some(entry) = entries.get_mut(&job.source)
    {
        entry.state = if result.is_ok() {
            EntryState::Complete
        } else {
            EntryState::Failed
        };
    }

    match result {
        Ok(()) => toast_extracted(&job.source),
        Err(e) => {
            let id = backend.map(|backend| backend.id()).unwrap_or("none");
            log::error!(
                "Failed to extract {} (backend {id}): {e}",
                job.source.display()
            );
            toast_extract_error(&job, &e);
        }
    }
}

/// Tell the watcher to keep reloading `temp` through event storms (the
/// extraction itself produces them) until it pauses or is switched away.
fn must_watch(temp: &Path) {
    GLOBAL::send_watcher(WatcherMessage::MustWatch(temp.to_path_buf()));
}

fn toast_entering() {
    TOAST::notice(ToastStyle::Info, "Entering archive");
}

fn toast_extracted(source: &Path) {
    let name = source
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| source.to_string_lossy().to_string());
    TOAST::msg(
        vec![
            Span::styled("Extracted: ", Style::new().fg(Color::Red)),
            Span::raw(name),
        ],
        true,
    );
}

fn toast_extract_error(
    job: &Job,
    msg: &str,
) {
    TOAST::notice(
        ToastStyle::Error,
        format!("Failed to extract {}: {msg}", job.source.display()),
    );
}

/// Stops the worker pool and removes every skeleton. Called on graceful
/// exit. Workers are detached rather than joined: exit must not block on an
/// in-flight extraction, and skeletons are ephemeral — writes that race the
/// removal below die with the process, whose per-process root is unique.
pub fn shutdown() {
    let Some(w) = WORKER.get() else {
        return;
    };

    // closing the channel makes the workers stop after their current jobs
    drop(w.jobs.lock().ok().and_then(|mut jobs| jobs.take()));

    if let Ok(mut entries) = w.entries.lock() {
        for (_, entry) in entries.drain() {
            if let Err(e) = std::fs::remove_dir_all(&entry.temp) {
                log::warn!("Failed to remove skeleton {:?}: {e}", entry.temp);
            }
        }
    }
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
    fn backend_detection() {
        let dir = std::env::temp_dir().join(format!("fist-unzip-detect-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();

        // content sniffing: zip and gzip magic
        let zip = dir.join("foo.zip");
        std::fs::write(&zip, b"PK\x03\x04").unwrap();
        assert!(DecompressBackend.detect(&zip));

        let gz = dir.join("foo.tar.gz");
        std::fs::write(&gz, b"\x1f\x8b\x08").unwrap();
        assert!(DecompressBackend.detect(&gz));

        // unrecognized content is not a decompress archive
        let txt = dir.join("bar.txt");
        std::fs::write(&txt, b"plain text").unwrap();
        assert!(!DecompressBackend.detect(&txt));

        // 7z stays extension-based
        assert!(SevenZBackend.detect(Path::new("foo.7z")));
        assert!(SevenZBackend.detect(Path::new("FOO.7Z")));
        assert!(!SevenZBackend.detect(Path::new("foo.zip")));

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn skeleton_names() {
        let path = Path::new("/tmp/fist-archives.x/archive.7z");
        assert_eq!(
            skeleton_name(path),
            format!(
                "ar--tmp--fist-archives.x--archive.7z--{:x}",
                hash_path(path)
            )
        );

        // a path at the root has no leading separator to trim
        let root_path = Path::new("/a.tar");
        assert_eq!(
            skeleton_name(root_path),
            format!("ar--a.tar--{:x}", hash_path(root_path))
        );
    }

    #[test]
    fn skeleton_contains_dirs_only() {
        let dir = std::env::temp_dir().join(format!("fist-unzip-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);

        skeleton_dir(
            &dir,
            &EntryMeta {
                path: PathBuf::from("a/b/c.txt"),
                is_dir: false,
            },
        );
        skeleton_dir(
            &dir,
            &EntryMeta {
                path: PathBuf::from("a/b/"),
                is_dir: true,
            },
        );
        skeleton_dir(
            &dir,
            &EntryMeta {
                path: PathBuf::from("empty/"),
                is_dir: true,
            },
        );
        skeleton_dir(
            &dir,
            &EntryMeta {
                path: PathBuf::from("flat"),
                is_dir: false,
            },
        );
        let evil = format!("../../fist-unzip-evil-{}", std::process::id());
        skeleton_dir(
            &dir,
            &EntryMeta {
                path: PathBuf::from(evil),
                is_dir: true,
            },
        );
        skeleton_dir(
            &dir,
            &EntryMeta {
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
