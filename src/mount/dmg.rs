//! The disk-image strategy: attach an image through the platform backend.
//!
//! A filesystem appears at its mount point all at once, never entry by entry,
//! so there is nothing to navigate into until the attach has finished. The
//! mount point is the backend's choice (macOS: `/Volumes/<volume name>`,
//! Linux: `/run/media/<user>/<label>`, each resolving name collisions), so it
//! is known only once the backend has mounted or reported an existing mount.

use std::{
    path::{Path, PathBuf},
    sync::Mutex,
};

use crate::{
    abspath::AbsPath,
    run::{
        action::FsAction,
        item::short_display,
        state::{GLOBAL, TASKS, TOAST, ToastStyle},
    },
};

use super::{MountRegistry, MountRow, backend, is_mount_point, lock};

/// Attaches this process is waiting on: a second advance into the same image
/// (or a mount that has not finished) must not attach it twice, since only one
/// of the two mounts would be ours to detach.
static MOUNTING: Mutex<Vec<PathBuf>> = Mutex::new(Vec::new());

/// Whether `path` is an image file mountable on this platform.
pub fn supported(path: &Path) -> bool {
    backend::supported(path)
}

/// Make the image at `source` navigable: adopt an existing external attach,
/// or start one and let its completion register the mount points.
pub fn mount(source: &AbsPath) -> Result<Option<AbsPath>, String> {
    let source = super::source_key(source.as_path());

    // already attached by someone else (Finder, another fist, an earlier run):
    // adopt that mount instead of attaching a second copy of the same image,
    // and show it in the picker without claiming it for shutdown
    if let Some(points) = backend::attached_at(&source) {
        log::debug!("Reusing existing mount of {}", source.display());
        register_points(&source, &points, false);
        if points.len() > 1 {
            GLOBAL::send_action(FsAction::ShowMounts);
            return Ok(None);
        }
        return Ok(points.into_iter().next().map(AbsPath::new_unchecked));
    }

    submit(&source)?;
    Ok(None)
}

/// Unmount the volume at `mount_point`.
pub fn unmount(mount_point: &Path) -> Result<(), String> {
    backend::detach(mount_point)
}

/// Whether a filesystem is still mounted at `dest`.
pub fn is_live(dest: &Path) -> bool {
    is_mount_point(dest)
}

/// The asynchronous half of [`mount`]: attach `source` on the blocking pool,
/// registering its mount points when they are up. A mount of the same image
/// already in flight makes this a no-op.
fn submit(source: &Path) -> Result<(), String> {
    if !start_mounting(source) {
        return Ok(());
    }

    let label = short_display(source);
    TOAST::push(ToastStyle::Info, "Mounting: ", [label.clone()]);

    let owned = source.to_path_buf();
    TASKS::spawn_blocking("mount image", move || {
        let result = backend::mount(&owned);
        stop_mounting(&owned);
        TOAST::pop("Mounting: ", &label);

        match result {
            Ok(points) => {
                register_points(&owned, &points, true);
                // the volume just mounted is the picker's first row, and the
                // picker is the only way to choose among several of them
                GLOBAL::send_action(FsAction::ShowMounts);
            }
            Err(err) => TOAST::notice(
                ToastStyle::Error,
                format!("Failed to mount {}: {err}", label.content),
            ),
        }
    });
    Ok(())
}

/// Register one row per mount point, as ours (this process attached them, so
/// shutdown detaches them) or adopted (someone else attached them).
fn register_points(
    source: &Path,
    points: &[PathBuf],
    ours: bool,
) {
    for point in points {
        let row = if ours {
            MountRow::attached(source.to_path_buf(), point.clone())
        } else {
            MountRow::adopted(source.to_path_buf(), point.clone())
        };
        MountRegistry::register(row);
    }
}

/// Claim the attach for `source`; false when an attach of it is already
/// running.
fn start_mounting(source: &Path) -> bool {
    let mut mounting = lock(&MOUNTING);
    if mounting.iter().any(|path| path == source) {
        return false;
    }
    mounting.push(source.to_path_buf());
    true
}

fn stop_mounting(source: &Path) {
    lock(&MOUNTING).retain(|path| path != source);
}
