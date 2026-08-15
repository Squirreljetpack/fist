use cba::{bog::BogUnwrapExt, prints};
use chrono::{DateTime, Local};
use comfy_table::{ContentArrangement, Row, Table, presets::UTF8_FULL};

use crate::db::{Entry, Epoch, zoxide};

/// Print a formatted table to stdout.
///
/// `lambda`: when `None`, the "Last Accessed" column shows a formatted
/// date; when `Some` (EMS mode), it shows the raw tick count and an extra
/// "Score" column appears, populated from [`zoxide::score`].
///
/// `now`: the reference epoch used for scoring. For EMS mode this should
/// be `MAX(atime)` (matching the SQL `ORDER BY` in
/// [`crate::db::Connection::get_entries_range`]); for wall-clock mode it
/// should be the current wall-clock time.
pub fn display_entries(
    entries: &[Entry],
    lambda: Option<f64>,
    now: Epoch,
) {
    let mut table = Table::new();

    // Style
    table.load_preset(UTF8_FULL);
    table.set_content_arrangement(ContentArrangement::Dynamic);

    // Header row
    let mut headers = vec![
        "Name",
        "Path",
        "Alias",
        if lambda.is_none() {
            "Last Accessed"
        } else {
            "Last Access (tick)"
        },
        "Count",
    ];
    if lambda.is_some() {
        headers.push("Score");
    }
    table.set_header(Row::from(headers));

    // Add rows
    for entry in entries {
        let atime_str = if lambda.is_none() {
            display_epoch(entry.atime)
        } else {
            entry.atime.to_string()
        };

        let mut row_cells = vec![
            entry.name.as_str().to_string(),
            entry.path.to_string_lossy().to_string(),
            entry.alias.clone(),
            atime_str,
            entry.count.to_string(),
        ];
        if lambda.is_some() {
            // Use the live, decayed score used to sort entries — matches the
            // SQL `score * exp(-λ * (MAX(atime) - atime))` order-by in
            // `Connection::get_entries_range`.
            row_cells.push(zoxide::score(now, entry, lambda).to_string());
        }

        let row = Row::from(row_cells);
        table.add_row(row);
    }

    // Print table
    prints!(table.to_string());
}

pub fn display_epoch(epoch: Epoch) -> String {
    let naive = DateTime::from_timestamp(epoch, 0)._ebog("Invalid epoch");
    let local_dt: DateTime<Local> = DateTime::from(naive);
    local_dt.format("%d-%m-%y %H:%M:%S").to_string()
}

/// Formats a byte count as a human-readable size, with a `decimal`
/// flag selecting the unit system:
/// - `decimal = true`  -> 1000-based SI units (B, KB, MB, GB, TB, PB)
/// - `decimal = false` -> 1024-based IEC units (B, KiB, MiB, GiB, TiB, PiB)
pub fn human_size(
    bytes: u64,
    decimal: bool,
) -> String {
    const DECIMAL_UNITS: [&str; 6] = ["B", "KB", "MB", "GB", "TB", "PB"];
    const BINARY_UNITS: [&str; 6] = ["B", "KiB", "MiB", "GiB", "TiB", "PiB"];

    if bytes == 0 {
        return "0 B".to_string();
    }

    let base = if decimal { 1000.0 } else { 1024.0 };
    let units = if decimal { DECIMAL_UNITS } else { BINARY_UNITS };

    let mut size = bytes as f64;
    let mut unit_idx = 0;
    while size >= base && unit_idx < units.len() - 1 {
        size /= base;
        unit_idx += 1;
    }

    if unit_idx == 0 {
        format!("{bytes} {}", units[0])
    } else {
        format!("{size:.1} {}", units[unit_idx])
    }
}
