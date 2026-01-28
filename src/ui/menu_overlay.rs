use std::path::Path;

use crate::abspath::AbsPath;
use crate::cli::paths::__cwd;
use crate::fs::{auto_dest, create_all, rename};
use crate::run::action::FsAction;
use crate::run::globals::{GLOBAL, TEMP, TOAST};
use crate::run::item::{PathItem, short_display};
use crate::run::stash::{STASH, StashItem};
use crate::spawn::open_wrapped;
use crate::ui::prompt_overlay::{PromptConfig, PromptOverlay};
use crate::utils::text::{ToastStyle, bold_indices};
use cli_boilerplate_automation::bath::{PathExt, RenamePolicy, auto_dest_for_src, root_dir};
use cli_boilerplate_automation::bog::BogUnwrapExt;
use matchmaker::action::Action;
use matchmaker::config::BorderSetting;
use matchmaker::ui::{Overlay, OverlayEffect};
use ratatui::widgets::Padding;
use ratatui::{
    prelude::*,
    widgets::{Block, Borders, Clear, Paragraph},
};

const MAX_ITEM_WIDTH: u16 = 9;

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct MenuConfig {
    pub border: BorderSetting,
}

impl Default for MenuConfig {
    fn default() -> Self {
        Self {
            border: BorderSetting {
                title: "Menu".into(),
                sides: Borders::ALL,
                padding: Padding::symmetric(2, 1),
                ..Default::default()
            },
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

    pub fn line(&self) -> Line<'static> {
        match self {
            MenuItem::New => Line::from(bold_indices("new", [0])),
            MenuItem::Rename => Line::from(bold_indices("rename", [0])),
            MenuItem::Cut => Line::from(bold_indices("cut (x)", [6])),
            MenuItem::Copy => Line::from(bold_indices("copy", [0])),
            MenuItem::Trash => Line::from(bold_indices("trash", [0])),
            MenuItem::Delete => Line::from(bold_indices("deleTe", [5])),
            MenuItem::Open => Line::from(bold_indices("open", [0])),
            MenuItem::OpenWith => Line::from(bold_indices("open with", [6])),
            MenuItem::Custom { name, .. } => Line::from(name.clone()),
        }
    }

    /// Execute an action.
    /// Returns an optional input to [`TEMP::set_prompt`]
    pub fn action(
        &self,
        path: AbsPath,
    ) -> Option<(PromptKind, Option<String>)> {
        match self {
            MenuItem::New => Some((PromptKind::New, None)),
            MenuItem::Rename => Some((PromptKind::Rename, Some(path.to_string_lossy().into()))),
            MenuItem::Cut | MenuItem::Copy => {
                TOAST::push(ToastStyle::Normal, "Cut: ", [short_display(&path)]);
                STASH::insert(vec![StashItem::mv(path)]);
                None
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
                None
            }
            MenuItem::Delete => {
                tokio::spawn(async move {
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
                None
            }
            MenuItem::Open => {
                let path_clone = path;
                let pool = GLOBAL::db();
                tokio::spawn(async move {
                    let conn = pool.get_conn(crate::db::DbTable::dirs).await?;
                    open_wrapped(conn, None, &[path_clone.inner().into()]).await?;
                    anyhow::Ok(())
                });
                None
            }
            MenuItem::OpenWith => {
                todo!()
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
    target: Result<PathItem, AbsPath>,
    items: Vec<MenuItem>,
}

impl MenuOverlay {
    pub fn new(
        config: MenuConfig,
        prompt_config: PromptConfig,
    ) -> Self {
        let items: Vec<MenuItem> = vec![
            MenuItem::New,
            MenuItem::Rename,
            MenuItem::Cut,
            MenuItem::Copy,
            MenuItem::Trash,
            MenuItem::Delete,
            MenuItem::Open,
            MenuItem::OpenWith,
        ];

        Self {
            cursor: 0,
            config,
            prompt_kind: None,
            prompt: PromptOverlay::new(prompt_config),
            target: Ok(PathItem::_uninit()),
            items,
        }
    }

    fn make_widget(&self) -> Paragraph<'_> {
        let lines: Vec<Line> = self
            .items
            .iter()
            .enumerate()
            .map(|(idx, item)| {
                let mut line = item.line();
                if idx == self.cursor {
                    line = line.patch_style(
                        Style::default()
                            .bg(Color::Black)
                            .add_modifier(Modifier::BOLD),
                    )
                }
                line
            })
            .collect();
        Paragraph::new(lines).block(self.config.border.as_block())
    }

    fn target_path(&self) -> AbsPath {
        match &self.target {
            Ok(p) => p.path.clone(),
            Err(p) => p.clone(),
        }
    }
    fn target_parent(&self) -> AbsPath {
        match &self.target {
            Ok(p) => p.path._parent(),
            Err(p) => p.clone(),
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
            if let Some((prompt, extra)) = match &self.target {
                Ok(target) => item.action(target.path.clone()),
                Err(_) => {
                    todo!()
                }
            } {
                self.set_prompt(prompt, extra);
            }

            OverlayEffect::None
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

                tokio::spawn(async move {
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

                tokio::spawn(async move {
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
                    tokio::spawn(async move {
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
        if let Some((prompt, extra)) = match &self.target {
            Ok(target) => self.items[self.cursor].action(target.path.clone()),
            Err(_) => todo!(),
        } {
            self.set_prompt(prompt, extra);
        }

        OverlayEffect::None
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
        let (p, s) = TEMP::take_input_bar();
        if let Some(p) = p {
            self.set_prompt(p, s.as_ref().ok().map(|s| s.path.basename()));
        }
        self.target = s;
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
    ) -> Result<Rect, [u16; 2]> {
        self.prompt.area(ui_area);
        Err([
            MAX_ITEM_WIDTH + self.config.border.width(),
            self.items.len() as u16 + self.config.border.height(),
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
