use std::{
    borrow::Cow,
    path::{Path, PathBuf},
};

use cli_boilerplate_automation::bath::PathExt;
use matchmaker::nucleo::{Color, Render, Span, Style, Text};

use crate::{
    abspath::AbsPath,
    cli::paths::__home,
    db::Entry,
    run::state::GLOBAL,
    ui::global::global_ui,
    utils::{
        categories::FileCategory,
        icons::{Icons, icon_for_file},
    },
};

/// The basic item underyling a line in the matchmaker
#[derive(Debug, Clone)]
pub struct PathItem {
    /// all components are [`Component::Normal`]
    /// Absolute
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
    /// only use in fspane: applies env.ancestor
    pub fn new(
        path: impl Into<PathBuf>,
        cwd: &Path,
    ) -> Self {
        let mut path_ = path.into().abs(cwd);
        if let Some(u) = GLOBAL::with_env(|s| s.ancestor) {
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
        let rendered = Text::from(entry.name);
        Self {
            path: entry.path,
            cmd: if entry.cmd.is_empty() {
                None
            } else {
                Some(entry.cmd)
            },
            rendered,
            tail: Text::from(entry.alias),
        }
    }

    // q: can compiler know to skip the initial render invocation
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
        s: String,
        delimiter: char,
        cwd: &Path,
    ) -> Self {
        let (first, tail) = match s.split_once(delimiter) {
            Some((head, rest)) => (head, rest),
            None => (s.as_str(), ""),
        };

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
    let cfg = global_ui();
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
                let ret = if cfg.dir_icons {
                    format!("{} ~", Icons::HOME)
                } else {
                    "~".to_string()
                };

                if cfg.dir_colors {
                    Text::from(Span::styled(ret, Style::default().fg(Color::Blue)))
                } else {
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
            let content = if cfg.dir_icons {
                format!("{} {}", icon_for_file(full_path), path.to_string_lossy())
            } else {
                path.to_string_lossy().to_string()
            };

            if cfg.dir_colors {
                let style = if full_path.is_symlink() {
                    Style::default().fg(Color::LightCyan)
                } else {
                    Style::default().fg(Color::Blue)
                };
                Text::from(Span::styled(content, style))
            } else {
                Text::from(content)
            }
        }
        _ => {
            let content = if cfg.file_icons {
                format!("{} {}", icon_for_file(full_path), path.to_string_lossy())
            } else {
                path.to_string_lossy().to_string()
            };

            if cfg.file_colors {
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
                Text::from(Span::styled(content, style))
            } else {
                Text::from(content)
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
    use crate::ui::global::global_ui_init;
    use crate::ui::styles_config::{PathDisplayConfig, StyleConfig};
    use std::path::Path;

    // Helper to set the config for a test and then restore it.
    fn with_config<F>(
        config: PathDisplayConfig,
        test_fn: F,
    ) where
        F: FnOnce(),
    {
        let original_config = global_ui().clone();
        global_ui_init(StyleConfig { path: config });
        test_fn();
        global_ui_init(StyleConfig {
            path: original_config,
        });
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
}
