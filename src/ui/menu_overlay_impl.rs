use crate::{
    abspath::AbsPath,
    db::DbTable,
    fs::{auto_dest, create_all, rename},
    run::{
        FsAction,
        item::short_display,
        state::{GLOBAL, STACK, TASKS, TOAST},
    },
    utils::text::ToastStyle,
};

use cba::bath::{PathExt, RenamePolicy, auto_dest_for_src};
use matchmaker::ui::{Overlay, OverlayEffect};
use std::path::Path;

use super::menu_overlay::MenuOverlay;

#[derive(Debug, strum::Display, Clone, Copy)]
pub enum PromptKind {
    New,
    #[strum(serialize = "New folder")]
    NewDir,
    Rename,
    #[strum(serialize = "Set alias")]
    SetAlias,
}

#[derive(Debug, Clone)]
pub enum MenuTarget {
    Item(AbsPath),
}

impl MenuTarget {
    pub fn abs_path(&self) -> &AbsPath {
        match self {
            Self::Item(p) => p,
        }
    }
}

impl MenuOverlay {
    pub fn target_path(&self) -> AbsPath {
        self.target
            .as_ref()
            .map(|t| t.abs_path().clone())
            .unwrap_or_else(STACK::_cwd)
    }

    pub fn target_parent(&self) -> AbsPath {
        self.target
            .as_ref()
            .map(|t| t.abs_path()._parent())
            .unwrap_or_else(STACK::_cwd)
    }

    pub fn on_prompt_accept(
        &mut self,
        prompt: PromptKind,
    ) -> OverlayEffect {
        match prompt {
            PromptKind::New => {
                let current_item_parent = self.target_parent();
                let input = self.prompt.input.value();
                let input_path = Path::new(&input);
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
                let input = self.prompt.input.value();
                let input_path = Path::new(&input);
                let dest = AbsPath::new_unchecked(input_path.abs(current_item_parent));
                let cd = input.ends_with(std::path::MAIN_SEPARATOR);

                TASKS::spawn(async move {
                    match std::fs::create_dir_all(&dest) {
                        Ok(_) => {
                            TOAST::push(ToastStyle::Success, "New: ", [short_display(&dest)]);
                            if cd {
                                GLOBAL::send_action(FsAction::Jump(vec![dest.into()]));
                            }
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
                    return OverlayEffect::None;
                }
                let dest = AbsPath::new_unchecked(
                    auto_dest_for_src(
                        &old_path,
                        self.prompt.input.value(),
                        &RenamePolicy::default(),
                    )
                    .abs(old_path.parent().unwrap()),
                );

                if dest == old_path {
                    TOAST::push_skipped();
                } else {
                    TASKS::spawn(async move {
                        match rename(&old_path, &dest).await {
                            Ok(_) => {
                                let new_display = dest.to_string_lossy().to_string().into();
                                TOAST::pair(
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
            PromptKind::SetAlias => {
                let path = self.target_path();
                let alias = self.prompt.input.value();
                let pool = GLOBAL::db();
                let table = if path.is_dir() {
                    DbTable::dirs
                } else {
                    DbTable::files
                };

                pool.set_path_alias(path.clone(), alias.clone(), table);

                if alias.is_empty() {
                    TOAST::push(
                        ToastStyle::Normal,
                        "Alias cleared: ",
                        [short_display(&path)],
                    );
                } else {
                    TOAST::push(ToastStyle::Success, "Alias set: ", [short_display(&path)]);
                }
            }
        }

        self.prompt_kind = None;
        self.prompt.on_disable();
        OverlayEffect::Disable
    }
}
