use crate::{
    run::{
        FsPane,
        action::FsAction,
        state::{FILTERS, GLOBAL, STACK},
    },
    utils::{serde::border_result, text::bold_indices},
};

use cba::bum::UsizeExt;
use fist_types::{When, filters::*};
use matchmaker::{
    action::Action,
    config::{BorderSetting, PartialBorderSetting},
    ui::{Overlay, OverlayEffect, SizeHint},
};

use ratatui::{
    prelude::*,
    widgets::{Clear, Paragraph},
};
use strum::IntoEnumIterator;

// todo: support compact
const PANE_WIDTH: u16 = const { 4 + 17 + 1 };

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct FilterBaseConfig {
    #[serde(with = "border_result")]
    pub border: Result<BorderSetting, PartialBorderSetting>,
    pub item_fg: Color,
    pub item_modifier: Modifier,
    pub alignment: HorizontalAlignment,
}

#[derive(Default, Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct FilterPaneConfig {
    pub border: BorderSetting,
    pub alignment: Option<HorizontalAlignment>,
}

impl Default for FilterBaseConfig {
    fn default() -> Self {
        let border = PartialBorderSetting {
            title: Some("Filters".into()),
            ..Default::default()
        };
        Self {
            border: Err(border),
            item_fg: Color::DarkGray,
            item_modifier: Default::default(),
            alignment: Default::default(),
        }
    }
}

#[derive(Debug, Default, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct FiltersConfig {
    #[serde(flatten)]
    pub base: FilterBaseConfig,
    pub filter: FilterPaneConfig,
    pub sort: FilterPaneConfig,
    pub pane: FilterPaneConfig,
}

impl FiltersConfig {
    pub fn into_tuple(self) -> (FilterBaseConfig, [FilterPaneConfig; 3]) {
        (self.base, [self.filter, self.sort, self.pane])
    }
}

#[derive(Default)]
pub struct FilterOverlay {
    cursor: [usize; 2], // [pane_index, item_index]
    pane_lens: [usize; 3],
    config: FilterBaseConfig,
    pub configs: [FilterPaneConfig; 3],
}

/// Renders a horizontal mural of paragraphs, declared in [`FilterOverlay::make_widgets`]
impl FilterOverlay {
    pub fn new(config: FiltersConfig) -> Self {
        let (config, configs) = config.into_tuple();
        Self {
            config,
            configs,
            ..Default::default()
        }
    }

    pub fn border(&self) -> &BorderSetting {
        self.config.border.as_ref().unwrap()
    }

    pub fn item_style(&self) -> Style {
        Style::default()
            .add_modifier(self.config.item_modifier)
            .fg(self.config.item_fg)
    }

    pub fn height(&self) -> u16 {
        self.pane_lens
            .iter()
            .max()
            .map(|v| *v as u16 + 2)
            .unwrap_or(2)
            + self.border().height()
    }
    pub fn width(&self) -> u16 {
        self.pane_lens.iter().filter(|&&v| v != 0).count() as u16 * PANE_WIDTH
            + self.border().width()
    }
    pub fn num_panes(&self) -> usize {
        self.pane_lens.iter().filter(|&&v| v > 0).count()
    }

    // ----------------- MAKE WIDGETS -------------------------------

    // Returns Vec<Span> for sort options
    // Returns items as Vec<(Vec<Span>, bool)> so make_widgets can add checkboxes
    fn get_sort_items(&self) -> Vec<(Vec<Span<'static>>, Option<bool>)> {
        let current_sort_order = FILTERS::sort();
        SortOrder::iter()
            .map(|so| {
                let spans = bold_indices(so.into(), [0], self.item_style());
                let checked = so == current_sort_order;
                (spans, Some(checked))
            })
            .collect()
    }

    // active or not
    fn get_visibility_items(&self) -> Vec<(Vec<Span<'static>>, Option<bool>)> {
        let vis = FILTERS::visibility();

        let hidden_label = if vis.hidden_only {
            let label = if vis.files {
                "Hidden+files"
            } else if vis.dirs {
                "Hidden+dirs"
            } else {
                "Hidden only"
            };
            bold_indices(label, [0], self.item_style())
        } else if vis.hidden {
            bold_indices("hidden (files: H)", [0, 15], self.item_style())
        } else {
            bold_indices("hidden", [0], self.item_style())
        };

        let dirs_label = if STACK::in_rg() {
            Default::default()
        } else {
            (bold_indices("Dirs", [0], self.item_style()), Some(vis.dirs))
        };
        let mut ret = vec![
            (hidden_label, Some(vis.hidden || vis.hidden_only)),
            (
                bold_indices("Ignore", [0], self.item_style()),
                Some(vis.ignore),
            ),
            dirs_label,
            (bold_indices("all", [0], self.item_style()), Some(vis.all())),
        ];

        ret
    }

    // Returns Vec<Span> for sort options
    // Returns items as Vec<(Vec<Span>, bool)> so make_widgets can add checkboxes
    fn get_pane_items(&self) -> Vec<(Vec<Span<'static>>, Option<bool>)> {
        STACK::with_current(|p| match p {
            FsPane::Search {
                context: [before, after],
                case,
                no_heading,
                fixed_strings,
                ..
            } => {
                // build context info line
                let mut context = vec![];
                let c = format!("[{before}, {after}] ").into();
                context.push(c);
                let mut hint = bold_indices("(B, A)", [1, 4], self.item_style())
                    .into_iter()
                    .map(|s| s.patch_style(Style::new().italic()))
                    .collect();
                context.append(&mut hint);

                let inc_context = bold_indices("+Context", [1], self.item_style());
                let dec_context = bold_indices("-context", [1], self.item_style());

                let sep = vec![];
                let case_str = match case {
                    When::Always => "case",
                    When::Auto => "Smart case",
                    When::Never => "case",
                };
                let single = bold_indices("1-line", [0], self.item_style());
                let regex = bold_indices("regex", [0], self.item_style());

                vec![
                    (context, None),
                    (inc_context, None),
                    (dec_context, None),
                    (sep, None),
                    (
                        bold_indices(case_str, [case_str.len() - 1], self.item_style()),
                        (*case).into(),
                    ),
                    (single, Some(*no_heading)),
                    (regex, Some(!*fixed_strings)),
                ]
            }
            // FsPane::Fd { .. } => {

            // }
            _ => vec![],
        })
    }

    fn on_inactive_y(&self) -> bool {
        let [x, y] = self.cursor;
        assert!(self.pane_lens[x] != 0);

        match x {
            2 => matches!(y, 0 | 3),
            1 => STACK::in_rg() && y == 2,
            _ => false,
        }
    }

    // make_widgets now just prepends checkboxes and handles cursor styling
    fn make_widgets(&self) -> Vec<Paragraph<'static>> {
        let mut make_pane = |pane_idx: usize, items: &[(Vec<Span<'static>>, Option<bool>)]| {
            let max_width = items
                .iter()
                .map(|(spans, checked)| {
                    let mut width = 0;

                    if let Some(checked) = checked {
                        width += 4; // "[x] " or "[ ] "
                    }

                    for span in spans {
                        width += span.width()
                    }

                    width
                })
                .max()
                .unwrap_or(0);

            let alignment = self.configs[pane_idx]
                .alignment
                .unwrap_or(self.config.alignment);

            let lines: Vec<Line> = items
                .iter()
                .enumerate()
                .map(|(idx, (spans, checked))| {
                    let mut line_spans = vec![];

                    if let Some(checked) = checked {
                        line_spans.push(Span::raw(if *checked { "[x] " } else { "[ ] " }))
                    }

                    line_spans.extend(spans.clone());

                    let mut line = Line::from(line_spans).alignment(alignment);

                    let right_pad = max_width.saturating_sub(line.width());
                    if right_pad > 0 {
                        let padding = " ".repeat(right_pad);
                        line.spans.push(Span::raw(padding));
                    }

                    if pane_idx == self.cursor[0] && idx == self.cursor[1] {
                        line = line.patch_style(Style::default().add_modifier(Modifier::BOLD));
                    }

                    line
                })
                .collect();

            Paragraph::new(lines).block(self.configs[pane_idx].border.as_static_block())
        };

        let mut widgets = vec![];
        for (i, x) in [
            self.get_visibility_items(),
            self.get_sort_items(),
            self.get_pane_items(),
        ]
        .iter()
        .enumerate()
        {
            if self.pane_lens[i] > 0 {
                let pane = make_pane(i, x);
                widgets.push(pane);
            }
        }

        widgets
    }

    // --------------------------------------------------------------

    /// Handler for cursor selection
    fn toggle_selected_option(&mut self) {
        let [x, y] = self.cursor;
        let mut refilter = !matches!(x, 2);
        let mut reload = !refilter;

        FILTERS::with_mut(|sort, vis| {
            match x {
                // visibility pane
                0 => {
                    if !matches!(y, 2 | 3) {
                        vis.set_all(false);
                    }
                    match y {
                        0 => {
                            (vis.hidden, vis.hidden_only) = if vis.hidden_only {
                                vis.files = false;
                                (false, false)
                            } else if vis.hidden {
                                if !vis.dirs {
                                    vis.files = true;
                                }
                                (false, true)
                            } else {
                                (true, false)
                            }
                        }
                        1 => vis.ignore = !vis.ignore,
                        2 => vis.dirs = !vis.dirs,
                        3 => vis.toggle_all(),
                        _ => {}
                    }
                }

                // sort pane
                1 => {
                    if let Some(new_sort_order) = SortOrder::iter().nth(y) {
                        *sort = new_sort_order;
                    }
                }

                2 => STACK::with_current_mut(|p| match p {
                    FsPane::Search {
                        context,
                        case,
                        no_heading,
                        fixed_strings,
                        ..
                    } => match y {
                        1 => {
                            context[0] += 1;
                            context[1] += 1;
                        }
                        2 => {
                            reload = *context != [0, 0];
                            context[0].ssub(1);
                            context[1].ssub(1);
                        }
                        4 => case.cycle(),
                        5 => *no_heading = !(*no_heading),
                        6 => *fixed_strings = !(*fixed_strings),
                        _ => {}
                    },

                    _ => {}
                }),

                _ => {}
            }
        });
        if refilter {
            FILTERS::refilter();
        };
        if reload {
            GLOBAL::send_action(FsAction::Reload);
        }
    }
}

impl Overlay for FilterOverlay {
    type A = FsAction;

    fn handle_input(
        &mut self,
        c: char,
    ) -> OverlayEffect {
        let mut refilter = true;
        let mut reload = false;
        // let mut found = false;

        match c {
            'q' => return OverlayEffect::Disable,

            // visibility toggles
            'h' | 'H' | 'I' | 'a' | 'd' | 'D' if self.pane_lens[0] > 0 => {
                FILTERS::with_mut(|_sort, vis| {
                    if !matches!(c, 'D' | 'a') {
                        vis.set_all(false);
                    }
                    match c {
                        'h' => (vis.hidden, vis.hidden_only) = (!vis.hidden, false),
                        'H' => {
                            if !vis.dirs {
                                vis.files = true;
                            }
                            (vis.hidden, vis.hidden_only) = (false, !vis.hidden_only)
                        }
                        'd' | 'D' => {
                            if !STACK::in_rg() {
                                vis.dirs = !vis.dirs
                            }
                        }
                        'I' => vis.ignore = !vis.ignore,
                        'a' => vis.toggle_all(),
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

            _ if self.pane_lens[2] > 0 => {
                refilter = false;
                reload = true;

                STACK::with_current_mut(|p| match p {
                    FsPane::Search {
                        context,
                        case,
                        no_heading,
                        fixed_strings,
                        ..
                    } => match c {
                        'a' => reload = context[1].ssub(1),
                        'A' => context[1] += 1,
                        'b' => reload = context[0].ssub(1),
                        'B' => context[0] += 1,
                        'c' => {
                            reload = *context != [0, 0];
                            context[0].ssub(1);
                            context[1].ssub(1);
                        }
                        'C' => {
                            context[0] += 1;
                            context[1] += 1;
                        }

                        'e' => case.cycle(),
                        '1' => *no_heading = !(*no_heading),
                        'r' => *fixed_strings = !(*fixed_strings),

                        _ => reload = false,
                    },
                    _ => {}
                });
            }

            _ => {}
        }

        if refilter {
            FILTERS::refilter();
        };
        if reload {
            GLOBAL::send_action(FsAction::Reload);
        }

        OverlayEffect::None
    }

    fn on_enable(
        &mut self,
        _area: &Rect,
    ) {
        self.pane_lens[0] = if STACK::with_current(|x| x.supports_vis()) {
            self.get_visibility_items().len()
        } else {
            0
        };

        self.pane_lens[1] = if STACK::with_current(|x| x.supports_sort()) {
            self.get_sort_items().len()
        } else {
            0
        };

        self.pane_lens[2] = self.get_pane_items().len();

        self.cursor = [
            self.pane_lens
                .iter()
                .position(|l| *l > 0)
                .unwrap_or_default(),
            0,
        ];
        while self.on_inactive_y() {
            self.cursor[1] += 1;
        }

        log::debug!(
            "Filter: lens: {:?}, cursor: {:?}",
            self.pane_lens,
            self.cursor
        );
    }

    fn handle_action(
        &mut self,
        action: &Action<Self::A>,
    ) -> OverlayEffect {
        let num_panes = self.pane_lens.len();
        if num_panes == 0 {
            return OverlayEffect::Disable;
        }
        let mut down = false;

        match action {
            Action::Up(_) | Action::Down(_) => {
                down = matches!(action, Action::Down(_));
                if down {
                    self.cursor[1] += 1
                } else {
                    self.cursor[1].ssub(1);
                }
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
                self.cursor[0].wsub(1, num_panes);

                while self.pane_lens[self.cursor[0]] == 0 {
                    self.cursor[0].wsub(1, num_panes);
                }
            }
            Action::Accept => self.toggle_selected_option(),
            Action::Quit(_) => return OverlayEffect::Disable,
            _ => {}
        }

        // Cap cursor-y
        while self.on_inactive_y() {
            if down {
                self.cursor[1] += 1
            } else {
                self.cursor[1].wsub(1, self.pane_lens[self.cursor[0]]);
            }
        }
        OverlayEffect::None
    }

    fn area(
        &mut self,
        _ui_area: &Rect,
    ) -> Result<Rect, [SizeHint; 2]> {
        Err([self.width().into(), self.height().into()])
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

        let mut inner_area = area;
        inner_area.width -= self.border().width();
        inner_area.height -= self.border().height();
        inner_area.x += self.border().left();
        inner_area.y += self.border().top();

        let chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints(constraints)
            .split(inner_area);

        for (i, widget) in widgets.into_iter().enumerate() {
            frame.render_widget(widget, chunks[i]);
        }

        frame.render_widget(self.border().as_block(), area);
    }
}
