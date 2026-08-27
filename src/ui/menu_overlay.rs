use std::collections::HashMap;

use cba::define_collection_wrapper;

use crate::run::state::GLOBAL::db;
use crate::{
    abspath::AbsPath,
    menu::{MenuActions, MenuEvaluationContext, MenuStrategy},
    run::{
        FsPane,
        action::FsAction,
        reload::fs_reload,
        item::{PathItem, short_display},
        queue::QUEUE,
        state::{
            GLOBAL, MenuCommandPaths, MenuPrompt, STACK, STORE, TOAST, ToastStyle, lessfilter_cfg,
        },
    },
    spawn::open_wrapped,
    ui::{
        OVERLAY_TICK_RATE,
        prompt_overlay::{PromptConfig, PromptOverlay},
    },
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

const MAX_ITEM_WIDTH: u16 = 9;

/// Column headers of the menu's two-column results table.
const MENU_COLUMNS: [&str; 2] = ["name", "alias"];

/// Width scaling points for the menu: at 40 columns the menu takes 50% of
/// the terminal width, kept constant past the last point.
const MENU_WIDTH_POINTS: &[(u16, Percentage)] = &[(40, Percentage::new(50))];

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct MenuConfig {
    #[serde(with = "border_result")]
    pub border: Result<BorderSetting, PartialBorderSetting>,
    /// When set, the builtin items get no aliases (custom actions keep their
    /// configured aliases).
    pub no_default_aliases: bool,
    pub results: ResultsConfig,
}

impl Default for MenuConfig {
    fn default() -> Self {
        let border = PartialBorderSetting {
            title: Some("Menu".into()),
            sides: Some(Borders::ALL),
            padding: Some(Padding::new(2, 2, 0, 1).into()),
            ..Default::default()
        };
        // column names are wider than the menu content; let the table engine
        // size columns from the item text instead
        let results = ResultsConfig {
            min_width_from_cols: false,
            ..Default::default()
        };
        Self {
            border: Err(border),
            no_default_aliases: false,
            results,
        }
    }
}

pub use super::menu_overlay_impl::*;

define_collection_wrapper!(
    /// Alias -> index into the menu items; a query matching an alias exactly
    /// accepts that item.
    #[derive(Debug, Clone)]
    AliasSet: HashMap<String, usize>
);

/// A menu item shown in the menu overlay.
#[derive(Clone)]
pub enum MenuItem {
    New,
    Rename,
    Move,
    Copy,
    Symlink,
    Goto,
    Trash,
    Delete,
    Open,
    OpenWith,
    Custom {
        name: String,
        action: String,
        alias: Option<String>,
    },
}

impl MenuItem {
    /// Column 0: the item name.
    pub fn label(&self) -> &str {
        match self {
            MenuItem::New => "new",
            MenuItem::Rename => "rename",
            MenuItem::Move => "move",
            MenuItem::Copy => "copy",
            MenuItem::Symlink => "symlink",
            MenuItem::Trash => "trash",
            MenuItem::Goto => "goto",
            MenuItem::Delete => "delete",
            MenuItem::Open => "open",
            MenuItem::OpenWith => "open with",
            MenuItem::Custom { name, .. } => name,
        }
    }

    /// Column 1: the alias that triggers the item when typed exactly.
    pub fn alias(&self) -> Option<&str> {
        match self {
            MenuItem::New => Some("N"),
            MenuItem::Rename => Some("R"),
            MenuItem::Move => Some("M"),
            MenuItem::Copy => Some("C"),
            MenuItem::Trash => Some("T"),
            MenuItem::Delete => Some("D"),
            MenuItem::Open => Some("O"),
            MenuItem::OpenWith => Some("W"),
            MenuItem::Symlink => None,
            MenuItem::Goto => None,
            MenuItem::Custom { alias, .. } => alias.as_deref(),
        }
    }

    /// Execute the item on `path`.
    /// Returns a [`MenuPrompt`] to open the input bar, or whether to keep the
    /// menu open.
    pub fn action(
        &self,
        path: AbsPath,
    ) -> Result<MenuPrompt, bool> {
        match self {
            MenuItem::New => Ok(MenuPrompt::new(PromptKind::New)),
            MenuItem::Rename => Ok(rename_prompt_for(&path)),
            MenuItem::Move => {
                TOAST::push(ToastStyle::Normal, "Move: ", [short_display(&path)]);
                QUEUE::enqueue("move".into(), vec![path]);
                Err(false)
            }
            MenuItem::Copy => {
                TOAST::push(ToastStyle::Normal, "Copied: ", [short_display(&path)]);
                QUEUE::enqueue("copy".into(), vec![path]);
                Err(false)
            }
            MenuItem::Symlink => {
                TOAST::push(
                    ToastStyle::Normal,
                    "Queued symlink: ",
                    [short_display(&path)],
                );
                QUEUE::enqueue("symlink".into(), vec![path]);
                Err(false)
            }
            MenuItem::Goto => Ok(MenuPrompt::new(PromptKind::Goto)),
            MenuItem::Trash => {
                match trash::delete(&path) {
                    Ok(()) => TOAST::push(ToastStyle::Success, "Trashed: ", [short_display(&path)]),
                    Err(e) => {
                        log::error!("Failed to trash {}: {e}", path.to_string_lossy());
                        TOAST::push(
                            ToastStyle::Error,
                            "Failed to trash: ",
                            [short_display(&path)],
                        )
                    }
                }
                Err(false)
            }
            MenuItem::Delete => {
                GLOBAL::send_action(FsAction::Delete(false));
                Err(false)
            }
            MenuItem::Open => {
                TOAST::push(ToastStyle::Normal, "Opened: ", [short_display(&path)]);
                let path_clone = path;
                tokio::spawn(async move {
                    let conn = db().get_conn(crate::db::DbTable::dirs).await?;
                    open_wrapped(conn, None, &[path_clone.inner().into()], true).await?;
                    anyhow::Ok(())
                });
                Err(false)
            }
            MenuItem::OpenWith => {
                unreachable!("OpenWith is handled in MenuOverlay::execute")
            }
            MenuItem::Custom { action, .. } => {
                unreachable!("custom items are routed through MenuOverlay::run_custom")
            }
        }
    }
}

/// A menu item with its alias resolved at build time: builtins carry their
/// default alias unless [`MenuConfig::no_default_aliases`] is set, custom
/// actions carry their configured alias.
#[derive(Clone)]
pub struct MenuEntry {
    pub item: MenuItem,
    pub alias: Option<String>,
}

impl MenuEntry {
    /// Column 0: the item name.
    pub fn label(&self) -> &str {
        self.item.label()
    }

    /// Column 0 display: the label with the alias hotkey letter capitalized
    /// and bold; when the letter does not occur in the label, it is
    /// appended as ` (X)`.
    pub fn hotkey_label(&self) -> Line<'static> {
        let label = self.label();
        let Some(letter) = self.alias.as_deref().and_then(|a| a.chars().next()) else {
            return Line::from(Span::raw(label.to_string()));
        };
        // default (builtin) actions get a bold white hotkey letter; custom
        // menu actions keep the italic-bold hotkey letter
        let style = match &self.item {
            MenuItem::Custom { .. } => {
                Style::default().add_modifier(Modifier::ITALIC | Modifier::BOLD)
            }
            _ => Style::default()
                .fg(Color::White)
                .add_modifier(Modifier::BOLD),
        };

        match label
            .char_indices()
            .find(|(_, c)| c.eq_ignore_ascii_case(&letter))
        {
            Some((byte_idx, c)) => {
                let spans = vec![
                    Span::raw(label[..byte_idx].to_string()),
                    Span::styled(c.to_uppercase().collect::<String>(), style),
                    Span::raw(label[byte_idx + c.len_utf8()..].to_string()),
                ];
                Line::from(spans)
            }
            None => {
                let spans = vec![
                    Span::raw(format!("{label} ")),
                    Span::styled(format!("({})", letter.to_uppercase()), style),
                ];
                Line::from(spans)
            }
        }
    }
}

impl ColumnIndexable for MenuEntry {
    fn get_str(
        &self,
        i: usize,
    ) -> std::borrow::Cow<'_, str> {
        match i {
            0 => self.label().into(),
            _ => String::new().into(),
        }
    }

    fn get_text(
        &self,
        i: usize,
    ) -> Text<'_> {
        match i {
            0 => Text::from(self.hotkey_label()),
            _ => Text::default(),
        }
    }
}

pub const MENU_ITEMS: [MenuItem; 10] = [
    MenuItem::New,
    MenuItem::Rename,
    MenuItem::Move,
    MenuItem::Copy,
    MenuItem::Trash,
    MenuItem::Delete,
    MenuItem::Open,
    MenuItem::OpenWith,
    MenuItem::Symlink,
    MenuItem::Goto,
];

/// The menu overlay: a two-column picker of the available actions
/// (name, alias), with a query line, backed by its own nucleo worker.
pub struct MenuOverlay {
    pub config: MenuConfig,
    query: QueryUI,
    results: ResultsUI,
    /// Built on enable, dropped on disable so the matcher thread only lives
    /// while the overlay is active.
    worker: Option<Worker<MenuEntry, ()>>,
    /// Items injected into the worker on each enable, with their aliases
    /// resolved.
    menu_items: Vec<MenuEntry>,
    /// Alias -> index into `menu_items`; a query matching an alias exactly
    /// accepts that item.
    aliases: AliasSet,
    area: Rect,
    /// Set whenever the query text changed; drives `worker.find` on the next draw.
    query_dirty: bool,

    pub prompt_kind: Option<PromptKind>,
    pub prompt: PromptOverlay,
    /// The custom actions; only those whose conditions pass are listed.
    pub actions: MenuActions,

    // required to update table
    selector: Selector,
}

impl MenuOverlay {
    pub fn new(
        config: MenuConfig,
        prompt_config: PromptConfig,
        actions: MenuActions,
    ) -> Self {
        // The query bar is not configurable: a default QueryUI with no prompt.
        let query = QueryUI::new(QueryConfig {
            prompt: String::new(),
            ..Default::default()
        });
        // The results indentation is always empty and rows always span the
        // full width, silently overriding the user settings.
        let mut results_config = config.results.clone();
        results_config.multi_prefix = String::new();
        results_config.row_connection = RowConnectionStyle::Full;
        let results = ResultsUI::new(results_config);
        Self {
            config,
            query,
            results,
            worker: None,
            menu_items: vec![],
            aliases: AliasSet::new(),
            area: Rect::default(),
            query_dirty: true,
            prompt_kind: None,
            prompt: PromptOverlay::new(prompt_config),
            actions,
            selector: Selector::new(),
        }
    }

    pub fn border(&self) -> &BorderSetting {
        self.config.border.as_ref().unwrap()
    }

    /// The nucleo index and the item currently under the cursor, if any.
    pub fn current_item(&self) -> Option<(u32, &MenuEntry)> {
        self.worker.as_ref()?.get_nth_indexed(self.results.index())
    }

    fn build_worker(&mut self) {
        debug_assert!(self.worker.is_none());
        let mut worker = Worker::new_indexable(MENU_COLUMNS, None);
        // coarse stability buckets (like the db panes): equal scores keep the
        // item order, so the menu order survives an empty query
        worker.set_stability(5);
        let items = self.menu_items.clone();
        let _ = worker.injector().extend(items.into_iter());
        self.results.init(&mut worker);
        self.worker = Some(worker);
    }

    fn set_prompt(
        &mut self,
        prompt: MenuPrompt,
        state: &mut MMState<'_, PathItem, ()>,
    ) {
        self.prompt_kind = Some(prompt.kind);
        if !prompt.title.is_empty() {
            self.prompt.input.config.border.title = prompt.title;
        }
        self.prompt.on_enable(&Rect::default(), state);

        if !prompt.initial.is_empty() {
            self.prompt.input.set_value(prompt.initial);
            self.prompt
                .input
                .inner
                .set(Option::<String>::None, prompt.cursor as u16);
        }
    }

    pub fn accept(
        &mut self,
        state: &mut MMState<'_, PathItem, ()>,
    ) -> OverlayEffect {
        let Some(worker) = self.worker.as_ref() else {
            return OverlayEffect::Disable;
        };
        let Some((_, entry)) = worker.get_nth_indexed(self.results.index()) else {
            return OverlayEffect::Disable;
        };
        self.execute(entry.item.clone(), state)
    }

    /// Run `item` on the current target, opening a prompt when the item
    /// returns one.
    fn execute(
        &mut self,
        item: MenuItem,
        state: &mut MMState<'_, PathItem, ()>,
    ) -> OverlayEffect {
        let custom_key = match &item {
            MenuItem::Custom { action, .. } => Some(action.clone()),
            _ => None,
        };
        if let Some(key) = custom_key {
            return self.run_custom(&key, state);
        }

        // OpenWith creates the app pane right there: the selected items
        // (or the cursor item when nothing is selected) preload it, then
        // the pane reloads — the same bookkeeping as FsAction::App, but
        // done inline since the selection is only known here.
        if matches!(item, MenuItem::OpenWith) {
            let selected: Vec<AbsPath> = state.map_selections_to_vec(|_, item| item.path.clone());
            let files = if selected.is_empty() {
                vec![self.target_path(state)]
            } else {
                selected
            };

            TOAST::push(
                ToastStyle::Normal,
                "Opening: ",
                files.iter().map(|p| short_display(p)),
            );

            // save input, then switch to the app pane and reload
            let (content, index) = state.get_content_and_index();
            STACK::save_input(content, index);
            STACK::push(FsPane::new_apps(files));
            fs_reload(state, true, false);
            return OverlayEffect::Disable;
        }

        let path = self.target_path(state);
        match item.action(path) {
            Ok(prompt) => {
                self.set_prompt(prompt, state);
                OverlayEffect::None
            }
            Err(true) => OverlayEffect::None,
            Err(false) => OverlayEffect::Disable,
        }
    }

    /// Rebuild the item list: the builtin items plus every custom action
    /// whose conditions pass against the picker state at open. [`FileData`]
    /// is computed once per file and reused across all condition evaluations.
    fn build_items(
        &mut self,
        state: &mut MMState<'_, PathItem, ()>,
    ) {
        let lcfg = lessfilter_cfg();
        let mut ctx = MenuEvaluationContext::new(state, lcfg);

        // builtins only act on filesystem items, so they are hidden in the
        // app pane, which lists custom actions instead
        let mut items = if STACK::in_app() {
            Vec::new()
        } else {
            MENU_ITEMS.to_vec()
        };
        for (key, action) in self.actions.iter() {
            if ctx.is_applicable(action) {
                items.push(MenuItem::Custom {
                    name: key.clone(),
                    action: key.clone(),
                    alias: action.alias.clone(),
                });
            }
        }
        // resolve the aliases once: builtins keep their default alias unless
        // default aliases are disabled, custom actions keep their configured
        // alias
        self.menu_items = items
            .into_iter()
            .map(|item| {
                let alias =
                    if self.config.no_default_aliases && !matches!(item, MenuItem::Custom { .. }) {
                        None
                    } else {
                        item.alias().map(str::to_string)
                    };
                MenuEntry { item, alias }
            })
            .collect();
        self.aliases = self
            .menu_items
            .iter()
            .enumerate()
            .filter_map(|(i, entry)| entry.alias.clone().map(|alias| (alias, i)))
            .collect();
    }

    /// Run a custom action on the target items (the current selection, or
    /// the target item when nothing is selected).
    fn run_custom(
        &mut self,
        key: &str,
        state: &mut MMState<'_, PathItem, ()>,
    ) -> OverlayEffect {
        let Some(action) = self.actions.get(key) else {
            log::error!("Menu action not found: {key}");
            return OverlayEffect::None;
        };
        let lcfg = lessfilter_cfg();
        let ctx = MenuEvaluationContext::new(state, lcfg);
        let targets = ctx.resolve_targets(action);
        let displays: Vec<Span<'static>> = targets.iter().map(|p| short_display(p)).collect();

        match action.strategy {
            MenuStrategy::Queue => {
                QUEUE::enqueue(key.to_string(), targets);
                TOAST::push(ToastStyle::Normal, "Queued: ", displays);
            }
            MenuStrategy::QueueBatch(n) => {
                // a zero batch size parses but chunks of size 0 are invalid
                let n = n.max(1);
                for chunk in targets.chunks(n) {
                    QUEUE::enqueue(key.to_string(), chunk.to_vec());
                }
                TOAST::push(ToastStyle::Normal, "Queued: ", displays);
            }
            MenuStrategy::Execute => {
                STORE::set(MenuCommandPaths::new_from(targets));
                GLOBAL::send_action(FsAction::MenuAction(key.to_string()));
            }
            MenuStrategy::ExecuteSilent => {
                STORE::set(MenuCommandPaths::new_from(targets));
                GLOBAL::send_action(FsAction::MenuActionSilent(action.command.clone()));
            }
            MenuStrategy::ExecPaged => {
                STORE::set(MenuCommandPaths::new_from(targets));
                GLOBAL::send_action(FsAction::MenuActionExecPaged(key.to_string()));
            }
        }

        if action.closes() {
            OverlayEffect::Disable
        } else {
            OverlayEffect::None
        }
    }
}

impl Overlay<FsAction, PathItem, ()> for MenuOverlay {
    fn on_enable(
        &mut self,
        _area: &Rect,
        state: &mut MMState<'_, PathItem, ()>,
    ) {
        // animate the menu: force the ticker to run while it is open
        GLOBAL::send_bind(BindDirective::OverrideTickrate(Some(OVERLAY_TICK_RATE)));

        self.prompt_kind = None;

        if let Some(prompt) = STORE::take::<MenuPrompt>() {
            self.set_prompt(prompt, state);
        }

        // list the custom actions whose conditions pass (evaluated once,
        // against the state at open)
        self.build_items(state);
        self.build_worker();
        self.query_dirty = true;
        self.results.set_dirty();
    }

    fn on_disable(&mut self) {
        GLOBAL::send_bind(BindDirective::OverrideTickrate(None));
        self.prompt.on_disable();
        // don't carry the search text into the next open
        self.query.clear();
        // Stops the matcher thread; the worker is rebuilt on the next enable.
        self.worker = None;
    }

    fn handle_input(
        &mut self,
        c: char,
        state: &mut MMState<'_, PathItem, ()>,
    ) -> OverlayEffect {
        if let Some(p) = self.prompt_kind {
            if let OverlayEffect::Disable = self.prompt.handle_input(c, state) {
                self.on_prompt_accept(p, state)
            } else {
                OverlayEffect::None
            }
        } else {
            self.query.push_char(c);
            self.query_dirty = true;
            self.results.set_dirty();
            // auto-accept: a query that exactly matches an alias triggers
            // that item
            if let Some(pos) = self.aliases.get(&self.query.input()) {
                let item = self.menu_items[*pos].item.clone();
                return self.execute(item, state);
            }
            OverlayEffect::None
        }
    }

    fn handle_action(
        &mut self,
        action: &Action<FsAction>,
        state: &mut MMState<'_, PathItem, ()>,
    ) -> OverlayEffect {
        if let Some(p) = self.prompt_kind {
            // defer to prompt
            match self.prompt.handle_action_(action) {
                None => {}
                Some(false) => self.prompt_kind = None,
                Some(true) => return self.on_prompt_accept(p, state),
            }
            return OverlayEffect::None;
        }

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
            Action::Quit(_) => return OverlayEffect::Disable,
            // the menu toggles closed on its own key; any other bound action
            // is ignored
            Action::Custom(fa) if *fa == FsAction::ShowMenu => return OverlayEffect::Disable,

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
        self.prompt.area(ui_area, layout);

        let max_item_width = self
            .menu_items
            .iter()
            .map(|entry| Text::from(entry.hotkey_label()).width() as u16)
            .max()
            .unwrap_or(MAX_ITEM_WIDTH)
            .max(MAX_ITEM_WIDTH);
        // the table additionally reserves the prefix indentation and the
        // inter-column spacing
        let content_width = self.results.indentation() as u16
            + max_item_width
            + self.config.results.column_spacing.0;

        self.area = utils::default_area(
            [
                SizeHint {
                    adaptive_percentage: MENU_WIDTH_POINTS,
                    min: 18,
                    max: content_width + self.border().width(),
                },
                SizeHint {
                    adaptive_percentage: &[],
                    // 8 item rows, the query line, a blank spacer and the border;
                    // with no actions the overlay shrinks to the message and border
                    min: if self.menu_items.is_empty() {
                        2 + self.border().height()
                    } else {
                        9 + self.query.height() + self.border().height()
                    },
                    max: 0,
                },
            ],
            layout,
            ui_area,
        );

        let inner = self.border().inner_of(self.area);

        // compact empty state: nothing to lay out, the message fills the box
        if self.menu_items.is_empty() {
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
        if self.prompt_kind.is_some() {
            self.prompt.draw(frame);
            return;
        }

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
        // menu rows are never selected: a fresh empty selector per draw
        self.results
            .update_table(worker, &self.selector, &mut matchmaker::matcher::matcher());

        frame.render_widget(Clear, self.area);
        frame.render_widget(self.border().as_block(), self.area);

        let inner = self.border().inner_of(self.area);

        // no actions available (e.g. the app pane lists custom actions
        // only, and none matched): a compact box with the message only
        if self.menu_items.is_empty() {
            frame.render_widget(
                Paragraph::new(Text::from("No available\nactions")).alignment(Alignment::Center),
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
        let (table, width) = self.results.get_table();
        let mut results_area = results;
        if matches!(
            self.results.config.row_connection,
            RowConnectionStyle::Capped
        ) {
            results_area.width = results_area.width.min(width);
        }
        frame.render_widget(table, results_area);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{db::Pool, run::FsPane, watcher::WatcherMessage};
    use fist_types::filters::{SortOrder, Visibility};
    use matchmaker::{
        config::RenderConfig,
        message::Event,
        nucleo::Column,
        render::State,
        ui::{DisplayUI, UI},
    };
    use matchmaker_partial::Apply;
    use ratatui::{Terminal, backend::TestBackend};

    const UI_AREA: Rect = Rect {
        x: 0,
        y: 0,
        width: 80,
        height: 24,
    };

    fn init_globals() -> tokio::sync::mpsc::UnboundedReceiver<BindDirective<FsAction>> {
        let (bind_tx, bind_rx) = tokio::sync::mpsc::unbounded_channel();
        let (render_tx, _render_rx) =
            tokio::sync::mpsc::unbounded_channel::<matchmaker::message::RenderCommand<FsAction>>();
        let (watcher_tx, _watcher_rx) = tokio::sync::mpsc::unbounded_channel::<WatcherMessage>();

        let pool = Pool {
            pool: sqlx::SqlitePool::connect_lazy("sqlite::memory:").unwrap(),
            lambda: None,
        };
        let pane = FsPane::Nav {
            cwd: AbsPath::new("/tmp"),
            sort: SortOrder::default(),
            vis: Visibility::default(),
            input: (String::new(), 0),
            complete: std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false)),
            depth: 0,
        };

        GLOBAL::init(
            crate::config::GlobalConfig::default(),
            render_tx,
            watcher_tx,
            pool,
            pane,
            bind_tx,
        );
        bind_rx
    }

    fn full_config() -> MenuConfig {
        // production applies the partial border onto the overlay border in
        // get_mm_cfg; mirror that so the default renders fully
        let mut config = MenuConfig::default();
        if let Err(partial) = config.border {
            let mut full = matchmaker::config::OverlayConfig::default().border;
            full.apply(partial);
            config.border = Ok(full);
        }
        config
    }

    fn offline_mm_state() -> (
        matchmaker::ui::UI,
        matchmaker::ui::PickerUI<PathItem, ()>,
        matchmaker::ui::DisplayUI,
        Option<matchmaker::ui::PreviewUI>,
        matchmaker::render::State,
        tokio::sync::mpsc::UnboundedSender<Event>,
    ) {
        let worker = Worker::new_with_preprocessors(
            [Column::new("path", |item: &PathItem, _: &()| {
                item.path.to_string_lossy().to_string().into()
            })
            .with_raw(|item: &PathItem, _: &()| item.path.to_string_lossy().to_string().into())],
            0,
            std::sync::Arc::new(|_: &PathItem| Some(())),
            std::sync::Arc::new(|_: &PathItem| ()),
        );
        let (ui, picker) = UI::new_offline(RenderConfig::default(), worker);
        let (tx, _rx) = tokio::sync::mpsc::unbounded_channel::<Event>();
        (ui, picker, DisplayUI::default(), None, State::new(), tx)
    }

    fn render(overlay: &mut MenuOverlay) -> String {
        let mut terminal = Terminal::new(TestBackend::new(80, 24)).unwrap();
        terminal.draw(|frame| overlay.draw(frame)).unwrap();
        let mut text = String::new();
        for y in 0..24 {
            for x in 0..80 {
                text.push_str(terminal.backend().buffer()[(x, y)].symbol());
            }
            text.push('\n');
        }
        text
    }

    #[tokio::test]
    async fn menu_lifecycle() {
        let mut bind_rx = init_globals();
        let mut overlay = MenuOverlay::new(
            full_config(),
            crate::ui::prompt_overlay::PromptConfig::default(),
            crate::menu::MenuActions::default(),
        );
        let (mut ui, mut picker, mut footer, mut preview, mut state, tx) = offline_mm_state();
        let mut mm_state = state.dispatcher(&mut ui, &mut picker, &mut footer, &mut preview, &tx);

        {
            let o = &mut overlay as &mut dyn Overlay<FsAction, PathItem, ()>;
            o.on_enable(&UI_AREA, &mut mm_state);
            o.area(
                &UI_AREA,
                &matchmaker::config::OverlayConfig::default().layout,
            );
        }

        if let Ok(dir) = bind_rx.try_recv() {
            assert!(matches!(
                dir,
                BindDirective::OverrideTickrate(Some(OVERLAY_TICK_RATE))
            ));
        }

        // the first draw only gathers column widths; subsequent draws render rows
        let _ = render(&mut overlay);
        let text = render(&mut overlay);
        assert!(
            text.contains("New"),
            "hotkey letter capitalized and bold: \n{text}"
        );
        assert!(
            text.contains("open With"),
            "hotkey letter inside the label: \n{text}"
        );
        assert!(
            text.contains("Move"),
            "the renamed action renders with its hotkey letter: \n{text}"
        );
        assert!(
            !text.contains("> "),
            "the menu query bar has no prompt: \n{text}"
        );

        // a bound FsAction is dropped while the menu is open
        let effect = {
            let o = &mut overlay as &mut dyn Overlay<FsAction, PathItem, ()>;
            o.handle_action(&Action::Custom(FsAction::Move), &mut mm_state)
        };
        assert!(matches!(effect, OverlayEffect::None));

        // typing an alias exactly accepts the matching item, while any other
        // input only filters
        let (alias_effect, filter_effect) = {
            let o = &mut overlay as &mut dyn Overlay<FsAction, PathItem, ()>;
            let alias = o.handle_input('M', &mut mm_state);
            let filter = o.handle_input('z', &mut mm_state);
            (alias, filter)
        };
        assert!(matches!(alias_effect, OverlayEffect::Disable));
        assert!(matches!(filter_effect, OverlayEffect::None));
        assert!(
            !overlay.query.input().is_empty(),
            "the filter text survives while the menu is open"
        );

        {
            let o = &mut overlay as &mut dyn Overlay<FsAction, PathItem, ()>;
            o.on_disable();
        }
        assert!(
            overlay.query.input().is_empty(),
            "closing the menu clears the search input"
        );
        assert!(matches!(
            bind_rx.try_recv().unwrap(),
            BindDirective::OverrideTickrate(None)
        ));
        assert!(bind_rx.try_recv().is_err(), "no further bind directives");

        // with default aliases disabled, builtins render no alias column and
        // typing an alias does not accept
        let mut config = full_config();
        config.no_default_aliases = true;
        let mut overlay = MenuOverlay::new(
            config,
            crate::ui::prompt_overlay::PromptConfig::default(),
            crate::menu::MenuActions::default(),
        );
        {
            let o = &mut overlay as &mut dyn Overlay<FsAction, PathItem, ()>;
            o.on_enable(&UI_AREA, &mut mm_state);
            o.area(
                &UI_AREA,
                &matchmaker::config::OverlayConfig::default().layout,
            );
        }
        let _ = render(&mut overlay);
        let text = render(&mut overlay);
        assert!(text.contains("open with"), "items still listed:\n{text}");
        assert!(!text.contains('W'), "no builtin aliases rendered:\n{text}");
        let effect = {
            let o = &mut overlay as &mut dyn Overlay<FsAction, PathItem, ()>;
            o.handle_input('W', &mut mm_state)
        };
        assert!(matches!(effect, OverlayEffect::None));
    }

    #[test]
    fn test_current_dir_suffix() {
        let cwd = std::env::current_dir().unwrap();
        let cwd_abs = AbsPath::new_unchecked(cwd.clone());
        assert_eq!(
            super::current_dir_suffix(&cwd_abs),
            Some(std::path::PathBuf::from(""))
        );

        if let Some(parent) = cwd.parent() {
            let parent_abs = AbsPath::new_unchecked(parent.to_path_buf());
            let expected_rel = cwd.strip_prefix(parent).unwrap();
            assert_eq!(
                super::current_dir_suffix(&parent_abs),
                Some(expected_rel.to_path_buf())
            );
        }

        let non_existent = AbsPath::new_unchecked(cwd.join("__non_existent_subpath_xyz__"));
        assert_eq!(super::current_dir_suffix(&non_existent), None);
    }
}
