use matchmaker::nucleo::{ColumnIndexable, Injector, WorkerInjector};
use ratatui::prelude::*;

use crate::{
    abspath::AbsPath,
    menu::{MenuActions, MenuEvaluationContext, MenuSnapshot},
    run::{
        action::FsAction,
        item::short_display,
        queue::QUEUE,
        state::{GLOBAL, GLOBAL::db, MenuPrompt, PromptKind, TOAST, ToastStyle, lessfilter_cfg},
    },
    spawn::open_wrapped,
};

use super::execute::{extract_prompt_for, rename_prompt_for};

/// Width reserved for a menu item label when sizing the menu to its content.
const MAX_ITEM_WIDTH: u16 = 10;

/// A menu item shown in the menu overlay.
#[derive(Clone, PartialEq, Eq)]
pub enum MenuItem {
    New,
    NewFolder,
    Rename,
    Move,
    Copy,
    Symlink,
    Jump,
    Trash,
    Delete,
    Open,
    OpenWith,
    Extract,
    Unmount,
    Mounts,
    Execute,
    Custom {
        name: String,
        action: String,
        alias: Option<String>,
    },
}

/// What running a [`MenuItem`] asks the overlay to do.
#[derive(Debug)]
pub enum MenuOutcome {
    /// Open the input bar with this prompt; cancelling returns to the menu.
    Prompt(MenuPrompt),
    /// Nothing to show: a toast already reported the refusal.
    Stay,
    /// The work was dispatched or queued; close the menu.
    Close,
}

impl MenuItem {
    /// Column 0: the item name.
    pub fn label(&self) -> &str {
        match self {
            MenuItem::New => "new",
            MenuItem::NewFolder => "new folder",
            MenuItem::Rename => "rename",
            MenuItem::Move => "move",
            MenuItem::Copy => "copy",
            MenuItem::Symlink => "symlink",
            MenuItem::Trash => "trash",
            MenuItem::Jump => "jump",
            MenuItem::Delete => "delete",
            MenuItem::Open => "open",
            MenuItem::OpenWith => "open with",
            MenuItem::Extract => "extract",
            MenuItem::Unmount => "unmount",
            MenuItem::Mounts => "show mounts",
            MenuItem::Execute => "execute",
            MenuItem::Custom { name, .. } => name,
        }
    }

    /// Column 1: the alias that triggers the item when typed exactly.
    pub fn alias(&self) -> Option<&str> {
        match self {
            MenuItem::New => Some("N"),
            MenuItem::NewFolder => Some("F"),
            MenuItem::Rename => Some("R"),
            MenuItem::Move => Some("M"),
            MenuItem::Copy => Some("C"),
            MenuItem::Trash => Some("T"),
            MenuItem::Delete => Some("D"),
            MenuItem::Open => Some("O"),
            MenuItem::OpenWith => Some("W"),
            MenuItem::Extract => Some("E"),
            MenuItem::Unmount => None,
            MenuItem::Mounts => None,
            MenuItem::Execute => Some("X"),
            MenuItem::Symlink => None,
            MenuItem::Jump => Some("J"),
            MenuItem::Custom { alias, .. } => alias.as_deref(),
        }
    }

    /// Whether this builtin item applies to `snapshot`, returning an error
    /// message when a precondition no longer holds. Custom items are evaluated
    /// via [`MenuEvaluationContext::is_applicable`].
    pub(super) fn check(
        &self,
        snapshot: &MenuSnapshot,
    ) -> Result<(), &'static str> {
        match self {
            MenuItem::Extract => {
                if snapshot.selected.is_empty()
                    && !snapshot.cursor_disabled
                    && snapshot
                        .cursor
                        .as_ref()
                        .is_some_and(|p| crate::unzip::supported(p.as_path()))
                {
                    Ok(())
                } else {
                    Err("Not a supported archive")
                }
            }
            MenuItem::Unmount => {
                if snapshot
                    .cursor
                    .as_ref()
                    .is_some_and(|p| crate::mount::unmountable(p.as_path()).is_some())
                {
                    Ok(())
                } else {
                    Err("Not inside a mounted archive")
                }
            }
            MenuItem::Mounts => {
                if !crate::mount::MountRegistry::is_empty() {
                    Ok(())
                } else {
                    Err("No active mounts")
                }
            }
            _ => Ok(()),
        }
    }

    /// Execute the item on `path`, the current cursor item.
    ///
    /// `targets` is the current selection, or `path` when nothing is selected;
    /// batch actions use it so they follow the selections-over-cursor rule.
    pub fn action(
        &self,
        path: AbsPath,
        targets: Vec<AbsPath>,
    ) -> MenuOutcome {
        match self {
            MenuItem::New => MenuOutcome::Prompt(MenuPrompt::new(PromptKind::New)),
            MenuItem::NewFolder => MenuOutcome::Prompt(MenuPrompt::new(PromptKind::NewFolder)),
            MenuItem::Rename => MenuOutcome::Prompt(rename_prompt_for(&path)),
            MenuItem::Move => {
                TOAST::SUSTAINED.push(
                    ToastStyle::Normal,
                    "Move",
                    targets.iter().map(|p| short_display(p)),
                );
                QUEUE::enqueue("move".into(), targets);
                MenuOutcome::Close
            }
            MenuItem::Copy => {
                TOAST::SUSTAINED.push(
                    ToastStyle::Normal,
                    "Copied",
                    targets.iter().map(|p| short_display(p)),
                );
                QUEUE::enqueue("copy".into(), targets);
                MenuOutcome::Close
            }
            MenuItem::Symlink => {
                TOAST::push(
                    ToastStyle::Normal,
                    "Queued symlink",
                    targets.iter().map(|p| short_display(p)),
                );
                QUEUE::enqueue("symlink".into(), targets);
                MenuOutcome::Close
            }
            MenuItem::Jump => MenuOutcome::Prompt(MenuPrompt::new(PromptKind::Jump)),
            MenuItem::Trash => {
                // the menu skips the confirmation: selection-aware like the
                // bound Trash action, but immediate
                GLOBAL::send_action(FsAction::Trash(true));
                MenuOutcome::Close
            }
            MenuItem::Delete => {
                GLOBAL::send_action(FsAction::Delete(false));
                MenuOutcome::Close
            }
            MenuItem::Open => {
                TOAST::push(ToastStyle::Normal, "Opened", [short_display(&path)]);
                let path_clone = path;
                tokio::spawn(async move {
                    let conn = db().get_conn(crate::db::DbTable::dirs).await?;
                    open_wrapped(Some(conn), None, &[path_clone.inner().into()], true).await?;
                    anyhow::Ok(())
                });
                MenuOutcome::Close
            }
            MenuItem::Extract => MenuOutcome::Prompt(extract_prompt_for(&path)),
            MenuItem::Unmount => {
                if let Some(mount_point) = crate::mount::unmountable(path.as_path()) {
                    GLOBAL::send_action(FsAction::Unmount(mount_point));
                    MenuOutcome::Close
                } else {
                    TOAST::notice(ToastStyle::Error, "Not inside a mounted archive");
                    MenuOutcome::Stay
                }
            }
            MenuItem::Mounts => {
                // queued so it is handled after the menu has closed; the
                // mounts picker itself disables on its own action
                GLOBAL::send_action(FsAction::ShowMounts);
                MenuOutcome::Close
            }
            MenuItem::Execute => MenuOutcome::Prompt(MenuPrompt::new(PromptKind::Execute)),
            MenuItem::OpenWith => {
                unreachable!("OpenWith is handled in MenuOverlay::execute")
            }
            MenuItem::Custom { .. } => {
                unreachable!("custom items are routed through MenuOverlay::run_custom")
            }
        }
    }
}

/// A menu item with its alias resolved at build time: builtins carry their
/// default alias unless `MenuConfig::no_default_aliases` is set, custom
/// actions carry their configured alias.
#[derive(Clone, PartialEq, Eq)]
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

const MENU_ITEMS: [MenuItem; 15] = [
    MenuItem::New,
    MenuItem::Rename,
    MenuItem::Move,
    MenuItem::Copy,
    MenuItem::Trash,
    MenuItem::Delete,
    MenuItem::Open,
    MenuItem::OpenWith,
    MenuItem::Extract,
    MenuItem::Unmount,
    MenuItem::Mounts,
    MenuItem::NewFolder,
    MenuItem::Symlink,
    MenuItem::Jump,
    MenuItem::Execute,
];

fn builtin_items(compact: bool) -> impl Iterator<Item = MenuItem> {
    MENU_ITEMS.into_iter().filter(move |item| {
        !compact
            || !matches!(
                item,
                MenuItem::Move | MenuItem::Copy | MenuItem::Open | MenuItem::Trash
            )
    })
}

fn resolve_entry(
    item: MenuItem,
    no_default_aliases: bool,
) -> MenuEntry {
    let alias = if no_default_aliases && !matches!(item, MenuItem::Custom { .. }) {
        None
    } else {
        item.alias().map(str::to_string)
    };
    MenuEntry { item, alias }
}

/// Compute the inner content width needed to display all configured menu items
/// unconditionally, used when `MenuConfig::max_width == 0`.
pub(super) fn compute_max_width(
    actions: &MenuActions,
    compact: bool,
    no_default_aliases: bool,
    column_spacing: u16,
) -> u16 {
    let builtins = builtin_items(compact).map(|item| resolve_entry(item, no_default_aliases));
    let custom = actions.iter().map(|(key, action)| {
        resolve_entry(
            MenuItem::Custom {
                name: action.display_name(key),
                action: key.clone(),
                alias: action.alias.clone(),
            },
            no_default_aliases,
        )
    });
    let max_item_width = builtins
        .chain(custom)
        .map(|entry| Text::from(entry.hotkey_label()).width() as u16)
        .max()
        .unwrap_or(MAX_ITEM_WIDTH)
        .max(MAX_ITEM_WIDTH);
    max_item_width + column_spacing
}

/// Evaluate builtin and custom menu items against `snapshot` off the render
/// thread, streaming passing entries directly into `injector`. Exits early
/// once the owning worker is restarted or dropped.
pub(super) fn stream_menu_items(
    snapshot: MenuSnapshot,
    actions: &MenuActions,
    compact: bool,
    no_default_aliases: bool,
    injector: WorkerInjector<MenuEntry, ()>,
) {
    if injector.is_closed() {
        return;
    }

    let lcfg = lessfilter_cfg();
    let mut ctx = MenuEvaluationContext::from_snapshot(snapshot, lcfg);

    for item in builtin_items(compact) {
        if injector.is_closed() {
            return;
        }
        if item.check(&ctx.snapshot).is_ok()
            && injector
                .push(resolve_entry(item, no_default_aliases))
                .is_err()
        {
            return;
        }
    }

    for (key, action) in actions.iter() {
        if injector.is_closed() {
            return;
        }
        if !ctx.is_applicable(action) {
            continue;
        }
        let item = MenuItem::Custom {
            name: action.display_name(key),
            action: key.clone(),
            alias: action.alias.clone(),
        };
        if injector
            .push(resolve_entry(item, no_default_aliases))
            .is_err()
        {
            return;
        }
    }
}
