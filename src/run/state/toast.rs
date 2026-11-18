//! Toast state and rendering, backed by [`matchmaker::ui::toast`].
//!
//! Toasts are rendered by matchmaker's `ToastUI` widget. Each entry in
//! [`matchmaker::ui::toast`] carries a styled prefix [`Span`] (without a
//! trailing `": "`, which `ToastUI` inserts on render), a list of [`Line`]
//! items joined by `", "`, and a caller-defined `discriminant: u8`.
//!
//! ### Persistence:
//! Discriminant `0` (including `matchmaker`'s own `Scoped(0)` entries) is
//! transient; [`ToastFlags`] bits control clears and scope:
//! - [`TOAST::clear`] runs on pane switches and drops transient toasts,
//!   keeping [`ToastFlags::SUSTAINED`] and [`ToastFlags::PERSIST`].
//! - [`TOAST::clear_sustained`] runs on cursor movement and drops both
//!   transient and [`ToastFlags::SUSTAINED`] toasts, keeping [`ToastFlags::PERSIST`].
//! - `TOAST::msg(..., replace = true)` evicts transient lines, keeping
//!   sustained results and in-flight progress.
//! - [`ToastFlags::LUA`] marks script-emitted transient toasts so targeted
//!   `replace`/`remove`/`pop` calls do not wipe them, while broad clears do.
//! - [`TOAST::pop`] and [`TOAST::remove`] retire non-Lua entries explicitly.
//!
//! Use `TOAST::SUSTAINED.push(...)` / `TOAST::PERSIST.push(...)` (or any
//! [`ToastFlags`] combination) to tag entries directly.
//!
//! The prefix styles are configurable per [`ToastStyle`] level through the UI
//! config (`[styles.toast]`, see [`crate::config::ui::StyleConfig::toast`]).

use std::borrow::Cow;

use bitflags::bitflags;
use matchmaker::{
    message::RenderCommand,
    nucleo::{Line, Span, Style},
    ui::toast::{self, Scoped, ToastEntry},
};
use ratatui::style::Color;

use super::{GLOBAL, ui::global_ui};

const SKIPPED_LABEL: &str = "Skipped";

bitflags! {
    /// Flags stored in [`ToastEntry::discriminant`] controlling clear behavior and scope.
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub struct ToastFlags: u8 {
        /// Survives pane switches ([`TOAST::clear`]), dropped by cursor
        /// movement ([`TOAST::clear_sustained`]). Carried by finished file
        /// operations (`"Copied"`, `"Move"`, `"New"`).
        const SUSTAINED = 1 << 0;
        /// Survives every clear: in-flight operation progress (`"Deleting"`,
        /// `"Extracting"`, `"Mounting"`), retired explicitly by
        /// [`TOAST::pop`] or [`TOAST::remove`].
        const PERSIST = 1 << 1;
        /// Transient Lua-originated toast: dropped by broad clears
        /// ([`TOAST::clear`], [`TOAST::clear_sustained`], `msg(..., true)`),
        /// ignored by targeted `replace`/`remove`/`pop`.
        const LUA = 1 << 2;
    }
}

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

/// Convert a [`ToastStyle`] level into a concrete [`matchmaker::nucleo::Style`] using
/// active UI config settings (`[styles.toast]`).
impl From<ToastStyle> for Style {
    fn from(val: ToastStyle) -> Self {
        let toast = &global_ui().toast;
        match val {
            ToastStyle::Normal => toast.normal.into(),
            ToastStyle::Info => toast.info.into(),
            ToastStyle::Success => toast.success.into(),
            ToastStyle::Warning => toast.warning.into(),
            ToastStyle::Error => toast.error.into(),
        }
    }
}

fn same_target_scope(
    entry: &ToastEntry,
    prefix: &str,
    flags: ToastFlags,
) -> bool {
    (entry.discriminant & ToastFlags::LUA.bits()) == (flags.bits() & ToastFlags::LUA.bits())
        && entry.prefix.content == prefix
}

fn keep_for_clear(entry: &ToastEntry) -> bool {
    (entry.discriminant & (ToastFlags::SUSTAINED | ToastFlags::PERSIST).bits()) != 0
}

fn keep_for_clear_sustained(entry: &ToastEntry) -> bool {
    (entry.discriminant & ToastFlags::PERSIST.bits()) != 0
}

fn wake_ui() {
    GLOBAL::send_mm(RenderCommand::Tick);
}

fn next_skipped_count(first: &str) -> usize {
    if first == SKIPPED_LABEL {
        2
    } else {
        first
            .strip_prefix(SKIPPED_LABEL)
            .and_then(|rest| {
                rest.trim()
                    .strip_prefix('(')
                    .and_then(|s| s.strip_suffix(')'))
                    .and_then(|s| s.parse::<usize>().ok())
            })
            .map(|n| n.saturating_add(1).max(2))
            .unwrap_or(2)
    }
}

impl ToastFlags {
    /// Push items under a styled prefix, merging deduplicated items into the
    /// latest matching `(prefix, flags)` entry if present.
    ///
    /// ### Example:
    /// ```text
    /// Copied: file_a.rs, file_b.rs
    /// ```
    pub fn push(
        self,
        style: ToastStyle,
        prefix: impl Into<Cow<'static, str>>,
        items: impl IntoIterator<Item = Span<'static>>,
    ) {
        let mut lines: Vec<Line<'static>> = Vec::new();
        for item in items {
            let line = Line::from(item);
            if !lines.contains(&line) {
                lines.push(line);
            }
        }
        if lines.is_empty() {
            return;
        }

        let prefix = prefix.into();
        let discriminant = self.bits();

        let changed = toast::with_log(|entries| {
            if let Some(entry) = entries
                .iter_mut()
                .rev()
                .find(|e| e.discriminant == discriminant && e.prefix.content == prefix)
            {
                let mut changed = false;
                for line in lines {
                    if !entry.message.contains(&line) {
                        entry.message.push(line);
                        changed = true;
                    }
                }
                changed
            } else {
                entries.push(ToastEntry {
                    prefix: Span::styled(prefix, style),
                    message: lines,
                    discriminant,
                });
                true
            }
        });

        if changed {
            wake_ui();
        }
    }

    /// Push a pair of items with an arrow separator (`A → B`), labeled by a prefix.
    ///
    /// ### Example:
    /// ```text
    /// Renamed: old_path.txt → new_path.txt
    /// ```
    pub fn pair(
        self,
        style: ToastStyle,
        prefix: &'static str,
        from: Span<'static>,
        to: Span<'static>,
    ) {
        Scoped(self.bits()).push(
            Span::styled(prefix, style),
            Line::from(vec![from, Span::raw(" → "), to]),
        );
        wake_ui();
    }

    /// Push a single notice line prefixed by the style's default label.
    ///
    /// ### Example:
    /// ```text
    /// Warning: Disk space is low
    /// Error: Permission denied
    /// ```
    pub fn notice(
        self,
        style: ToastStyle,
        msg: impl Into<Cow<'static, str>>,
    ) {
        Scoped(self.bits()).push(
            Span::styled(style.to_string(), style),
            Line::from(msg.into()),
        );
        wake_ui();
    }

    /// Push a toast line, replacing any existing matching-prefix entry in the
    /// same scope (`LUA` vs non-`LUA`) in place.
    pub fn replace(
        self,
        style: impl Into<Style>,
        prefix: impl Into<Cow<'static, str>>,
        line: impl Into<Line<'static>>,
    ) {
        let prefix_span = Span::styled(prefix.into(), style.into());
        let new_line = line.into();
        let discriminant = self.bits();

        toast::with_log(|entries| {
            let mut replaced = false;
            entries.retain_mut(|entry| {
                if same_target_scope(entry, &prefix_span.content, self) {
                    if !replaced {
                        replaced = true;
                        entry.prefix = prefix_span.clone();
                        entry.message = vec![new_line.clone()];
                        entry.discriminant = discriminant;
                        return true;
                    }
                    return false;
                }
                true
            });
            if !replaced {
                entries.push(ToastEntry {
                    prefix: prefix_span,
                    message: vec![new_line],
                    discriminant,
                });
            }
        });
        wake_ui();
    }

    /// Push an un-prefixed raw message line.
    ///
    /// When `replace` is `true`, all transient lines are evicted first so only
    /// the latest status message is displayed; [`ToastFlags::SUSTAINED`] and
    /// [`ToastFlags::PERSIST`] stay.
    pub fn msg(
        self,
        line: impl Into<Line<'static>>,
        replace: bool,
    ) {
        let line = line.into();
        toast::with_log(|entries| {
            if replace {
                entries.retain(keep_for_clear);
            }
            entries.push(ToastEntry {
                prefix: Span::raw(""),
                message: vec![line],
                discriminant: self.bits(),
            });
        });
        wake_ui();
    }
}

// ------------- TOAST ----------------------------

/// Global controller for managing toast notifications via [`matchmaker::ui::toast`].
pub struct TOAST {}

impl TOAST {
    /// Scoped handle for finished file-operation results that survive pane switches.
    pub const SUSTAINED: ToastFlags = ToastFlags::SUSTAINED;
    /// Scoped handle for in-flight operation progress that survives all clears.
    pub const PERSIST: ToastFlags = ToastFlags::PERSIST;
    /// Scoped handle for Lua-originated transient toasts, isolated from targeted
    /// `replace`/`remove`/`pop` calls.
    pub const LUA: ToastFlags = ToastFlags::LUA;

    /// Pane switch: drop transient toasts, keep [`ToastFlags::SUSTAINED`] and
    /// [`ToastFlags::PERSIST`].
    pub fn clear() {
        if toast::retain(keep_for_clear) {
            wake_ui();
        }
    }

    /// Cursor movement: drop transient and [`ToastFlags::SUSTAINED`] toasts,
    /// keep [`ToastFlags::PERSIST`].
    pub fn clear_sustained() {
        if toast::retain(keep_for_clear_sustained) {
            wake_ui();
        }
    }

    /// Remove an item from a grouped non-Lua toast by prefix, dropping the
    /// entry if no items remain. Matches single-span item lines by content.
    pub fn pop(
        prefix: &str,
        item: &Span<'static>,
    ) {
        let changed = toast::with_log(|entries| {
            let mut changed = false;
            entries.retain_mut(|entry| {
                if same_target_scope(entry, prefix, ToastFlags::empty()) {
                    let before = entry.message.len();
                    entry.message.retain(|line| {
                        !(line.spans.len() == 1 && line.spans[0].content == item.content)
                    });
                    if entry.message.len() != before {
                        changed = true;
                    }
                    return !entry.message.is_empty();
                }
                true
            });
            changed
        });
        if changed {
            wake_ui();
        }
    }

    /// Remove any active non-Lua toast entry matching the prefix.
    pub fn remove(prefix: &str) {
        if toast::retain(|entry| !same_target_scope(entry, prefix, ToastFlags::empty())) {
            wake_ui();
        }
    }

    /// Push transient items under a styled prefix, merging deduplicated items into
    /// the latest matching entry if present.
    pub fn push(
        style: ToastStyle,
        prefix: impl Into<Cow<'static, str>>,
        items: impl IntoIterator<Item = Span<'static>>,
    ) {
        ToastFlags::empty().push(style, prefix, items);
    }

    /// Push a transient pair of items with an arrow separator (`A → B`).
    pub fn pair(
        style: ToastStyle,
        prefix: &'static str,
        from: Span<'static>,
        to: Span<'static>,
    ) {
        ToastFlags::empty().pair(style, prefix, from, to);
    }

    /// Push a transient single notice line prefixed by the style's default label.
    pub fn notice(
        style: ToastStyle,
        msg: impl Into<Cow<'static, str>>,
    ) {
        ToastFlags::empty().notice(style, msg);
    }

    /// Push a transient toast line, replacing any existing non-Lua toast with
    /// matching prefix in place.
    pub fn replace(
        style: impl Into<Style>,
        prefix: impl Into<Cow<'static, str>>,
        line: impl Into<Line<'static>>,
    ) {
        ToastFlags::empty().replace(style, prefix, line);
    }

    /// Push an un-prefixed raw message line.
    pub fn msg(
        line: impl Into<Line<'static>>,
        replace: bool,
    ) {
        ToastFlags::empty().msg(line, replace);
    }

    /// Convenience helper to display a dimmed `"No entries"` status message,
    /// replacing any previous un-prefixed messages.
    pub fn toast_empty() {
        TOAST::msg(
            Span::styled("No entries", Style::new().fg(Color::DarkGray).italic()),
            true,
        );
    }

    /// Increment or insert a dimmed `"Skipped"` counter line.
    ///
    /// - If no `"Skipped"` line exists: renders `"Skipped"`.
    /// - If `"Skipped"` already exists: increments the count in-place to `"Skipped (2)"`, `"Skipped (3)"`, etc.
    pub fn push_skipped() {
        toast::with_log(|entries| {
            if let Some(entry) = entries.iter_mut().rev().find(|entry| {
                entry.discriminant == 0
                    && entry.prefix.content.is_empty()
                    && entry.message.first().is_some_and(|line| {
                        line.spans
                            .first()
                            .is_some_and(|s| s.content.starts_with(SKIPPED_LABEL))
                    })
            }) {
                let first = &entry.message[0].spans[0].content;
                let next = next_skipped_count(first);
                entry.message[0].spans[0] = Span::styled(
                    format!("{SKIPPED_LABEL} ({next})"),
                    Style::new().dim().italic(),
                );
            } else {
                entries.push(ToastEntry {
                    prefix: Span::raw(""),
                    message: vec![Line::from(Span::styled(
                        SKIPPED_LABEL,
                        Style::new().dim().italic(),
                    ))],
                    discriminant: 0,
                });
            }
        });
        wake_ui();
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;

    use cba::bait::MutexExt;

    use super::*;

    /// The toast tests share the process-global [`matchmaker::ui::toast`] log,
    /// so they must not run concurrently.
    static TEST_LOCK: Mutex<()> = Mutex::new(());

    #[test]
    fn test_skipped_count() {
        assert_eq!(next_skipped_count("Skipped"), 2);
        assert_eq!(next_skipped_count("Skipped (2)"), 3);
        assert_eq!(next_skipped_count("Skipped (14)"), 15);
    }

    #[test]
    fn test_toast_push_and_pop() {
        let _guard = TEST_LOCK._lock();
        GLOBAL::init_test_senders();

        const PREFIX: &str = "Test extracting";
        TOAST::remove(PREFIX);
        TOAST::PERSIST.push(
            ToastStyle::Info,
            PREFIX,
            [Span::raw("archive1.zip"), Span::raw("archive2.zip")],
        );

        {
            let entries = toast::entries();
            let entry = entries
                .iter()
                .find(|e| e.prefix.content == PREFIX)
                .expect("Expected grouped toast");
            assert_eq!(entry.discriminant, ToastFlags::PERSIST.bits());
            assert_eq!(entry.message.len(), 2);
            assert_eq!(entry.message[0], Line::from("archive1.zip"));
            assert_eq!(entry.message[1], Line::from("archive2.zip"));
        }

        TOAST::pop(PREFIX, &Span::raw("archive1.zip"));
        {
            let entries = toast::entries();
            let entry = entries
                .iter()
                .find(|e| e.prefix.content == PREFIX)
                .expect("Expected grouped toast");
            assert_eq!(entry.message, vec![Line::from("archive2.zip")]);
        }

        TOAST::pop(PREFIX, &Span::raw("archive2.zip"));
        assert!(toast::entries().iter().all(|e| e.prefix.content != PREFIX));
    }

    #[test]
    fn test_scoped_flags_survive_clear() {
        let _guard = TEST_LOCK._lock();
        GLOBAL::init_test_senders();

        const TRANSIENT: &str = "Test transient";
        const SUSTAINED: &str = "Test sustained";
        const PERSISTENT: &str = "Test persistent";
        const MM_NATIVE: &str = "Test mm native";
        const LUA_SHARED: &str = "Test lua shared";
        toast::remove(TRANSIENT);
        toast::remove(SUSTAINED);
        toast::remove(PERSISTENT);
        toast::remove(MM_NATIVE);
        toast::remove(LUA_SHARED);

        Scoped(0).push(Span::raw(MM_NATIVE), Line::from("transient"));
        TOAST::push(ToastStyle::Normal, TRANSIENT, [Span::raw("tmp")]);
        TOAST::LUA.push(ToastStyle::Normal, LUA_SHARED, [Span::raw("from lua")]);
        TOAST::SUSTAINED.push(ToastStyle::Success, SUSTAINED, [Span::raw("a.rs")]);
        TOAST::PERSIST.push(ToastStyle::Info, PERSISTENT, [Span::raw("b.zip")]);

        // Targeted remove/replace ignores Lua entries.
        TOAST::remove(LUA_SHARED);
        TOAST::replace(ToastStyle::Normal, LUA_SHARED, Span::raw("from rust"));
        assert_eq!(
            toast::entries()
                .iter()
                .filter(|e| e.prefix.content == LUA_SHARED)
                .count(),
            2
        );

        TOAST::clear();
        {
            let entries = toast::entries();
            assert!(entries.iter().all(|e| e.prefix.content != TRANSIENT));
            assert!(entries.iter().all(|e| e.prefix.content != MM_NATIVE));
            assert!(entries.iter().all(|e| e.prefix.content != LUA_SHARED));
            assert!(entries.iter().any(|e| e.prefix.content == SUSTAINED));
            assert!(entries.iter().any(|e| e.prefix.content == PERSISTENT));
        }

        TOAST::clear_sustained();
        {
            let entries = toast::entries();
            assert!(entries.iter().all(|e| e.prefix.content != SUSTAINED));
            assert!(entries.iter().any(|e| e.prefix.content == PERSISTENT));
        }

        TOAST::remove(PERSISTENT);
    }

    #[test]
    fn test_toast_remove_and_replace_in_place() {
        let _guard = TEST_LOCK._lock();
        GLOBAL::init_test_senders();

        const SORT: &str = "Test sorting";
        const SHOW: &str = "Test showing";
        TOAST::remove(SORT);
        TOAST::remove(SHOW);
        TOAST::SUSTAINED.push(ToastStyle::Normal, SORT, [Span::raw("initial")]);
        TOAST::replace(ToastStyle::Normal, SHOW, Span::raw("hidden"));
        TOAST::replace(ToastStyle::Normal, SORT, Span::raw("recent"));

        {
            let entries = toast::entries();
            let positions: Vec<&str> = entries
                .iter()
                .filter(|e| e.prefix.content == SORT || e.prefix.content == SHOW)
                .map(|e| e.prefix.content.as_ref())
                .collect();
            assert_eq!(positions, vec![SORT, SHOW]);
            let sort_entry = entries.iter().find(|e| e.prefix.content == SORT).unwrap();
            assert_eq!(sort_entry.message, vec![Line::from("recent")]);
            assert_eq!(sort_entry.discriminant, 0);
        }

        TOAST::remove(SORT);
        {
            let entries = toast::entries();
            assert!(entries.iter().all(|e| e.prefix.content != SORT));
            assert!(entries.iter().any(|e| e.prefix.content == SHOW));
        }

        TOAST::remove(SHOW);
        assert!(toast::entries().iter().all(|e| e.prefix.content != SHOW));
    }
}
