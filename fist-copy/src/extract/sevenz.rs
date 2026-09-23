//! 7z archives via `sevenz-rust2`.
//!
//! Listing reads the header only. Extraction streams through
//! [`sevenz_rust2::decompress_file_with_extract_fn`], yielding per-entry
//! progress, chunked byte progress, and mid-stream cancellation.

use std::fs;
use std::io::{self, Write};
use std::path::Path;

#[cfg(target_os = "macos")]
use std::os::macos::fs::FileTimesExt;
#[cfg(windows)]
use std::os::windows::fs::FileTimesExt;

use super::ctx::ExtractCtx;
use super::{ArchiveEntry, safety};

fn open(source: &Path) -> io::Result<sevenz_rust2::Archive> {
    sevenz_rust2::Archive::open(source).map_err(|e| io::Error::other(e.to_string()))
}

pub(crate) fn list(source: &Path) -> io::Result<Vec<ArchiveEntry>> {
    let archive = open(source)?;
    Ok(archive
        .files
        .iter()
        .map(|entry| ArchiveEntry {
            path: entry.name().into(),
            is_dir: entry.is_directory(),
        })
        .collect())
}

/// Extracts every entry into `dest`. Entries report progress as they are
/// written, byte progress is tracked in 64 KiB chunks, and cancellation is
/// checked mid-archive.
pub(crate) fn extract(
    source: &Path,
    dest: &Path,
    ctx: &ExtractCtx<'_>,
) -> io::Result<()> {
    let archive = open(source)?;
    let total_entries = archive.files.len() as u32;
    let mut total_bytes = 0u64;
    let mut unsafe_count = 0u32;

    for entry in &archive.files {
        let normalized = entry.name().replace('\\', "/");
        if !safety::is_safe(Path::new(&normalized)) {
            unsafe_count += 1;
        }
        total_bytes += entry.size();
    }

    if ctx.cancelled() {
        return Err(super::ctx::cancelled());
    }

    ctx.register_bytes(total_bytes);
    ctx.register_entries(total_entries);

    if unsafe_count > 0 {
        for _ in 0..total_entries {
            ctx.entry_failed();
        }
        return Err(io::Error::other(format!(
            "refusing 7z with {unsafe_count} unsafe path(s)"
        )));
    }

    fs::create_dir_all(dest)?;

    let is_solid = archive.is_solid;
    let file = fs::File::open(source)?;
    let mut reader =
        sevenz_rust2::ArchiveReader::from_archive(archive, file, sevenz_rust2::Password::empty());

    let thread_count = if is_solid {
        1
    } else {
        std::thread::available_parallelism()
            .map(|n| n.get() as u32)
            .unwrap_or(1)
    };
    reader.set_thread_count(thread_count);

    let res = reader.for_each_entries(|entry, file_reader| {
        if ctx.cancelled() {
            return Err(sevenz_rust2::Error::from(super::ctx::cancelled()));
        }

        let normalized = entry.name().replace('\\', "/");
        let dest_path = dest.join(normalized);

        if entry.is_directory() {
            if !dest_path.exists() {
                if let Err(e) = fs::create_dir_all(&dest_path) {
                    log::warn!("7z: failed to create dir {:?}: {e}", dest_path);
                    ctx.entry_failed();
                    return Ok(true);
                }
            }
            ctx.entry_ok();
            return Ok(true);
        }

        let write_res = (|| -> io::Result<()> {
            if let Some(parent) = dest_path.parent() {
                if !parent.exists() {
                    fs::create_dir_all(parent)?;
                }
            }

            let file = fs::File::create(&dest_path)?;
            if entry.size() > 0 {
                let mut writer = io::BufWriter::new(file);
                let mut buf = [0u8; 64 * 1024];
                loop {
                    if ctx.cancelled() {
                        return Err(super::ctx::cancelled());
                    }
                    let n = file_reader.read(&mut buf)?;
                    if n == 0 {
                        break;
                    }
                    writer.write_all(&buf[..n])?;
                    ctx.add_copied(n as u64);
                }
                writer.flush()?;
                let file = writer.get_mut();
                apply_file_times(file, entry);
            } else {
                apply_file_times(&file, entry);
            }

            #[cfg(unix)]
            {
                let unix_mode = (entry.windows_attributes() >> 16) as u32;
                if unix_mode != 0 {
                    use std::os::unix::fs::PermissionsExt;
                    let _ = fs::set_permissions(&dest_path, fs::Permissions::from_mode(unix_mode));
                }
            }

            Ok(())
        })();

        match write_res {
            Ok(()) => {
                ctx.entry_ok();
                Ok(true)
            }
            Err(_) if ctx.cancelled() => Err(sevenz_rust2::Error::from(super::ctx::cancelled())),
            Err(e) => {
                log::warn!("7z: failed to extract {:?}: {e}", entry.name());
                ctx.entry_failed();
                let _ = io::copy(file_reader, &mut io::sink());
                Ok(true)
            }
        }
    });

    match res {
        Ok(()) => Ok(()),
        Err(e) => {
            if ctx.cancelled() {
                Err(super::ctx::cancelled())
            } else {
                Err(io::Error::other(e.to_string()))
            }
        }
    }
}

fn apply_file_times(
    file: &fs::File,
    entry: &sevenz_rust2::ArchiveEntry,
) {
    #[allow(unused_mut)]
    let mut file_times = std::fs::FileTimes::new()
        .set_accessed(entry.access_date().into())
        .set_modified(entry.last_modified_date().into());

    #[cfg(any(windows, target_os = "macos"))]
    {
        file_times = file_times.set_created(entry.creation_date().into());
    }

    let _ = file.set_times(file_times);
}
