use chrono::{DateTime, Local};
use cli_boilerplate_automation::{bog::BogUnwrapExt, prints};
use comfy_table::{ContentArrangement, Row, Table, presets::UTF8_FULL};

use crate::db::{Entry, Epoch};

pub fn display_entries(entries: &[Entry]) {
    let mut table = Table::new();

    // Style
    table.load_preset(UTF8_FULL);
    table.set_content_arrangement(ContentArrangement::Dynamic);

    // Header row
    table.set_header(Row::from(vec![
        "Name",
        "Path",
        "Alias",
        "Last Accessed",
        "Count",
    ]));

    // Add rows
    for entry in entries {
        let row = Row::from(vec![
            entry.name.as_str(),
            &entry.path.to_string_lossy(),
            &entry.alias,
            &display_epoch(entry.atime),
            &entry.count.to_string(),
        ]);
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
