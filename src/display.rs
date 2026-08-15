use cba::{bog::BogUnwrapExt, prints};
use chrono::{DateTime, Local};
use comfy_table::{presets::UTF8_FULL, ContentArrangement, Row, Table};
use fist_types::{filetypes::FileType, FileCategory};
use std::str::FromStr;
use strum::{EnumMessage, IntoEnumIterator};

use crate::{
    db::{zoxide, Entry, Epoch},
    lessfilter::Categories,
};

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

/// Print an overview of every value the `-t`/`--types` flag accepts.
///
/// File types and categories are enumerated from `fist-types` (variants,
/// aliases and doc comments are the single source of truth); custom
/// categories are read from the lessfilter config's `[categories]` table.
pub fn display_types_overview(custom_categories: &Categories) {
    let mut out =
        String::from("Values for the -t/--types flag (comma-separated, e.g. -t image,.rs,d)\n\n");

    out.push_str("File types — how fd classifies an entry (fd --type):\n");
    for ft in FileType::iter() {
        out.push_str(&format!(
            "    {ft:<2}  {}\n",
            ft.get_documentation().unwrap_or("")
        ));
    }

    out.push_str(concat!(
        "\nCategories — pre-configured groups of file extensions under a friendly name:\n",
        "    a category matches every file whose extension belongs to its set (e.g. `image`\n",
        "    matches .png, .jpg, ...). Categories are defined in fist-types and also power\n",
        "    the `cat:` conditions in the lessfilter configuration file. Single-letter aliases shadowed by a\n",
        "    file type are omitted below (e.g. `-t b` is the block device, not `build`).\n",
    ));
    let width = FileCategory::iter()
        .map(|cat| {
            let aliases = reachable_aliases(&cat);
            if aliases.is_empty() {
                cat.to_string().len()
            } else {
                format!("{} ({})", cat, aliases.join(", ")).len()
            }
        })
        .max()
        .unwrap_or(0);
    for cat in FileCategory::iter() {
        let aliases = reachable_aliases(&cat);
        let label = if aliases.is_empty() {
            cat.to_string()
        } else {
            format!("{} ({})", cat, aliases.join(", "))
        };
        out.push_str(&format!(
            "    {label:<width$}  {}\n",
            cat.get_documentation().unwrap_or("")
        ));
    }

    out.push_str("\nExtensions:\n    .ext    a single extension, e.g. -t .rs, .tar.gz\n");

    out.push_str(concat!(
        "\nCustom categories (the [categories] table of lessfilter.toml):\n",
        "    In addition to the built-ins above, you can define your own named\n",
        "    category as a list of mime strings — exact ones like `application/pdf`,\n",
        "    or wildcards like `image/*`, `*/gzip`, `*/*`:\n",
        "\n",
        "        [categories]\n",
        "        raster = [\"image/png\", \"image/jpeg\"]\n",
        "\n",
        "    Custom categories work wherever built-in ones do: `-t raster` filters\n",
        "    the find pane by expanding each mime to the extensions known for it,\n",
        "\n",
        "    Currently configured:\n",
    ));
    if custom_categories.is_empty() {
        out.push_str("    (none configured)\n");
    } else {
        let mut custom: Vec<_> = custom_categories.iter().collect();
        custom.sort_by(|a, b| a.0.cmp(b.0));
        for (name, mimes) in custom {
            let mimes = mimes
                .iter()
                .map(|m| m.to_string())
                .collect::<Vec<_>>()
                .join(", ");
            out.push_str(&format!("    {name}\n        {mimes}\n"));
        }
    }

    out.push_str("\nOther:\n    ''    files with no extension\n");

    prints!(out);
}

/// Aliases of a category that actually reach it through `-t`: aliases that
/// parse as a file type are shadowed (file types are matched first).
fn reachable_aliases(cat: &FileCategory) -> Vec<&'static str> {
    cat.aliases()
        .iter()
        .copied()
        .filter(|alias| FileType::from_str(alias).is_err())
        .collect()
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
