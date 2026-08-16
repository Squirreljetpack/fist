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
//! Every source path maps to one current skeleton for the lifetime of the
//! process: re-entering an archive reuses its skeleton unless the archive
//! changed after the skeleton was created, in which case the skeleton is
//! regenerated. The skeleton dir is named `<percent-encoded path>--<unix
//! seconds>` under the unzip root, and holds one subfolder named after the
//! archive (extension included) which is the extraction workdir — so the
//! archive's absolute path is recoverable from the skeleton dir name alone
//! (see [`recover_archive`]). Removing all skeletons happens on exit; a
//! hard exit strands the per-process root under the system temp dir, where
//! the system tmp cleaner reclaims it.

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
    config::ArchiveConfig,
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
pub fn root() -> PathBuf {
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
    /// Skeleton dir holding the archive-name workdir.
    temp: PathBuf,
    /// The extraction workdir (the archive-name subfolder) the user
    /// navigates into.
    workdir: PathBuf,
    /// Active-user counter, reserved for refcounted cleanup.
    #[allow(dead_code)]
    active: u32,
    state: EntryState,
}

/// One extraction job handed to a worker thread.
struct Job {
    /// Canonical archive path (also the map key).
    source: PathBuf,
    /// Extraction destination: the archive-name workdir.
    temp: PathBuf,
}

struct Worker {
    /// Job channel; dropping the sender stops the worker pool.
    jobs: Mutex<Option<mpsc::Sender<Job>>>,
    /// Source path → entry, consulted by [`init`] to avoid re-extracting.
    entries: Mutex<HashMap<PathBuf, Entry>>,
    /// Extraction settings.
    config: ArchiveConfig,
}

static WORKER: OnceLock<Worker> = OnceLock::new();

fn worker() -> &'static Worker {
    WORKER.get().expect("unzip::start not called")
}

/// Spawn the extraction worker pool.
pub fn start(config: ArchiveConfig) {
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
/// extraction workdir.
///
/// Returns the workdir to navigate into; `None` means the archive could
/// not be listed (or previously failed) and the caller should handle the
/// path normally.
pub fn init(path: &Path) -> Option<AbsPath> {
    let source = path.canonicalize().ok()?;
    let backend = backend_for(&source)?;
    let w = worker();

    // Re-entering an archive reuses its skeleton — extraction may still be
    // running or already done — but only while the skeleton is newer than
    // the archive itself; an archive changed after its skeleton was
    // created makes the cached tree stale and it is regenerated.
    if let Some(entry) = w.entries.lock().ok()?.get(&source) {
        if matches!(entry.state, EntryState::Failed) {
            TOAST::notice(
                ToastStyle::Error,
                format!("Extraction previously failed for {}", entry.temp.display()),
            );
            return None;
        }
        if skeleton_is_fresh(&entry.temp, &source) {
            must_watch(&entry.workdir);
            toast_entering();
            return Some(AbsPath::new_unchecked(entry.workdir.clone()));
        }
        log::info!("Stale skeleton for {}, re-extracting", source.display());
        if w.config.cleanup_duplicates {
            remove_stale_skeletons(&source);
        }
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

    let workdir = alloc_dir(&source)?;
    for entry in &listing {
        skeleton_dir(&workdir, entry);
    }

    // register before queuing so a re-entry can never double-extract
    w.entries.lock().ok()?.insert(
        source.clone(),
        Entry {
            temp: workdir
                .parent()
                .expect("the workdir sits inside its skeleton")
                .to_path_buf(),
            workdir: workdir.clone(),
            active: 0,
            state: EntryState::Skeleton,
        },
    );

    if let Some(jobs) = w.jobs.lock().ok().and_then(|jobs| jobs.clone())
        && jobs
            .send(Job {
                source,
                temp: workdir.clone(),
            })
            .is_err()
    {
        log::error!(
            "Unzip worker not running; extraction of {} was dropped",
            workdir.display()
        );
    }

    must_watch(&workdir);
    toast_entering();
    Some(AbsPath::new_unchecked(workdir))
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
