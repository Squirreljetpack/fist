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
    icons::{Icons, icon_for_file},
};

/// The basic item underyling a line in the matchmaker
///
/// Only created in [`crate::run::FsPane::populate`].
#[derive(Debug)]
pub struct PathItem {
    pub path: AbsPath,
    /// rg panes: (line << 32) | col   (top 32 = line, low 32 = col)
    /// sorted panes: the sort value (mtime/atime unix seconds, or size in bytes)
    /// default `u64::MAX` = unset (empty sort cell / col-3 guard, stage 4.2)
    /// the two never coexist — rg panes never hard-sort (stage 5.8)
    pub metadata: AtomicU64,
    /// column-2 text. `Ok([tail, override])`: plain strings — `[0]` is the tail (col 2),
    /// nonempty `[1]` is the col-1 display override (app name). `Err(Text)`: pre-rendered
    /// styled text (rg context blocks).
    pub tail: Result<[String; 2], Text<'static>>,
}

impl Clone for PathItem {
    fn clone(&self) -> Self {
        Self {
            path: self.path.clone(),
            metadata: AtomicU64::new(self.value()),
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
            metadata: AtomicU64::new(u64::MAX),
            tail: Ok([String::new(), String::new()]),
        }
    }

    pub fn new_app(entry: Entry) -> Self {
        Self {
            path: entry.path,
            metadata: AtomicU64::new(u64::MAX),
            tail: Ok([entry.alias, entry.name]),
        }
    }

    /// Construct from an already-absolute path.
    pub fn new_unchecked(path: PathBuf) -> Self {
        let path = AbsPath::new_unchecked(path);
        Self {
            path,
            metadata: AtomicU64::new(u64::MAX),
            tail: Ok([String::new(), String::new()]),
        }
    }

    pub fn render(&self) -> Text<'static> {
        render(&self.path, render_path().as_ref().map(|p| p.as_ref()))
    }

    pub fn tail_text(&self) -> Text<'static> {
        match &self.tail {
            Ok([s, _]) => Text::from(s.clone()),
            Err(t) => t.clone(),
        }
    }

    /// Col-1 sort/filter key: the `tail[1]` display override when set, else the raw path
    /// string (the unstyled equivalent of what `render()` shows — no icons/colors).
    pub fn display_name(&self) -> String {
        if let Ok([_, o]) = &self.tail {
            if !o.is_empty() {
                return o.clone();
            }
        }
        self.render().to_string()
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
        let v = ((line as u64) << 32) | (col as u64);
        self.metadata.store(v, Ordering::Relaxed);
    }

    pub fn loc(&self) -> (u32, u32) {
        let v = self.value();
        ((v >> 32) as u32, (v & 0xFFFF_FFFF) as u32)
    }

    pub fn set_value(
        &self,
        v: u64,
    ) {
        self.metadata.store(v, Ordering::Relaxed);
    }

    pub fn value(&self) -> u64 {
        self.metadata.load(Ordering::Relaxed)
    }
}

pub fn short_display(path: &Path) -> Span<'static> {
    let text = path.basename();
    if path.is_symlink() {
        Span::styled(text, Color::Green)
    } else if path.is_dir() {
        Span::styled(text, Color::LightBlue)
    } else {
        Span::styled(text, Color::White)
    }
}

fn render(
    path: &Path,
    cwd: Option<&Path>,
) -> Text<'static> {
    render_with(&global_ui().path, path, cwd)
}

/// Pure rendering logic, given the display config.
///
/// `cwd: None` (no render path — initial pane) disables relative display:
/// `path.relative` never triggers and paths render absolute.
fn render_with(
    cfg: &PathDisplayConfig,
    mut path: &Path,
    cwd: Option<&Path>,
) -> Text<'static> {
    let full_path = path;
    let relative = cfg.relative;

    if relative
        && let Some(cwd) = cwd
        && let Ok(stripped) = path.strip_prefix(cwd)
    {
        path = if stripped.is_empty() {
            Path::new(".")
        } else {
            stripped
        }
    }

    // collapse home to ~
    let path = if cfg.collapse_home
        && let Ok(stripped) = path.strip_prefix(__home())
    {
        if stripped.is_empty() {
            return {
                if cfg.dir_colors {
                    let style = Style::default().fg(Color::LightBlue);
                    if cfg.dir_icons && cfg.icon_colors {
                        Text::from(Line::from(vec![
                            Span::styled(Icons::HOME.to_string(), style),
                            Span::raw(" ~"),
                        ]))
                    } else {
                        let ret = if cfg.dir_icons {
                            format!("{} ~", Icons::HOME)
                        } else {
                            "~".to_string()
                        };
                        Text::from(Span::styled(ret, style))
                    }
                } else {
                    let ret = if cfg.dir_icons {
                        format!("{} ~", Icons::HOME)
                    } else {
                        "~".to_string()
                    };
                    Text::from(ret)
                }
            };
        } else {
            PathBuf::from("~").join(stripped)
        }
    } else {
        path.to_owned()
    };

    match full_path.is_dir() {
        true => {
            let icon = icon_for_file(full_path);
            let path_str = path.to_string_lossy();
            let style = if cfg.dir_colors {
                let style = if full_path.is_symlink() {
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
            let icon = icon_for_file(full_path);
            let path_str = path.to_string_lossy();
            let style = if cfg.file_colors {
                let mut style = FileCategory::get(&path)
                    .map(|c| cfg.file_styles.style(&c))
                    .unwrap_or_default();

                if full_path.is_symlink() {
                    style = if full_path.exists() {
                        style.fg(Color::LightCyan)
                    } else {
                        style.fg(Color::Red)
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
    use std::path::Path;
    use std::sync::Once;

    /// `render()`/`display_name()` read the global UI config; initialize it once
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
        let rendered = render_with(&config, &path_in_cwd, Some(cwd));
        eprintln!("{}", rendered);
        let icon = icon_for_file(&path_in_cwd);
        assert_eq!(rendered.to_string(), format!("{} src/main.rs", icon));

        let rendered = render_with(&config, &path_in_home, Some(cwd));
        eprintln!("{}", rendered);
        let icon = icon_for_file(&path_in_home);
        assert_eq!(rendered.to_string(), format!("{} ~/.config/app.conf", icon));

        let rendered = render_with(&config, absolute_path, Some(cwd));
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
        let rendered = render_with(&config, &path_in_home, Some(cwd));
        assert_eq!(
            rendered.to_string(),
            path_in_home.to_string_lossy().to_string()
        );

        let rendered = render_with(&config, &path_in_cwd, Some(cwd));
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
        let rendered = render_with(&config, &path_in_cwd, Some(cwd));
        assert_eq!(rendered.to_string(), "src/main.rs");

        let rendered = render_with(&config, home, Some(cwd));
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
        let rendered = render_with(&config, &path_in_cwd, Some(cwd));
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
        let rendered = render_with(&config, &path_in_cwd, Some(cwd));
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
        let rendered = render_with(&config, home, Some(cwd));
        assert_eq!(rendered.lines.len(), 1);
        let line = &rendered.lines[0];
        assert_eq!(line.spans.len(), 2);
        assert!(line.spans[0].style.fg.is_some());
        assert_eq!(line.spans[0].content, Icons::HOME.to_string());
        assert!(line.spans[1].style.fg.is_none());
        assert_eq!(line.spans[1].content, " ~");
    }

    #[test]
    fn test_display_name_relative() {
        init_global_ui_once();
        let cwd = __cwd();
        // render() reads the global render path (set per populate) — point it
        // at the cwd so relative display is exercised
        set_render_path(Some(AbsPath::new_unchecked(cwd)));
        let path_in_cwd = cwd.join(".cargo");
        let item = PathItem::new(path_in_cwd, cwd);
        assert_eq!(item.display_name(), item.render().to_string());
        assert!(
            !item
                .display_name()
                .contains(&cwd.to_string_lossy().to_string())
        );
    }
}
