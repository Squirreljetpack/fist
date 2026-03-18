use crate::{
    abspath::AbsPath,
    cli::paths::__home,
    find::size::format_size,
    run::{
        action::FsAction,
        stash::{STASH, STASH_STATE, StashItem, StashItemState, StashItemStatus},
        state::TOAST,
    },
    ui::input::{InputWidget, InputWidgetConfig},
    utils::{serde::border_result, text::ToastStyle},
};

use cba::bath::PathExt;
use cba::bring::StrExt;
use matchmaker::{
    action::Action,
    config::{BorderSetting, PartialBorderSetting, StyleSetting},
    ui::{Overlay, OverlayEffect, SizeHint},
};
use ratatui::{
    prelude::*,
    widgets::{Cell, Clear, Paragraph, Row, Table, TableState},
};
use unicode_width::UnicodeWidthStr;

use std::{collections::BTreeSet, fmt::Alignment, sync::atomic::Ordering};

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct StashConfig {
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

impl Default for StashConfig {
    fn default() -> Self {
        let border = PartialBorderSetting {
            title: Some("Stash".into()),
            ..Default::default()
        };
        Self {
            border: Err(border),
            // bar_width: 15,
            column_spacing: 2,
            cell: Default::default(),
            current_style: StyleSetting {
                bg: Color::Black,
                ..Default::default()
            },
            selected_style: StyleSetting {
                fg: Color::Yellow,
                ..Default::default()
            },
            editing_row_style: Default::default(),
            editing_style: StyleSetting {
                bg: Color::Black,
                ..Default::default()
            },
        }
    }
}

pub struct TableSelection {
    pub state: TableState,
    pub selected: BTreeSet<usize>,
    pub editing: Option<(usize, usize, InputWidget)>,

    pub exclusive: bool,
    // exclusive and shared have different table layouts
    pub path_dst_cols: [usize; 2],
    pub available_w: u16,
    pub initial_widths: Vec<u16>,
}

impl TableSelection {
    pub fn new(exclusive: bool) -> Self {
        let path_dst_cols = if exclusive { [0, 1] } else { [1, 2] };
        Self {
            state: TableState::new(),
            selected: BTreeSet::new(),
            exclusive,
            editing: None,
            path_dst_cols,
            available_w: 0,
            initial_widths: vec![],
        }
    }

    pub fn update_widths(
        &mut self,
        widths: &mut [u16],
        border_width: u16,
    ) -> OverlayEffect {
        if let Some((_, col, input)) = &mut self.editing {
            let val_width = input.inner.input.width() as u16 + 1;
            let current_col_w = widths[*col];

            if val_width != current_col_w {
                if val_width > current_col_w {
                    // Only grow if currently small (<= 16) and won't overflow
                    if current_col_w <= 16 {
                        let current_total: u16 = widths.iter().sum();
                        if current_total < self.available_w {
                            let diff =
                                (val_width - current_col_w).min(self.available_w - current_total);
                            widths[*col] += diff;
                        }
                    }
                } else if val_width < current_col_w {
                    let initial = self.initial_widths.get(*col).cloned().unwrap_or(16); // unwrap should be fine here
                    widths[*col] = val_width.max(initial);
                }

                if widths[*col] != current_col_w {
                    let new_total_w = widths.iter().sum::<u16>() + border_width;
                    let input_width = widths[*col].saturating_sub(1);
                    input.update_scroll(input_width);
                    return OverlayEffect::UpdateArea(Some(new_total_w), None);
                }
            }
            let input_width = widths[*col].saturating_sub(1);
            input.update_scroll(input_width);
        }
        OverlayEffect::None
    }

    pub fn handle_input(
        &mut self,
        c: char,
        widths: &mut [u16],
        border_width: u16,
    ) -> OverlayEffect {
        if let Some((_, _, input)) = &mut self.editing {
            input.handle_input(c);
            return self.update_widths(widths, border_width);
        }
        OverlayEffect::None
    }

    pub fn handle_action(
        &mut self,
        action: &Action<FsAction>,
        widths: &mut [u16],
        border_width: u16,
    ) -> OverlayEffect {
        let len = if self.exclusive {
            let kind = STASH::current_exclusive();
            STASH_STATE
                .lock()
                .unwrap()
                .exclusive
                .get(&kind)
                .map(|v| v.len())
                .unwrap_or(0)
        } else {
            STASH_STATE.lock().unwrap().shared.len()
        };
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
                            STASH::update(self.exclusive, *row, Some(path), None);
                            self.editing = None;
                        } else {
                            TOAST::notice(ToastStyle::Error, "Path does not exist");
                        }
                    } else {
                        // dst was updated
                        STASH::update(self.exclusive, *row, None, Some(value.into()));
                        self.editing = None;
                    }
                } else {
                    self.editing = None;
                }
                return OverlayEffect::None;
            }
            return self.update_widths(widths, border_width);
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
            Action::Toggle => {
                if let Some(i) = self.state.selected() {
                    if !self.selected.insert(i) {
                        self.selected.remove(&i);
                    }
                }
            }
            Action::PreviewUp(_) => {
                if let Some(i) = self.state.selected() {
                    if i > 0 {
                        STASH::swap(self.exclusive, i, i - 1);
                        self.state.select(Some(i - 1));
                    }
                }
            }
            Action::PreviewDown(_) => {
                if let Some(i) = self.state.selected() {
                    if i + 1 < len {
                        STASH::swap(self.exclusive, i, i + 1);
                        self.state.select(Some(i + 1));
                    }
                }
            }
            Action::DeleteChar | Action::Custom(FsAction::Trash | FsAction::Delete(_)) => {
                if let Some(i) = self.state.selected() {
                    STASH::remove(self.exclusive, i);
                }
            }
            Action::Accept => {
                if !self.selected.is_empty() {
                    STASH::execute_all(self.exclusive, &self.selected);
                    self.selected.clear();
                } else if let Some(i) = self.state.selected() {
                    STASH::execute(self.exclusive, i);
                }
            }
            Action::Custom(FsAction::ShowMenu) => {
                if let Some(i) = self.state.selected() {
                    if let Some((p, _)) = STASH::get(self.exclusive, i) {
                        let mut input = InputWidget::new(InputWidgetConfig {
                            no_scroll_padding: true,
                            ..Default::default()
                        });
                        let val = p.to_string_lossy().into_owned();
                        input.set_value(val.clone());
                        let col = self.path_dst_cols[0];
                        self.editing = Some((i, col, input));
                        return self.update_widths(widths, border_width);
                    }
                }
            }
            Action::Custom(FsAction::Rename) => {
                if let Some(i) = self.state.selected() {
                    if let Some((_, d)) = STASH::get(self.exclusive, i) {
                        let mut input = InputWidget::new(InputWidgetConfig {
                            no_scroll_padding: true,
                            ..Default::default()
                        });
                        let val = d.to_string_lossy().into_owned();
                        input.set_value(val.clone());
                        let col = self.path_dst_cols[1];
                        self.editing = Some((i, col, input));
                        return self.update_widths(widths, border_width);
                    }
                }
            }
            Action::Custom(FsAction::Undo) if self.exclusive => {
                STASH::cycle_exclusive(false);
            }
            Action::Custom(FsAction::Redo) if self.exclusive => {
                STASH::cycle_exclusive(true);
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
            let x_offset = widths[0..*col].iter().sum::<u16>() + border_left;
            let y_offset = (*row - self.state.offset()) as u16 + border_top + 1;

            let input_width = widths[*col].saturating_sub(1);
            let span = input.make_input(input_width, style);

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

pub struct SharedStashOverlay {
    state: TableSelection,
    config: StashConfig,
    widths: [u16; 4],
    headers: [String; 4],
}

impl SharedStashOverlay {
    pub fn new(config: StashConfig) -> Self {
        Self {
            state: TableSelection::new(false),
            config,
            widths: Default::default(),
            headers: [
                "Kind".pad(1, 0),
                "Source".pad(1, 1),
                "To".pad(1, 1),
                "Size".pad(0, 1),
            ],
        }
    }

    pub fn border(&self) -> &BorderSetting {
        self.config.border.as_ref().unwrap()
    }

    fn save_widths(
        &mut self,
        items: &[StashItem],
        available_ui_w: u16,
    ) {
        if self.state.editing.is_some() {
            return;
        }

        let mut kind_w = self.headers[0].len() as u16 + 1;
        for item in items {
            kind_w = kind_w.max(item.kind.len() as u16 + 1);
        }

        let mut path_w = 16;
        for item in items {
            path_w = path_w.max(item.display().width() as u16);
        }

        let mut dst_w = self.headers[2].len() as u16;
        for item in items {
            dst_w = dst_w.max(item.dst.to_string_lossy().width() as u16);
        }

        let size_w = 10;
        let available_path_w = available_ui_w
            .saturating_sub(self.border().width())
            .saturating_sub(kind_w + dst_w + size_w)
            .max(16);

        path_w = path_w.min(available_path_w);

        dst_w = available_ui_w
            .saturating_sub(self.border().width())
            .saturating_sub(kind_w + path_w + size_w)
            .max(16)
            .min(dst_w);

        self.widths = [kind_w, path_w, dst_w, size_w];
        self.state.initial_widths = self.widths.to_vec();
    }
}

impl Overlay for SharedStashOverlay {
    type A = FsAction;

    fn on_enable(
        &mut self,
        _area: &Rect,
    ) {
        self.state.state.select(Some(0));
        STASH::check_validity();
    }

    fn on_disable(&mut self) {
        STASH::clear_completed();
    }

    fn handle_input(
        &mut self,
        c: char,
    ) -> OverlayEffect {
        if self.state.editing.is_some() {
            let border_w = self.border().width();
            return self.state.handle_input(c, &mut self.widths, border_w);
        }
        match c {
            'q' => OverlayEffect::Disable,
            _ => OverlayEffect::None,
        }
    }

    fn area(
        &mut self,
        ui_area: &Rect,
    ) -> Result<Rect, [SizeHint; 2]> {
        let state = STASH_STATE.lock().unwrap();
        self.state.available_w = ui_area.width.saturating_sub(self.border().width());
        self.save_widths(&state.shared, self.state.available_w);
        let width = self.widths.iter().sum::<u16>() + self.border().width();
        Err([width.into(), SizeHint::Min(self.border().height() + 4)])
    }

    fn handle_action(
        &mut self,
        action: &Action<Self::A>,
    ) -> OverlayEffect {
        let border_w = self.border().width();
        self.state.handle_action(action, &mut self.widths, border_w)
    }

    fn draw(
        &mut self,
        frame: &mut matchmaker::ui::Frame<'_>,
        area: Rect,
    ) {
        let state = STASH_STATE.lock().unwrap();
        if state.shared.is_empty() {
            frame.render_widget(Clear, area);
            frame.render_widget(
                Paragraph::new("Stash is empty").block(self.border().as_block()),
                area,
            );
            return;
        }

        let editing_info = self.state.editing.as_ref().map(|(r, c, _)| (*r, *c));

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
                    Cell::from(item.dst.to_string_lossy().into_owned().pad(1, 1))
                };
                let size = Cell::from(item.status.render(&self.config));

                Row::new(vec![kind, path_cell, dst_cell, size]).style(row_style)
            })
            .collect();

        let table = Table::new(rows, self.widths)
            .header(header)
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

pub struct ExclusiveStashOverlay {
    state: TableSelection,
    config: StashConfig,
    widths: Vec<u16>,
}

impl ExclusiveStashOverlay {
    pub fn new(config: StashConfig) -> Self {
        let mut border = config.border.clone();
        if let Ok(b) = &mut border {
            b.title = "Exclusive Stash".into();
        }
        Self {
            state: TableSelection::new(true),
            config: StashConfig { border, ..config },
            widths: Vec::new(),
        }
    }

    pub fn border(&self) -> &BorderSetting {
        self.config.border.as_ref().unwrap()
    }
}

impl Overlay for ExclusiveStashOverlay {
    type A = FsAction;

    fn on_enable(
        &mut self,
        _area: &Rect,
    ) {
        self.state.state.select(Some(0));
    }

    fn on_disable(&mut self) {}

    fn handle_input(
        &mut self,
        c: char,
    ) -> OverlayEffect {
        if self.state.editing.is_some() {
            let border_w = self.border().width();
            return self.state.handle_input(c, &mut self.widths, border_w);
        }
        match c {
            'q' => OverlayEffect::Disable,
            _ => OverlayEffect::None,
        }
    }

    fn area(
        &mut self,
        ui_area: &Rect,
    ) -> Result<Rect, [SizeHint; 2]> {
        self.state.available_w = ui_area.width.saturating_sub(self.border().width());
        if self.state.editing.is_some() {
            let width = self.widths.iter().sum::<u16>() + self.border().width();
            return Err([width.into(), SizeHint::Min(self.border().height() + 4)]);
        }

        let kind = STASH::current_exclusive();
        let target = STASH::has_target(&kind);
        let state = STASH_STATE.lock().unwrap();
        let items = state
            .exclusive
            .get(&kind)
            .map(|v| v.as_slice())
            .unwrap_or_default();

        let mut path_w = 16u16;
        for (p, _) in &items {
            path_w = path_w.max(p.display_short(__home()).width() as u16);
        }

        if target {
            let mut dst_w = 16u16;
            for (_, d) in &items {
                dst_w = dst_w.max(d.to_string_lossy().width() as u16);
            }
            self.widths = vec![path_w, dst_w];
        } else {
            self.widths = vec![path_w];
        }
        self.state.initial_widths = self.widths.to_vec();

        let width = self.widths.iter().sum::<u16>() + self.border().width();
        Err([width.into(), SizeHint::Min(self.border().height() + 4)])
    }

    fn handle_action(
        &mut self,
        action: &Action<Self::A>,
    ) -> OverlayEffect {
        let border_w = self.border().width();
        self.state.handle_action(action, &mut self.widths, border_w)
    }

    fn draw(
        &mut self,
        frame: &mut matchmaker::ui::Frame<'_>,
        area: Rect,
    ) {
        let kind = STASH::current_exclusive();
        let state = STASH_STATE.lock().unwrap();
        let items = state
            .exclusive
            .get(&kind)
            .map(|v| v.as_slice())
            .unwrap_or_default();

        if items.is_empty() {
            frame.render_widget(Clear, area);
            frame.render_widget(
                Paragraph::new("Stash is empty").block(self.border().as_block()),
                area,
            );
            return;
        }

        let editing_info = self.state.editing.as_ref().map(|(r, c, _)| (*r, *c));

        let rows: Vec<Row> = items
            .iter()
            .enumerate()
            .map(|(i, (p, d))| {
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
                if self.widths.len() > 1 {
                    let dst = if is_editing && editing_info.unwrap().1 == 1 {
                        Cell::from("")
                    } else {
                        Cell::from(d.to_string_lossy().into_owned())
                    };
                    Row::new(vec![path, dst]).style(row_style)
                } else {
                    Row::new(vec![path]).style(row_style)
                }
            })
            .collect();

        let table = Table::new(
            rows,
            self.widths
                .iter()
                .map(|w| Constraint::Length(*w))
                .collect::<Vec<_>>(),
        )
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

impl StashItemStatus {
    pub fn render(
        &self,
        cfg: &StashConfig,
    ) -> Line<'static> {
        let size = self.size.load(Ordering::Relaxed);
        let progress = self.progress.load(Ordering::Relaxed);
        let state = self.state.load();

        let style = match state {
            StashItemState::Pending => Style::default(),
            StashItemState::Started => {
                let percent = progress as f32 / 255.0;
                let text =
                    format!("{:5.2}%", percent * 100.0).pad_to(10, std::fmt::Alignment::Center);
                return Line::from(Span::styled(text, Style::default().bg(Color::Cyan)));
            }
            StashItemState::CompleteOk => Style::default().fg(Color::Green),
            StashItemState::PendingErr => Style::default().fg(Color::LightRed),
            StashItemState::CompleteErr => Style::default().fg(Color::Red),
        };

        Line::styled(format_size(size).pad_to(10, Alignment::Left), style)
    }
}
