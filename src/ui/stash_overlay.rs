use crate::{
    cli::paths::__home,
    find::size::{format_size, sort_by_size},
    run::{
        action::FsAction,
        stash::{
            AlternateStashItem, CustomStashActionActionState, STASH, StashAction, StashItem,
            StashItemState, StashItemStatus,
        },
        state::STACK,
    },
    utils::serde::border_result,
};

use cli_boilerplate_automation::{
    bath::PathExt, bring::StrExt, bum::Float32Ext, define_transparent_wrapper, vec_,
};
use matchmaker::{
    action::Action,
    config::{self, BorderSetting, PartialBorderSetting},
    ui::{Overlay, OverlayEffect, SizeHint},
};
use ratatui::{
    prelude::*,
    widgets::{
        Block, BorderType, Borders, Cell, Clear, HighlightSpacing, Paragraph, Row, Table,
        TableState,
    },
};
use unicode_width::UnicodeWidthStr;

use std::{
    fmt::Alignment,
    sync::atomic::{AtomicBool, AtomicU8, Ordering},
};

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct StashConfig {
    #[serde(with = "border_result")]
    pub border: Result<BorderSetting, PartialBorderSetting>,
    pub bar_width: u16,
    pub column_spacing: usize,
}

impl Default for StashConfig {
    fn default() -> Self {
        let border = PartialBorderSetting {
            title: Some("Stash".into()),
            ..Default::default()
        };
        Self {
            border: Err(border),
            bar_width: 15,
            column_spacing: 2,
        }
    }
}

// ratatui table
// current row is highlighted
// columns: item.path, size, details (currently just says cp, mv or sym)
pub struct StashOverlay {
    table_state: TableState,
    editing: Option<(usize, String)>, // editing column + original buffer item
    config: StashConfig,
    widths: [u16; 4], // stored to help compute table widths
    headers: [String; 4],
    available_path_w: u16,
}

impl StashOverlay {
    /// Creates a new `ScratchOverlay`.
    pub fn new(config: StashConfig) -> Self {
        Self {
            table_state: TableState::new(),
            editing: None,
            config,
            widths: Default::default(),
            headers: [
                "Action".pad(1, 0),
                "Source".pad(1, 1),
                "To".pad(1, 1),
                "Size".pad(0, 1),
            ],
            available_path_w: 0,
        }
    }

    pub fn border(&self) -> &BorderSetting {
        self.config.border.as_ref().unwrap()
    }

    pub fn enter_edit(&mut self) {
        let e = todo!();
        self.editing = Some(e);
    }

    pub fn width(&self) -> u16 {
        self.widths.iter().sum::<u16>() + self.border().width()
    }

    /// Computes and stores the column widths for Path, Size, and Flags
    pub fn save_widths(
        &mut self,
        items: &[StashItem],
        available_ui_w: u16,
    ) {
        // Pre-render Size column
        let size_col: Vec<_> = items
            .iter()
            .map(|item| format_size(item.status.size.load(Ordering::Relaxed)))
            .collect();

        // Compute max widths
        let mut kind_w = self.headers[0].len() as u16 + 1;

        for item in items {
            kind_w = kind_w.max(item.kind.to_string().len() as u16);
        }

        let mut path_w = 16;

        for item in items {
            path_w = path_w.max(item.display().width() as u16);
        }

        let mut dst_w = self.headers[2].len() as u16;
        for item in items {
            dst_w = dst_w.max(item.dst.to_string_lossy().width() as u16);
        }

        let mut size_w = 10;

        let available_path_w = available_ui_w
            .saturating_sub(self.border().width())
            .saturating_sub(kind_w + dst_w + size_w)
            .max(16);

        self.available_path_w = available_path_w;
        path_w = path_w.min(available_path_w);

        self.widths = [kind_w, path_w, dst_w, size_w];
    }

    /// Creates a `Table` widget from a slice of `StackItem`s using stored column widths
    pub fn make_table(
        &self,
        items: &[StashItem],
        custom_action: CustomStashActionActionState,
    ) -> Table<'static> {
        let config = &self.config;
        let header =
            Row::new(self.headers.clone()).style(Style::new().add_modifier(Modifier::BOLD));

        let rows: Vec<Row<'static>> = items
            .iter()
            .enumerate()
            .map(|(i, item)| {
                let kind = if item.is_custom() {
                    custom_action.to_string()
                } else {
                    item.kind.to_string()
                }
                .pad(1, 1);

                let dst = item.dst.to_string_lossy().pad(1, 1);
                let size = item.status.render(config);

                let path = Span::from(
                    item.display()
                        .ellipsize(self.widths[1] as usize, Alignment::Right)
                        .pad(
                            ((self.widths[1] + 1) < self.available_path_w) as usize,
                            (self.widths[1] < self.available_path_w) as usize,
                        ),
                );

                if Some(i) == self.table_state.selected() {
                    // manual highlight to keep cell styles
                    let style = Style::default().bg(Color::Black);
                    Row::new(vec![
                        Cell::from(kind).style(style),
                        Cell::from(path).style(style),
                        Cell::from(dst).style(style),
                        Cell::from(size).style(style),
                    ])
                } else {
                    let style = Style::default();
                    Row::new(vec![
                        Cell::from(kind),
                        Cell::from(path),
                        Cell::from(dst),
                        Cell::from(size),
                    ])
                }
            })
            .collect();

        Table::new(rows, self.widths)
            .column_spacing(0)
            .header(header)
            // .row_highlight_style(Style::default().bg(Color::Black))
            .block(self.border().as_static_block())
    }

    pub fn make_alternate_table(
        &self,
        items: &[AlternateStashItem], // we rely on the actual index, so no iterators
        header: Row<'static>,
    ) -> Table<'static> {
        let rows: Vec<Row<'static>> = items
            .iter()
            .enumerate()
            .map(|(i, item)| {
                let content = Span::from(item.display_short(__home()));

                if Some(i) == self.table_state.selected() {
                    let style = Style::default().bg(Color::Black);
                    Row::new(vec![Cell::from(content).style(style)])
                } else {
                    Row::new(vec![Cell::from(content)])
                }
            })
            .collect();

        Table::new(rows, [Constraint::Percentage(100)])
            .column_spacing(0)
            .header(header)
            .block(self.border().as_static_block())
    }
}

impl Overlay for StashOverlay {
    type A = FsAction;

    fn on_enable(
        &mut self,
        area: &Rect,
    ) {
        self.table_state = Default::default();
        STASH::check_validity();
    }
    fn on_disable(&mut self) {
        STASH::clear_completed();
    }
    fn handle_input(
        &mut self,
        c: char,
    ) -> OverlayEffect {
        if self.editing.is_none() {
            return match c {
                'q' => OverlayEffect::Disable,
                _ => OverlayEffect::None,
            };
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
        if STACK::in_app() {
            self.widths = Default::default();
            let pref_width = STASH::with_alternate(|scratch| {
                scratch
                    .iter()
                    .map(|s| s.to_string_lossy().len() + 2)
                    .max()
                    .unwrap_or_default() as u16
            })
            .min(ui_area.width * 9 / 10);

            Err([pref_width.into(), SizeHint::Min(self.border().height() + 4)])
        } else {
            STASH::with(|scratch| {
                self.save_widths(scratch, ui_area.width.saturating_sub(self.border().width()));
            });
            log::debug!("Stash widths: {:?}", self.widths);
            Err([
                self.width().into(),
                SizeHint::Min(self.border().height() + 4),
            ])
        }
    }

    fn handle_action(
        &mut self,
        action: &Action<Self::A>,
    ) -> OverlayEffect {
        if self.editing.is_none() {
            let len = STASH::with(|s| s.len());
            if len == 0 {
                return OverlayEffect::Disable;
            }

            match action {
                Action::Up(x) => {
                    if let Some(i) = self.table_state.selected_mut() {
                        let len = len as isize;
                        let cur = *i as isize;
                        let next = (cur - *x as isize).rem_euclid(len);
                        *i = next as usize;
                    }
                }

                Action::Down(x) => {
                    if let Some(i) = self.table_state.selected_mut() {
                        let len = len as isize;
                        let cur = *i as isize;
                        let next = (cur + *x as isize).rem_euclid(len);
                        *i = next as usize;
                    }
                }

                Action::Accept | Action::Select => {
                    if STACK::in_app() {
                        // todo: lowpri: spawn currently selected item directly?
                    } else if let Some(i) = self.table_state.selected() {
                        STASH::accept(i);
                    }
                }

                Action::DeleteChar => {
                    if let Some(i) = self.table_state.selected() {
                        STASH::remove(i);
                    }
                }

                Action::Custom(FsAction::Trash | FsAction::Delete) => {
                    // undecided
                    if let Some(i) = self.table_state.selected() {
                        STASH::remove(i);
                    }
                }

                Action::Print(s) if s.is_empty() => {
                    // undecided
                }

                Action::Print(s) if s.is_empty() => {
                    // undecided
                }

                Action::Custom(f) => match f {
                    FsAction::Menu => {
                        self.enter_edit();
                    }
                    FsAction::Undo => {
                        STASH::cycle_cas(true);
                    }
                    FsAction::Redo => {
                        STASH::cycle_cas(false);
                    }
                    _ => {}
                },

                Action::Quit(_) => return OverlayEffect::Disable,
                _ => {}
            }
        } else {
            match action {
                Action::BackwardWord => {
                    todo!()
                }
                Action::ForwardWord => {
                    todo!()
                }
                Action::BackwardChar => {
                    todo!()
                }
                Action::ForwardChar => {
                    todo!()
                }
                Action::BackwardWord => {
                    todo!()
                }
                Action::BackwardWord => {
                    todo!()
                }
                Action::DeleteChar => {
                    todo!()
                }
                Action::DeleteWord => {
                    todo!()
                }
                Action::Accept => {
                    self.editing = None;
                }
                Action::Quit(_) => {
                    self.editing = None;
                }
                _ => {}
            }
        }

        OverlayEffect::None
    }

    fn draw(
        &mut self,
        frame: &mut matchmaker::ui::Frame<'_>,
        mut area: matchmaker::ui::Rect,
    ) {
        let custom_action = STASH::cas();
        if matches!(STASH::cas(), CustomStashActionActionState::App) {
            let header =
                Row::new(vec!["To open:"]).style(Style::new().add_modifier(Modifier::BOLD));
            STASH::with_alternate(|scratch| {
                if scratch.is_empty() {
                    self.table_state.select(None);
                    let msg = "Scratch is empty";
                    area.height = 3 + self.border().height();
                    frame.render_widget(Clear, area);
                    frame.render_widget(
                        Paragraph::new(vec![
                            Line::raw("".pad(area.width as usize, 0)),
                            Line::raw(msg).alignment(HorizontalAlignment::Center),
                            Line::raw("".pad(area.width as usize, 0)),
                        ])
                        .block(self.border().as_block()),
                        area,
                    );

                    return;
                }

                // 1. ensure state
                let len = scratch.len();
                if let Some(selected) = self.table_state.selected() {
                    if selected >= len {
                        self.table_state.select(Some(len - 1));
                    }
                } else {
                    self.table_state.select(Some(0));
                }

                // 2. make table
                let table = self.make_alternate_table(scratch, header);

                // 3. render
                frame.render_widget(Clear, area);
                frame.render_stateful_widget(table, area, &mut self.table_state);
            });
        } else {
            STASH::with(|scratch| {
                if scratch.is_empty() {
                    self.table_state.select(None);
                    let msg = "Scratch is empty";
                    area.height = 3 + self.border().height();
                    frame.render_widget(Clear, area);
                    frame.render_widget(
                        Paragraph::new(vec![
                            Line::raw("".pad(area.width as usize, 0)),
                            Line::raw(msg).alignment(HorizontalAlignment::Center),
                            Line::raw("".pad(area.width as usize, 0)),
                        ])
                        .block(self.border().as_block()),
                        area,
                    );

                    return;
                }

                // 1. ensure state
                let len = scratch.len();
                if let Some(selected) = self.table_state.selected() {
                    if selected >= len {
                        self.table_state.select(Some(len - 1));
                    }
                } else {
                    self.table_state.select(Some(0));
                }

                // 2. make table
                let table = self.make_table(scratch, custom_action);

                log::debug!("{scratch:?}");

                // 3. render
                frame.render_widget(Clear, area);
                frame.render_stateful_widget(table, area, &mut self.table_state);
            });
        }
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

        // bar is too hard to size although it would be cool
        // (
        //     format!(
        //         "[{}{} {}]", // bar_width + 8
        //         "█".repeat(filled_width as usize),
        //         "░".repeat(empty_width as usize),
        //         progress_text
        //     ),
        //     8,
        // )

        let style = match state {
            StashItemState::Pending => Style::default(),
            StashItemState::Started => {
                let percent = (progress as f32 / 255.0);
                let mut text =
                    format!("{:5.2}%", percent * 100.0).pad_to(10, std::fmt::Alignment::Center);
                let (left, right) = if percent == 1.0 {
                    (text.as_str(), "")
                } else {
                    text.split_at((percent * 10.0)._trunc())
                };

                return Line::default().spans([
                    Span::styled(left.to_string(), Style::default().bg(Color::Cyan)),
                    Span::styled(right.to_string(), Color::Cyan),
                ]);
            }
            StashItemState::CompleteOk => Style::default().fg(Color::Green),
            StashItemState::PendingErr => Style::default().fg(Color::LightRed),
            StashItemState::CompleteErr => Style::default().fg(Color::Red),
        };

        let size_text = format_size(size);
        let bar_text = size_text
            .pad((size_text.len() <= 9) as usize, 0)
            .pad_to(10, std::fmt::Alignment::Left);

        Line::styled(bar_text, style)
    }
}
