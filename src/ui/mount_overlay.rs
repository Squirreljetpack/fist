//! Picker over the destinations this process materialized.
//!
//! Opened after an image mount is submitted and from the menu
//! ([`FsAction::ShowMounts`]): it lists every registry row — mounted volumes
//! and preview extractions — most recent first, with the image or archive
//! source in the second column. It is a fuzzy picker like the menu overlay,
//! because a volume path is long.

use std::borrow::Cow;

use crate::{
    abspath::AbsPath,
    mount::{MountRegistry, MountRow, MountStrategy},
    run::{
        action::FsAction,
        item::PathItem,
        reload::enter_dir_pane,
        state::{GLOBAL, TOAST, ToastStyle},
    },
    ui::OVERLAY_TICK_RATE,
    utils::serde::border_result,
};
use matchmaker::{
    Selector,
    action::Action,
    config::{
        BorderSetting, CursorSetting, OverlayLayoutSettings, PartialBorderSetting, Percentage,
        QueryConfig, ResultsConfig, RowConnectionStyle,
    },
    message::BindDirective,
    nucleo::{ColumnIndexable, Injector, Worker},
    render::MMState,
    ui::{
        Constraint, Direction, Frame, Layout, Overlay, OverlayEffect, QueryUI, Rect, ResultsUI,
        SizeHint, utils,
    },
};
use ratatui::{
    prelude::*,
    widgets::{Borders, Clear, Padding, Paragraph},
};
use unicode_width::UnicodeWidthStr;

/// Column headers of the picker's two-column results table.
const MOUNT_COLUMNS: [&str; 2] = ["mount", "source"];

/// Width scaling points for the picker: at 40 columns it takes half the terminal
/// width, kept constant past the last point.
const MOUNT_WIDTH_POINTS: &[(u16, Percentage)] = &[(40, Percentage::new(50))];

/// Minimum picker width, borders excluded.
const MIN_WIDTH: u16 = 16;

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct MountConfig {
    #[serde(with = "border_result")]
    pub border: Result<BorderSetting, PartialBorderSetting>,
    pub results: ResultsConfig,
}

impl Default for MountConfig {
    fn default() -> Self {
        let border = PartialBorderSetting {
            title: Some("Mounts".into()),
            sides: Some(Borders::ALL),
            padding: Some(Padding::new(2, 2, 0, 1).into()),
            ..Default::default()
        };
        // the column names are wider than a short mount point: let the table
        // engine size columns from the row text instead, and keep the end of
        // the long paths in view
        let mut results = ResultsConfig {
            min_width_from_cols: false,
            ..Default::default()
        };
        results.autoscroll.end = true;
        Self {
            border: Err(border),
            results,
        }
    }
}

impl ColumnIndexable for MountRow {
    fn get_str(
        &self,
        i: usize,
    ) -> Cow<'_, str> {
        match i {
            0 => self.dest.to_string_lossy(),
            1 => self.source.to_string_lossy(),
            _ => Cow::Borrowed(""),
        }
    }

    fn get_text(
        &self,
        i: usize,
    ) -> Text<'_> {
        match i {
            0 => Text::raw(self.dest.to_string_lossy().into_owned()),
            1 => Text::raw(self.source.to_string_lossy().into_owned()),
            _ => Text::default(),
        }
    }
}

/// The picker itself: one query line over the registry's destinations.
pub struct MountOverlay {
    pub config: MountConfig,
    query: QueryUI,
    results: ResultsUI,
    /// Built on enable, dropped on disable so the matcher thread only lives
    /// while the overlay is active.
    worker: Option<Worker<MountRow, ()>>,
    /// The registry rows, most recent first.
    entries: Vec<MountRow>,
    area: Rect,
    /// Set whenever the query text changed; drives `worker.find` on the next draw.
    query_dirty: bool,
    // required to update table
    selector: Selector,
}

impl MountOverlay {
    pub fn new(config: MountConfig) -> Self {
        // The query bar is not configurable: a default QueryUI with no prompt.
        let query = QueryUI::new(QueryConfig {
            prompt: String::new(),
            ..Default::default()
        });
        // The results indentation is always empty and rows always span the full
        // width, silently overriding the user settings.
        let mut results_config = config.results.clone();
        results_config.multi_prefix = String::new();
        results_config.row_connection = RowConnectionStyle::Full;
        let results = ResultsUI::new(results_config);
        Self {
            config,
            query,
            results,
            worker: None,
            entries: vec![],
            area: Rect::default(),
            query_dirty: true,
            selector: Selector::new(),
        }
    }

    /// The overlay border.
    fn border(&self) -> BorderSetting {
        self.config.border.as_ref().unwrap().clone()
    }

    fn build_worker(&mut self) {
        debug_assert!(self.worker.is_none());
        let mut worker = Worker::new_indexable(MOUNT_COLUMNS, None);
        // equal scores keep the reported order, not the query's
        worker.set_stability(5);
        let items = self.entries.clone();
        let _ = worker.injector().extend(items.into_iter());
        self.results.init(&mut worker);
        self.worker = Some(worker);
    }

    /// Enter the highlighted destination, or report a stale path and drop it
    /// from the registry.
    pub fn accept(
        &mut self,
        state: &mut MMState<'_, PathItem, ()>,
    ) -> OverlayEffect {
        let Some((index, row)) = self.selected_row() else {
            return OverlayEffect::Disable;
        };
        if !row.is_live() {
            TOAST::notice(
                ToastStyle::Error,
                format!("Not mounted: {}", row.dest.display()),
            );
            return self.drop_entry(index, &row);
        }
        enter_dir_pane(state, AbsPath::new_unchecked(row.dest));
        OverlayEffect::Disable
    }

    /// Unmount the highlighted volume, or forget the highlighted extraction
    /// (its directory is left alone).
    fn remove_selected(&mut self) -> OverlayEffect {
        let Some((index, row)) = self.selected_row() else {
            return OverlayEffect::None;
        };
        match row.strategy {
            // detaching is slow and belongs on the blocking pool: the main
            // handler runs it once this overlay has closed
            MountStrategy::Dmg => {
                GLOBAL::send_action(FsAction::Unmount(row.dest));
                OverlayEffect::Disable
            }
            MountStrategy::Extract => {
                TOAST::notice(
                    ToastStyle::Normal,
                    format!("Forgotten: {}", row.dest.display()),
                );
                self.drop_entry(index, &row)
            }
        }
    }

    /// The highlighted row: its index into `entries` and the row itself.
    fn selected_row(&self) -> Option<(usize, MountRow)> {
        let worker = self.worker.as_ref()?;
        let (index, row) = worker.get_nth_indexed(self.results.index())?;
        Some((index as usize, row.clone()))
    }

    /// Drop the row at `index` and its registry entry, then rebuild the
    /// worker so the list reflects the registry.
    fn drop_entry(
        &mut self,
        index: usize,
        row: &MountRow,
    ) -> OverlayEffect {
        MountRegistry::forget(&row.dest);
        self.worker = None;
        if index < self.entries.len() {
            self.entries.remove(index);
        }
        self.build_worker();
        self.results.cursor_jump(0);
        self.query_dirty = true;
        self.results.set_dirty();
        OverlayEffect::None
    }
}

impl Overlay<FsAction, PathItem, ()> for MountOverlay {
    fn on_enable(
        &mut self,
        _area: &Rect,
        _state: &mut MMState<'_, PathItem, ()>,
    ) {
        // animate the picker: force the ticker to run while it is open
        GLOBAL::send_bind(BindDirective::OverrideTickrate(Some(OVERLAY_TICK_RATE)));

        // every registry row, newest first
        self.entries = MountRegistry::rows();
        log::debug!(
            "mounts picker showing {:?}",
            self.entries
                .iter()
                .map(|row| row.dest.display().to_string())
                .collect::<Vec<_>>()
        );

        self.results.cursor_jump(0);
        self.build_worker();
        self.query_dirty = true;
        self.results.set_dirty();
    }

    fn on_disable(&mut self) {
        GLOBAL::send_bind(BindDirective::OverrideTickrate(None));
        // don't carry the search text or the previous rows into the next open
        self.query.clear();
        self.entries.clear();
        // Stops the matcher thread; the worker is rebuilt on the next enable.
        self.worker = None;
    }

    fn handle_input(
        &mut self,
        c: char,
        _state: &mut MMState<'_, PathItem, ()>,
    ) -> OverlayEffect {
        self.query.push_char(c);
        self.query_dirty = true;
        self.results.set_dirty();
        OverlayEffect::None
    }

    fn handle_action(
        &mut self,
        action: &Action<FsAction>,
        state: &mut MMState<'_, PathItem, ()>,
    ) -> OverlayEffect {
        match action {
            Action::Up(n) => {
                for _ in 0..*n {
                    self.results.cursor_prev();
                }
            }
            Action::Down(n) => {
                for _ in 0..*n {
                    self.results.cursor_next();
                }
            }
            Action::Accept => return self.accept(state),
            Action::Custom(FsAction::Help) => {
                crate::run::register::request_overlay_help(
                    crate::run::register::OverlayHelp::Mount,
                );
                return OverlayEffect::Disable;
            }
            // Delete/Trash unmount the highlighted volume (or forget an
            // extraction), the row-removal convention the queue uses
            Action::Custom(FsAction::Unmount(_) | FsAction::Trash(_) | FsAction::Delete(_)) => {
                return self.remove_selected();
            }
            Action::Quit(_) => return OverlayEffect::Disable,
            // the picker toggles closed on its own action, like the menu
            Action::Custom(fa) if *fa == FsAction::ShowMounts => return OverlayEffect::Disable,

            // edit actions, mirrored from the main dispatch
            Action::SetQuery(context) => self.query.set(context.clone(), u16::MAX),
            Action::InsertQuery(context) => self.query.insert_str(context),
            Action::ForwardChar => self.query.forward_char(),
            Action::BackwardChar => self.query.backward_char(),
            Action::ForwardWord => self.query.forward_word(),
            Action::BackwardWord => self.query.backward_word(),
            Action::DeleteChar => self.query.delete(),
            Action::DeleteWord => self.query.delete_word(),
            Action::DeleteLineStart => self.query.delete_line_start(),
            Action::DeleteLineEnd => self.query.delete_line_end(),
            Action::ClearQuery => self.query.clear(),
            _ => return OverlayEffect::None,
        }
        if matches!(
            action,
            Action::SetQuery(_)
                | Action::InsertQuery(_)
                | Action::DeleteChar
                | Action::DeleteWord
                | Action::DeleteLineStart
                | Action::DeleteLineEnd
                | Action::ClearQuery
        ) {
            self.query_dirty = true;
            self.results.set_dirty();
        }
        OverlayEffect::None
    }

    fn area(
        &mut self,
        ui_area: &Rect,
        layout: &OverlayLayoutSettings,
    ) {
        let border = self.border();

        // the table additionally reserves the prefix indentation and the
        // inter-column spacing
        let column_width = |i: usize| -> u16 {
            self.entries
                .iter()
                .map(|row| row.get_str(i).width() as u16)
                .max()
                .unwrap_or(0)
        };
        let content_width = self.results.indentation() as u16
            + column_width(0)
            + column_width(1)
            + self.config.results.column_spacing.0;
        let min_width = MIN_WIDTH + border.width();

        // the query line, its separator and one row per volume; without volumes
        // the box shrinks to the message and border
        let height = if self.entries.is_empty() {
            2 + border.height()
        } else {
            self.entries.len() as u16 + 3 + border.height()
        };

        self.area = utils::default_area(
            [
                SizeHint {
                    adaptive_percentage: MOUNT_WIDTH_POINTS,
                    min: min_width,
                    max: (content_width + border.width()).max(min_width),
                },
                SizeHint {
                    adaptive_percentage: &[],
                    min: height,
                    max: height,
                },
            ],
            layout,
            ui_area,
        );

        let inner = border.inner_of(self.area);

        // compact empty state: nothing to lay out, the message fills the box
        if self.entries.is_empty() {
            return;
        }

        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(self.query.height()),
                Constraint::Length(1),
                Constraint::Fill(1),
            ])
            .split(inner);
        let input = chunks[0];
        let results = chunks[2];
        self.query.update_width(input.width);

        let old = (self.results.width(), self.results.height());
        self.results.update_dimensions(results);
        if (self.results.width(), self.results.height()) != old {
            self.results.invalidate_widths();
        }
    }

    fn draw(
        &mut self,
        frame: &mut Frame,
    ) {
        let Some(worker) = self.worker.as_mut() else {
            return;
        };

        // Same update pipeline as the main picker: find -> active column -> table.
        if self.query_dirty {
            worker.find(&self.query.input());
            self.query_dirty = false;
        }
        let cursor_byte = self.query.byte_index(self.query.cursor() as usize);
        self.results
            .update_active_column(worker.query.active_column_index(cursor_byte));
        // picker rows are never selected: a fresh empty selector per draw
        self.results
            .update_table(worker, &self.selector, &mut matchmaker::matcher::matcher());

        let border = self.border();
        frame.render_widget(Clear, self.area);
        frame.render_widget(border.as_block(), self.area);

        let inner = border.inner_of(self.area);

        // no volumes to enter: a compact box with the message only
        if self.entries.is_empty() {
            frame.render_widget(
                Paragraph::new(Text::from("Nothing mounted")).alignment(Alignment::Center),
                inner,
            );
            return;
        }

        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(self.query.height()),
                Constraint::Length(1),
                Constraint::Fill(1),
            ])
            .split(inner);
        let input = chunks[0];
        let results = chunks[2];

        // Query input
        self.query.update_width(input.width);
        self.query.scroll_to_cursor();
        let p = self.query.cursor_offset(&input);
        if let CursorSetting::Default = self.query.config.cursor {
            frame.set_cursor_position(p);
        }
        frame.render_widget(self.query.make_input(), input);

        // Results
        self.results.render(frame, results);
    }
}
