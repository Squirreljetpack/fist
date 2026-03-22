#![allow(unstable_name_collisions)]

use std::{
    borrow::Cow,
    path::{Path, PathBuf},
};

use cba::bath::PathExt;
use matchmaker::nucleo::{Color, Line, Render, Span, Style, Text};

use crate::{
    abspath::AbsPath,
    cli::{env::EnvOpts, paths::__home},
    db::Entry,
    run::state::ui::global_ui,
};
use fist_types::{
    FileCategory,
    icons::{Icons, icon_for_file},
};

/// The basic item underyling a line in the matchmaker
///
/// Only created in [`crate::run::FsPane::populate`].
#[derive(Debug, Clone)]
pub struct PathItem {
    pub path: AbsPath,
    pub cmd: Option<String>,
    pub tail: Text<'static>,
    rendered: Text<'static>,
}

fn up_to_nth_ancestor(
    path: PathBuf,
    n: usize,
) -> PathBuf {
    path.ancestors()
        .nth(n)
        .unwrap_or_else(|| path.ancestors().last().unwrap())
        .to_path_buf()
}

impl PathItem {
    /// # Notes
    /// applies env.ancestor
    pub fn new(
        path: impl Into<PathBuf>,
        cwd: &Path,
    ) -> Self {
        let mut path_ = path.into().abs(cwd);
        if let Some(u) = EnvOpts::with_env(|s| s.ancestor) {
            path_ = up_to_nth_ancestor(path_, u)
        }

        let path = AbsPath::new_unchecked(path_);
        let rendered = render(&path, cwd);
        Self {
            path,
            rendered,
            cmd: None,
            tail: Text::from(""),
        }
    }

    pub fn new_app(entry: Entry) -> Self {
        let rendered = Text::from_iter([entry.name]);
        Self {
            path: entry.path,
            cmd: if entry.cmd.is_empty() {
                None
            } else {
                Some(entry.cmd.to_string_lossy().to_string())
            },
            rendered,
            tail: Text::from_iter([entry.alias]),
        }
    }

    // q: can compiler know to skip the initial render write
    pub fn override_rendered(
        &mut self,
        rendered: Text<'static>,
    ) -> &mut Self {
        self.rendered = rendered;
        self
    }

    /// cwd is used for reductions in rendering
    pub fn new_unchecked(
        path: PathBuf,
        cwd: &Path,
    ) -> Self {
        let rendered = render(&path, cwd);
        let path = AbsPath::new_unchecked(path);
        Self {
            path,
            rendered,
            cmd: None,
            tail: Text::from(""),
        }
    }

    // whats the generic way which supports writing utf-16?
    pub fn display(&self) -> Cow<'_, str> {
        self.path.to_string_lossy()
    }

    pub fn new_from_split(
        [first, tail]: [&str; 2],
        cwd: &Path,
    ) -> Self {
        let path = AbsPath::new_unchecked(first.abs(cwd));
        let rendered = render(&path, cwd);

        Self {
            path,
            rendered,
            cmd: None,
            tail: Text::from(tail.to_string()),
        }
    }

    pub fn _uninit() -> Self {
        PathItem::new_unchecked(
            crate::cli::paths::__cwd().into(),
            crate::cli::paths::__cwd(),
        )
    }

    // pub fn new_cwd(cwd: PathBuf) -> Self {
    //     Self {
    //         path: cwd,
    //         rendered: Text::from("."),
    //     }
    // }
}

// todo: just relative + replace home? what about icons/colors?
pub fn short_display(path: &Path) -> Span<'static> {
    let text = path.basename();
    if path.is_symlink() {
        Span::styled(text, Color::Green)
    } else if path.is_dir() {
        Span::styled(text, Color::Blue)
    } else {
        Span::styled(text, Color::White)
    }
}

fn render(
    mut path: &Path,
    cwd: &Path,
) -> Text<'static> {
    let full_path = path;
    let cfg = global_ui().path.clone();
    // let ft = FileType::get(path);

    if cfg.relative
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
                    let style = Style::default().fg(Color::Blue);
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
                    Style::default().fg(Color::Blue)
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

impl Render for PathItem {
    fn as_text(&self) -> Text<'_> {
        self.rendered.clone() // todo: is this clone cheap?
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
    use crate::run::state::ui::global_ui_init;
    use std::path::Path;

    // Helper to set the config for a test and then restore it.
    fn with_config<F>(
        config: PathDisplayConfig,
        test_fn: F,
    ) where
        F: FnOnce(),
    {
        let original_config = global_ui().clone();
        global_ui_init(StyleConfig {
            path: config,
            ..Default::default()
        });
        test_fn();
        global_ui_init(original_config);
    }

    // todo: test windows
    #[test]
    fn test_render_logical_paths() {
        // This test uses logical paths and does not interact with the filesystem.
        // As a result, `path.is_dir()` inside the `render` function will always
        // be false for the paths constructed here.

        let home = __home();
        let cwd = __cwd();

        // A logical path inside the current working directory.
        let path_in_cwd = cwd.join("src").join("main.rs");

        // A logical path inside the home directory.
        let path_in_home = home.join(".config").join("app.conf");

        // A logical absolute path outside of home or cwd.
        let absolute_path = Path::new("/var/log/syslog");

        // --- Test Cases ---

        // Test capabilities: relative=true, collapse_home=true, icons=true
        let config = PathDisplayConfig::DEFAULT;

        with_config(config, || {
            // Path in cwd should be relative
            eprintln!();
            let rendered = render(&path_in_cwd, cwd);
            eprintln!("{}", rendered);
            let icon = icon_for_file(&path_in_cwd);
            assert_eq!(rendered.to_string(), format!("{} src/main.rs", icon));

            // Path in home should be collapsed to ~
            let rendered = render(&path_in_home, cwd);
            eprintln!("{}", rendered);
            let icon = icon_for_file(&path_in_home);
            assert_eq!(rendered.to_string(), format!("{} ~/.config/app.conf", icon));

            // Non-collapsible path should be unchanged
            let rendered = render(absolute_path, cwd);
            eprintln!("{}", rendered);
            let icon = icon_for_file(absolute_path);
            assert_eq!(rendered.to_string(), format!("{} /var/log/syslog", icon));
        });

        // `collapse_home = relative = false`.
        // todo: test styles
        let config = PathDisplayConfig {
            collapse_home: false,
            relative: false,
            file_icons: false,
            dir_icons: false,
            file_colors: false,
            dir_colors: false,
            ..Default::default()
        };
        with_config(config, || {
            let rendered = render(&path_in_home, cwd);
            assert_eq!(
                rendered.to_string(),
                path_in_home.to_string_lossy().to_string()
            );

            let rendered = render(&path_in_cwd, cwd);
            assert_eq!(
                rendered.to_string(),
                path_in_cwd.to_string_lossy().to_string()
            );
        });

        // Test with styles disabled
        let config = PathDisplayConfig {
            relative: true,
            collapse_home: true,
            file_icons: false,
            dir_icons: false,
            file_colors: false,
            dir_colors: false,
            ..Default::default()
        };
        with_config(config, || {
            let rendered = render(&path_in_cwd, cwd);
            assert_eq!(rendered.to_string(), "src/main.rs");

            let rendered = render(home, cwd);
            assert_eq!(rendered.to_string(), "~");
        });
    }

    #[test]
    fn test_render_icon_colors() {
        let cwd = __cwd();
        let path_in_cwd = cwd.join("src").join("main.rs");

        // Case 1: file_colors=true, icon_colors=true
        let config = PathDisplayConfig {
            file_colors: true,
            icon_colors: true,
            file_icons: true,
            ..PathDisplayConfig::DEFAULT
        };
        with_config(config, || {
            let rendered = render(&path_in_cwd, cwd);
            // rendered should have 1 line, and that line should have 2 spans
            assert_eq!(rendered.lines.len(), 1);
            let line = &rendered.lines[0];
            assert_eq!(line.spans.len(), 2);

            // First span is the icon, should be styled
            assert!(line.spans[0].style.fg.is_some());

            // Second span is the path, should be raw (default style)
            assert!(line.spans[1].style.fg.is_none());
            assert_eq!(line.spans[1].content, " src/main.rs");
        });

        // Case 2: file_colors=true, icon_colors=false
        let config = PathDisplayConfig {
            file_colors: true,
            icon_colors: false,
            file_icons: true,
            ..PathDisplayConfig::DEFAULT
        };
        with_config(config, || {
            let rendered = render(&path_in_cwd, cwd);
            // rendered should have 1 line, and that line should have 1 span (the whole thing styled)
            assert_eq!(rendered.lines.len(), 1);
            let line = &rendered.lines[0];
            assert_eq!(line.spans.len(), 1);
            assert!(line.spans[0].style.fg.is_some());
        });
    }

    #[test]
    fn test_render_home_icon_colors() {
        let home = __home();
        let cwd = __cwd();

        // Case: collapse_home=true, dir_colors=true, icon_colors=true
        let config = PathDisplayConfig {
            collapse_home: true,
            dir_colors: true,
            icon_colors: true,
            dir_icons: true,
            ..PathDisplayConfig::DEFAULT
        };
        with_config(config, || {
            let rendered = render(home, cwd);
            // rendered should have 1 line, and that line should have 2 spans
            assert_eq!(rendered.lines.len(), 1);
            let line = &rendered.lines[0];
            assert_eq!(line.spans.len(), 2);

            // First span is the icon, should be styled
            assert!(line.spans[0].style.fg.is_some());
            assert_eq!(line.spans[0].content, Icons::HOME.to_string());

            // Second span is " ~", should be raw
            assert!(line.spans[1].style.fg.is_none());
            assert_eq!(line.spans[1].content, " ~");
        });
    }
}
