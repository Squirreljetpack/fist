//! Toast state and rendering.
//!
//! Toasts are footer messages displayed at the bottom of the picker UI. Each toast
//! entry is a [`ToastLine`]: a styled prefix [`Span`] paired with a [`ToastContent`]
//! variant (a comma-separated list of items, an arrow-delimited pair `A → B`, or a
//! raw line), plus [`ToastFlags`] deciding which clear operations it survives.
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
//! ### Persistence:
//! - **Categorized / Grouped Toasts** ([`TOAST::push`], [`TOAST::pair`], [`TOAST::notice`])
//!   carry [`ToastFlags::PERSIST_PANE`]: they survive pane switches
//!   ([`TOAST::clear_msgs`]) but are dropped by cursor movement ([`TOAST::clear`]).
//! - **Ad-hoc / Un-prefixed Messages** ([`TOAST::msg`], [`TOAST::toast_empty`],
//!   [`TOAST::push_skipped`]) carry no flags: [`TOAST::clear`] and
//!   [`TOAST::clear_msgs`] discard them, and `TOAST::msg(..., replace = true)`
//!   evicts them before showing the latest status line.
//! - [`TOAST::push_with_flag`] can also set [`ToastFlags::PERSIST_CURSOR`] so a
//!   toast survives cursor movement until it is explicitly removed ([`TOAST::pop`]).
//!
//! The prefix styles are configurable per [`ToastStyle`] level through the UI config
//! (`[styles.toast]`, see [`crate::config::ui::StyleConfig::toast`]).

use std::sync::Mutex;

use bitflags::bitflags;
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
            ToastStyle::Normal => toast.normal.into(),
            ToastStyle::Info => toast.info.into(),
            ToastStyle::Success => toast.success.into(),
            ToastStyle::Warning => toast.warning.into(),
            ToastStyle::Error => toast.error.into(),
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

bitflags! {
    /// Which clear operations a toast line survives.
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub struct ToastFlags: u8 {
        /// Survives [`TOAST::clear`] (cursor movement).
        const PERSIST_CURSOR = 1 << 0;
        /// Survives [`TOAST::clear_msgs`] (pane switches).
        const PERSIST_PANE = 1 << 1;
    }
}

/// One active toast entry: a styled prefix span, a content payload, and its
/// clear-behavior flags.
#[derive(Debug)]
pub struct ToastLine {
    /// Styled prefix (empty for ad-hoc messages).
    pub prefix: Span<'static>,
    /// The content payload.
    pub content: ToastContent,
    /// Which clear operations this line survives.
    pub flags: ToastFlags,
}

/// Assemble active toast entries into a multi-line [`Text`] widget for the footer.
fn make_toast(toasts: &[ToastLine]) -> Text<'static> {
    let lines = toasts.iter().map(|line| {
        let mut spans = Vec::new();
        spans.push(line.prefix.clone());

        match &line.content {
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
static TOAST: Mutex<Vec<ToastLine>> = Mutex::new(Vec::new());

/// Global controller for managing and rendering footer toast notifications.
pub struct TOAST {}

impl TOAST {
    /// Remove all active toasts and clear the footer display completely.
    pub fn clear() {
        let mut state = TOAST.lock().unwrap();
        state.retain(|line| line.flags.contains(ToastFlags::PERSIST_CURSOR));
        debug!("Cleared toasts: {state:?}");
        let footer = if state.is_empty() {
            None
        } else {
            Some(make_toast(&state))
        };
        GLOBAL::send_action(FsAction::set_footer(footer));
    }

    /// Increment or insert a dimmed `"Skipped"` counter line in the footer.
    ///
    /// - If no `"Skipped"` line exists: renders `"Skipped"`.
    /// - If `"Skipped"` already exists: increments the count in-place to `"Skipped (2)"`, `"Skipped (3)"`, etc.
    pub fn push_skipped() {
        let mut state = TOAST.lock().unwrap();

        const SKIPPED: &str = "Skipped";

        if let Some(line) = state.iter_mut().find(|line| {
            line.prefix.content.is_empty()
                && matches!(
                    &line.content,
                    ToastContent::Line(l)
                    if l.spans.first().map(|s| s.content.starts_with(SKIPPED)) == Some(true)
                )
        }) {
            let next = match &line.content {
                ToastContent::Line(existing) => {
                    let first = &existing.spans[0].content;

                    if first == SKIPPED {
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
                    }
                }
                ToastContent::List(_) | ToastContent::Pair(_, _) => unreachable!(),
            };

            if let ToastContent::Line(existing) = &mut line.content {
                existing.spans[0] =
                    Span::styled(format!("{SKIPPED} ({next})"), Style::new().dim().italic());
            }
        } else {
            let prefix_span = Span::raw("");
            let line = Line::from(Span::styled(SKIPPED, Style::new().dim().italic()));
            state.push(ToastLine {
                prefix: prefix_span,
                content: ToastContent::Line(line),
                flags: ToastFlags::empty(),
            });
        }

        let toast = make_toast(&state);
        GLOBAL::send_action(FsAction::set_footer(toast));
    }

    /// Clear all transient messages while preserving toasts carrying
    /// [`ToastFlags::PERSIST_PANE`] (grouped copy/delete lists, notices, and
    /// other categorized toasts).
    pub fn clear_msgs() {
        let mut state = TOAST.lock().unwrap();

        state.retain(|line| line.flags.contains(ToastFlags::PERSIST_PANE));

        let footer = if state.is_empty() {
            None
        } else {
            Some(make_toast(&state))
        };
        GLOBAL::send_action(FsAction::set_footer(footer));
    }

    /// Remove an item from a list toast by prefix, and remove the toast entry if the list becomes empty.
    pub fn pop(
        prefix: &str,
        item: &Span<'static>,
    ) {
        let mut state = TOAST.lock().unwrap();
        state.retain_mut(|line| {
            if line.prefix.content == prefix {
                if let ToastContent::List(items) = &mut line.content {
                    items.retain(|i| i != item);
                    return !items.is_empty();
                }
            }
            true
        });

        let footer = if state.is_empty() {
            None
        } else {
            Some(make_toast(&state))
        };
        GLOBAL::send_action(FsAction::set_footer(footer));
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
        prefix: impl Into<std::borrow::Cow<'static, str>>,
        items: impl IntoIterator<Item = Span<'static>>,
    ) {
        Self::push_with_flag(style, prefix, items, ToastFlags::PERSIST_PANE);
    }

    /// Push a grouped list toast with explicit clear-behavior flags.
    ///
    /// [`TOAST::push`] defers to this with [`ToastFlags::PERSIST_PANE`]; callers
    /// that must also survive cursor movement (e.g. in-progress operations
    /// like archive extraction) add [`ToastFlags::PERSIST_CURSOR`].
    pub fn push_with_flag(
        style: ToastStyle,
        prefix: impl Into<std::borrow::Cow<'static, str>>,
        items: impl IntoIterator<Item = Span<'static>>,
        flags: ToastFlags,
    ) {
        let mut state = TOAST.lock().unwrap();
        let prefix_cow = prefix.into();
        if let Some(line) = state
            .iter_mut()
            .find(|line| line.prefix.content == prefix_cow)
        {
            if let ToastContent::List(existing_items) = &mut line.content {
                for i in items {
                    if !existing_items.contains(&i) {
                        existing_items.push(i);
                    }
                }
            } else {
                // Overwrite if not already a list
                line.content = ToastContent::List(items.into_iter().collect());
            }
        } else {
            let prefix_span = Span::styled(prefix_cow, style);
            state.push(ToastLine {
                prefix: prefix_span,
                content: ToastContent::List(items.into_iter().collect()),
                flags,
            });
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
        state.push(ToastLine {
            prefix: prefix_span,
            content: ToastContent::Pair(from, to),
            flags: ToastFlags::PERSIST_PANE,
        });

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
        state.push(ToastLine {
            prefix: prefix_span,
            content: ToastContent::Line(msg.into().into()),
            flags: ToastFlags::PERSIST_PANE,
        });

        let toast = make_toast(&state);
        GLOBAL::send_action(FsAction::set_footer(toast));
    }

    /// Push an un-prefixed raw message line.
    ///
    /// When `replace` is `true`, all other transient lines (carrying no
    /// [`ToastFlags`]) are evicted first so only the latest status message is
    /// displayed.
    pub fn msg(
        line: impl Into<Line<'static>>,
        replace: bool,
    ) {
        let mut state = TOAST.lock().unwrap();

        if replace {
            state.retain(|line| line.flags.contains(ToastFlags::PERSIST_PANE));
        }

        let prefix_span = Span::raw("");
        state.push(ToastLine {
            prefix: prefix_span,
            content: ToastContent::Line(line.into()),
            flags: ToastFlags::empty(),
        });

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

#[cfg(test)]
mod tests {
    use super::*;

    /// The toast tests share the process-global [`TOAST`] static, so they
    /// must not run concurrently.
    static TEST_LOCK: Mutex<()> = Mutex::new(());

    #[test]
    fn test_toast_push_and_pop() {
        let _guard = TEST_LOCK.lock().unwrap();
        GLOBAL::init_test_senders();
        // clear initial state
        {
            let mut state = TOAST.lock().unwrap();
            state.clear();
        }

        TOAST::push_with_flag(
            ToastStyle::Info,
            "Extracting: ",
            [Span::raw("archive1.zip"), Span::raw("archive2.zip")],
            ToastFlags::PERSIST_CURSOR | ToastFlags::PERSIST_PANE,
        );

        {
            let state = TOAST.lock().unwrap();
            assert_eq!(state.len(), 1);
            assert_eq!(state[0].prefix.content, "Extracting: ");
            assert_eq!(
                state[0].flags,
                ToastFlags::PERSIST_CURSOR | ToastFlags::PERSIST_PANE
            );
            if let ToastContent::List(items) = &state[0].content {
                assert_eq!(items.len(), 2);
            } else {
                panic!("Expected ToastContent::List");
            }
        }

        // Remove archive1.zip
        TOAST::pop("Extracting: ", &Span::raw("archive1.zip"));
        {
            let state = TOAST.lock().unwrap();
            assert_eq!(state.len(), 1);
            if let ToastContent::List(items) = &state[0].content {
                assert_eq!(items.len(), 1);
                assert_eq!(items[0], Span::raw("archive2.zip"));
            } else {
                panic!("Expected ToastContent::List");
            }
        }

        // Remove archive2.zip - entry should be removed entirely
        TOAST::pop("Extracting: ", &Span::raw("archive2.zip"));
        {
            let state = TOAST.lock().unwrap();
            assert_eq!(state.len(), 0);
        }
    }

    #[test]
    fn test_toast_clear_msgs_preserves_prefixed() {
        let _guard = TEST_LOCK.lock().unwrap();
        GLOBAL::init_test_senders();
        {
            let mut state = TOAST.lock().unwrap();
            state.clear();
        }

        TOAST::push(ToastStyle::Info, "Extracting: ", [Span::raw("test.zip")]);
        TOAST::msg(Span::styled("Entering archive", ToastStyle::Info), false);

        {
            let state = TOAST.lock().unwrap();
            assert_eq!(state.len(), 2);
        }

        TOAST::clear_msgs();

        {
            let state = TOAST.lock().unwrap();
            assert_eq!(state.len(), 1);
            assert_eq!(state[0].prefix.content, "Extracting: ");
        }
    }

    #[test]
    fn test_push_with_flag_survives_clear() {
        let _guard = TEST_LOCK.lock().unwrap();
        GLOBAL::init_test_senders();
        {
            let mut state = TOAST.lock().unwrap();
            state.clear();
        }

        // a plain grouped toast is dropped by cursor movement
        TOAST::push(ToastStyle::Success, "Copied: ", [Span::raw("a.rs")]);
        // a flagged toast survives it
        TOAST::push_with_flag(
            ToastStyle::Info,
            "Extracting: ",
            [Span::raw("b.zip")],
            ToastFlags::PERSIST_CURSOR | ToastFlags::PERSIST_PANE,
        );

        TOAST::clear();

        {
            let state = TOAST.lock().unwrap();
            assert_eq!(state.len(), 1);
            assert_eq!(state[0].prefix.content, "Extracting: ");
            assert_eq!(
                state[0].flags,
                ToastFlags::PERSIST_CURSOR | ToastFlags::PERSIST_PANE
            );
        }
    }
}
