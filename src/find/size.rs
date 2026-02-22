use rayon::prelude::*;
use std::{
    fs, io,
    path::{Path, PathBuf},
};

fn get_dir_size(path: &Path) -> io::Result<u64> {
    let entries: Vec<_> = fs::read_dir(path)?.collect();

    let total = entries
        .par_iter()
        .filter_map(|entry| match entry {
            Ok(entry) => match entry.metadata() {
                Ok(meta) if meta.is_file() => Some(meta.len()),
                Ok(meta) if meta.is_dir() => get_dir_size(&entry.path()).ok(),
                _ => None,
            },
            _ => None,
        })
        .sum();

    Ok(total)
}

pub fn file_size(path: &Path) -> u64 {
    std::fs::metadata(path).map(|m| m.len()).unwrap_or(0)
}

pub fn recursive_size(path: &Path) -> io::Result<u64> {
    match fs::symlink_metadata(path) {
        Ok(meta) if meta.is_file() => Ok(meta.len()),
        Ok(meta) if meta.is_dir() => get_dir_size(path),
        Ok(_) => Ok(0),
        Err(e) => Err(e),
    }
}

pub fn format_size(bytes: u64) -> String {
    const KB: f64 = 1024.0;
    const MB: f64 = KB * 1024.0;
    const GB: f64 = MB * 1024.0;

    let b = bytes as f64;

    if b >= GB {
        format!("{:.2} GB", b / GB)
    } else if b >= MB {
        format!("{:.2} MB", b / MB)
    } else if b >= KB {
        format!("{:.2} KB", b / KB)
    } else {
        format!("{} B", bytes)
    }
}

pub fn sort_by_size(paths: &mut [PathBuf]) {
    paths.par_sort_unstable_by(|a, b| {
        let sa = recursive_size(a).unwrap_or(0);
        let sb = recursive_size(b).unwrap_or(0);
        sb.cmp(&sa) // descending
    });
}
