use crate::{
    filters::*,
    run::globals::{FILTERS, GLOBAL, STACK},
    utils::text::bold_indices,
};
use matchmaker::{
    action::Action,
    config::BorderSetting,
    efx,
    render::Effect,
    ui::{Overlay, OverlayEffect},
};
use ratatui::{
    prelude::*,
    widgets::{Block, Borders, Clear, Paragraph},
};
use strum::IntoEnumIterator;

use crate::run::fsaction::FsAction;

// todo: -8 on compact
const PANE_WIDTH: u16 = const { 4 + 17 + 1 };

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct FiltersConfig {
    border: BorderSetting,
    filter_border: BorderSetting,
    sort_border: BorderSetting,
}

impl Default for FiltersConfig {
    fn default() -> Self {
        let filter_border = BorderSetting {
            ..Default::default()
        };
        let sort_border = BorderSetting {
            ..Default::default()
        };
        Self {
            border: Default::default(),
            filter_border,
            sort_border,
        }
    }
}

#[derive(Default)]
pub struct FilterOverlay {
    cursor: [usize; 2], // [pane_index, item_index]
    pane_lens: [usize; 2],
    config: FiltersConfig,
}

impl FilterOverlay {
    pub fn new(config: FiltersConfig) -> Self {
        Self {
            config,
            ..Default::default()
        }
    }

    pub fn height(&self) -> u16 {
        self.pane_lens
            .iter()
            .max()
            .map(|v| *v as u16 + 2)
            .unwrap_or(2)
            + self.config.border.height()
    }
    pub fn width(&self) -> u16 {
        self.pane_lens.iter().filter(|&&v| v != 0).count() as u16 * PANE_WIDTH
            + self.config.border.width()
    }
    pub fn num_panes(&self) -> usize {
        self.pane_lens.iter().filter(|&&v| v > 0).count()
    }

    // ----------------- MAKE WIDGETS -------------------------------

    // Returns Vec<Span> for sort options
    // Returns items as Vec<(Vec<Span>, bool)> so make_widgets can add checkboxes
    fn get_sort_items() -> Vec<(Vec<Span<'static>>, bool)> {
        let current_sort_order = FILTERS::sort();
        SortOrder::iter()
            .map(|so| {
                let spans = bold_indices(so.into(), [0]);
                let checked = so == current_sort_order;
                (spans, checked)
            })
            .collect()
    }

    fn get_visibility_items() -> Vec<(Vec<Span<'static>>, bool)> {
        let visibility = FILTERS::visibility();

        let hidden_label = if visibility.hidden_files {
            bold_indices("Hidden files", [0])
        } else if visibility.hidden {
            bold_indices("hidden (files: H)", [0, 15])
        } else {
            bold_indices("hidden", [0])
        };

        vec![
            (hidden_label, visibility.hidden || visibility.hidden_files),
            (bold_indices("Ignore", [0]), visibility.ignore),
            (bold_indices("Dirs", [0]), visibility.dirs),
            (bold_indices("all", [0]), visibility.all()),
        ]
    }

    // make_widgets now just prepends checkboxes and handles cursor styling
    fn make_widgets(&self) -> Vec<Paragraph<'static>> {
        let mut widgets = Vec::new();
        let mut current_pane_idx = 0;

        let mut push_pane = |items: &[(Vec<Span<'static>>, bool)]| {
            let lines: Vec<Line> = items
                .iter()
                .enumerate()
                .map(|(idx, (spans, checked))| {
                    let mut line_spans = vec![Span::raw(if *checked { "[x] " } else { "[ ] " })];
                    line_spans.extend(spans.clone());

                    let mut line = Line::from(line_spans);

                    if current_pane_idx == self.cursor[0] && idx == self.cursor[1] {
                        line = line.patch_style(Style::default().add_modifier(Modifier::BOLD));
                    }

                    line
                })
                .collect();

            widgets.push(Paragraph::new(lines).block(Block::default()));
            current_pane_idx += 1;
        };

        if self.pane_lens[0] > 0 {
            push_pane(&Self::get_visibility_items());
        }

        if self.pane_lens[1] > 0 {
            push_pane(&Self::get_sort_items());
        }

        widgets
    }

    // --------------------------------------------------------------

    /// Handler for cursor selection
    fn toggle_selected_option(&mut self) {
        FILTERS::with_mut(|sort, visibility| {
            let [x, y] = self.cursor;
            match x {
                // visibility pane
                0 => {
                    if !matches!(y, 2 | 3) {
                        visibility.set_all(false);
                    }
                    match y {
                        0 => {
                            (visibility.hidden, visibility.hidden_files) =
                                if visibility.hidden_files {
                                    (false, false)
                                } else if visibility.hidden {
                                    (false, true)
                                } else {
                                    (true, false)
                                }
                        }
                        1 => visibility.ignore = !visibility.ignore,
                        2 => visibility.dirs = !visibility.dirs,
                        3 => visibility.toggle_all(),
                        _ => {}
                    }
                }

                // sort pane
                1 => {
                    if let Some(new_sort_order) = SortOrder::iter().nth(y) {
                        *sort = new_sort_order;
                    }
                }

                _ => {}
            }
        });
    }
}

impl Overlay for FilterOverlay {
    type A = FsAction;

    fn on_enable(
        &mut self,
        _area: &Rect,
    ) {
        self.pane_lens[0] = if STACK::current().supports_sort() {
            Self::get_visibility_items().len()
        } else {
            0
        };

        self.pane_lens[1] = if true {
            Self::get_sort_items().len()
        } else {
            0
        };

        self.cursor = [0, 0];
    }

    fn handle_input(
        &mut self,
        c: char,
    ) -> OverlayEffect {
        match c {
            'q' => return OverlayEffect::Disable,

            // visibility toggles
            'h' | 'H' | 'I' | 'a' | 'd' | 'D' => {
                FILTERS::with_mut(|_sort, visibility| {
                    if !matches!(c, 'D' | 'a') {
                        visibility.set_all(false);
                    }
                    match c {
                        'h' => {
                            (visibility.hidden, visibility.hidden_files) =
                                (!visibility.hidden, false)
                        }
                        'H' => {
                            (visibility.hidden, visibility.hidden_files) =
                                (false, !visibility.hidden_files)
                        }
                        'd' | 'D' => visibility.dirs = !visibility.dirs,
                        'I' => visibility.ignore = !visibility.ignore,
                        'a' => visibility.toggle_all(),
                        _ => {}
                    }
                });
            }

            'n' | 'm' | 's' if self.pane_lens[1] > 0 => {
                let toggle_sort = |target: SortOrder| {
                    FILTERS::with_mut(|sort, _| {
                        *sort = if *sort == target {
                            SortOrder::none
                        } else {
                            target
                        };
                    });
                };

                match c {
                    'n' => toggle_sort(SortOrder::name),
                    'm' => toggle_sort(SortOrder::mtime),
                    's' => toggle_sort(SortOrder::size),
                    _ => {}
                }
            }

            _ => {}
        }

        FILTERS::refilter();

        OverlayEffect::None
    }

    fn handle_action(
        &mut self,
        action: &Action<Self::A>,
    ) -> OverlayEffect {
        let num_panes = self.pane_lens.iter().filter(|&&v| v > 0).count();
        if num_panes == 0 {
            return OverlayEffect::Disable;
        }

        match action {
            Action::Up(_) => {
                self.cursor[1] = self.cursor[1].saturating_sub(1);
            }
            Action::Down(_) => {
                self.cursor[1] += 1;
            }
            Action::ForwardChar => {
                // Right
                self.cursor[0] = (self.cursor[0] + 1) % num_panes;
                while self.pane_lens[self.cursor[0]] == 0 {
                    self.cursor[0] = (self.cursor[0] + 1) % num_panes;
                }
            }
            Action::BackwardChar => {
                // Left
                self.cursor[0] = (self.cursor[0] + 1) % num_panes;
                while self.pane_lens[self.cursor[0]] == 0 {
                    self.cursor[0] = (self.cursor[0] - 1) % num_panes;
                }
            }
            Action::Accept => self.toggle_selected_option(),
            Action::Quit(_) => return OverlayEffect::Disable,
            _ => {}
        }

        // Cap cursor-y
        self.cursor[1] = self.cursor[1].min(self.pane_lens[self.cursor[0]] - 1);
        log::debug!("{:?}", self.cursor);
        OverlayEffect::None
    }

    fn area(
        &mut self,
        _ui_area: &Rect,
    ) -> Result<Rect, [u16; 2]> {
        Err([self.width(), self.height()])
    }

    fn draw(
        &mut self,
        frame: &mut matchmaker::ui::Frame,
        area: matchmaker::ui::Rect,
    ) {
        frame.render_widget(Clear, area);

        let widgets = self.make_widgets();
        if widgets.is_empty() {
            return;
        }

        // make layout of constant width panes
        let constraints: Vec<Constraint> = (0..widgets.len())
            .map(|_| Constraint::Length(PANE_WIDTH))
            .collect();
        let chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints(constraints)
            .split(area);

        for (i, widget) in widgets.into_iter().enumerate() {
            frame.render_widget(widget, chunks[i]);
        }

        frame.render_widget(self.config.border.as_block(), area);
    }
}
