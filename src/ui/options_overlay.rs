use crate::{
    run::{
        action::FsAction,
        item::PathItem,
        state::{FILTERS, GLOBAL, STACK},
        FsPane,
    },
    utils::{serde::border_result, text::bold_indices},
};

use cba::bum::UsizeExt;
use fist_types::{filters::*, When};
use matchmaker::{
    action::Action,
    config::{BorderSetting, OverlayLayoutSettings, PartialBorderSetting},
    render::MMState,
    ui::{utils, Overlay, OverlayEffect},
};

use ratatui::{
    prelude::*,
    widgets::{Clear, Paragraph},
};

// todo: support compact
const PANE_WIDTH: u16 = const { 4 + 17 + 1 };

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct OptionsBaseConfig {
    #[serde(with = "border_result")]
    pub border: Result<BorderSetting, PartialBorderSetting>,
    pub item_fg: Color,
    pub item_modifier: Modifier,
    pub alignment: HorizontalAlignment,
}

#[derive(Default, Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct OptionsPaneConfig {
    pub border: BorderSetting,
    pub alignment: Option<HorizontalAlignment>,
}

impl Default for OptionsBaseConfig {
    fn default() -> Self {
        let border = PartialBorderSetting {
            title: Some("Options".into()),
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
pub struct OptionsConfig {
    #[serde(flatten)]
    pub base: OptionsBaseConfig,
    pub filter: OptionsPaneConfig,
    pub sort: OptionsPaneConfig,
    pub pane: OptionsPaneConfig,
}

impl OptionsConfig {
    pub fn into_tuple(self) -> (OptionsBaseConfig, [OptionsPaneConfig; 3]) {
        (self.base, [self.filter, self.sort, self.pane])
    }
}

#[derive(Default)]
pub struct OptionsOverlay {
    cursor: [usize; 2], // [pane_index, item_index]
    pane_lens: [usize; 3],
    config: OptionsBaseConfig,
    pub configs: [OptionsPaneConfig; 3],
    area: Rect, // inner area
}

/// Renders a horizontal mural of paragraphs, declared in [`OptionsOverlay::make_widgets`]
impl OptionsOverlay {
    pub fn new(config: OptionsConfig) -> Self {
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

    pub fn handle_action_nav(
        &mut self,
        action: &Action<FsAction>,
    ) -> OverlayEffect {
        let num_panes = self.pane_lens.len();
        if num_panes == 0 {
            return OverlayEffect::Disable;
        }
        let mut down = false;

        match action {
            Action::Up(_) | Action::Down(_) => {
                down = matches!(action, Action::Down(_));
                let max_y = self.pane_lens[self.cursor[0]].saturating_sub(1);
                if down {
                    self.cursor[1] = (self.cursor[1] + 1).min(max_y);
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

        // Cap cursor-y to pane length and skip inactive items
        let max_y = self.pane_lens[self.cursor[0]].saturating_sub(1);
        self.cursor[1] = self.cursor[1].min(max_y);
        while self.on_inactive_y() {
            if down {
                if self.cursor[1] < max_y {
                    self.cursor[1] += 1;
                } else {
                    self.cursor[1] = self.cursor[1].saturating_sub(1);
                    break;
                }
            } else if self.cursor[1] > 0 {
                self.cursor[1] -= 1;
            } else {
                break;
            }
        }
        OverlayEffect::None
    }

    // ----------------- MAKE WIDGETS -------------------------------

    /// The sort orders the modal lists. Other panes hide atime, which takes
    /// over the mtime row while it is the active sort. SQL-sorted db panes
    /// instead hide mtime — reachable via the sort keys with no row of its
    /// own — and give atime a row above size.
    fn sort_orders(&self) -> Vec<SortOrder> {
        STACK::with_current(|p| {
            let sql_db = matches!(
                p,
                FsPane::Files { .. } | FsPane::Folders { .. } | FsPane::Apps { .. }
            );
            p.sort_options()
                .iter()
                .copied()
                .filter(|so| {
                    !matches!(
                        (sql_db, so),
                        (true, SortOrder::mtime) | (false, SortOrder::atime)
                    )
                })
                .collect()
        })
    }

    // Returns Vec<Span> for sort options
    // Returns items as Vec<(Vec<Span>, bool)> so make_widgets can add checkboxes
    fn get_sort_items(&self) -> Vec<(Vec<Span<'static>>, Option<bool>)> {
        let (current_sort_order, db) = STACK::with_current(|p| {
            // SQL-sorted db panes use name/count/frecency labels; Stash is
            // nucleo-sorted like Nav/fd and uses the plain labels
            let db = matches!(
                p,
                FsPane::Files { .. } | FsPane::Folders { .. } | FsPane::Apps { .. }
            );
            (p.sort_order(), db)
        });
        // the pane's options are the list: non-db panes hide atime (it
        // replaces the mtime row's label while it is the active sort), db
        // panes list atime above size and hide mtime
        self.sort_orders()
            .iter()
            .map(|so| {
                // while atime is active, the mtime row shows 'atime' and
                // stays checked — the two share one time slot; the db
                // atime row is a row of its own
                let label = if *so == SortOrder::mtime && current_sort_order == SortOrder::atime {
                    "atime"
                } else {
                    so.label(db)
                };
                // the time rows highlight their second letter — the 't' in
                // mtime/atime — the cycle key; other rows keep their first
                let spans = bold_indices(
                    label,
                    if *so == SortOrder::mtime || (db && *so == SortOrder::atime) {
                        [1]
                    } else {
                        [0]
                    },
                    self.item_style(),
                );
                let checked = *so == current_sort_order
                    || (*so == SortOrder::mtime && current_sort_order == SortOrder::atime);
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
            if vis.files {
                (
                    bold_indices("files (D)", [7], self.item_style()),
                    Some(vis.files),
                )
            } else {
                (bold_indices("Dirs", [0], self.item_style()), Some(vis.dirs))
            }
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
                one_line,
                fixed_strings,
                ..
            } => {
                // build context info line
                let mut context = vec![];
                let c = format!("[{before}, {after}] ").into();
                context.push(c);
                let mut hint = bold_indices("(B, D)", [1, 4], self.item_style())
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
                    (single, Some(*one_line)),
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
            // the sort pane only lists orders the current pane supports,
            // so no row is inactive there
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
        let mut refilter = false;
        let mut reload = false;

        match x {
            // visibility pane
            0 => {
                refilter = true;
                FILTERS::with_mut(|vis| {
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
                        2 => {
                            if vis.files {
                                vis.files = !vis.files;
                            } else {
                                vis.dirs = !vis.dirs
                            }
                        }
                        3 => vis.toggle_all(),
                        _ => {}
                    }
                });
            }

            // sort pane: the pane is the source of truth for sort
            1 => {
                let orders = self.sort_orders();
                refilter = STACK::with_current_mut(|p| {
                    if let Some(&new_sort_order) = orders.get(y) {
                        if new_sort_order != *p.sort_mut() {
                            *p.sort_mut() = new_sort_order;
                            true
                        } else {
                            false
                        }
                    } else {
                        false
                    }
                });
            }

            2 => {
                reload = true;
                STACK::with_current_mut(|p| match p {
                    FsPane::Search {
                        context,
                        case,
                        one_line,
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
                        5 => *one_line = !(*one_line),
                        6 => *fixed_strings = !(*fixed_strings),
                        _ => {}
                    },

                    _ => {}
                });
            }

            _ => {}
        }
        if refilter {
            GLOBAL::send_action(FsAction::Refilter);
        };
        if reload {
            GLOBAL::send_action(FsAction::Reload);
        }
    }
}

impl Overlay<FsAction, PathItem, ()> for OptionsOverlay {
    fn handle_input(
        &mut self,
        c: char,
        _state: &mut MMState<'_, PathItem, ()>,
    ) -> OverlayEffect {
        let mut refilter = false;
        let mut reload = false;

        match c {
            'q' => return OverlayEffect::Disable,

            // context keys. The rg pane shows the context pane alongside
            // the sort and visibility panes, and 'c' (a sort key on db
            // panes) and 'd'/'D' (dirs/files visibility keys) collide
            // with those panes' keys, so they must be dispatched first.
            'b' | 'B' | 'd' | 'D' | 'c' | 'C' | 'e' | '1' | 'r' if self.pane_lens[2] > 0 => {
                reload = true;

                STACK::with_current_mut(|p| match p {
                    FsPane::Search {
                        context,
                        case,
                        one_line,
                        fixed_strings,
                        ..
                    } => match c {
                        'b' => reload = context[0].ssub(1),
                        'B' => context[0] += 1,
                        'd' => reload = context[1].ssub(1),
                        'D' => context[1] += 1,
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
                        '1' => *one_line = !(*one_line),
                        'r' => *fixed_strings = !(*fixed_strings),

                        _ => reload = false,
                    },
                    _ => {}
                });
            }

            // time-sort cycle key (the highlighted 't'): any other sort ->
            // mtime, mtime -> atime, atime -> none, none -> mtime. Every
            // pane supports the full cycle.
            't' if self.pane_lens[1] > 0 => {
                // the cycle only moves between orders the pane supports,
                // so the fallback to none is defensive
                refilter = STACK::with_current_mut(|p| {
                    let next = match p.sort_order() {
                        SortOrder::mtime => SortOrder::atime,
                        SortOrder::atime => SortOrder::none,
                        _ => SortOrder::mtime,
                    };
                    let target = if p.sort_options().contains(&next) {
                        next
                    } else {
                        SortOrder::none
                    };
                    let changed = target != *p.sort_mut();
                    *p.sort_mut() = target;
                    changed
                });
            }

            // sort toggles: write the pane's sort, dispatch Refilter.
            // db panes (Files/Folders/Apps) label their sorts name/atime/
            // count/frecency and key them n/c/f; other panes key name/size
            // n/s. Toggling an active sort off lands on mtime (insertion
            // order, no row) on SQL db panes, the default frecency
            // elsewhere.
            'n' | 's' | 'c' | 'f' if self.pane_lens[1] > 0 => {
                let (is_db, sql_db) = STACK::with_current(|p| {
                    let sql_db = matches!(
                        p,
                        FsPane::Files { .. } | FsPane::Folders { .. } | FsPane::Apps { .. }
                    );
                    (sql_db || matches!(p, FsPane::Stash { .. }), sql_db)
                });
                let named = match (c, is_db) {
                    ('n', _) => SortOrder::name,
                    ('s', false) => SortOrder::size,
                    ('c', true) => SortOrder::size,
                    ('f', true) => SortOrder::none,
                    _ => return OverlayEffect::None,
                };
                // keys target their named order when the pane supports it,
                // else fall back to none ('s' on an rg pane toggles none)
                let target = STACK::with_current(|p| {
                    if p.sort_options().contains(&named) {
                        named
                    } else {
                        SortOrder::none
                    }
                });
                refilter = STACK::with_current_mut(|p| {
                    let sort = p.sort_mut();
                    let new_sort = if *sort == target {
                        // SQL db panes: the 'other' state is insertion
                        // order (mtime), shown with no row checked
                        if sql_db {
                            SortOrder::mtime
                        } else {
                            SortOrder::none
                        }
                    } else {
                        target
                    };
                    let changed = new_sort != *sort;
                    *sort = new_sort;
                    changed
                });
            }

            // visibility toggles
            'h' | 'H' | 'I' | 'd' | 'D' | 'a' if self.pane_lens[0] > 0 => {
                refilter = FILTERS::with_mut(|vis| {
                    let before = *vis;
                    if !matches!(c, 'D' | 'a') {
                        vis.set_all(false);
                    }
                    match c {
                        // 'a' is the highlighted key of the 'all' row
                        'a' => vis.toggle_all(),
                        'h' => (vis.hidden, vis.hidden_only) = (!vis.hidden, false),
                        'H' => {
                            if !vis.dirs {
                                vis.files = true;
                            }
                            (vis.hidden, vis.hidden_only) = (false, !vis.hidden_only)
                        }
                        'd' | 'D' => {
                            if !STACK::in_rg() {
                                if vis.files {
                                    vis.files = !vis.files
                                } else {
                                    vis.dirs = !vis.dirs
                                }
                            }
                        }
                        'I' => vis.ignore = !vis.ignore,
                        _ => {}
                    }
                    *vis != before
                });
            }

            // any other key is a no-op: refilter only fires when a key
            // actually changed visibility or sort
            _ => {}
        }

        if refilter {
            GLOBAL::send_action(FsAction::Refilter);
        };
        if reload {
            GLOBAL::send_action(FsAction::Reload);
        }

        OverlayEffect::None
    }

    fn on_enable(
        &mut self,
        _area: &Rect,
        _state: &mut MMState<'_, PathItem, ()>,
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
        action: &Action<FsAction>,
        _state: &mut MMState<'_, PathItem, ()>,
    ) -> OverlayEffect {
        self.handle_action_nav(action)
    }

    fn area(
        &mut self,
        ui_area: &Rect,
        layout: &OverlayLayoutSettings,
    ) {
        self.area =
            utils::default_area([self.width().into(), self.height().into()], layout, ui_area);
        log::trace!("Computed filters overlay dimensions: {:?}", self.area);
    }

    fn draw(
        &mut self,
        frame: &mut matchmaker::ui::Frame,
    ) {
        let area = self.area;
        frame.render_widget(Clear, area);

        let widgets = self.make_widgets();
        if widgets.is_empty() {
            return;
        }

        // make layout of constant width panes
        let constraints: Vec<Constraint> = (0..widgets.len())
            .map(|_| Constraint::Length(PANE_WIDTH))
            .collect();

        let mut inner_area = self.border().as_block().inner(area);

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_options_overlay_cursor_clamping() {
        let mut overlay = OptionsOverlay::default();
        overlay.pane_lens = [4, 3, 0];
        overlay.cursor = [0, 0];

        // Down multiple times past 4 items (indices 0..3)
        for _ in 0..10 {
            overlay.handle_action_nav(&Action::Down(1));
        }
        assert_eq!(overlay.cursor[1], 3);

        // Switch right to pane 1 (length 3, indices 0..2)
        overlay.handle_action_nav(&Action::ForwardChar);
        assert_eq!(overlay.cursor[0], 1);
        assert_eq!(overlay.cursor[1], 2);
    }
}
