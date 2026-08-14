use crate::{
    abspath::AbsPath,
    lessfilter::file_rule::FileData,
    run::{
        action::FsAction,
        item::{PathItem, short_display},
        queue::QUEUE,
        state::{GLOBAL, InPrompt, MenuCommandPaths, MenuPrompt, STACK, STORE, TOAST, lessfilter_cfg},
    },
    spawn::{
        menu_action::{condition_passes, MenuActions, MenuCondition, MenuStrategy},
        open_wrapped,
    },
    ui::prompt_overlay::{PromptConfig, PromptOverlay},
    utils::{
        serde::border_result,
        text::{ToastStyle, bold_indices},
    },
};

use matchmaker::{
    action::Action,
    config::{BorderSetting, OverlayLayoutSettings, PartialBorderSetting, StyleSetting},
    render::MMState,
    ui::{Overlay, OverlayEffect, utils},
};
use ratatui::{
    prelude::*,
    widgets::{Borders, Clear, Padding, Paragraph},
};
const MAX_ITEM_WIDTH: u16 = 9;

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct MenuConfig {
    #[serde(with = "border_result")]
    pub border: Result<BorderSetting, PartialBorderSetting>,
    pub item_style: StyleSetting,
    pub current_style: StyleSetting,
}

impl Default for MenuConfig {
    fn default() -> Self {
        let border = PartialBorderSetting {
            title: Some("Menu".into()),
            sides: Some(Borders::ALL),
            padding: Some(Padding::symmetric(2, 1).into()),
            ..Default::default()
        };
        Self {
            border: Err(border),
            item_style: Default::default(),
            current_style: StyleSetting {
                fg: None,
                bg: Some(Color::Black),
                modifier: Modifier::BOLD,
            },
        }
    }
}

pub use super::menu_overlay_impl::*;

/// MenuItem enum with stateless action
#[derive(Clone)]
pub enum MenuItem {
    New,
    Rename,
    Cut,
    Copy,
    Trash,
    Delete,
    Open,
    OpenWith,
    Custom { name: String, action: String },
}

impl MenuItem {
    pub fn from_key(c: char) -> Option<Self> {
        match c {
            'n' => Some(MenuItem::New),
            'r' => Some(MenuItem::Rename),
            'x' => Some(MenuItem::Cut),
            'c' => Some(MenuItem::Copy),
            't' => Some(MenuItem::Trash),
            'T' => Some(MenuItem::Delete),
            'o' => Some(MenuItem::Open),
            'w' => Some(MenuItem::OpenWith),
            _ => None, // custom items cannot be triggered by key here
        }
    }

    pub fn line(
        &self,
        menu_config: &MenuConfig,
    ) -> Line<'static> {
        let style = menu_config.item_style.into();

        match self {
            MenuItem::New => Line::from(bold_indices("new", [0], style)),
            MenuItem::Rename => Line::from(bold_indices("rename", [0], style)),
            MenuItem::Cut => Line::from(bold_indices("cut (x)", [6], style)),
            MenuItem::Copy => Line::from(bold_indices("copy", [0], style)),
            MenuItem::Trash => Line::from(bold_indices("trash", [0], style)),
            MenuItem::Delete => Line::from(bold_indices("deleTe", [5], style)),
            MenuItem::Open => Line::from(bold_indices("open", [0], style)),
            MenuItem::OpenWith => Line::from(bold_indices("open with", [5], style)),
            MenuItem::Custom { name, .. } => Line::from(name.clone()).style(style),
        }
    }

    /// Execute an action.
    /// Returns a [`MenuPrompt`] to open the input bar, or whether to keep menu open.
    pub fn action(
        &self,
        path: AbsPath,
    ) -> Result<MenuPrompt, bool> {
        match self {
            MenuItem::New => Ok(MenuPrompt::new(PromptKind::New)),
            MenuItem::Rename => {
                let filename = path.to_string_lossy().into_owned();
                let cursor_pos = path
                    .with_file_name(path.file_stem().unwrap_or_default())
                    .to_string_lossy()
                    .len();
                Ok(MenuPrompt {
                    kind: PromptKind::Rename,
                    title: "Rename".to_string(),
                    initial: filename,
                    cursor: cursor_pos,
                })
            }
            MenuItem::Cut => {
                TOAST::push(ToastStyle::Normal, "Cut: ", [short_display(&path)]);
                QUEUE::extend("cut", vec![path]);
                Err(false)
            }
            MenuItem::Copy => {
                TOAST::push(ToastStyle::Normal, "Copied: ", [short_display(&path)]);
                QUEUE::extend("copy", vec![path]);
                Err(false)
            }

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
                let path_clone = path;
                let pool = GLOBAL::db();
                tokio::spawn(async move {
                    let conn = pool.get_conn(crate::db::DbTable::dirs).await?;
                    open_wrapped(conn, None, &[path_clone.inner().into()], true).await?;
                    anyhow::Ok(())
                });
                Err(false)
            }
            MenuItem::OpenWith => {
                QUEUE::stash("app", path);
                GLOBAL::send_action(FsAction::App);
                Err(false)
            }
            MenuItem::Custom { action, .. } => {
                unreachable!("custom items are routed through MenuOverlay::run_custom")
            }
        }
    }
}

/// The main MenuOverlay
pub struct MenuOverlay {
    pub cursor: usize,
    pub config: MenuConfig,
    pub prompt_kind: Option<PromptKind>,
    pub prompt: PromptOverlay,
    pub items: Vec<MenuItem>,
    /// The custom actions; only those whose conditions pass are listed.
    pub actions: MenuActions,
    pub area: Rect,
}

pub const MENU_ITEMS: [MenuItem; 8] = [
    MenuItem::New,
    MenuItem::Rename,
    MenuItem::Cut,
    MenuItem::Copy,
    MenuItem::Trash,
    MenuItem::Delete,
    MenuItem::Open,
    MenuItem::OpenWith,
];

impl MenuOverlay {
    pub fn new(
        config: MenuConfig,
        prompt_config: PromptConfig,
        actions: MenuActions,
    ) -> Self {
        Self {
            cursor: 0,
            config,
            prompt_kind: None,
            prompt: PromptOverlay::new(prompt_config),
            items: MENU_ITEMS.to_vec(),
            actions,
            area: Rect::default(),
        }
    }

    pub fn border(&self) -> &BorderSetting {
        self.config.border.as_ref().unwrap()
    }

    fn make_widget(&self) -> Paragraph<'_> {
        let lines: Vec<Line> = self
            .items
            .iter()
            .enumerate()
            .filter_map(|(idx, item)| {
                if STACK::in_app() && !matches!(item, MenuItem::Custom { .. }) {
                    return None;
                }
                let mut line = item.line(&self.config);

                if idx == self.cursor {
                    line = line.style(self.config.current_style)
                }
                Some(line)
            })
            .collect();
        Paragraph::new(lines).block(self.border().as_block())
    }

    fn set_prompt(
        &mut self,
        prompt: MenuPrompt,
        state: &mut MMState<'_, '_, PathItem, ()>,
    ) {
        self.prompt_kind = Some(prompt.kind);
        if !prompt.title.is_empty() {
            self.prompt.input.config.border.title = prompt.title;
        }
        self.prompt.on_enable(&Rect::default(), state);

        if !prompt.initial.is_empty() {
            self.prompt.input.set_value(prompt.initial);
            self.prompt.input.inner.set(Option::<String>::None, prompt.cursor as u16);
        }
    }

    fn handle_menu_input(
        &mut self,
        c: char,
        state: &mut MMState<'_, '_, PathItem, ()>,
    ) -> OverlayEffect {
        if let Some(item) = MenuItem::from_key(c) {
            let path = self.target_path(state);
            let action_result = item.action(path);
            match action_result {
                Ok(prompt) => {
                    self.set_prompt(prompt, state);
                    OverlayEffect::None
                }
                Err(true) => OverlayEffect::None,
                Err(false) => OverlayEffect::Disable,
            }
        } else if c == 'q' {
            OverlayEffect::Disable
        } else {
            OverlayEffect::None
        }
    }

    pub fn accept(&mut self, state: &mut MMState<'_, '_, PathItem, ()>) -> OverlayEffect {
        let custom_key = match &self.items[self.cursor] {
            MenuItem::Custom { action, .. } => Some(action.clone()),
            _ => None,
        };
        if let Some(key) = custom_key {
            return self.run_custom(&key, state);
        }
        let item = &self.items[self.cursor];
        let path = self.target_path(state);
        let action_result = item.action(path);
        match action_result {
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
    fn build_items(&mut self, state: &mut MMState<'_, '_, PathItem, ()>) {
        let lcfg = lessfilter_cfg();
        let mut cache: Vec<(AbsPath, FileData<'_>)> = Vec::new();

        let selected: Vec<AbsPath> = state.map_selections_to_vec(|_, item| item.path.clone());
        let cursor = if state.picker_ui.results.cursor_disabled() {
            None
        } else {
            state.current_raw().map(|item| item.path.clone())
        };
        let in_prompt = STORE::contains::<InPrompt>();
        let cwd = STACK::cwd();

        let mut items = MENU_ITEMS.to_vec();
        for (key, action) in self.actions.iter() {
            if condition_passes(
                &action.condition,
                &selected,
                cursor.as_ref(),
                in_prompt,
                cwd.as_ref(),
                &mut cache,
                &lcfg.settings,
                &lcfg.categories,
            ) {
                items.push(MenuItem::Custom {
                    name: key.clone(),
                    action: key.clone(),
                });
            }
        }
        self.items = items;
    }

    /// Run a custom action on the target items (the current selection, or
    /// the target item when nothing is selected).
    fn run_custom(
        &mut self,
        key: &str,
        state: &mut MMState<'_, '_, PathItem, ()>,
    ) -> OverlayEffect {
        let Some(action) = self.actions.get(key) else {
            log::error!("Menu action not found: {key}");
            return OverlayEffect::None;
        };
        let selected: Vec<AbsPath> = state.map_selections_to_vec(|_, item| item.path.clone());
        // count = 0 conditions are evaluated against the pane cwd, so their
        // targets are the cwd (visibility guarantees it exists).
        let targets: Vec<AbsPath> = if action
            .condition
            .iter()
            .any(|c| matches!(c, MenuCondition::Repeat { count: Some(0), .. }))
        {
            STACK::cwd().map(|p| vec![p]).unwrap_or_else(|| vec![self.target_path(state)])
        } else if selected.is_empty() {
            vec![self.target_path(state)]
        } else {
            selected
        };
        let displays: Vec<Span<'static>> = targets.iter().map(|p| short_display(p)).collect();

        match action.strategy {
            MenuStrategy::Stash => {
                QUEUE::enqueue(key, targets);
                TOAST::push(ToastStyle::Normal, "Queued: ", displays);
            }
            MenuStrategy::Batch(n) => {
                // a zero batch size parses but chunks of size 0 are invalid
                let n = n.max(1);
                for chunk in targets.chunks(n) {
                    QUEUE::enqueue(key, chunk.to_vec());
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

    pub fn move_cursor_up(&mut self) {
        if self.cursor == 0 {
            self.cursor = self.items.len() - 1;
        } else {
            self.cursor -= 1;
        }
    }

    pub fn move_cursor_down(&mut self) {
        self.cursor = (self.cursor + 1) % self.items.len();
    }
}

impl Overlay<FsAction, PathItem, ()> for MenuOverlay {
    fn on_enable(
        &mut self,
        _area: &Rect,
        state: &mut MMState<'_, '_, PathItem, ()>,
    ) {
        self.cursor = 0;
        self.prompt_kind = None;

        if let Some(prompt) = STORE::take::<MenuPrompt>() {
            self.set_prompt(prompt, state);
        }

        // list the custom actions whose conditions pass (evaluated once,
        // against the state at open)
        self.build_items(state);
    }

    fn on_disable(&mut self) {
        self.prompt.on_disable();
    }

    fn handle_input(
        &mut self,
        c: char,
        state: &mut MMState<'_, '_, PathItem, ()>,
    ) -> OverlayEffect {
        if let Some(p) = self.prompt_kind {
            if let OverlayEffect::Disable = self.prompt.handle_input(c, state) {
                self.on_prompt_accept(p, state)
            } else {
                OverlayEffect::None
            }
        } else {
            self.handle_menu_input(c, state)
        }
    }

    fn handle_action(
        &mut self,
        action: &Action<FsAction>,
        state: &mut MMState<'_, '_, PathItem, ()>,
    ) -> OverlayEffect {
        if let Some(p) = self.prompt_kind {
            // defer to prompt
            match self.prompt.handle_action_(action) {
                None => {}
                Some(false) => self.prompt_kind = None,
                Some(true) => return self.on_prompt_accept(p, state),
            }
        } else {
            match action {
                Action::Up(_) => self.move_cursor_up(),
                Action::Down(_) => self.move_cursor_down(),
                Action::Accept => return self.accept(state),
                Action::Quit(_) => return OverlayEffect::Disable,
                _ => {}
            }
        }
        OverlayEffect::None
    }

    fn area(
        &mut self,
        ui_area: &Rect,
        layout: &OverlayLayoutSettings,
    ) {
        self.prompt.area(ui_area, layout);
        self.area = utils::default_area(
            [
                (MAX_ITEM_WIDTH + self.border().width()).into(),
                (self.items.len() as u16 + self.border().height()).into(),
            ],
            layout,
            ui_area,
        );
    }

    fn draw(
        &mut self,
        frame: &mut matchmaker::ui::Frame,
    ) {
        if self.prompt_kind.is_some() {
            self.prompt.draw(frame);
        } else {
            frame.render_widget(Clear, self.area);
            frame.render_widget(self.make_widget(), self.area);
        }
    }
}
