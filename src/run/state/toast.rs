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
//! [`TOAST::clear`] runs on pane switches and drops every transient toast,
//! including the per-pane indicators (`ordering: `, `showing: `, `pattern: `).
//! Results of a finished file operation (`Copied: `, `Move: `, `New: `) carry
//! [`ToastFlags::SUSTAINED`] so they outlive the switch; moving the cursor
//! acknowledges them ([`TOAST::clear_sustained`], which drops them along with
//! the transient lines). In-flight operation progress (the `"Deleting: "`,
//! `"Extracting: "` and `"Mounting: "` lists) carries [`ToastFlags::PERSIST`],
//! survives both, and is retired explicitly with [`TOAST::pop`] or
//! [`TOAST::remove`]. `TOAST::msg(..., replace = true)` evicts only plain
//! status lines, keeping sustained results and progress.
//!
//! The prefix styles are configurable per [`ToastStyle`] level through the UI config
//! (`[styles.toast]`, see [`crate::config::ui::StyleConfig::toast`]).

use std::sync::Mutex;

use bitflags::bitflags;
use cba::bait::MutexExt;
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
    /// Which clears a toast line outlives.
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub struct ToastFlags: u8 {
        /// Survives pane switches ([`TOAST::clear`]), dropped by cursor
        /// movement ([`TOAST::clear_sustained`]). Carried by the results of a
        /// finished file operation: `"Copied: "`, `"Move: "`, `"New: "`.
        const SUSTAINED = 1 << 0;
        /// Survives every clear: in-flight operation progress, retired
        /// explicitly by [`TOAST::pop`] or [`TOAST::remove`].
        const PERSIST = 1 << 1;
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
    /// Pane switch: drop every transient toast (per-pane indicators, plain
    /// status lines), keep results of finished file operations
    /// ([`ToastFlags::SUSTAINED`]) and in-flight progress
    /// ([`ToastFlags::PERSIST`]).
    pub fn clear() {
        let mut state = TOAST._lock();
        state.retain(|line| {
            line.flags
                .intersects(ToastFlags::SUSTAINED | ToastFlags::PERSIST)
        });
        debug!("Cleared toasts: {state:?}");
        let footer = if state.is_empty() {
            None
        } else {
            Some(make_toast(&state))
        };
        GLOBAL::send_action(FsAction::set_footer(footer));
    }

    /// Cursor movement: drop transient toasts and the results of finished file
    /// operations ([`ToastFlags::SUSTAINED`]), keep in-flight progress.
    pub fn clear_sustained() {
        let mut state = TOAST._lock();
        state.retain(|line| line.flags.contains(ToastFlags::PERSIST));
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
        let mut state = TOAST._lock();
        state.retain_mut(|line| {
            if line.prefix.content == prefix {
                if let ToastContent::List(items) = &mut line.content {
                    items.retain(|i| i.content != item.content);
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

    /// Remove any active toast line matching the given prefix.
    pub fn remove(prefix: &str) {
        let mut state = TOAST._lock();
        state.retain(|line| line.prefix.content != prefix);

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
        Self::push_with_flag(style, prefix, items, ToastFlags::empty());
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
        let mut state = TOAST._lock();
        let prefix_span = Span::styled(prefix, style);
        state.push(ToastLine {
            prefix: prefix_span,
            content: ToastContent::Pair(from, to),
            flags: ToastFlags::empty(),
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
        let mut state = TOAST._lock();
        let prefix_span = Span::styled(format!("{style}: "), style);
        state.push(ToastLine {
            prefix: prefix_span,
            content: ToastContent::Line(msg.into().into()),
            flags: ToastFlags::empty(),
        });

        let toast = make_toast(&state);
        GLOBAL::send_action(FsAction::set_footer(toast));
    }

    /// Push a toast line, replacing any existing toast with matching prefix.
    ///
    /// If an active toast already has the same `prefix`, its prefix and content
    /// are replaced in-place, preserving its position in the toast list.
    /// Otherwise, a new toast line is appended.
    pub fn replace(
        style: impl Into<Style>,
        prefix: impl Into<std::borrow::Cow<'static, str>>,
        line: impl Into<Line<'static>>,
    ) {
        let mut state = TOAST._lock();
        let prefix_cow = prefix.into();
        let prefix_span = Span::styled(prefix_cow.clone(), style.into());
        let new_line = line.into();

        if let Some(existing) = state
            .iter_mut()
            .find(|line| line.prefix.content == prefix_cow)
        {
            existing.prefix = prefix_span;
            existing.content = ToastContent::Line(new_line);
            existing.flags = ToastFlags::empty();
        } else {
            state.push(ToastLine {
                prefix: prefix_span,
                content: ToastContent::Line(new_line),
                flags: ToastFlags::empty(),
            });
        }

        let toast = make_toast(&state);
        GLOBAL::send_action(FsAction::set_footer(toast));
    }

    /// Push an un-prefixed raw message line.
    ///
    /// When `replace` is `true`, all other plain status lines are evicted first
    /// so only the latest status message is displayed; results
    /// ([`ToastFlags::SUSTAINED`]) and progress ([`ToastFlags::PERSIST`])
    /// stay.
    pub fn msg(
        line: impl Into<Line<'static>>,
        replace: bool,
    ) {
        let mut state = TOAST._lock();

        if replace {
            state.retain(|line| {
                line.flags
                    .intersects(ToastFlags::SUSTAINED | ToastFlags::PERSIST)
            });
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

impl TOAST {
    /// Increment or insert a dimmed `"Skipped"` counter line in the footer.
    ///
    /// - If no `"Skipped"` line exists: renders `"Skipped"`.
    /// - If `"Skipped"` already exists: increments the count in-place to `"Skipped (2)"`, `"Skipped (3)"`, etc.
    pub fn push_skipped() {
        let mut state = TOAST._lock();

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

    /// Push a grouped list toast with explicit clear-behavior flags.
    ///
    /// [`TOAST::push`] defers to this with no flags; callers tracking an
    /// in-flight operation (e.g. archive extraction) pass
    /// [`ToastFlags::PERSIST`] so the list outlives cursor and pane changes.
    pub fn push_with_flag(
        style: ToastStyle,
        prefix: impl Into<std::borrow::Cow<'static, str>>,
        items: impl IntoIterator<Item = Span<'static>>,
        flags: ToastFlags,
    ) {
        let mut state = TOAST._lock();
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
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The toast tests share the process-global [`TOAST`] store, so they must
    /// not run concurrently: [`TOAST::clear_sustained`] drops every transient
    /// line, including those other tests put there.
    static TEST_LOCK: Mutex<()> = Mutex::new(());

    /// Production paths exercised by sibling tests push toasts through the
    /// same global store, so each test asserts only on its own prefix.
    #[test]
    fn test_toast_push_and_pop() {
        let _guard = TEST_LOCK._lock();
        GLOBAL::init_test_senders();

        const PREFIX: &str = "Test extracting: ";
        TOAST::push_with_flag(
            ToastStyle::Info,
            PREFIX,
            [Span::raw("archive1.zip"), Span::raw("archive2.zip")],
            ToastFlags::PERSIST,
        );

        {
            let state = TOAST._lock();
            let line = state
                .iter()
                .find(|line| line.prefix.content == PREFIX)
                .expect("Expected grouped toast");
            assert_eq!(line.flags, ToastFlags::PERSIST);
            let ToastContent::List(items) = &line.content else {
                panic!("Expected ToastContent::List");
            };
            assert_eq!(items.len(), 2);
            assert_eq!(items[0], Span::raw("archive1.zip"));
            assert_eq!(items[1], Span::raw("archive2.zip"));
        }

        // Remove archive1.zip
        TOAST::pop(PREFIX, &Span::raw("archive1.zip"));
        {
            let state = TOAST._lock();
            let line = state
                .iter()
                .find(|line| line.prefix.content == PREFIX)
                .expect("Expected grouped toast");
            let ToastContent::List(items) = &line.content else {
                panic!("Expected ToastContent::List");
            };
            assert_eq!(items.len(), 1);
            assert_eq!(items[0], Span::raw("archive2.zip"));
        }

        // Remove archive2.zip - entry should be removed entirely
        TOAST::pop(PREFIX, &Span::raw("archive2.zip"));
        assert!(
            TOAST
                ._lock()
                .iter()
                .all(|line| line.prefix.content != PREFIX)
        );
    }

    #[test]
    fn test_push_with_flag_survives_clear() {
        let _guard = TEST_LOCK._lock();
        GLOBAL::init_test_senders();

        const TRANSIENT: &str = "Test copied: ";
        const PERSISTENT: &str = "Test persistent: ";
        TOAST::push(ToastStyle::Success, TRANSIENT, [Span::raw("a.rs")]);
        TOAST::push_with_flag(
            ToastStyle::Info,
            PERSISTENT,
            [Span::raw("b.zip")],
            ToastFlags::PERSIST,
        );

        TOAST::clear_sustained();

        let state = TOAST._lock();
        assert!(state.iter().all(|line| line.prefix.content != TRANSIENT));
        let line = state
            .iter()
            .find(|line| line.prefix.content == PERSISTENT)
            .expect("Expected persistent toast");
        assert_eq!(line.flags, ToastFlags::PERSIST);
    }

    #[test]
    fn test_toast_remove() {
        let _guard = TEST_LOCK._lock();
        GLOBAL::init_test_senders();

        const SORT: &str = "Test sorting: ";
        const SHOW: &str = "Test showing: ";
        TOAST::replace(ToastStyle::Normal, SORT, Span::raw("recent"));
        TOAST::replace(ToastStyle::Normal, SHOW, Span::raw("hidden"));

        {
            let state = TOAST._lock();
            assert!(state.iter().any(|line| line.prefix.content == SORT));
            assert!(state.iter().any(|line| line.prefix.content == SHOW));
        }

        TOAST::remove(SORT);

        {
            let state = TOAST._lock();
            assert!(state.iter().all(|line| line.prefix.content != SORT));
            assert!(state.iter().any(|line| line.prefix.content == SHOW));
        }

        TOAST::remove(SHOW);
        assert!(TOAST._lock().iter().all(|line| line.prefix.content != SHOW));
    }
}
