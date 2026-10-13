//! Help documents and paging.
//!
//! [`FsAction::Help`](crate::run::FsAction::Help) pages the generated keybind
//! help (the [`matchmaker::HelpFactory`] built in `start`). While an overlay
//! is focused it instead pages that overlay's authored document from
//! `assets/help`, with the bind column of every markdown table filled from
//! the live resolved keybinds ([`fill_bind_column`]). The document is chosen
//! through `state.discriminant_payload` ([`OverlayHelp`]), the same channel
//! [`ExecutionMode`] uses, so the pager opens through the usual execute
//! interrupt (terminal suspended, then restored).

use std::{io::Cursor, sync::Arc};

use arc_swap::ArcSwap;
use comfy_table::{ContentArrangement, Row, Table, presets::UTF8_FULL};
use matchmaker::{
    HelpFactory, Text, action::Action, binds::ResolvedBindMap, config::HelpDisplayConfig,
    message::Interrupt, resolve_static_preview, utils::text_to_ansi,
};

use crate::{
    pager::PagerOpts,
    run::{
        register::ExecutionMode,
        state::{GLOBAL, STORE},
    },
};

/// Overlay help documents, addressed through `state.discriminant_payload`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OverlayHelp {
    Menu = 10,
    Queue = 11,
    Options = 12,
    Mount = 13,
}

impl OverlayHelp {
    pub fn discriminant(self) -> u8 {
        self as u8
    }

    fn from_discriminant(discriminant: u8) -> Option<Self> {
        match discriminant {
            10 => Some(Self::Menu),
            11 => Some(Self::Queue),
            12 => Some(Self::Options),
            13 => Some(Self::Mount),
            _ => None,
        }
    }

    fn contents(self) -> &'static str {
        match self {
            Self::Menu => include_str!("../../../assets/help/menu.txt"),
            Self::Queue => include_str!("../../../assets/help/queue.txt"),
            Self::Options => include_str!("../../../assets/help/options.txt"),
            Self::Mount => include_str!("../../../assets/help/mount.txt"),
        }
    }

    /// The overlay this document belongs to, for reopening it after paging.
    fn overlay_index(self) -> usize {
        match self {
            Self::Menu => 4,
            Self::Queue => 0,
            Self::Options => 2,
            Self::Mount => 5,
        }
    }
}

/// Open this overlay's help document: the focused overlay closes and the
/// pager is requested through the main help interrupt (an overlay's own
/// action stream never reaches the interrupt handler).
pub fn request_overlay_help(document: OverlayHelp) {
    STORE::set(document);
    GLOBAL::send_action(crate::run::FsAction::Help);
}

pub(super) fn register_help_handler(
    mm: &mut crate::run::FsMatchmaker,
    help_factory: HelpFactory,
    help_config: HelpDisplayConfig,
    binds: Arc<ArcSwap<ResolvedBindMap<crate::run::FsAction>>>,
) {
    mm.register_interrupt_handler(Interrupt::Execute, move |state| {
        let Some(payload) = state.discriminant_payload else {
            return;
        };

        if payload == ExecutionMode::Help.discriminant() {
            state.discriminant_payload = None;
            state.clear_interrupt();

            let resolved = resolve_static_preview(&Text::default(), &help_factory, &help_config);
            let ansi_text = text_to_ansi(&resolved);
            let _ = crate::pager::page_reader(
                Cursor::new(ansi_text.into_bytes()),
                true,
                None,
                PagerOpts::default(),
            );
            return;
        }

        let Some(document) = OverlayHelp::from_discriminant(payload) else {
            return;
        };
        state.discriminant_payload = None;
        state.clear_interrupt();

        let page = fill_bind_column(document.contents(), &binds.load());
        let _ = crate::pager::page_reader(
            Cursor::new(page.into_bytes()),
            true,
            None,
            PagerOpts::default(),
        );
        // return to the overlay that asked for the document
        GLOBAL::send_action(Action::Overlay(document.overlay_index()));
    });
}

/// Replace the bind column of every markdown table in `doc` with the
/// trigger(s) whose resolved action list is exactly the action named in the
/// first column; unbound actions (and rows whose first cell does not parse
/// as an [`Action`]) stay empty. The table is re-rendered with comfy-table,
/// so the authored file only needs `| action | bind | description |` rows
/// with empty bind cells.
fn fill_bind_column(
    doc: &str,
    binds: &ResolvedBindMap<crate::run::FsAction>,
) -> String {
    let mut out = String::with_capacity(doc.len());
    let mut table: Option<Vec<Vec<String>>> = None;

    let flush = |table: &mut Option<Vec<Vec<String>>>, out: &mut String| {
        if let Some(rows) = table.take() {
            out.push_str(&render_table(rows, binds));
            out.push('\n');
        }
    };

    for line in doc.lines() {
        match markdown_row(line) {
            Some(cells) => {
                let rows = table.get_or_insert_with(Vec::new);
                // the `|---|` separator only exists to make the source valid
                // markdown; comfy-table draws its own rules
                if !cells
                    .first()
                    .is_some_and(|cell| cell.chars().all(|c| c == '-') || cell.is_empty())
                {
                    rows.push(cells);
                }
            }
            None => {
                flush(&mut table, &mut out);
                out.push_str(line);
                out.push('\n');
            }
        }
    }
    flush(&mut table, &mut out);

    out
}

/// Split a `| a | b | c |` markdown row into trimmed cells.
fn markdown_row(line: &str) -> Option<Vec<String>> {
    let trimmed = line.trim();
    let inner = trimmed.strip_prefix('|')?.strip_suffix('|')?;
    Some(
        inner
            .split('|')
            .map(|cell| cell.trim().to_string())
            .collect(),
    )
}

fn render_table(
    rows: Vec<Vec<String>>,
    binds: &ResolvedBindMap<crate::run::FsAction>,
) -> String {
    let mut rows = rows.into_iter();
    let Some(header) = rows.next() else {
        return String::new();
    };

    let mut table = Table::new();
    table.load_preset(UTF8_FULL);
    table.set_content_arrangement(ContentArrangement::Dynamic);
    table.set_header(Row::from(header));

    for mut cells in rows {
        let action = cells
            .first()
            .and_then(|cell| cell.parse::<Action<crate::run::FsAction>>().ok());
        if let Some(bind) = cells.get_mut(1)
            && bind.is_empty()
            && let Some(action) = &action
        {
            *bind = bind_keys(action, binds);
        }
        table.add_row(Row::from(cells));
    }

    table.to_string()
}

/// The printable triggers bound to exactly `action`, sorted and deduplicated.
fn bind_keys(
    action: &Action<crate::run::FsAction>,
    binds: &ResolvedBindMap<crate::run::FsAction>,
) -> String {
    let mut keys: Vec<String> = binds
        .iter()
        .filter(|(_, actions)| actions.len() == 1 && actions[0] == *action)
        .map(|(kind, _)| kind.to_string())
        .collect();
    keys.sort();
    keys.dedup();
    keys.join(", ")
}
