use crate::{
    abspath::AbsPath,
    fs::{auto_dest, create_all, rename},
    run::{
        item::{PathItem, short_display},
        state::{TASKS, TOAST},
    },
    utils::text::ToastStyle,
};

use cba::bath::{PathExt, RenamePolicy, auto_dest_for_src};
use matchmaker::ui::{Overlay, OverlayEffect};
use std::path::Path;

use super::menu_overlay::MenuOverlay;
use MenuTarget::*;

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

impl MenuOverlay {
    pub fn target_path(&self) -> AbsPath {
        match &self.target {
            Item(p) => p.path.clone(),
            Cwd(p) => p.clone(),
        }
    }
    pub fn target_parent(&self) -> AbsPath {
        match &self.target {
            Item(p) => p.path._parent(),
            Cwd(p) => p.clone(),
        }
    }

    pub fn on_prompt_accept(
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
                    return OverlayEffect::None;
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
}
