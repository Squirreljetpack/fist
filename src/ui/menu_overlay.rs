use crate::{
    abspath::AbsPath,
    cli::paths::__cwd,
    fs::{auto_dest, create_all, rename},
    run::{
        action::FsAction,
        item::{PathItem, short_display},
        stash::{CustomStashActionActionState, STASH, StashItem},
        state::{APP, GLOBAL, STACK, TASKS, TOAST, TlsStore},
    },
    spawn::{menu_action::MenuActions, open_wrapped},
    ui::prompt_overlay::{PromptConfig, PromptOverlay},
    utils::{
        serde::border_result,
        text::{ToastStyle, bold_indices},
    },
};

use cli_boilerplate_automation::{
    bath::{PathExt, RenamePolicy, auto_dest_for_src, root_dir},
    bog::BogUnwrapExt,
};
use matchmaker::{
    action::Action,
    config::{BorderSetting, PartialBorderSetting},
    ui::{Overlay, OverlayEffect, SizeHint},
};
use ratatui::{
    prelude::*,
    widgets::{Block, Borders, Clear, Padding, Paragraph},
};
use std::path::Path;
const MAX_ITEM_WIDTH: u16 = 9;

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct MenuConfig {
    #[serde(with = "border_result")]
    pub border: Result<BorderSetting, PartialBorderSetting>,
    pub item_fg: Color,
    pub item_modifier: Modifier,
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
            item_fg: Default::default(),
            item_modifier: Default::default(),
        }
    }
}

#[derive(Debug, strum::Display, Clone, Copy)]
pub enum PromptKind {
    New,
    #[strum(serialize = "New folder")]
    NewDir,
    Rename,
}

#[derive(Debug)]
pub enum MenuTarget {
    Item(PathItem),
    Cwd(AbsPath),
}
impl Default for MenuTarget {
    fn default() -> Self {
        Self::Cwd(AbsPath::empty())
    }
}
impl MenuTarget {
    pub fn title(&self) -> Option<String> {
        match self {
            Item(s) => Some(s.path.basename()),
            _ => None,
        }
    }
}

use MenuTarget::*;

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
        let style = Style::new()
            .add_modifier(menu_config.item_modifier)
            .fg(menu_config.item_fg);

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
    /// Returns an optional input to [`TEMP::set_prompt`], or whether to keep menu open.
    pub fn action(
        &self,
        path: AbsPath,
    ) -> Result<(PromptKind, Option<String>), bool> {
        match self {
            MenuItem::New => Ok((PromptKind::New, None)),
            MenuItem::Rename => Ok((PromptKind::Rename, Some(path.to_string_lossy().into()))),
            MenuItem::Cut | MenuItem::Copy => {
                TOAST::push(ToastStyle::Normal, "Cut: ", [short_display(&path)]);
                STASH::extend(vec![StashItem::mv(path)]);
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
                TASKS::spawn(async move {
                    match if path.is_dir() {
                        tokio::fs::remove_dir_all(&path).await
                    } else {
                        tokio::fs::remove_file(&path).await
                    } {
                        Ok(_) => {
                            TOAST::push(ToastStyle::Success, "Deleted: ", [short_display(&path)])
                        }
                        Err(e) => {
                            log::error!("Failed to delete {}: {e}", path.to_string_lossy());
                            TOAST::push(
                                ToastStyle::Error,
                                "Failed to delete: ",
                                [short_display(&path)],
                            )
                        }
                    }
                });
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
                STASH::set_cas(CustomStashActionActionState::App);
                STASH::push_custom(path);
                GLOBAL::send_action(FsAction::App);
                Err(false)
            }
            MenuItem::Custom { action, .. } => {
                todo!()
            }
        }
    }
}

/// The main MenuOverlay
pub struct MenuOverlay {
    cursor: usize,
    config: MenuConfig,
    prompt_kind: Option<PromptKind>,
    prompt: PromptOverlay,
    /// See [TEMP::set_input_bar]
    target: MenuTarget,
    items: Vec<MenuItem>,
}

pub static MENU_ITEMS: [MenuItem; 8] = [
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
            target: Default::default(),
            items: MENU_ITEMS.to_vec(),
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
                    line = line.patch_style(
                        Style::default()
                            .bg(Color::Black)
                            .add_modifier(Modifier::BOLD),
                    )
                }
                Some(line)
            })
            .collect();
        Paragraph::new(lines).block(self.border().as_block())
    }

    fn target_path(&self) -> AbsPath {
        match &self.target {
            Item(p) => p.path.clone(),
            Cwd(p) => p.clone(),
        }
    }
    fn target_parent(&self) -> AbsPath {
        match &self.target {
            Item(p) => p.path._parent(),
            Cwd(p) => p.clone(),
        }
    }

    fn set_prompt(
        &mut self,
        prompt: PromptKind,
        extra_title: Option<String>,
    ) {
        self.prompt_kind = Some(prompt);
        self.prompt.config.border.title = match extra_title {
            Some(s) => format!("{}: {}", prompt, s),
            None => prompt.to_string(),
        };
        self.prompt.on_enable(&Rect::default());
    }

    fn handle_menu_input(
        &mut self,
        c: char,
    ) -> OverlayEffect {
        if let Some(item) = MenuItem::from_key(c) {
            let action_result = match &self.target {
                Item(target) => item.action(target.path.clone()),
                Cwd(_) => {
                    todo!()
                }
            };
            match action_result {
                Ok((prompt, extra)) => {
                    self.set_prompt(prompt, extra);
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

    fn on_prompt_accept(
        &mut self,
        prompt: PromptKind,
    ) -> OverlayEffect {
        match prompt {
            PromptKind::New => {
                let current_item_parent = self.target_parent();
                let input_path = Path::new(&self.prompt.input);
                let dest = auto_dest(input_path, &current_item_parent); // replaced if input is absolute
                let dest_slice = [dest];

                TASKS::spawn(async move {
                    match create_all(&dest_slice).await {
                        Ok(_) => {
                            let dest_path = match &dest_slice[0] {
                                Ok(p) | Err(p) => p,
                            };
                            TOAST::push(ToastStyle::Success, "New: ", [short_display(dest_path)]);
                        }
                        Err(_) => {
                            let dest_path = match &dest_slice[0] {
                                Ok(p) | Err(p) => p,
                            };
                            TOAST::push(
                                ToastStyle::Error,
                                "Failed to create: ",
                                [short_display(dest_path)],
                            );
                        }
                    }
                });
            }
            PromptKind::NewDir => {
                let current_item_parent = self.target_parent();
                let input_path = Path::new(&self.prompt.input);
                let dest = AbsPath::new_unchecked(input_path.abs(current_item_parent));

                TASKS::spawn(async move {
                    match std::fs::create_dir_all(&dest) {
                        Ok(_) => {
                            TOAST::push(ToastStyle::Success, "New: ", [short_display(&dest)]);
                        }
                        Err(_) => {
                            TOAST::push(
                                ToastStyle::Error,
                                "Failed to create: ",
                                [short_display(&dest)],
                            );
                        }
                    }
                });
            }
            PromptKind::Rename => {
                let old_path = self.target_path();
                if old_path.file_name().is_none() {
                    OverlayEffect::None;
                }
                let dest = AbsPath::new_unchecked(
                    auto_dest_for_src(&old_path, &self.prompt.input, &RenamePolicy::default())
                        .abs(old_path.parent().unwrap()),
                );

                if dest == old_path {
                    TOAST::push_skipped();
                } else {
                    TASKS::spawn(async move {
                        match rename(&old_path, &dest).await {
                            Ok(_) => {
                                let new_display = dest.to_string_lossy().to_string().into();
                                TOAST::push_pair(
                                    ToastStyle::Success,
                                    "Renamed: ",
                                    short_display(&old_path),
                                    new_display,
                                );
                            }
                            Err(_) => {
                                TOAST::push(
                                    ToastStyle::Error,
                                    "Failed to rename: ",
                                    [short_display(&old_path)],
                                );
                            }
                        }
                    });
                }
            }
        }

        self.prompt_kind = None;
        self.prompt.on_disable();
        OverlayEffect::Disable
    }

    pub fn accept(&mut self) -> OverlayEffect {
        let item = &self.items[self.cursor];

        let action_result = match &self.target {
            Item(target) => item.action(target.path.clone()),
            Cwd(_) => {
                todo!()
            }
        };
        match action_result {
            Ok((prompt, extra)) => {
                self.set_prompt(prompt, extra);
                OverlayEffect::None
            }
            Err(true) => OverlayEffect::None,
            Err(false) => OverlayEffect::Disable,
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

impl Overlay for MenuOverlay {
    type A = FsAction;

    fn on_enable(
        &mut self,
        area: &Rect,
    ) {
        self.cursor = 0;
        self.prompt_kind = None;
        let p = TlsStore::take();
        let target: MenuTarget = TlsStore::take().unwrap_or_default();

        if let Some(p) = p {
            self.set_prompt(p, target.title());
        }
        self.target = target;
    }

    fn on_disable(&mut self) {
        self.prompt.on_disable();
    }

    fn handle_input(
        &mut self,
        c: char,
    ) -> OverlayEffect {
        if let Some(p) = self.prompt_kind {
            if let OverlayEffect::Disable = self.prompt.handle_input(c) {
                self.on_prompt_accept(p)
            } else {
                OverlayEffect::None
            }
        } else {
            self.handle_menu_input(c)
        }
    }

    fn handle_action(
        &mut self,
        action: &Action<Self::A>,
    ) -> OverlayEffect {
        if let Some(p) = self.prompt_kind {
            match self.prompt.handle_action_(action) {
                None => {}
                Some(false) => self.prompt_kind = None,
                Some(true) => return self.on_prompt_accept(p),
            }
        } else {
            match action {
                Action::Up(_) => self.move_cursor_up(),
                Action::Down(_) => self.move_cursor_down(),
                Action::Accept => return self.accept(),
                Action::Quit(_) => return OverlayEffect::Disable,
                _ => {}
            }
        }
        OverlayEffect::None
    }

    fn area(
        &mut self,
        ui_area: &Rect,
    ) -> Result<Rect, [SizeHint; 2]> {
        self.prompt.area(ui_area);
        Err([
            (MAX_ITEM_WIDTH + self.border().width()).into(),
            (self.items.len() as u16 + self.border().height()).into(),
        ])
    }

    fn draw(
        &mut self,
        frame: &mut matchmaker::ui::Frame,
        mut area: matchmaker::ui::Rect,
    ) {
        if self.prompt_kind.is_some() {
            self.prompt.draw(frame, Rect::default());
        } else {
            frame.render_widget(Clear, area);
            frame.render_widget(self.make_widget(), area);
        }
    }
}
