//! Toast state and rendering.
//!
//! Toasts are footer messages displayed at the bottom of the picker UI. Each toast
//! entry consists of a styled prefix [`Span`] paired with a [`ToastContent`] variant
//! (a comma-separated list of items, an arrow-delimited pair `A → B`, or a raw line).
//!
//! ### Visual Representation:
//! ```text
//! ┌─────────────────────────────────────────────────────────────┐
//! │ Copied: file1.rs, file2.rs                                  │ <- List: [Prefix][Item], [Item]
//! │ Renamed: old_name.rs → new_name.rs                          │ <- Pair: [Prefix][From] → [To]
//! │ Warning: Read-only filesystem                               │ <- Notice: [Prefix: ][Message]
//! │ No entries                                                  │ <- Raw message (empty prefix)
//! └─────────────────────────────────────────────────────────────┘
//! ```
//!
//! ### Message Types & Prefix Filtering:
//! - **Categorized / Grouped Toasts** ([`TOAST::push`], [`TOAST::pair`], [`TOAST::notice`]):
//!   Have a non-empty prefix span (e.g., `"Copied: "`, `"Warning: "`).
//! - **Ad-hoc / Un-prefixed Messages** ([`TOAST::msg`], [`TOAST::toast_empty`], [`TOAST::push_skipped`]):
//!   Have an empty prefix span `Span::raw("")`.
//! - Functions like [`TOAST::clear_msgs`] and `TOAST::msg(..., replace = true)` use
//!   `state.retain(|(prefix, _)| !prefix.content.is_empty())` to discard transient
//!   un-prefixed messages while preserving persistent categorized toasts.
//!
//! The prefix styles are configurable per [`ToastStyle`] level through the UI config
//! (`[styles.toast]`, see [`crate::config::ui::StyleConfig::toast`]).

use std::sync::Mutex;

use log::debug;
use matchmaker::nucleo::{Line, Span, Style, Text};
use ratatui::style::Color;

use crate::{config::ui::ToastStyles, run::action::FsAction};

use super::GLOBAL;
use super::ui::try_global_ui;

/// Severity / category level for toast notifications.
///
/// Determines the color and text modifier applied to the toast's prefix span.
#[derive(Copy, Clone, Debug, Default, strum_macros::Display)]
pub enum ToastStyle {
    /// Standard notification (default label `"Note"`), styled dim/italic.
    #[default]
    #[strum(serialize = "Note")]
    Normal,
    /// Informational message (light blue).
    Info,
    /// Successful operation feedback (green).
    Success,
    /// Non-fatal issue or caution (yellow).
    Warning,
    /// Failure or error condition (red).
    Error,
}

/// Fallback for toasts fired before the UI config is initialized (unit
/// tests): the built-in defaults, identical to the shipped `[styles.toast]`.
static DEFAULT_TOAST_STYLES: ToastStyles = ToastStyles::DEFAULT;

/// Convert a [`ToastStyle`] level into a concrete [`matchmaker::nucleo::Style`] using
/// active UI config settings (`[styles.toast]`).
impl From<ToastStyle> for Style {
    fn from(val: ToastStyle) -> Self {
        let toast = try_global_ui()
            .map(|ui| &ui.toast)
            .unwrap_or(&DEFAULT_TOAST_STYLES);
        match val {
            ToastStyle::Normal => toast.normal,
            ToastStyle::Info => toast.info,
            ToastStyle::Success => toast.success,
            ToastStyle::Warning => toast.warning,
            ToastStyle::Error => toast.error,
        }
    }
}

/// The content payload of a toast line.
#[derive(Debug)]
pub enum ToastContent {
    /// A comma-separated sequence of spans:
    /// `[Prefix] item1, item2, item3`
    List(Vec<Span<'static>>),
    /// A source-to-destination pair with an arrow separator:
    /// `[Prefix] source → destination`
    Pair(Span<'static>, Span<'static>),
    /// A direct line of spans appended directly to the prefix:
    /// `[Prefix] content...`
    Line(Line<'static>),
}

/// Assemble active toast entries into a multi-line [`Text`] widget for the footer.
fn make_toast(toasts: &[(Span<'static>, ToastContent)]) -> Text<'static> {
    let lines = toasts.iter().map(|(prefix, content)| {
        let mut spans = Vec::new();
        spans.push(prefix.clone());

        match content {
            ToastContent::List(items) => {
                for (i, item) in items.iter().cloned().enumerate() {
                    if i > 0 {
                        spans.push(Span::raw(", "));
                    }
                    spans.push(item);
                }
            }
            ToastContent::Pair(a, b) => {
                spans.push(a.clone());
                spans.push(" → ".into());
                spans.push(b.clone());
            }
            ToastContent::Line(line) => {
                spans.extend(line.clone());
            }
        }

        Line::from(spans)
    });

    Text::from(lines.collect::<Vec<_>>())
}

// ------------- TOAST ----------------------------
/// Global thread-safe storage for active toast notifications.
static TOAST: Mutex<Vec<(Span<'static>, ToastContent)>> = Mutex::new(Vec::new());

/// Global controller for managing and rendering footer toast notifications.
pub struct TOAST {}

impl TOAST {
    /// Remove all active toasts and clear the footer display completely.
    pub fn clear() {
        let mut state = TOAST.lock().unwrap();
        state.clear();
        debug!("Cleared toasts: {state:?}");
        GLOBAL::send_action(FsAction::set_footer(None));
    }

    /// Increment or insert a dimmed `"Skipped"` counter line in the footer.
    ///
    /// - If no `"Skipped"` line exists: renders `"Skipped"`.
    /// - If `"Skipped"` already exists: increments the count in-place to `"Skipped (2)"`, `"Skipped (3)"`, etc.
    pub fn push_skipped() {
        let mut state = TOAST.lock().unwrap();

        const SKIPPED: &str = "Skipped";

        if let Some((_, ToastContent::Line(existing))) = state.iter_mut().find(|(span, content)| {
            span.content.is_empty()
                && matches!(
                    content,
                    ToastContent::Line(l)
                    if l.spans.first().map(|s| s.content.starts_with(SKIPPED)) == Some(true)
                )
        }) {
            let first = &existing.spans[0].content;

            let next = if first == SKIPPED {
                2
            } else {
                first
                    .strip_prefix(SKIPPED)
                    .and_then(|rest| {
                        rest.trim_start_matches('(')
                            .trim_end_matches(')')
                            .parse::<usize>()
                            .ok()
                    })
                    .map(|n| n + 1)
                    .unwrap_or(2)
            };

            existing.spans[0] =
                Span::styled(format!("{SKIPPED} ({next})"), Style::new().dim().italic());
        } else {
            let prefix_span = Span::raw("");
            let line = Line::from(Span::styled(SKIPPED, Style::new().dim().italic()));
            state.push((prefix_span, ToastContent::Line(line)));
        }

        let toast = make_toast(&state);
        GLOBAL::send_action(FsAction::set_footer(toast));
    }

    /// Clear all transient un-prefixed messages while preserving grouped toasts.
    ///
    /// Uses `state.retain(|(span, _)| !span.content.is_empty())` to keep only entries
    /// with a non-empty prefix span (such as grouped copy/delete lists), dropping
    /// standalone messages like `"No entries"` or ad-hoc notices.
    pub fn clear_msgs() {
        let mut state = TOAST.lock().unwrap();

        state.retain(|(span, _)| !span.content.is_empty());

        GLOBAL::send_action(FsAction::set_footer(None));
    }

    /// Push items under a styled prefix group, merging into existing groups if present.
    ///
    /// ### Example:
    /// ```text
    /// Copied: file_a.rs, file_b.rs
    /// ```
    /// If a toast with `prefix` already exists, new items are appended (deduplicated).
    pub fn push(
        style: ToastStyle,
        prefix: &'static str,
        items: impl IntoIterator<Item = Span<'static>>,
    ) {
        let mut state = TOAST.lock().unwrap();
        if let Some((_, existing_content)) =
            state.iter_mut().find(|(p, _)| p.content.as_ref() == prefix)
        {
            if let ToastContent::List(existing_items) = existing_content {
                for i in items {
                    if !existing_items.contains(&i) {
                        existing_items.push(i);
                    }
                }
            } else {
                // Overwrite if not already a list
                *existing_content = ToastContent::List(items.into_iter().collect());
            }
        } else {
            let prefix_span = Span::styled(prefix, style);
            state.push((prefix_span, ToastContent::List(items.into_iter().collect())));
        }

        let toast = make_toast(&state);
        GLOBAL::send_action(FsAction::set_footer(toast));
    }

    /// Push a pair of items with an arrow separator (`A → B`), labeled by a prefix.
    ///
    /// ### Example:
    /// ```text
    /// Renamed: old_path.txt → new_path.txt
    /// ```
    pub fn pair(
        style: ToastStyle,
        prefix: &'static str,
        from: Span<'static>,
        to: Span<'static>,
    ) {
        let mut state = TOAST.lock().unwrap();
        let prefix_span = Span::styled(prefix, style);
        state.push((prefix_span, ToastContent::Pair(from, to)));

        let toast = make_toast(&state);
        GLOBAL::send_action(FsAction::set_footer(toast));
    }

    /// Push a single notice line prefixed by the style's default label.
    ///
    /// ### Example:
    /// ```text
    /// Warning: Disk space is low
    /// Error: Permission denied
    /// ```
    pub fn notice(
        style: ToastStyle,
        msg: impl Into<std::borrow::Cow<'static, str>>,
    ) {
        let mut state = TOAST.lock().unwrap();
        let prefix_span = Span::styled(format!("{style}: "), style);
        state.push((prefix_span, ToastContent::Line(msg.into().into())));

        let toast = make_toast(&state);
        GLOBAL::send_action(FsAction::set_footer(toast));
    }

    /// Push an un-prefixed raw message line.
    ///
    /// When `replace` is `true`, all other un-prefixed messages (`prefix.content.is_empty()`)
    /// are evicted first so only the latest status message is displayed.
    pub fn msg(
        line: impl Into<Line<'static>>,
        replace: bool,
    ) {
        let mut state = TOAST.lock().unwrap();

        if replace {
            state.retain(|(prefix, _)| !prefix.content.is_empty());
        }

        let prefix_span = Span::raw("");
        state.push((prefix_span, ToastContent::Line(line.into())));

        let toast = make_toast(&state);
        GLOBAL::send_action(FsAction::set_footer(toast));
    }

    /// Convenience helper to display a dimmed `"No entries"` status message,
    /// replacing any previous un-prefixed messages.
    pub fn toast_empty() {
        TOAST::msg(
            Span::styled("No entries", Style::new().fg(Color::DarkGray).italic()),
            true,
        );
    }
}

