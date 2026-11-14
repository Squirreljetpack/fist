#![allow(unstable_name_collisions)]

use std::{
    borrow::Cow,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};

use cba::bath::PathExt;
use matchmaker::nucleo::{Color, Line, Span, Style, Text};

use crate::{
    abspath::AbsPath,
    cli::paths::__home,
    config::ui::PathDisplayConfig,
    db::Entry,
    run::state::{render_path, ui::global_ui},
};
use fist_types::{
    FileCategory,
    icons::{Icons, icon_for_entry},
};

/// Cached filesystem kind for a [`PathItem`], stored in the top byte of [`ItemMeta`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum EntryKind {
    Unknown = 0,
    File = 1,
    Dir = 2,
    SymlinkToFile = 3,
    SymlinkToDir = 4,
    BrokenSymlink = 5,
}

impl EntryKind {
    fn from_bits(bits: u8) -> Self {
        match bits {
            1 => Self::File,
            2 => Self::Dir,
            3 => Self::SymlinkToFile,
            4 => Self::SymlinkToDir,
            5 => Self::BrokenSymlink,
            _ => Self::Unknown,
        }
    }

    /// Resolve the entry kind from filesystem metadata in at most one `lstat`
    /// (plus one `stat` when the entry is a symlink).
    pub fn probe(path: &Path) -> Self {
        let Ok(sym) = std::fs::symlink_metadata(path) else {
            return Self::File;
        };
        if sym.file_type().is_symlink() {
            match std::fs::metadata(path) {
                Ok(target) if target.is_dir() => Self::SymlinkToDir,
                Ok(_) => Self::SymlinkToFile,
                Err(_) => Self::BrokenSymlink,
            }
        } else if sym.is_dir() {
            Self::Dir
        } else {
            Self::File
        }
    }

    pub fn is_known(self) -> bool {
        self != Self::Unknown
    }

    pub fn is_dir(self) -> bool {
        matches!(self, Self::Dir | Self::SymlinkToDir)
    }

    pub fn is_symlink(self) -> bool {
        matches!(
            self,
            Self::SymlinkToFile | Self::SymlinkToDir | Self::BrokenSymlink
        )
    }

    pub fn is_broken_symlink(self) -> bool {
        self == Self::BrokenSymlink
    }
}

/// Packed atomic metadata for a [`PathItem`].
///
/// Reserves the top 8 bits (`56..63`) for render/status flags ([`EntryKind`] in
/// bits `56..58`, with bits `59..63` reserved) and uses the lower 56 bits
/// (`0..55`) for the mutually exclusive payload (sort timestamp or `rg` location).
#[derive(Debug)]
pub struct ItemMeta(AtomicU64);

impl ItemMeta {
    const FLAGS_SHIFT: u32 = 56;
    const FLAGS_MASK: u64 = 0xFF << Self::FLAGS_SHIFT;
    const PAYLOAD_MASK: u64 = !Self::FLAGS_MASK;
    const PAYLOAD_UNSET: u64 = Self::PAYLOAD_MASK;

    const KIND_MASK: u8 = 0b0000_0111;

    const COL_BITS: u32 = 24;
    const COL_MASK: u64 = (1 << Self::COL_BITS) - 1;

    pub fn new() -> Self {
        Self(AtomicU64::new(Self::PAYLOAD_UNSET))
    }

    fn set_payload(
        &self,
        payload: u64,
    ) {
        let payload = payload.min(Self::PAYLOAD_UNSET - 1);
        let _ = self
            .0
            .try_update(Ordering::Relaxed, Ordering::Relaxed, |cur| {
                Some((cur & Self::FLAGS_MASK) | payload)
            });
    }

    fn update_flags(
        &self,
        clear_byte_mask: u8,
        set_byte_bits: u8,
    ) {
        let clear = (clear_byte_mask as u64) << Self::FLAGS_SHIFT;
        let set = ((set_byte_bits & clear_byte_mask) as u64) << Self::FLAGS_SHIFT;
        let _ = self
            .0
            .try_update(Ordering::Relaxed, Ordering::Relaxed, |cur| {
                Some((cur & !clear) | set)
            });
    }

    pub fn value(&self) -> Option<u64> {
        let payload = self.0.load(Ordering::Relaxed) & Self::PAYLOAD_MASK;
        (payload != Self::PAYLOAD_UNSET).then_some(payload)
    }

    pub fn set_value(
        &self,
        v: u64,
    ) {
        self.set_payload(v);
    }

    pub fn loc(&self) -> Option<(u32, u32)> {
        let payload = self.0.load(Ordering::Relaxed) & Self::PAYLOAD_MASK;
        if payload == Self::PAYLOAD_UNSET {
            return None;
        }
        Some((
            (payload >> Self::COL_BITS) as u32,
            (payload & Self::COL_MASK) as u32,
        ))
    }

    pub fn set_loc(
        &self,
        line: u32,
        col: u32,
    ) {
        let packed = ((line as u64) << Self::COL_BITS) | (col as u64).min(Self::COL_MASK);
        self.set_payload(packed);
    }

    pub fn kind(&self) -> EntryKind {
        let flags = (self.0.load(Ordering::Relaxed) >> Self::FLAGS_SHIFT) as u8;
        EntryKind::from_bits(flags & Self::KIND_MASK)
    }

    pub fn set_kind(
        &self,
        kind: EntryKind,
    ) {
        self.update_flags(Self::KIND_MASK, kind as u8);
    }
}

impl Clone for ItemMeta {
    fn clone(&self) -> Self {
        Self(AtomicU64::new(self.0.load(Ordering::Relaxed)))
    }
}

/// The basic item underyling a line in the matchmaker
///
/// Only created in [`crate::run::FsPane::populate`].
#[derive(Debug)]
pub struct PathItem {
    pub path: AbsPath,
    /// Packed sort/`rg` payload + cached entry-kind flags.
    metadata: ItemMeta,
    /// column-2 text. `Ok([tail, override])`: plain strings — `[0]` is the tail (col 2),
    /// nonempty `[1]` is the col-1 display override (app name). `Err(Text)`: pre-rendered
    /// styled text (rg context blocks).
    pub tail: Result<[String; 2], Text<'static>>,
}

impl Clone for PathItem {
    fn clone(&self) -> Self {
        Self {
            path: self.path.clone(),
            metadata: self.metadata.clone(),
            tail: match &self.tail {
                Ok([a, b]) => Ok([a.clone(), b.clone()]),
                Err(t) => Err(t.clone()),
            },
        }
    }
}

impl PathItem {
    pub fn new(
        path: impl Into<PathBuf>,
        cwd: &Path,
    ) -> Self {
        let path_ = path.into().abs(cwd);
        let path = AbsPath::new_unchecked(path_);
        Self {
            path,
            metadata: ItemMeta::new(),
            tail: Ok([String::new(), String::new()]),
        }
    }

    pub fn new_app(entry: Entry) -> Self {
        Self {
            path: entry.path,
            metadata: ItemMeta::new(),
            tail: Ok([entry.alias, entry.name]),
        }
    }

    /// Construct from an already-absolute path.
    pub fn new_unchecked(path: PathBuf) -> Self {
        let path = AbsPath::new_unchecked(path);
        Self {
            path,
            metadata: ItemMeta::new(),
            tail: Ok([String::new(), String::new()]),
        }
    }

    pub fn render(&self) -> Text<'static> {
        let mut kind = self.kind();
        if !kind.is_known() {
            kind = EntryKind::probe(&self.path.inner());
            self.set_kind(kind);
        }
        render(&self.path, render_path().as_ref().map(|p| p.as_ref()), kind)
    }

    pub fn tail_text(&self) -> Text<'static> {
        match &self.tail {
            Ok([s, _]) => Text::from(s.clone()),
            Err(t) => t.clone(),
        }
    }

    /// Plain-text tail value (column 2), the tail without the col-1 display
    /// override. rg context blocks are flattened to their rendered text.
    pub fn tail_raw(&self) -> Cow<'_, str> {
        match &self.tail {
            Ok([s, _]) => Cow::Borrowed(s.as_str()),
            Err(t) => Cow::Owned(t.to_string()),
        }
    }

    /// Col-1 match/filter key: the `tail[1]` display override when set, else the
    /// clean, icon-free path string shown by [`Self::render`].
    pub fn match_name(&self) -> Cow<'_, str> {
        if let Ok([_, o]) = &self.tail {
            if !o.is_empty() {
                return Cow::Borrowed(o.as_str());
            }
        }

        let cwd = render_path();
        match display_path(
            &global_ui().path,
            &self.path,
            cwd.as_ref().map(|p| p.as_ref()),
        ) {
            Cow::Borrowed(path) => path.to_string_lossy(),
            Cow::Owned(path) => Cow::Owned(path.to_string_lossy().into_owned()),
        }
    }

    /// Col-1 sort key: tries the `tail[1]` display override first when non-empty,
    /// else returns `self.path.to_string_lossy()`.
    pub fn sort_name(&self) -> Cow<'_, str> {
        if let Ok([_, o]) = &self.tail {
            if !o.is_empty() {
                return Cow::Borrowed(o.as_str());
            }
        }
        self.path.to_string_lossy()
    }

    pub fn set_loc(
        &self,
        line: u32,
        col: u32,
    ) {
        self.metadata.set_loc(line, col);
    }

    pub fn loc(&self) -> Option<(u32, u32)> {
        self.metadata.loc()
    }

    pub fn set_value(
        &self,
        v: u64,
    ) {
        self.metadata.set_value(v);
    }

    pub fn value(&self) -> Option<u64> {
        self.metadata.value()
    }

    pub fn kind(&self) -> EntryKind {
        self.metadata.kind()
    }

    pub fn set_kind(
        &self,
        kind: EntryKind,
    ) {
        self.metadata.set_kind(kind);
    }
}

pub fn short_display(path: &Path) -> Span<'static> {
    let text = path.basename();
    if path.is_symlink() {
        Span::styled(text, Color::LightCyan)
    } else if path.is_dir() {
        Span::styled(text, Color::LightBlue)
    } else {
        Span::styled(text, Color::White)
    }
}

fn render(
    path: &Path,
    cwd: Option<&Path>,
    kind: EntryKind,
) -> Text<'static> {
    render_with(&global_ui().path, path, cwd, kind)
}

/// The clean path shown in the UI and scored by the matcher, before
/// icons/colors are added.
fn display_path<'a>(
    cfg: &PathDisplayConfig,
    mut path: &'a Path,
    cwd: Option<&Path>,
) -> Cow<'a, Path> {
    if cfg.relative
        && let Some(cwd) = cwd
        && let Ok(stripped) = path.strip_prefix(cwd)
    {
        path = if stripped.is_empty() {
            Path::new(".")
        } else {
            stripped
        }
    }

    if cfg.collapse_home
        && let Ok(stripped) = path.strip_prefix(__home())
    {
        return if stripped.is_empty() {
            Cow::Borrowed(Path::new("~"))
        } else {
            Cow::Owned(PathBuf::from("~").join(stripped))
        };
    }

    Cow::Borrowed(path)
}

/// Pure rendering logic, given the display config.
///
/// `cwd: None` (no render path — initial pane) disables relative display:
/// `path.relative` never triggers and paths render absolute.
fn render_with(
    cfg: &PathDisplayConfig,
    path: &Path,
    cwd: Option<&Path>,
    kind: EntryKind,
) -> Text<'static> {
    let full_path = path;
    let path = display_path(cfg, path, cwd);

    match kind.is_dir() {
        true => {
            let icon = if path.as_ref() == Path::new("~") {
                Icons::HOME
            } else {
                icon_for_entry(full_path, true)
            };
            let path_str = path.to_string_lossy();
            let style = if cfg.dir_colors {
                let style = if kind.is_symlink() {
                    Style::default().fg(Color::LightCyan)
                } else {
                    Style::default().fg(Color::LightBlue)
                };
                Some(style)
            } else {
                None
            };

            match style {
                Some(style) => {
                    if cfg.dir_icons && cfg.icon_colors {
                        Text::from(Line::from(vec![
                            Span::styled(icon.to_string(), style),
                            Span::raw(format!(" {}", path_str)),
                        ]))
                    } else {
                        let content = if cfg.dir_icons {
                            format!("{} {}", icon, path_str)
                        } else {
                            path_str.to_string()
                        };
                        Text::from(Span::styled(content, style))
                    }
                }
                None => {
                    let content = if cfg.dir_icons {
                        format!("{} {}", icon, path_str)
                    } else {
                        path_str.to_string()
                    };
                    Text::from(content)
                }
            }
        }
        _ => {
            let icon = icon_for_entry(full_path, false);
            let path_str = path.to_string_lossy();
            let style = if cfg.file_colors {
                let mut style = FileCategory::get(path.as_ref())
                    .map(|c| cfg.file_styles.style(&c))
                    .unwrap_or_default();

                if kind.is_symlink() {
                    style = if kind.is_broken_symlink() {
                        style.fg(Color::Red)
                    } else {
                        style.fg(Color::LightCyan)
                    }
                }
                Some(style)
            } else {
                None
            };

            match style {
                Some(style) => {
                    if cfg.file_icons && cfg.icon_colors {
                        Text::from(Line::from(vec![
                            Span::styled(icon.to_string(), style),
                            Span::raw(format!(" {}", path_str)),
                        ]))
                    } else {
                        let content = if cfg.file_icons {
                            format!("{} {}", icon, path_str)
                        } else {
                            path_str.to_string()
                        };
                        Text::from(Span::styled(content, style))
                    }
                }
                None => {
                    let content = if cfg.file_icons {
                        format!("{} {}", icon, path_str)
                    } else {
                        path_str.to_string()
                    };
                    Text::from(content)
                }
            }
        }
    }
}

impl PartialEq for PathItem {
    fn eq(
        &self,
        other: &Self,
    ) -> bool {
        self.path == other.path
    }
}

impl Eq for PathItem {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cli::paths::{__cwd, __home};
    use crate::config::ui::{PathDisplayConfig, StyleConfig};
    use crate::run::state::set_render_path;
    use crate::run::state::ui::global_ui_init;
    use fist_types::icons::icon_for_file;
    use std::path::Path;
    use std::sync::Once;

    /// `render()`/`match_name()` read the global UI config; initialize it once
    /// (OnceLock set is one-shot — per-test swaps are impossible).
    static INIT_GLOBAL_UI: Once = Once::new();
    fn init_global_ui_once() {
        INIT_GLOBAL_UI.call_once(|| global_ui_init(StyleConfig::DEFAULT));
    }

    #[test]
    fn test_render_logical_paths() {
        let home = __home();
        let cwd = __cwd();

        let path_in_cwd = cwd.join("src").join("main.rs");
        let path_in_home = home.join(".config").join("app.conf");
        let absolute_path = Path::new("/var/log/syslog");

        let config = PathDisplayConfig::DEFAULT;

        eprintln!();
        let rendered = render_with(
            &config,
            &path_in_cwd,
            Some(cwd),
            EntryKind::probe(&path_in_cwd),
        );
        eprintln!("{}", rendered);
        let icon = icon_for_file(&path_in_cwd);
        assert_eq!(rendered.to_string(), format!("{} src/main.rs", icon));

        let rendered = render_with(
            &config,
            &path_in_home,
            Some(cwd),
            EntryKind::probe(&path_in_home),
        );
        eprintln!("{}", rendered);
        let icon = icon_for_file(&path_in_home);
        assert_eq!(rendered.to_string(), format!("{} ~/.config/app.conf", icon));

        let rendered = render_with(
            &config,
            absolute_path,
            Some(cwd),
            EntryKind::probe(absolute_path),
        );
        eprintln!("{}", rendered);
        let icon = icon_for_file(absolute_path);
        assert_eq!(rendered.to_string(), format!("{} /var/log/syslog", icon));

        let config = PathDisplayConfig {
            collapse_home: false,
            relative: false,
            file_icons: false,
            dir_icons: false,
            file_colors: false,
            dir_colors: false,
            ..Default::default()
        };
        let rendered = render_with(
            &config,
            &path_in_home,
            Some(cwd),
            EntryKind::probe(&path_in_home),
        );
        assert_eq!(
            rendered.to_string(),
            path_in_home.to_string_lossy().to_string()
        );

        let rendered = render_with(
            &config,
            &path_in_cwd,
            Some(cwd),
            EntryKind::probe(&path_in_cwd),
        );
        assert_eq!(
            rendered.to_string(),
            path_in_cwd.to_string_lossy().to_string()
        );

        let config = PathDisplayConfig {
            relative: true,
            collapse_home: true,
            file_icons: false,
            dir_icons: false,
            file_colors: false,
            dir_colors: false,
            ..Default::default()
        };
        let rendered = render_with(
            &config,
            &path_in_cwd,
            Some(cwd),
            EntryKind::probe(&path_in_cwd),
        );
        assert_eq!(rendered.to_string(), "src/main.rs");

        let rendered = render_with(&config, home, Some(cwd), EntryKind::probe(home));
        assert_eq!(rendered.to_string(), "~");
    }

    #[test]
    fn test_render_icon_colors() {
        let cwd = __cwd();
        let path_in_cwd = cwd.join("src").join("main.rs");

        let config = PathDisplayConfig {
            file_colors: true,
            icon_colors: true,
            file_icons: true,
            ..PathDisplayConfig::DEFAULT
        };
        let rendered = render_with(
            &config,
            &path_in_cwd,
            Some(cwd),
            EntryKind::probe(&path_in_cwd),
        );
        assert_eq!(rendered.lines.len(), 1);
        let line = &rendered.lines[0];
        assert_eq!(line.spans.len(), 2);
        assert!(line.spans[0].style.fg.is_some());
        assert!(line.spans[1].style.fg.is_none());
        assert_eq!(line.spans[1].content, " src/main.rs");

        let config = PathDisplayConfig {
            file_colors: true,
            icon_colors: false,
            file_icons: true,
            ..PathDisplayConfig::DEFAULT
        };
        let rendered = render_with(
            &config,
            &path_in_cwd,
            Some(cwd),
            EntryKind::probe(&path_in_cwd),
        );
        assert_eq!(rendered.lines.len(), 1);
        let line = &rendered.lines[0];
        assert_eq!(line.spans.len(), 1);
        assert!(line.spans[0].style.fg.is_some());
    }

    #[test]
    fn test_render_home_icon_colors() {
        let home = __home();
        let cwd = __cwd();

        let config = PathDisplayConfig {
            collapse_home: true,
            dir_colors: true,
            icon_colors: true,
            dir_icons: true,
            ..PathDisplayConfig::DEFAULT
        };
        let rendered = render_with(&config, home, Some(cwd), EntryKind::probe(home));
        assert_eq!(rendered.lines.len(), 1);
        let line = &rendered.lines[0];
        assert_eq!(line.spans.len(), 2);
        assert!(line.spans[0].style.fg.is_some());
        assert_eq!(line.spans[0].content, Icons::HOME.to_string());
        assert!(line.spans[1].style.fg.is_none());
        assert_eq!(line.spans[1].content, " ~");
    }

    #[test]
    fn test_match_name_relative() {
        init_global_ui_once();
        let cwd = __cwd();
        // render() reads the global render path (set per populate) — point it
        // at the cwd so relative display is exercised
        set_render_path(Some(AbsPath::new_unchecked(cwd)));
        let path_in_cwd = cwd.join(".cargo");
        let item = PathItem::new(path_in_cwd, cwd);
        assert_eq!(item.match_name(), ".cargo");
        assert!(
            item.render()
                .to_string()
                .ends_with(item.match_name().as_ref())
        );
        assert!(
            !item
                .match_name()
                .contains(&cwd.to_string_lossy().to_string())
        );
    }

    #[test]
    fn test_item_meta_preserves_flags_and_payload() {
        let meta = ItemMeta::new();
        assert_eq!(meta.value(), None);
        assert_eq!(meta.loc(), None);
        assert_eq!(meta.kind(), EntryKind::Unknown);

        meta.set_kind(EntryKind::SymlinkToDir);
        meta.set_value(1_700_000_000);
        assert_eq!(meta.kind(), EntryKind::SymlinkToDir);
        assert_eq!(meta.value(), Some(1_700_000_000));

        meta.set_kind(EntryKind::BrokenSymlink);
        assert_eq!(meta.value(), Some(1_700_000_000));

        meta.set_loc(42, 10);
        assert_eq!(meta.kind(), EntryKind::BrokenSymlink);
        assert_eq!(meta.loc(), Some((42, 10)));

        // Saturated loc still stays distinct from unset sentinel.
        meta.set_loc(u32::MAX, u32::MAX);
        assert!(meta.loc().is_some());
        assert_eq!(meta.kind(), EntryKind::BrokenSymlink);
    }
}
