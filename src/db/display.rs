use cba::{bog::BogUnwrapExt, prints};
use chrono::{DateTime, Local};
use comfy_table::{ContentArrangement, Row, Table, presets::UTF8_FULL};

use crate::db::{Entry, Epoch};

/// Print a formatted table to stdout.
///
/// `lambda`: when `None`, the "Last Accessed" column shows a formatted
/// date; when `Some` (EMS mode), it shows the raw tick count and an extra
/// "Score" column appears with `{:.2}` formatting.
pub fn display_entries(
    entries: &[Entry],
    lambda: Option<f64>,
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
            row_cells.push(format!("{:.2}", entry.score));
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
