use crate::{
    abspath::AbsPath,
    cli::paths::__home,
    display::human_size,
    run::{
        action::FsAction,
        item::PathItem,
        queue::{QUEUE, QUEUE_STATE, QueueItem, QueueItemState, QueueItemStatus, QueueView},
        state::TOAST,
    },
    ui::input::{InputWidget, InputWidgetConfig},
    utils::{serde::border_result, text::ToastStyle},
};

use cba::bath::PathExt;
use cba::bring::StrExt;
use matchmaker::{
    action::Action,
    config::{BorderSetting, OverlayLayoutSettings, PartialBorderSetting, StyleSetting},
    render::MMState,
    ui::{Overlay, OverlayEffect, SizeHint, utils},
};
use ratatui::{
    prelude::*,
    widgets::{Cell, Clear, Paragraph, Row, Table, TableState},
};
use unicode_width::UnicodeWidthStr;

use std::{
    collections::BTreeSet,
    fmt::Alignment,
    sync::atomic::Ordering,
};

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct QueueConfig {
    #[serde(with = "border_result")]
    pub border: Result<BorderSetting, PartialBorderSetting>,
    // pub bar_width: u16,
    pub column_spacing: usize,

    pub cell: StyleSetting,
    pub current_style: StyleSetting,
    pub selected_style: StyleSetting,
    pub editing_style: StyleSetting,
    pub editing_row_style: StyleSetting,
}

impl Default for QueueConfig {
    fn default() -> Self {
        let border = PartialBorderSetting {
            title: Some("Queue".into()),
            ..Default::default()
        };
        Self {
            border: Err(border),
            // bar_width: 15,
            column_spacing: 2,
            cell: Default::default(),
            current_style: StyleSetting {
                bg: Some(Color::Black),
                ..Default::default()
            },
            selected_style: StyleSetting {
                fg: Some(Color::Yellow),
                ..Default::default()
            },
            editing_row_style: Default::default(),
            editing_style: StyleSetting {
                bg: Some(Color::Black),
                ..Default::default()
            },
        }
    }
}

/// Shared table-selection state for the stash overlays. The shared stash
/// edits source/destination columns `[1, 2]`; the app view is a single
/// editable path column `[0, 0]` (dst edits are not offered).
pub struct TableSelection {
    pub state: TableState,
    pub selected: BTreeSet<usize>,
    pub editing: Option<(usize, usize, InputWidget)>,

    pub view: QueueView,
    pub path_dst_cols: [usize; 2],
    pub available_w: u16,
    pub initial_widths: Vec<u16>,
    pub dirty: bool,
    pub reiinit: bool,
}

impl TableSelection {
    pub fn new(view: QueueView) -> Self {
        let path_dst_cols = match view {
            QueueView::Shared => [1, 2],
            QueueView::Apps => [0, 0],
        };
        Self {
            state: TableState::new(),
            selected: BTreeSet::new(),
            view,
            editing: None,
            path_dst_cols,
            available_w: 0,
            initial_widths: vec![],
            dirty: false,
            reiinit: false,
        }
    }

    pub fn update_editing_widths(
        &mut self,
        widths: &mut [u16],
        area: &mut Rect,
        border_width: u16,
    ) {
        self.dirty = false;
        if let Some((_, col, input)) = &mut self.editing {
            let val_width = input.inner.input().width() as u16;
            let original_col_w = widths[*col];

            input.update_width(original_col_w + 1);

            if val_width != original_col_w {
                if val_width > original_col_w {
                    // Only grow if currently small (<= 32) and won't overflow
                    let current_total: u16 = widths.iter().sum();

                    if original_col_w < 32 && current_total < self.available_w {
                        let diff =
                            (val_width - original_col_w).min(self.available_w - current_total);
                        widths[*col] += diff;
                    }
                // responsively shrink to original width
                } else if val_width < original_col_w {
                    let initial = self.initial_widths.get(*col).cloned().unwrap_or(16); // unwrap should be fine here
                    widths[*col] = val_width.max(initial);
                }

                // update
                if widths[*col] != original_col_w {
                    input.update_width(widths[*col] + 1);
                    // need recenter
                    let new_total_w = widths.iter().sum::<u16>()
                        + border_width
                        + widths.len().saturating_sub(1) as u16;
                    utils::update_area(area, Some(new_total_w), None);
                    log::trace!("recentered: {area:?} {new_total_w:?} {widths:?}");
                }
            }
        }
    }

    pub fn handle_input(
        &mut self,
        c: char,
    ) -> OverlayEffect {
        if let Some((_, _, input)) = &mut self.editing {
            input.handle_input(c);
            self.dirty = true;
            return OverlayEffect::None;
        }
        OverlayEffect::None
    }

    pub fn handle_action(
        &mut self,
        action: &Action<FsAction>,
    ) -> OverlayEffect {
        let len = QUEUE::view_len(self.view);
        if len == 0 {
            return OverlayEffect::Disable;
        }

        if let Some((row, col, input)) = &mut self.editing {
            if let Some(accepted) = input.handle_action(action) {
                if accepted {
                    let value = input.value();
                    if *col == self.path_dst_cols[0] {
                        let path = AbsPath::new_unchecked(std::path::PathBuf::from(value));
                        if std::fs::symlink_metadata(&path).is_ok() {
                            QUEUE::view_update(self.view, *row, Some(path), None);
                            self.editing = None;
                        } else {
                            TOAST::notice(ToastStyle::Error, "Path does not exist");
                        }
                    } else {
                        // dst was updated
                        QUEUE::view_update(self.view, *row, None, Some(value.into()));
                        self.editing = None;
                    }
                } else {
                    self.editing = None;
                }
                if self.editing.is_none() {
                    self.reiinit = true;
                }
                return OverlayEffect::None;
            }
            self.dirty = true;
            return OverlayEffect::None;
        }

        match action {
            Action::Up(x) => {
                if let Some(i) = self.state.selected_mut() {
                    *i = i.saturating_sub(*x as usize);
                }
            }
            Action::Down(x) => {
                if let Some(i) = self.state.selected_mut() {
                    *i = (*i + *x as usize).min(len.saturating_sub(1));
                }
            }
            Action::Select => {
                if let Some(i) = self.state.selected() {
                    self.selected.insert(i);
                }
            }
            Action::Deselect => {
                if let Some(i) = self.state.selected() {
                    self.selected.remove(&i);
                }
            }
            Action::ToggleSelection => {
                if let Some(i) = self.state.selected() {
                    if !self.selected.insert(i) {
                        self.selected.remove(&i);
                    }
                }
            }
            Action::PreviewUp(_) => {
                if let Some(i) = self.state.selected() {
                    if i > 0 {
                        QUEUE::view_swap(self.view, i, i - 1);
                        self.state.select(Some(i - 1));
                    }
                }
            }
            Action::PreviewDown(_) => {
                if let Some(i) = self.state.selected() {
                    if i + 1 < len {
                        QUEUE::view_swap(self.view, i, i + 1);
                        self.state.select(Some(i + 1));
                    }
                }
            }
            Action::DeleteChar | Action::Custom(FsAction::Trash(_) | FsAction::Delete(_)) => {
                if let Some(i) = self.state.selected() {
                    QUEUE::view_remove(self.view, i);
                }
            }
            Action::Accept => {
                if self.view == QueueView::Shared {
                    if !self.selected.is_empty() {
                        QUEUE::execute_all(&self.selected);
                        self.selected.clear();
                    } else if let Some(i) = self.state.selected() {
                        QUEUE::execute(i);
                    }
                } else {
                    // app entries are not executable — they are files to open
                    self.selected.clear();
                }
            }
            Action::Custom(FsAction::ShowMenu) => {
                if let Some(i) = self.state.selected()
                    // multi-path items do not support src editing
                    && let Some((src, _)) = QUEUE::view_get(self.view, i)
                    && let [p] = src.as_slice()
                {
                    let mut input = InputWidget::new(InputWidgetConfig {
                        ..Default::default()
                    });
                    let val = p.to_string_lossy().into_owned();
                    input.set_value(val.clone());
                    let col = self.path_dst_cols[0];
                    self.editing = Some((i, col, input));
                    self.dirty = true;
                    return OverlayEffect::None;
                }
            }
            Action::Custom(FsAction::Rename) if self.view == QueueView::Shared => {
                if let Some(i) = self.state.selected() {
                    if let Some((_, d)) = QUEUE::view_get(self.view, i) {
                        let mut input = InputWidget::new(InputWidgetConfig {
                            ..Default::default()
                        });
                        let val = d.to_string_lossy().into_owned();
                        input.set_value(val.clone());
                        let col = self.path_dst_cols[1];
                        self.editing = Some((i, col, input));
                        self.dirty = true;
                        return OverlayEffect::None;
                    }
                }
            }
            Action::Quit(_) => return OverlayEffect::Disable,
            _ => {}
        };
        OverlayEffect::None
    }

    pub fn render_editing(
        &mut self,
        frame: &mut matchmaker::ui::Frame<'_>,
        area: Rect,
        widths: &[u16],
        border_left: u16,
        border_top: u16,
        style: Style,
    ) {
        if let Some((row, col, input)) = &mut self.editing {
            let x_offset = widths[0..*col].iter().sum::<u16>() + border_left + *col as u16;
            let y_offset = (*row - self.state.offset()) as u16 + border_top + 1;

            // input width is updated in update_widths
            input.scroll_to_cursor();
            let span = input.make_input(style);

            let input_area = Rect {
                x: area.x + x_offset,
                y: area.y + y_offset,
                width: widths[*col],
                height: 1,
            };

            frame.render_widget(Paragraph::new(Line::from(span)), input_area);

            let pos = Position::new(input_area.x + input.inner.cursor_rel_offset(), input_area.y);
            frame.set_cursor_position(pos);
        }
    }
}

pub struct QueueOverlay {
    state: TableSelection,
    config: QueueConfig,
    widths: [u16; 4],
    headers: [String; 4],
    area: Rect,

    extra: (OverlayLayoutSettings, Rect),
}

impl QueueOverlay {
    pub fn new(config: QueueConfig) -> Self {
        Self {
            state: TableSelection::new(QueueView::Shared),
            config,
            widths: Default::default(),
            headers: [
                "Kind".pad(1, 0),
                "Source".pad(1, 1),
                "To".pad(1, 1),
                "Progress".pad(0, 1),
            ],
            area: Rect::default(),
            extra: Default::default(),
        }
    }

    pub fn border(&self) -> &BorderSetting {
        self.config.border.as_ref().unwrap()
    }

    pub fn width(&self) -> u16 {
        self.area.width.saturating_sub(self.border().width())
    }

    fn update_widths(
        &mut self,
        items: &[QueueItem],
        available_ui_w: u16,
    ) {
        log::trace!("available: {available_ui_w}");
        if self.state.editing.is_some() {
            log::error!("Unexpected editing");
            return;
        }

        let mut kind_w = self.headers[0].len() as u16 + 1;
        for item in items {
            kind_w = kind_w.max(item.kind.len() as u16 + 1);
        }

        let mut path_w = 16;
        for item in items {
            let width = item.display().width() as u16;
            log::trace!("iw: {width}, {}", item.display());
            path_w = path_w.max(width);
        }

        let mut dst_w = self.headers[2].len() as u16;
        for item in items {
            dst_w = dst_w.max(item.dst.to_string_lossy().width() as u16);
        }

        let mut size_w = 10;

        let available_path_w = available_ui_w
            .saturating_sub(kind_w + dst_w + size_w)
            .max(16);

        path_w = path_w.min(available_path_w);

        let mut dst_w_ = available_ui_w
            .saturating_sub(kind_w + path_w + size_w)
            .max(16)
            .min(dst_w);

        self.widths = [kind_w, path_w, dst_w_, size_w];
        let mut extra = self
            .widths
            .iter()
            .sum::<u16>()
            .saturating_sub(available_ui_w);

        let mut reduce = |val: &mut u16, min_val: u16| {
            if extra > 0 {
                let can_reduce = val.saturating_sub(min_val);
                let reduction = can_reduce.min(extra);

                *val -= reduction;
                extra -= reduction;
            }
        };

        reduce(&mut path_w, 10);
        reduce(&mut dst_w_, dst_w.min(6));
        reduce(&mut size_w, 6);
        reduce(&mut kind_w, 5);
        reduce(&mut path_w, 6);
        reduce(&mut size_w, 3);
        reduce(&mut kind_w, 3);
        reduce(&mut dst_w, 3);
        reduce(&mut kind_w, 3);

        self.widths = [kind_w, path_w, dst_w_, size_w];

        self.state.initial_widths = self.widths.to_vec();
    }

    fn set_area(&mut self) {
        log::trace!("new widths {:?}", self.widths);

        let width = self.widths.iter().sum::<u16>() + self.border().width() + 3;
        self.area = utils::default_area(
            [width.into(), SizeHint::Min(self.border().height() + 4)],
            &self.extra.0,
            &self.extra.1,
        );
    }
}

impl Overlay<FsAction, PathItem, ()> for QueueOverlay {

    fn on_enable(
        &mut self,
        _area: &Rect,
        _state: &mut MMState<'_, '_, PathItem, ()>,
    ) {
        self.state.state.select(Some(0));
        QUEUE::check_validity();
    }

    fn on_disable(&mut self) {
        QUEUE::clear_completed_shared();
    }

    fn handle_input(
        &mut self,
        c: char,
        _state: &mut MMState<'_, '_, PathItem, ()>,
    ) -> OverlayEffect {
        if self.state.editing.is_some() {
            return self.state.handle_input(c);
        }
        match c {
            'q' => OverlayEffect::Disable,
            _ => OverlayEffect::None,
        }
    }

    fn area(
        &mut self,
        ui_area: &Rect,
        layout: &OverlayLayoutSettings,
    ) {
        let state = QUEUE_STATE.lock().unwrap();
        self.state.available_w = ui_area
            .width
            .saturating_sub(self.border().width())
            .saturating_sub(3); // column spacing
        self.update_widths(&state.shared, self.state.available_w);

        self.extra = (layout.clone(), *ui_area); // lazy method to help recompute area
        self.set_area();
    }

    fn handle_action(
        &mut self,
        action: &Action<FsAction>,
        _state: &mut MMState<'_, '_, PathItem, ()>,
    ) -> OverlayEffect {
        self.state.handle_action(action)
    }

    fn draw(
        &mut self,
        frame: &mut matchmaker::ui::Frame<'_>,
    ) {
        let state = QUEUE_STATE.lock().unwrap();

        if state.shared.is_empty() {
            frame.render_widget(Clear, self.area);
            frame.render_widget(
                Paragraph::new("Queue is empty").block(self.border().as_block()),
                self.area,
            );
            return;
        }

        if self.state.reiinit {
            self.update_widths(&state.shared, self.state.available_w);
            self.state.dirty = false;
            self.state.reiinit = false;
            log::trace!("new widths {:?}", self.widths);
            self.set_area();
        } else if self.state.dirty {
            let border_w = self.border().width();
            self.state
                .update_editing_widths(&mut self.widths, &mut self.area, border_w);
        }
        let area = self.area;

        let editing_info = self.state.editing.as_ref().map(|(r, c, _)| (*r, *c));

        // build table
        let header =
            Row::new(self.headers.clone()).style(Style::new().add_modifier(Modifier::BOLD));
        let rows: Vec<Row> = state
            .shared
            .iter()
            .enumerate()
            .map(|(i, item)| {
                let is_current = self.state.state.selected() == Some(i);
                let is_selected = self.state.selected.contains(&i);
                let is_editing = editing_info.is_some_and(|(r, _)| r == i);

                let mut row_style = self.config.cell;
                if is_current {
                    row_style = if editing_info.is_some() {
                        self.config.editing_row_style
                    } else {
                        self.config.current_style
                    };
                } else if is_selected {
                    row_style = self.config.selected_style;
                }

                let kind = Cell::from(item.kind.clone().pad(1, 1));
                let path_cell = if is_editing && editing_info.unwrap().1 == 1 {
                    Cell::from("")
                } else {
                    Cell::from(
                        item.display()
                            .ellipsize(self.widths[1] as usize, Alignment::Right),
                    )
                };
                let dst_cell = if is_editing && editing_info.unwrap().1 == 2 {
                    Cell::from("")
                } else {
                    Cell::from(item.dst.to_string_lossy().into_owned().pad(0, 1))
                };
                let size = Cell::from(item.status.render(&self.config));

                Row::new(vec![kind, path_cell, dst_cell, size]).style(row_style)
            })
            .collect();

        // render table
        let table = Table::new(rows, self.widths)
            .header(header)
            .column_spacing(1)
            .block(self.border().as_static_block());
        frame.render_widget(Clear, area);
        frame.render_stateful_widget(table, area, &mut self.state.state);

        self.state.render_editing(
            frame,
            area,
            &self.widths,
            self.border().left(),
            self.border().top(),
            self.config.editing_style.into(),
        );
    }
}

/// The app view (overlay index 1): single-column list of the paths collected
/// while in the app pane, titled by its config border title (the "Queue"
/// default, like the queue overlay), with a bold "To open:" header.
/// Repurposed from the old 2-column scratch overlay; app entries have no
/// destination column.
pub struct AppOverlay {
    state: TableSelection,
    config: QueueConfig,
    widths: Vec<u16>,
    area: Rect,

    extra: (OverlayLayoutSettings, Rect),
}

impl AppOverlay {
    pub fn new(config: QueueConfig) -> Self {
        Self {
            state: TableSelection::new(QueueView::Apps),
            config,
            widths: Vec::new(),
            area: Rect::default(),
            extra: Default::default(),
        }
    }

    pub fn border(&self) -> &BorderSetting {
        self.config.border.as_ref().unwrap()
    }

    fn update_widths(
        &mut self,
        items: &[std::path::PathBuf],
        available_ui_w: u16,
    ) {
        if self.state.editing.is_some() {
            return;
        }

        let mut path_w = 16u16;
        for p in items {
            path_w = path_w.max(p.display_short(__home()).width() as u16);
        }
        self.widths = vec![path_w.min(available_ui_w.max(16))];
        self.state.initial_widths = self.widths.to_vec();
    }

    fn set_area(&mut self) {
        log::trace!("new widths {:?}", self.widths);

        let width = self.widths.iter().sum::<u16>() + self.border().width();
        self.area = utils::default_area(
            [width.into(), SizeHint::Min(self.border().height() + 4)],
            &self.extra.0,
            &self.extra.1,
        );
    }
}

impl Overlay<FsAction, PathItem, ()> for AppOverlay {

    fn on_enable(
        &mut self,
        _area: &Rect,
        _state: &mut MMState<'_, '_, PathItem, ()>,
    ) {
        self.state.state.select(Some(0));
    }

    fn on_disable(&mut self) {}

    fn handle_input(
        &mut self,
        c: char,
        _state: &mut MMState<'_, '_, PathItem, ()>,
    ) -> OverlayEffect {
        if self.state.editing.is_some() {
            return self.state.handle_input(c);
        }
        match c {
            'q' => OverlayEffect::Disable,
            _ => OverlayEffect::None,
        }
    }

    fn area(
        &mut self,
        ui_area: &Rect,
        layout: &OverlayLayoutSettings,
    ) {
        let state = QUEUE_STATE.lock().unwrap();
        self.state.available_w = ui_area
            .width
            .saturating_sub(self.border().width())
            .saturating_sub(self.widths.len().saturating_sub(1) as u16);
        self.update_widths(&state.apps, self.state.available_w);

        self.extra = (layout.clone(), *ui_area);
        self.set_area();
    }

    fn handle_action(
        &mut self,
        action: &Action<FsAction>,
        _state: &mut MMState<'_, '_, PathItem, ()>,
    ) -> OverlayEffect {
        self.state.handle_action(action)
    }

    fn draw(
        &mut self,
        frame: &mut matchmaker::ui::Frame<'_>,
    ) {
        let state = QUEUE_STATE.lock().unwrap();
        let items = &state.apps;

        if self.state.reiinit {
            self.update_widths(items, self.state.available_w);
            self.set_area();

            self.state.dirty = false;
            self.state.reiinit = false;
        } else if self.state.dirty {
            let border_w = self.border().width();
            self.state
                .update_editing_widths(&mut self.widths, &mut self.area, border_w);
        }

        let area = self.area;

        if false {
            // Scaffold for a two-column (path + destination) editable table.
            // A future data source replaces the dummy rows and flips this to
            // `true`; the rendering mirrors the old scratch overlay's
            // two-column view, without scratch-source switching.
            let dummy: Vec<(std::path::PathBuf, String)> = vec![
                (std::path::PathBuf::from("~/docs/notes.md"), "~/backup/notes.md".into()),
                (std::path::PathBuf::from("~/pics/photo.png"), String::new()),
            ];

            let mut path_w = 16u16;
            let mut dst_w = 16u16;
            for (p, d) in &dummy {
                path_w = path_w.max(p.display_short(__home()).width() as u16);
                dst_w = dst_w.max(d.width() as u16);
            }
            let widths = [path_w, dst_w];

            let editing_info = self.state.editing.as_ref().map(|(r, c, _)| (*r, *c));
            let rows: Vec<Row> = dummy
                .iter()
                .enumerate()
                .map(|(i, (p, d))| {
                    let is_current = self.state.state.selected() == Some(i);
                    let is_editing = editing_info.is_some_and(|(r, _)| r == i);

                    let row_style = if is_current {
                        if editing_info.is_some() {
                            self.config.editing_row_style
                        } else {
                            self.config.current_style
                        }
                    } else {
                        self.config.cell
                    };
                    let path = if is_editing && editing_info.unwrap().1 == 0 {
                        Cell::from("")
                    } else {
                        Cell::from(p.display_short(__home()))
                    };
                    let dst = if is_editing && editing_info.unwrap().1 == 1 {
                        Cell::from("")
                    } else {
                        Cell::from(d.as_str())
                    };
                    Row::new(vec![path, dst]).style(row_style)
                })
                .collect();

            let header = Row::new(vec![Cell::from("To open:"), Cell::from("To:")])
                .style(Style::new().add_modifier(Modifier::BOLD));

            let table = Table::new(
                rows,
                widths
                    .iter()
                    .map(|w| Constraint::Length(*w))
                    .collect::<Vec<_>>(),
            )
            .header(header)
            .column_spacing(1)
            .block(self.border().as_static_block());

            frame.render_widget(Clear, area);
            frame.render_stateful_widget(table, area, &mut self.state.state);
            return;
        }

        let editing_info = self.state.editing.as_ref().map(|(r, c, _)| (*r, *c));

        let rows: Vec<Row> = items
            .iter()
            .enumerate()
            .map(|(i, p)| {
                let is_current = self.state.state.selected() == Some(i);
                let is_selected = self.state.selected.contains(&i);
                let is_editing = editing_info.is_some_and(|(r, _)| r == i);

                let mut row_style = self.config.cell;
                if is_current {
                    row_style = if editing_info.is_some() {
                        self.config.editing_row_style
                    } else {
                        self.config.current_style
                    };
                } else if is_selected {
                    row_style = self.config.selected_style;
                }

                let path = if is_editing && editing_info.unwrap().1 == 0 {
                    Cell::from("")
                } else {
                    Cell::from(p.display_short(__home()))
                };
                Row::new(vec![path]).style(row_style)
            })
            .collect();

        let header =
            Row::new(vec![Cell::from("To open:")]).style(Style::new().add_modifier(Modifier::BOLD));

        let table = Table::new(
            rows,
            self.widths
                .iter()
                .map(|w| Constraint::Length(*w))
                .collect::<Vec<_>>(),
        )
        .header(header)
        .column_spacing(1)
        .block(self.border().as_static_block());

        frame.render_widget(Clear, area);
        frame.render_stateful_widget(table, area, &mut self.state.state);

        self.state.render_editing(
            frame,
            area,
            &self.widths,
            self.border().left(),
            self.border().top(),
            self.config.editing_style.into(),
        );
    }
}

impl QueueItemStatus {
    pub fn render(
        &self,
        cfg: &QueueConfig,
    ) -> Line<'static> {
        let size = self.size.load(Ordering::Relaxed);
        let progress = self.progress.load(Ordering::Relaxed);
        let state = self.state.load();

        let style = match state {
            QueueItemState::Pending => Style::default(),
            QueueItemState::Started => {
                let percent = progress as f32 / 255.0;
                let text =
                    format!("{:5.2}%", percent * 100.0).pad_to(10, std::fmt::Alignment::Center);
                return Line::from(Span::styled(text, Style::default().bg(Color::Cyan)));
            }
            QueueItemState::CompleteOk => Style::default().fg(Color::Green),
            QueueItemState::PendingErr => Style::default().fg(Color::LightRed),
            QueueItemState::CompleteErr => Style::default().fg(Color::Red),
        };

        Line::styled(human_size(size, true).pad_to(10, Alignment::Left), style)
    }
}
