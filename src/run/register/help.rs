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
    HelpFactory, Line, Text, action::Action, binds::ResolvedBindMap, config::HelpDisplayConfig,
    message::Interrupt, resolve_static_preview, utils::text_to_ansi,
};

use crate::{
    pager::PagerOpts,
    run::{
        FsAction, FsPane,
        register::ExecutionMode,
        state::{GLOBAL, STACK, STORE},
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

/// Help entries appended to the menu's Action Guidance section: the visible
/// custom actions, with their non-empty `help` field.
#[derive(Debug, Clone)]
pub struct MenuActionHelp(pub Vec<(String, String)>);

/// Open this overlay's help document: the focused overlay closes and the
/// pager is requested through the main help interrupt (an overlay's own
/// action stream never reaches the interrupt handler).
pub fn request_overlay_help(document: OverlayHelp) {
    request_overlay_help_with(document, None);
}

/// Open the menu help with the visible custom actions' guidance appended.
pub fn request_menu_help(actions: Vec<(String, String)>) {
    request_overlay_help_with(OverlayHelp::Menu, Some(actions));
}

fn request_overlay_help_with(
    document: OverlayHelp,
    menu_help: Option<Vec<(String, String)>>,
) {
    match menu_help {
        Some(actions) => STORE::set(MenuActionHelp(actions)),
        None => {
            STORE::take::<MenuActionHelp>();
        }
    }
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

            let mut resolved =
                resolve_static_preview(&Text::default(), &help_factory, &help_config);
            // Pane-specific guidance follows the binds, separated by a rule.
            if let Some(extra) = pane_extra_help(&binds.load()) {
                resolved.lines.push(Line::from(""));
                resolved.lines.push(Line::from("---"));
                resolved
                    .lines
                    .extend(extra.lines().map(|line| Line::from(line.to_string())));
            }
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

        let mut page = fill_bind_column(document.contents(), &binds.load());
        if document == OverlayHelp::Menu {
            let actions = STORE::take::<MenuActionHelp>()
                .map(|help| help.0)
                .unwrap_or_default();
            page = append_menu_action_help(page, actions);
        }
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

/// Append the visible custom menu actions' guidance to the authored menu
/// document. Each entry is an action name followed by its indented help and a
/// blank line.
fn append_menu_action_help(
    mut doc: String,
    actions: Vec<(String, String)>,
) -> String {
    if actions.is_empty() {
        return doc;
    }
    if !doc.ends_with('\n') {
        doc.push('\n');
    }
    if !doc.ends_with("\n\n") {
        doc.push('\n');
    }
    for (name, help) in actions {
        doc.push_str(&name);
        doc.push('\n');
        for line in help.lines() {
            doc.push_str("  ");
            doc.push_str(line);
            doc.push('\n');
        }
        doc.push('\n');
    }
    doc
}

/// Extra guidance for panes whose behavior is not obvious from the binds
/// alone. The text follows the bind list, separated by a rule.
fn pane_extra_help(binds: &ResolvedBindMap<FsAction>) -> Option<String> {
    STACK::with_current(|pane| match pane {
        FsPane::Nav { .. } => Some(nav_extra_help(binds)),
        FsPane::Search { .. } => Some(search_extra_help(binds)),
        _ => None,
    })
}

fn nav_extra_help(binds: &ResolvedBindMap<FsAction>) -> String {
    let options = bind_keys(&FsAction::ShowOptions.into(), binds);
    let lock = bind_keys(&FsAction::LockPrompt(None).into(), binds);
    let parent = keys_or_unbound(FsAction::Parent.into(), binds);
    let advance = keys_or_unbound(FsAction::Advance.into(), binds);
    let backward = keys_or_unbound(Action::BackwardChar, binds);
    let forward = keys_or_unbound(Action::ForwardChar, binds);

    let mut out = String::from(
        "The simplest pane in f:st, listing the contents of a directory. Type \
         to filter the list; Enter accepts the highlighted item.",
    );

    out.push_str("\n\n");
    if options.is_empty() {
        out.push_str("The options overlay adjusts filters and sorting.");
    } else {
        out.push_str(&format!(
            "The options overlay ({options}) adjusts filters and sorting."
        ));
    }
    out.push_str(
        " Press up on the first result to focus the current directory: the \
         prompt then shows that directory and actions apply to it instead of \
         the highlighted item.",
    );
    if !lock.is_empty() {
        out.push_str(&format!(" Toggle the prompt lock with {lock}."));
    }

    out.push_str("\n\n");
    out.push_str(&format!(
        "A note on the arrow keys: {parent} / {advance} are bound to Parent / \
         Advance (move between directories), while {backward} / {forward} are \
         bound to BackwardChar / ForwardChar (move within the query). The two \
         pairs are easy to confuse."
    ));
    out
}

fn search_extra_help(binds: &ResolvedBindMap<FsAction>) -> String {
    let toggle = bind_keys(&FsAction::Search.into(), binds);
    let toggle = if toggle.is_empty() {
        "The Search binding".to_string()
    } else {
        toggle
    };
    format!(
        "{toggle} toggles between searching for a new pattern and filtering \
         the matches already found. Filtering narrows the existing results \
         without rerunning rg."
    )
}

/// The printable triggers bound to exactly `action`, or `unbound` when none.
fn keys_or_unbound(
    action: Action<FsAction>,
    binds: &ResolvedBindMap<FsAction>,
) -> String {
    let keys = bind_keys(&action, binds);
    if keys.is_empty() {
        "unbound".to_string()
    } else {
        keys
    }
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
    table.load_style(UTF8_FULL);
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
