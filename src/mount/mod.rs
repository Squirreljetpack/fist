//! Making files mountable as navigable directories.
//!
//! Two strategies turn a file into a directory:
//!
//! - [`MountStrategy::Dmg`] attaches a disk image through the platform
//!   backend (udisks on Linux, hdiutil on macOS); a filesystem appears at its
//!   mount point all at once, so there is nothing to navigate into until the
//!   attach has finished.
//! - [`MountStrategy::Extract`] unpacks an archive into a free-named
//!   directory under the per-process extraction root; the directory fills in
//!   entry by entry, which the watcher surfaces live.
//!
//! [`MountStrategy::mount`] is how an advance turns a file into a directory:
//! `Ok(Some(dest))` when there is something to enter right away, `Ok(None)`
//! when the mount has to settle first, `Err` when it cannot be made. Every
//! destination is recorded in the [`MountRegistry`] — one row per destination
//! — which is what the mounts picker lists and what shutdown walks.

#[cfg(any(target_os = "macos", test))]
mod hdiutil;
#[cfg(not(target_os = "macos"))]
mod udisks;

#[cfg(target_os = "macos")]
use self::hdiutil as backend;
#[cfg(not(target_os = "macos"))]
use self::udisks as backend;

mod dmg;

use std::{
    path::{Path, PathBuf},
    sync::{Mutex, MutexGuard},
};

use crate::abspath::AbsPath;

/// How a file is made navigable.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MountStrategy {
    /// A disk image attached by the platform backend.
    Dmg,
    /// An archive extracted into the per-process extraction root.
    Extract,
}

impl MountStrategy {
    /// The strategy that handles `path`, if any.
    pub fn detect(path: &Path) -> Option<Self> {
        if crate::unzip::supported(path) {
            Some(Self::Extract)
        } else if dmg::supported(path) {
            Some(Self::Dmg)
        } else {
            None
        }
    }

    /// Make `source` navigable. `Ok(Some(dest))` is the directory to enter
    /// right away; `Ok(None)` means the mount is being prepared (its registry
    /// row lands on completion) or has several destinations to choose from;
    /// `Err` carries the strategy's own failure message.
    pub fn mount(
        self,
        source: &AbsPath,
    ) -> Result<Option<AbsPath>, String> {
        match self {
            Self::Dmg => dmg::mount(source),
            Self::Extract => crate::unzip::mount(source),
        }
    }

    /// Undo the materialization at `dest`, when the strategy has anything to
    /// undo.
    pub fn unmount(
        self,
        dest: &Path,
    ) -> Result<(), String> {
        match self {
            Self::Dmg => dmg::unmount(dest),
            Self::Extract => crate::unzip::unmount(dest),
        }
    }

    /// Whether the destination is still usable: a mounted filesystem, or a
    /// directory holding an extraction's contents.
    pub fn is_live(
        self,
        dest: &Path,
    ) -> bool {
        match self {
            Self::Dmg => dmg::is_live(dest),
            Self::Extract => crate::unzip::is_live(dest),
        }
    }
}

/// How a mount ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MountStatus {
    /// The destination is complete (an extraction) / the volumes are up (an
    /// image).
    Success,
    /// The extraction failed; the destination holds partial contents.
    Failed,
}

/// One destination this process materialized: where it came from, how, how it
/// ended, and whether exiting takes it down.
#[derive(Debug, Clone, PartialEq)]
pub struct MountRow {
    /// The file that was mounted (canonical when it could be resolved).
    pub source: PathBuf,
    /// The directory it was mounted at.
    pub dest: PathBuf,
    pub strategy: MountStrategy,
    pub status: MountStatus,
    /// Whether this process materialized the destination, as opposed to
    /// adopting a mount someone else made. Authorship is tracked separately
    /// from [`Self::cleanup`] because the exit policy is per entry: an adopted
    /// mount can be claimed, and a mount of our own can be left alone.
    pub ours: bool,
    /// Whether [`shutdown`] undoes this row. True for images this process
    /// attached; false for adopted mounts (someone else's to unmount) and for
    /// extractions (the per-process extraction root is removed as a whole, and
    /// destinations chosen through the extract menu belong to the user).
    pub cleanup: bool,
}

impl MountRow {
    /// An image this process attached: ours to detach on exit.
    pub fn attached(
        source: PathBuf,
        dest: PathBuf,
    ) -> Self {
        Self {
            source,
            dest,
            strategy: MountStrategy::Dmg,
            status: MountStatus::Success,
            ours: true,
            cleanup: true,
        }
    }

    /// An image that was already attached when the advance saw it: navigable,
    /// but someone else's to unmount.
    pub fn adopted(
        source: PathBuf,
        dest: PathBuf,
    ) -> Self {
        Self {
            source,
            dest,
            strategy: MountStrategy::Dmg,
            status: MountStatus::Success,
            ours: false,
            cleanup: false,
        }
    }

    /// An extraction this process made: a preview under the per-process root,
    /// or a destination the user chose through the extract menu.
    pub fn extraction(
        source: PathBuf,
        dest: PathBuf,
        status: MountStatus,
    ) -> Self {
        Self {
            source,
            dest,
            strategy: MountStrategy::Extract,
            status,
            ours: true,
            cleanup: false,
        }
    }

    /// Whether the destination is still there.
    pub fn is_live(&self) -> bool {
        self.strategy.is_live(&self.dest)
    }
}

/// Every destination this process materialized, oldest first. The picker and
/// [`MountRegistry::rows_for`] read it, mount completions write it.
static REGISTRY: Mutex<Vec<MountRow>> = Mutex::new(Vec::new());

/// Namespace for the mount registry.
pub struct MountRegistry;

impl MountRegistry {
    /// Record a settled mount, replacing an earlier row for the same
    /// destination so a retried mount does not leave a stale duplicate.
    pub fn register(row: MountRow) {
        arm_exit_hook();
        let mut registry = lock(&REGISTRY);
        registry.retain(|existing| existing.dest != row.dest);
        registry.push(row);
    }

    /// The rows `source` materialized, oldest first: none when it has not
    /// been mounted before, one for the usual single destination, several for
    /// a multi-volume image or repeated attempts. Reading prunes rows whose
    /// destination is gone, and it moves the source's rows to the newest
    /// position of the picker's order.
    pub fn rows_for(source: &Path) -> Vec<MountRow> {
        let source = source_key(source);
        let mut registry = lock(&REGISTRY);
        registry.retain(MountRow::is_live);

        let (used, rest): (Vec<MountRow>, Vec<MountRow>) =
            registry.drain(..).partition(|row| row.source == source);
        registry.extend(used.iter().cloned());
        registry.extend(rest);

        used
    }

    /// Every live row, newest first, for the mounts picker.
    pub fn rows() -> Vec<MountRow> {
        let mut registry = lock(&REGISTRY);
        registry.retain(MountRow::is_live);
        registry.iter().rev().cloned().collect()
    }

    /// Whether any live row is registered, for the menu's mounts entry.
    pub fn is_empty() -> bool {
        let mut registry = lock(&REGISTRY);
        registry.retain(MountRow::is_live);
        registry.is_empty()
    }

    /// The source that materialized `dest`: a settled registry row, or the
    /// extraction currently writing there.
    pub fn source_of(dest: &Path) -> Option<PathBuf> {
        Self::find_dest(dest)
            .map(|row| row.source)
            .or_else(|| crate::unzip::running_source(dest))
    }

    /// The live row for `dest`, if any.
    fn find_dest(dest: &Path) -> Option<MountRow> {
        lock(&REGISTRY)
            .iter()
            .find(|row| row.dest == dest && row.is_live())
            .cloned()
    }

    /// Drop the rows for `dest` without unmounting anything; used when the
    /// destination is already gone.
    pub fn forget(dest: &Path) -> bool {
        let mut registry = lock(&REGISTRY);
        let before = registry.len();
        registry.retain(|row| row.dest != dest);
        before != registry.len()
    }
}

/// The mount point `path` lies in, when this process has an image mounted
/// there. Adopted mounts qualify: an explicit unmount is the user's call,
/// only detaching on exit is limited to our own attaches.
pub fn unmountable(path: &Path) -> Option<PathBuf> {
    let rows = MountRegistry::rows();
    let mut current = if path.is_dir() {
        Some(path)
    } else {
        path.parent()
    };

    while let Some(dir) = current {
        if rows
            .iter()
            .any(|row| row.dest == dir && row.strategy == MountStrategy::Dmg)
        {
            return Some(dir.to_path_buf());
        }
        current = dir.parent();
    }
    None
}

/// Detach the volume mounted at `mount_point` and drop its registry rows.
pub fn unmount(mount_point: &Path) -> Result<(), String> {
    MountRegistry::forget(mount_point);
    dmg::unmount(mount_point)
}

/// Undo every mount flagged `cleanup`, synchronously. Runs on the shutdown
/// path next to [`crate::unzip::shutdown`], once background work has stopped.
pub fn shutdown() {
    for row in std::mem::take(&mut *lock(&REGISTRY)) {
        // one detach per image is enough: the attachers act on the device, so
        // the remaining points of a multi-volume image are gone with the
        // first, and an externally ejected one has nothing left to detach
        if !row.cleanup || !row.is_live() {
            continue;
        }
        if let Err(err) = row.strategy.unmount(&row.dest) {
            log::warn!("Failed to unmount {}: {err}", row.dest.display());
        }
    }
}

/// Canonical key for the registry: the resolved source path when it can be
/// resolved, the given path otherwise.
fn source_key(source: &Path) -> PathBuf {
    source
        .canonicalize()
        .unwrap_or_else(|_| source.to_path_buf())
}

/// A registry lock, tolerating poisoning: the payloads are plain data, so a
/// panic elsewhere must not make mounting (or the exit hook) panic in turn.
fn lock<T>(mutex: &'static Mutex<T>) -> MutexGuard<'static, T> {
    match mutex.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    }
}

/// Whether a filesystem is mounted at `path`, i.e. its device differs from its
/// parent's. False when either path cannot be stat'ed.
#[cfg(unix)]
pub fn is_mount_point(path: &Path) -> bool {
    use std::os::unix::fs::MetadataExt;

    let dev = |p: &Path| p.metadata().map(|meta| meta.dev()).ok();
    match (path.parent().and_then(dev), dev(path)) {
        (Some(parent), Some(current)) => parent != current,
        _ => false,
    }
}

#[cfg(not(unix))]
pub fn is_mount_point(_path: &Path) -> bool {
    false
}

/// Arms the best-effort exit hook once, on first registration: mounts are
/// detached even on exit paths that skip the shutdown sequence.
#[cfg(target_os = "macos")]
fn arm_exit_hook() {
    use std::sync::OnceLock;

    static ARMED: OnceLock<()> = OnceLock::new();
    ARMED.get_or_init(|| unsafe {
        libc::atexit(detach_on_exit);
    });
}

#[cfg(target_os = "macos")]
extern "C" fn detach_on_exit() {
    shutdown();
}

#[cfg(not(target_os = "macos"))]
fn arm_exit_hook() {}
