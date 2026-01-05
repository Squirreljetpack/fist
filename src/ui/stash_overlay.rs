use crate::{
    run::{
        fsaction::FsAction,
        stash::{STASH, StackAction, StackItem, StackItemState, StackItemStatus},
    },
    utils::format_size,
};
use cli_boilerplate_automation::{bath::PathExt, impl_transparent_wrapper, text::StrExt, vec_};
use matchmaker::{
    action::{Action, Count},
    config::{self, BorderSetting},
    ui::{Overlay, OverlayEffect},
};
use ratatui::{
    prelude::*,
    widgets::{
        Block, BorderType, Borders, Cell, Clear, HighlightSpacing, Paragraph, Row, Table,
        TableState,
    },
};
use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};
use unicode_width::UnicodeWidthStr;

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct StackConfig {
    pub border: BorderSetting,
    pub bar_width: BarWidth,
    pub column_spacing: usize,
}

impl Default for StackConfig {
    fn default() -> Self {
        let border = BorderSetting {
            sides: Borders::ALL,
            ..Default::default()
        };
        Self {
            border,
            bar_width: Default::default(),
            column_spacing: 2,
        }
    }
}
impl_transparent_wrapper!(BarWidth, u8, 15);

// ratatui table
// current row is highlighted
// columns: item.path, size, details (currently just says cp, mv or sym)
pub struct StackOverlay {
    table_state: TableState,
    editing: Option<(usize, String)>, // editing column + original buffer item
    config: StackConfig,
    widths: [u16; 4], // stored to help compute table widths
    headers: [String; 4],
    left_pad: bool, // this is a mistake but we'll keep it
}

impl StackOverlay {
    /// Creates a new `ScratchOverlay`.
    pub fn new(config: StackConfig) -> Self {
        Self {
            table_state: TableState::new(),
            editing: None,
            config,
            widths: Default::default(),
            left_pad: false,
            headers: [
                "Action".pad(1, 0),
                "Source".pad(1, 1),
                "Dst".pad(1, 1),
                "Size".pad(0, 1),
            ],
        }
    }

    pub fn enter_edit(&mut self) {
        let e = todo!();
        self.editing = Some(e);
    }

    pub fn cursor_prev(&mut self) {
        if let Some(i) = self.table_state.selected_mut() {
            *i = i.saturating_sub(1);
        }
    }
    pub fn cursor_next(&mut self) {
        if let Some(i) = self.table_state.selected_mut() {
            *i = i.saturating_add(1);
        }
    }

    pub fn width(&self) -> u16 {
        self.widths.iter().sum::<u16>() + self.config.border.width()
    }

    /// Computes and stores the column widths for Path, Size, and Flags
    pub fn save_widths(
        &mut self,
        items: &[StackItem],
        available_w: u16,
    ) {
        // Pre-render Size column
        let size_col: Vec<_> = items
            .iter()
            .map(|item| item.status.render(&self.config))
            .collect();

        // Compute max widths
        let mut actions_w = self.headers[0].len() as u16 + 1;

        for item in items {
            actions_w = actions_w.max(item.kind.to_string().len() as u16);
        }

        let mut size_w = self.headers[3].len() as u16;
        for s in &size_col {
            size_w = size_w.max(s.width() as u16);
        }

        let dst_w = self.headers[2].len() as u16;

        let mut path_w = 16;

        for item in items {
            path_w = path_w.max(item.display().width() as u16);
        }

        let available_path_w = available_w
            .saturating_sub(size_w + actions_w + dst_w + path_w)
            .max(16);

        if path_w >= available_path_w {
            self.left_pad = false;
            path_w = available_path_w
        } else if self.left_pad {
            path_w += 1
        }

        self.widths = [actions_w, path_w, dst_w, size_w];
    }

    /// Creates a `Table` widget from a slice of `StackItem`s using stored column widths
    pub fn make_table(
        &self,
        items: &[StackItem],
    ) -> Table<'static> {
        let config = &self.config;
        let header =
            Row::new(self.headers.clone()).style(Style::new().add_modifier(Modifier::BOLD));

        let rows: Vec<Row<'static>> = items
            .iter()
            .enumerate()
            .map(|(i, item)| {
                let kind = item.kind.to_string().pad(1, 1);
                let path = Span::from(
                    item.display()
                        .truncate_left(self.widths[1] as usize - self.left_pad as usize)
                        .pad(self.left_pad as usize, 1),
                );
                let dst = item.dest.pad(1, 1);
                let size = item.status.render(config);

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
                        Cell::from(path),
                        Cell::from(dst),
                        Cell::from(size),
                        Cell::from(kind),
                    ])
                }
            })
            .collect();

        Table::new(rows, self.widths)
            .column_spacing(0)
            .header(header)
            // .row_highlight_style(Style::default().bg(Color::Black))
            .block(self.config.border.as_static_block())
    }
}

impl Overlay for StackOverlay {
    type A = FsAction;

    fn on_enable(
        &mut self,
        area: &Rect,
    ) {
        STASH::check_validity();
    }
    fn on_disable(&mut self) {
        STASH::clear_invalid_and_completed();
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
    ) -> Result<Rect, [u16; 2]> {
        STASH::with(|scratch| {
            let items = &scratch.stack;
            self.save_widths(items, ui_area.width);
        });
        Err([self.width(), 0])
    }

    fn handle_action(
        &mut self,
        action: &Action<Self::A>,
    ) -> OverlayEffect {
        if self.editing.is_none() {
            let len = STASH::with(|s| s.stack.len());
            if len == 0 {
                return OverlayEffect::Disable;
            }

            match action {
                Action::Up(x) => {
                    for _ in 0..x.0 {
                        self.cursor_prev();
                    }
                }
                Action::Down(x) => {
                    if let Some(i) = self.table_state.selected() {
                        for _ in 0..(len.saturating_sub(i + 1).max(x.0 as usize)) {
                            self.cursor_next();
                        }
                    }
                }
                Action::Accept | Action::Select => {
                    if let Some(i) = self.table_state.selected() {
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

                Action::Custom(FsAction::Menu) => {
                    self.enter_edit();
                }

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
        STASH::with(|scratch| {
            let items = &scratch.stack;
            if items.is_empty() {
                self.table_state.select(None);
                let msg = "Scratch is empty";
                area.height = 3 + self.config.border.height();
                frame.render_widget(Clear, area);
                frame.render_widget(
                    Paragraph::new(vec![
                        Line::raw("".pad(area.width as usize, 0)),
                        Line::raw(msg).alignment(Alignment::Center),
                        Line::raw("".pad(area.width as usize, 0)),
                    ])
                    .block(self.config.border.as_block()),
                    area,
                );
                return;
            }

            // 1. ensure state
            let len = items.len();
            if let Some(selected) = self.table_state.selected() {
                if selected >= len {
                    self.table_state.select(Some(len - 1));
                }
            } else {
                self.table_state.select(Some(0));
            }

            // 2. make table from config
            let table = self.make_table(items);

            // 3. render
            frame.render_widget(Clear, area);
            frame.render_stateful_widget(table, area, &mut self.table_state);
        });
    }
}
