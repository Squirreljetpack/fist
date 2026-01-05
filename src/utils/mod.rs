pub mod categories;
pub mod colors;
pub mod filetypes;
pub mod icons;
pub mod path;
pub mod serde;
pub mod text;

use std::path::Path;

pub fn file_size(path: &Path) -> u64 {
    std::fs::metadata(path).map(|m| m.len()).unwrap_or(0)
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

#[macro_export]
macro_rules! arr {
    ( $( $x:expr ),* $(,)? ) => {
        {
            arrayvec::ArrayVec::from_iter([$($x),*])
        }
    };
}
