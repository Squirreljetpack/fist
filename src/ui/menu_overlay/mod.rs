mod execute;
mod items;

pub(crate) use execute::capture_prompt_target;
pub use execute::rename_prompt_for;

use std::sync::Arc;

use execute::ActivePrompt;

use matchmaker::{
    Selector,
    action::Action,
    config::{
        CursorSetting, FrameSetting, OverlayLayoutSettings, PartialFrameSetting, Percentage,
        QueryConfig, ResultsConfig, RowConnectionStyle,
    },
    message::BindDirective,
    nucleo::Worker,
    render::MMState,
    ui::{
        Constraint, Direction, Frame, Layout, Overlay, OverlayEffect, QueryUI, Rect, ResultsUI,
        SizeHint, utils,
    },
};
use ratatui::widgets::{Borders, Clear, Padding};

use crate::{
    menu::{MenuActions, MenuSnapshot},
    run::{
        action::FsAction,
        item::PathItem,
        state::{GLOBAL, MenuPrompt, PromptKind, STORE},
    },
    ui::{
        OVERLAY_TICK_RATE,
        prompt_overlay::{PromptConfig, PromptOverlay},
    },
    utils::serde::frame_result,
};

use items::{MenuEntry, MenuItem, compute_max_width, stream_menu_items};

/// Column headers of the menu's two-column results table.
const MENU_COLUMNS: [&str; 2] = ["name", "alias"];

/// Width scaling points for the menu: at 40 columns the menu takes 50% of
/// the terminal width, kept constant past the last point.
const MENU_WIDTH_POINTS: &[(u16, Percentage)] = &[(40, Percentage::new(50))];

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct MenuConfig {
    #[serde(with = "frame_result")]
    pub frame: Result<FrameSetting, PartialFrameSetting>,
    /// When set, the builtin items get no aliases (custom actions keep their
    /// configured aliases).
    pub no_default_aliases: bool,
    /// When set, removes Move, Copy, Open, and Trash from the menu.
    pub compact: bool,
    /// Minimum menu width.
    pub min_width: u16,
    /// Maximum menu content width (`0` = compute automatically from all
    /// configured menu items at initialization).
    pub max_width: u16,
    /// Minimum menu height.
    pub min_height: u16,
    pub results: ResultsConfig,
}

impl Default for MenuConfig {
    fn default() -> Self {
        let frame = PartialFrameSetting {
            title: Some("Menu".into()),
            sides: Some(Borders::ALL),
            padding: Some(Padding::new(2, 2, 0, 1).into()),
            ..Default::default()
        };
        // column names are wider than the menu content; let the table engine
        // size columns from the item text instead, and hold previous rows on
        // screen while a CursorChange reload streams updated items
        let results = ResultsConfig {
            min_width_from_cols: false,
            delay_clear: true,
            ..Default::default()
        };
        Self {
            frame: Err(frame),
            no_default_aliases: false,
            compact: false,
            min_width: 16,
            max_width: 0,
            min_height: 12,
            results,
        }
    }
}

/// The menu overlay: a two-column picker of the available actions
/// (name, alias), with a query line, backed by its own nucleo worker.
pub struct MenuOverlay {
    pub config: MenuConfig,
    query: QueryUI,
    results: ResultsUI,
    /// Created on enable and dropped on disable so the matcher state only
    /// lives while the overlay is active.
    worker: Option<Worker<MenuEntry, ()>>,
    area: Rect,
    /// Set whenever the query text changed; drives `worker.find` on the next draw.
    query_dirty: bool,

    /// Prompt state and paths snapshotted when the prompt opens so cursor
    /// movement while typing does not redirect Rename / Extract / New / SetAlias.
    active_prompt: Option<ActivePrompt>,
    pub prompt: PromptOverlay,
    /// The custom actions; only those whose conditions pass are listed.
    pub actions: Arc<MenuActions>,

    // required to update table
    selector: Selector,

    /// Set when `ReloadMenu` arrives while a prompt is open so we refresh
    /// once the prompt is cancelled.
    reload_pending: bool,
}

impl MenuOverlay {
    pub fn new(
        mut config: MenuConfig,
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
        results_config.default_prefix = String::new();
        results_config.row_connection = RowConnectionStyle::Full;
        let results = ResultsUI::new(results_config);
        if config.max_width == 0 {
            config.max_width = compute_max_width(
                &actions,
                config.compact,
                config.no_default_aliases,
                config.results.column_spacing.0,
            );
        }
        Self {
            config,
            query,
            results,
            worker: None,
            area: Rect::default(),
            query_dirty: true,
            active_prompt: None,
            prompt: PromptOverlay::new(prompt_config),
            actions: Arc::new(actions),
            selector: Selector::new(),
            reload_pending: false,
        }
    }

    pub fn frame(&self) -> &FrameSetting {
        self.config.frame.as_ref().unwrap()
    }

    fn make_worker() -> Worker<MenuEntry, ()> {
        let mut worker = Worker::new_indexable(MENU_COLUMNS, None);
        // coarse stability buckets (like the db panes): equal scores keep the
        // item order, so the menu order survives an empty query
        worker.set_stability(5);
        worker
    }

    /// Snapshot the current picker/stack state and stream passing menu items
    /// into the worker on the blocking pool.
    fn start_populate(
        &mut self,
        state: &MMState<'_, PathItem, ()>,
        restart: bool,
    ) {
        let Some(worker) = self.worker.as_mut() else {
            return;
        };
        self.reload_pending = false;
        if restart {
            worker.restart(true);
            self.results.invalidate_widths();
            self.results.set_dirty();
        }
        let injector = worker.injector();

        let snapshot = MenuSnapshot::capture(state);
        let actions = Arc::clone(&self.actions);
        let compact = self.config.compact;
        let no_default_aliases = self.config.no_default_aliases;

        tokio::task::spawn_blocking(move || {
            stream_menu_items(snapshot, &actions, compact, no_default_aliases, injector);
        });
    }

    /// Split `inner` into `(input, results)` rows, reserving a 1-row gap.
    fn inner_chunks(
        &self,
        inner: Rect,
    ) -> [Rect; 2] {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(self.query.height()),
                Constraint::Length(1),
                Constraint::Fill(1),
            ])
            .split(inner);
        [chunks[0], chunks[2]]
    }

    /// Guidance for the custom actions currently listed in the menu, in menu
    /// order, with their non-empty `help` field.
    fn action_guidance(&self) -> Vec<(String, String)> {
        let Some(worker) = self.worker.as_ref() else {
            return Vec::new();
        };
        let snapshot = worker.nucleo.snapshot();
        (0..snapshot.item_count())
            .filter_map(|i| snapshot.get_item(i))
            .filter_map(|item| {
                let MenuItem::Custom { name, action, .. } = &item.data.item else {
                    return None;
                };
                let help = self.actions.get(action)?.help.clone();
                (!help.trim().is_empty()).then_some((name.clone(), help))
            })
            .collect()
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

        self.active_prompt = None;
        self.reload_pending = false;

        if let Some(prompt) = STORE::take::<MenuPrompt>() {
            self.set_prompt(prompt, state);
        }

        let mut worker = Self::make_worker();
        self.results.init(&mut worker);
        self.results.invalidate_widths();
        self.results.cursor_jump(0);
        // Clear any leftover table rows from the previous menu session while
        // `active_injectors() == 0` so `delay_clear` cannot flash stale rows.
        self.results.update_table(
            &mut worker,
            &self.selector,
            &mut matchmaker::matcher::matcher(),
        );
        self.worker = Some(worker);
        self.query_dirty = true;
        self.results.set_dirty();

        self.start_populate(state, false);
    }

    fn on_disable(&mut self) {
        GLOBAL::send_bind(BindDirective::OverrideTickrate(None));
        self.reload_pending = false;
        self.active_prompt = None;
        self.prompt.on_disable();
        // don't carry the search text into the next open
        self.query.clear();
        // Stops the matcher worker; rebuilt on the next enable.
        self.worker = None;
    }

    fn handle_input(
        &mut self,
        c: char,
        state: &mut MMState<'_, PathItem, ()>,
    ) -> OverlayEffect {
        if let Some(active) = self.active_prompt.clone() {
            if let OverlayEffect::Disable = self.prompt.handle_input(c, state) {
                self.on_prompt_accept(active, false)
            } else {
                OverlayEffect::None
            }
        } else {
            self.query.push_char(c);
            self.query_dirty = true;
            self.results.set_dirty();
            // auto-accept: a query that exactly matches an alias triggers
            // that item
            let input = self.query.input();
            if let Some(worker) = self.worker.as_ref() {
                let snapshot = worker.nucleo.snapshot();
                if let Some(item) = (0..snapshot.item_count())
                    .filter_map(|i| snapshot.get_item(i))
                    .find(|it| it.data.alias.as_deref() == Some(input.as_str()))
                    .map(|it| it.data.item.clone())
                {
                    return self.execute(item, state);
                }
            }
            OverlayEffect::None
        }
    }

    fn handle_action(
        &mut self,
        action: &Action<FsAction>,
        state: &mut MMState<'_, PathItem, ()>,
    ) -> OverlayEffect {
        if let Action::Custom(FsAction::ReloadMenu) = action {
            if self.active_prompt.is_some() {
                self.reload_pending = true;
            } else {
                self.start_populate(state, true);
            }
            return OverlayEffect::None;
        }

        // help is available even while a prompt is open; opening it closes
        // the menu (the pager is requested by re-queued help)
        if let Action::Custom(FsAction::Help) = action {
            let guidance = self.action_guidance();
            crate::run::register::request_menu_help(guidance);
            return OverlayEffect::Disable;
        }

        if let Some(active) = self.active_prompt.clone() {
            // Print("") is the alternative accept for the execute prompt:
            // detach instead of waiting.
            if matches!(active.kind, PromptKind::Execute)
                && matches!(action, Action::Print(s) if s.is_empty())
            {
                return self.on_prompt_accept(active, true);
            }
            // defer to prompt
            match self.prompt.handle_action_(action) {
                None => {}
                Some(false) => {
                    self.active_prompt = None;
                    // a prompt opened from a pane keybinding has no menu
                    // behind it the user asked for: close instead
                    if !active.return_menu {
                        return OverlayEffect::Disable;
                    }
                    if std::mem::take(&mut self.reload_pending) {
                        self.start_populate(state, true);
                    }
                }
                Some(true) => return self.on_prompt_accept(active, false),
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

        let frame_width = self.frame().width();
        let min_width = self.config.min_width.saturating_add(frame_width);
        let max_width = self
            .config
            .max_width
            .saturating_add(frame_width)
            .max(min_width);
        self.area = utils::default_area(
            [
                SizeHint {
                    adaptive_percentage: MENU_WIDTH_POINTS,
                    min: min_width,
                    max: max_width,
                },
                SizeHint {
                    adaptive_percentage: &[],
                    min: self.config.min_height.saturating_add(self.frame().height()),
                    max: 0,
                },
            ],
            layout,
            ui_area,
        );

        let inner = self.frame().inner_of(self.area);
        let [input, results] = self.inner_chunks(inner);
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
        if self.active_prompt.is_some() {
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
        frame.render_widget(self.frame().as_block(), self.area);

        let inner = self.frame().inner_of(self.area);
        let [input, results] = self.inner_chunks(inner);

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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{abspath::AbsPath, db::Pool, run::FsPane, watcher::WatcherMessage};
    use fist_types::filters::{SortOrder, Visibility};
    use matchmaker::{
        config::RenderConfig,
        message::Event,
        nucleo::{Column, Injector},
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
            exclude: None,
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
        if let Err(partial) = config.frame {
            let mut full = matchmaker::config::OverlayConfig::default().frame;
            full.apply(partial);
            config.frame = Ok(full);
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

    async fn wait_for_population(overlay: &mut MenuOverlay) {
        let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(2);
        loop {
            if let Some(worker) = overlay.worker.as_mut() {
                overlay.results.update_table(
                    worker,
                    &overlay.selector,
                    &mut matchmaker::matcher::matcher(),
                );
                // `running` drops between injector pushes; only a worker with
                // no live injector has streamed every item
                if worker.nucleo.active_injectors() == 0 && !overlay.results.status.running {
                    break;
                }
            }
            assert!(
                tokio::time::Instant::now() < deadline,
                "timed out waiting for menu population"
            );
            tokio::time::sleep(std::time::Duration::from_millis(5)).await;
        }
    }

    fn snapshot_labels(overlay: &MenuOverlay) -> Vec<String> {
        let snapshot = overlay.worker.as_ref().unwrap().nucleo.snapshot();
        (0..snapshot.item_count())
            .filter_map(|i| snapshot.get_item(i).map(|it| it.data.label().to_string()))
            .collect()
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
        assert!(overlay.worker.is_some());
        wait_for_population(&mut overlay).await;

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
            text.contains("new Folder"),
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
        wait_for_population(&mut overlay).await;
        let _ = render(&mut overlay);
        let text = render(&mut overlay);
        assert!(text.contains("new folder"), "items still listed:\n{text}");
        assert!(!text.contains('F'), "no builtin aliases rendered:\n{text}");
        let effect = {
            let o = &mut overlay as &mut dyn Overlay<FsAction, PathItem, ()>;
            o.handle_input('F', &mut mm_state)
        };
        assert!(matches!(effect, OverlayEffect::None));

        // with compact enabled, Move, Copy, Open, and Trash are omitted
        let mut config = full_config();
        config.compact = true;
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
        wait_for_population(&mut overlay).await;
        let _ = render(&mut overlay);
        let text = render(&mut overlay);
        assert!(
            !text.contains("Move"),
            "Move removed in compact mode:\n{text}"
        );
    }

    #[tokio::test]
    async fn menu_reloads_on_cursor_change() {
        let _bind_rx = init_globals();
        let actions: crate::menu::MenuActions = toml::from_str(
            r#"
            [txt-only]
            condition = { selected = "cursor", condition = ["ext:txt"] }
            command = "print('txt')"
            prefix = ""
            [zip-only]
            condition = { selected = "cursor", condition = ["ext:zip"] }
            command = "print('zip')"
            prefix = ""
            "#,
        )
        .unwrap();

        let (mut ui, mut picker, mut footer, mut preview, mut state, tx) = offline_mm_state();
        picker.worker.set_stability(5);
        picker.results.update_dimensions(UI_AREA);
        let inj = picker.worker.injector();
        let _ = inj.push(PathItem::new_unchecked(std::path::PathBuf::from(
            "/tmp/alpha.txt",
        )));
        let _ = inj.push(PathItem::new_unchecked(std::path::PathBuf::from(
            "/tmp/beta.zip",
        )));
        drop(inj);
        picker.worker.find("");
        // Let nucleo ingest and match the two items so current_raw() and results status settle.
        let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(2);
        loop {
            picker.results.update_table(
                &mut picker.worker,
                &picker.selector,
                &mut matchmaker::matcher::matcher(),
            );
            if picker.results.status.matched_count == 2 && !picker.results.status.running {
                break;
            }
            assert!(
                tokio::time::Instant::now() < deadline,
                "timed out waiting for test picker worker"
            );
            tokio::time::sleep(std::time::Duration::from_millis(5)).await;
        }
        assert_eq!(
            picker.current_indexed().unwrap().1.path.as_path(),
            std::path::Path::new("/tmp/alpha.txt")
        );

        let mut overlay = MenuOverlay::new(
            full_config(),
            crate::ui::prompt_overlay::PromptConfig::default(),
            actions,
        );
        {
            let mut mm_state =
                state.dispatcher(&mut ui, &mut picker, &mut footer, &mut preview, &tx);
            let o = &mut overlay as &mut dyn Overlay<FsAction, PathItem, ()>;
            o.on_enable(&UI_AREA, &mut mm_state);
            o.area(
                &UI_AREA,
                &matchmaker::config::OverlayConfig::default().layout,
            );
        }
        wait_for_population(&mut overlay).await;
        let labels = snapshot_labels(&overlay);
        assert!(
            labels.iter().any(|l| l == "txt-only"),
            "txt-only shown for alpha.txt: {labels:?}"
        );
        assert!(
            !labels.iter().any(|l| l == "zip-only"),
            "zip-only hidden for alpha.txt: {labels:?}"
        );

        // Simulate scrolling the main picker to beta.zip while the menu remains
        // open, then dispatching ReloadMenu as the CursorChange handler does.
        picker.results.cursor_next();
        assert_eq!(
            picker.current_indexed().unwrap().1.path.as_path(),
            std::path::Path::new("/tmp/beta.zip")
        );
        {
            let mut mm_state =
                state.dispatcher(&mut ui, &mut picker, &mut footer, &mut preview, &tx);
            let o = &mut overlay as &mut dyn Overlay<FsAction, PathItem, ()>;
            let effect = o.handle_action(&Action::Custom(FsAction::ReloadMenu), &mut mm_state);
            assert!(matches!(effect, OverlayEffect::None));
        }
        wait_for_population(&mut overlay).await;
        let labels = snapshot_labels(&overlay);
        assert!(
            labels.iter().any(|l| l == "zip-only"),
            "zip-only shown after cursor moves to beta.zip: {labels:?}"
        );
        assert!(
            !labels.iter().any(|l| l == "txt-only"),
            "txt-only removed after cursor moves to beta.zip: {labels:?}"
        );
    }

    #[tokio::test]
    async fn prompt_cancel_follows_return_menu() {
        let _bind_rx = init_globals();
        let (mut ui, mut picker, mut footer, mut preview, mut state, tx) = offline_mm_state();
        let mut mm_state = state.dispatcher(&mut ui, &mut picker, &mut footer, &mut preview, &tx);

        let mut overlay = MenuOverlay::new(
            full_config(),
            crate::ui::prompt_overlay::PromptConfig::default(),
            crate::menu::MenuActions::default(),
        );

        // a prompt opened from a pane keybinding only borrows the menu as a
        // host: cancelling closes instead of revealing the menu list
        STORE::set_menu_prompt(Some(MenuPrompt::new(PromptKind::New).return_menu(false)));
        {
            let o = &mut overlay as &mut dyn Overlay<FsAction, PathItem, ()>;
            o.on_enable(&UI_AREA, &mut mm_state);
            o.area(
                &UI_AREA,
                &matchmaker::config::OverlayConfig::default().layout,
            );
        }
        assert!(overlay.active_prompt.is_some(), "the stored prompt opens");
        let effect = {
            let o = &mut overlay as &mut dyn Overlay<FsAction, PathItem, ()>;
            o.handle_action(&Action::Quit(1), &mut mm_state)
        };
        assert!(matches!(effect, OverlayEffect::Disable));

        // a prompt launched by a menu item keeps the menu behind it
        {
            let o = &mut overlay as &mut dyn Overlay<FsAction, PathItem, ()>;
            o.on_enable(&UI_AREA, &mut mm_state);
            o.area(
                &UI_AREA,
                &matchmaker::config::OverlayConfig::default().layout,
            );
        }
        overlay.set_prompt(MenuPrompt::new(PromptKind::New), &mut mm_state);
        let effect = {
            let o = &mut overlay as &mut dyn Overlay<FsAction, PathItem, ()>;
            o.handle_action(&Action::Quit(1), &mut mm_state)
        };
        assert!(matches!(effect, OverlayEffect::None));
        assert!(overlay.active_prompt.is_none(), "cancel closes the prompt");
    }
    #[test]
    fn test_current_dir_suffix() {
        let cwd = std::env::current_dir().unwrap();
        let cwd_abs = AbsPath::new_unchecked(cwd.clone());
        assert_eq!(
            execute::current_dir_suffix(&cwd_abs),
            Some(std::path::PathBuf::from(""))
        );

        if let Some(parent) = cwd.parent() {
            let parent_abs = AbsPath::new_unchecked(parent.to_path_buf());
            let expected_rel = cwd.strip_prefix(parent).unwrap();
            assert_eq!(
                execute::current_dir_suffix(&parent_abs),
                Some(expected_rel.to_path_buf())
            );
        }

        let non_existent = AbsPath::new_unchecked(cwd.join("__non_existent_subpath_xyz__"));
        assert_eq!(execute::current_dir_suffix(&non_existent), None);
    }

    #[tokio::test]
    async fn test_menu_area_size_hints() {
        let _bind_rx = init_globals();
        let (mut ui, mut picker, mut footer, mut preview, mut state, tx) = offline_mm_state();
        let mut mm_state = state.dispatcher(&mut ui, &mut picker, &mut footer, &mut preview, &tx);

        let mut config = full_config();
        config.min_width = 20;
        config.min_height = 12;
        let mut overlay = MenuOverlay::new(
            config,
            crate::ui::prompt_overlay::PromptConfig::default(),
            crate::menu::MenuActions::default(),
        );

        {
            let o = &mut overlay as &mut dyn Overlay<FsAction, PathItem, ()>;
            o.on_enable(&UI_AREA, &mut mm_state);
            let layout = matchmaker::config::OverlayConfig::default().layout;
            o.area(&UI_AREA, &layout);
        }
        wait_for_population(&mut overlay).await;

        // In a large terminal (80x24), width is at least min_width + border.width() (20 + 2 = 22)
        // and height is min_height + border.height() (12 + 3 = 15)
        assert!(overlay.area.width >= 22);
        assert_eq!(overlay.area.height, 15);

        // In a terminal smaller than min width / height, default_area safely clamps to ui_area
        let small_ui = Rect {
            x: 0,
            y: 0,
            width: 12,
            height: 8,
        };
        {
            let o = &mut overlay as &mut dyn Overlay<FsAction, PathItem, ()>;
            let layout = matchmaker::config::OverlayConfig::default().layout;
            o.area(&small_ui, &layout);
        }
        assert!(overlay.area.width <= 12);
        assert!(overlay.area.height <= 8);
    }
}
