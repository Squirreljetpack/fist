//! FreeDesktop icon theme lookup for desktop entries.
//!
//! Follows the Icon Theme specification: the desktop's active theme is
//! searched first, then the themes it inherits from, then `hicolor` and
//! `Adwaita`, then the legacy `pixmaps` directories. `index.theme` files
//! supply the directory list with contexts and sizes; themes shipping no
//! index are probed in the conventional layouts.

use std::collections::{HashMap, HashSet};
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::OnceLock;

/// Extensions accepted for icon files, most preferred first. An `Icon=` value
/// ending in one of these is looked up under the bare name.
const EXTENSIONS: [&str; 4] = ["png", "svg", "xpm", "ico"];

/// Themes consulted after the active theme and its inheritance chain.
const FALLBACK_THEMES: [&str; 2] = ["hicolor", "Adwaita"];

/// Layouts probed for icons of themes shipping no `index.theme`, largest
/// first.
const SIZES: [(u32, &str); 10] = [
    (512, "512x512"),
    (256, "256x256"),
    (128, "128x128"),
    (96, "96x96"),
    (64, "64x64"),
    (48, "48x48"),
    (32, "32x32"),
    (24, "24x24"),
    (22, "22x22"),
    (16, "16x16"),
];
const CONTEXTS: [&str; 2] = ["apps", "categories"];

/// Read the `Icon=` value of a desktop file's `[Desktop Entry]` group.
pub fn desktop_icon_name(path: &Path) -> Option<String> {
    let content = fs::read_to_string(path).ok()?;
    let mut in_entry = false;

    for line in content.lines() {
        let line = line.trim();
        if line.starts_with('[') {
            if in_entry && line.ends_with(']') {
                break;
            }
            in_entry = line == "[Desktop Entry]";
            continue;
        }

        if in_entry {
            if let Some(icon) = line.strip_prefix("Icon=") {
                let icon = icon.trim();
                if !icon.is_empty() {
                    return Some(icon.to_string());
                }
            }
        }
    }

    None
}

/// Resolve an `Icon=` value to candidate icon files, most preferred first.
pub fn resolve(icon: &str) -> Vec<PathBuf> {
    let name = icon_name(icon);
    if name.contains('/') {
        return Vec::new();
    }

    let mut candidates = Vec::new();
    let mut seen = HashSet::new();

    // Applications context first, then any context: some themes place app
    // icons elsewhere (hicolor ships `cs-universal-access` under `categories`),
    // and a misplaced icon beats no icon.
    for applications_only in [true, false] {
        for theme in theme_chain() {
            for dir in &theme.dirs {
                if applications_only && !dir.applications {
                    continue;
                }
                for extension in EXTENSIONS {
                    push_candidate(
                        &mut candidates,
                        &mut seen,
                        dir.path.join(format!("{name}.{extension}")),
                    );
                }
            }
        }
    }

    for root in data_roots() {
        for extension in EXTENSIONS {
            push_candidate(
                &mut candidates,
                &mut seen,
                root.join("pixmaps").join(format!("{name}.{extension}")),
            );
        }
    }

    candidates
}

fn icon_name(icon: &str) -> &str {
    for extension in EXTENSIONS {
        if let Some(name) = icon.strip_suffix(&format!(".{extension}")) {
            return name;
        }
    }
    icon
}

fn push_candidate(
    candidates: &mut Vec<PathBuf>,
    seen: &mut HashSet<PathBuf>,
    candidate: PathBuf,
) {
    if candidate.is_file() && seen.insert(candidate.clone()) {
        candidates.push(candidate);
    }
}

/// Fetched once per process: environment, theme settings, and `index.theme`
/// files do not change while a preview runs.
fn theme_chain() -> &'static [ThemeData] {
    static CHAIN: OnceLock<Vec<ThemeData>> = OnceLock::new();
    CHAIN.get_or_init(|| {
        let mut chain = Vec::new();
        let mut seen = HashSet::new();
        for theme in detected_themes() {
            visit_theme(&theme, &mut seen, &mut chain);
        }
        for theme in FALLBACK_THEMES {
            visit_theme(theme, &mut seen, &mut chain);
        }
        chain
    })
}

/// Append `theme` and, depth first, the themes it inherits from.
fn visit_theme(
    theme: &str,
    seen: &mut HashSet<String>,
    chain: &mut Vec<ThemeData>,
) {
    if !seen.insert(theme.to_lowercase()) {
        return;
    }
    let Some(data) = load_theme(theme) else {
        return;
    };
    let inherits = data.inherits.clone();
    chain.push(data);
    for parent in inherits {
        visit_theme(&parent, seen, chain);
    }
}

/// A theme found in one or more icon roots, with its directories ordered by
/// preference.
struct ThemeData {
    dirs: Vec<IconDir>,
    inherits: Vec<String>,
}

struct IconDir {
    path: PathBuf,
    applications: bool,
    scalable: bool,
    size: u32,
    scale: u32,
}

fn dir_order(dir: &IconDir) -> (bool, bool, std::cmp::Reverse<u32>, u32) {
    (
        !dir.applications,
        !dir.scalable,
        std::cmp::Reverse(dir.size),
        dir.scale,
    )
}

fn load_theme(theme: &str) -> Option<ThemeData> {
    let mut dirs = Vec::new();
    let mut inherits: Vec<String> = Vec::new();
    let mut found = false;

    for root in icon_roots() {
        let Some(theme_dir) = theme_dir(root, theme) else {
            continue;
        };
        found = true;

        let index = theme_dir.join("index.theme");
        if index.is_file() {
            let (indexed, parents) = parse_index(&index, &theme_dir);
            for parent in parents {
                if !inherits
                    .iter()
                    .any(|known| known.eq_ignore_ascii_case(&parent))
                {
                    inherits.push(parent);
                }
            }
            if !indexed.is_empty() {
                dirs.extend(indexed);
                continue;
            }
        }

        dirs.extend(conventional_dirs(&theme_dir));
    }

    if !found {
        return None;
    }

    dirs.sort_by_key(dir_order);
    Some(ThemeData { dirs, inherits })
}

/// Locate `theme`'s directory under `root`, tolerating the naming variants
/// used by `Inherits=` (`Breeze Dark` for `breeze-dark`).
fn theme_dir(
    root: &Path,
    theme: &str,
) -> Option<PathBuf> {
    let mut names = Vec::new();
    for name in [
        theme.to_owned(),
        theme.to_lowercase(),
        theme.to_lowercase().replace(' ', "-"),
        theme.replace(' ', ""),
    ] {
        if !name.is_empty() && !names.contains(&name) {
            names.push(name);
        }
    }

    for name in &names {
        let path = root.join(name);
        if path.is_dir() {
            return Some(path);
        }
    }

    let wanted = theme.to_lowercase();
    fs::read_dir(root)
        .ok()?
        .flatten()
        .find(|entry| entry.file_name().to_string_lossy().to_lowercase() == wanted)
        .map(|entry| entry.path())
        .filter(|path| path.is_dir())
}

/// Parse a theme's `index.theme`: its `Directories=` list with per-directory
/// `Context`/`Size`/`Type`, and its `Inherits=` chain.
fn parse_index(
    index: &Path,
    theme_dir: &Path,
) -> (Vec<IconDir>, Vec<String>) {
    let Ok(content) = fs::read_to_string(index) else {
        return (Vec::new(), Vec::new());
    };

    let mut inherits = Vec::new();
    let mut listed = Vec::new();
    let mut metas: HashMap<String, DirMeta> = HashMap::new();
    let mut section = String::new();

    for raw in content.lines() {
        let line = raw.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if let Some(name) = line
            .strip_prefix('[')
            .and_then(|rest| rest.strip_suffix(']'))
        {
            section = name.trim().to_owned();
            continue;
        }
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        let (key, value) = (key.trim(), value.trim());

        if section.eq_ignore_ascii_case("Icon Theme") {
            match key {
                "Inherits" => inherits = split_list(value),
                "Directories" => listed = split_list(value),
                _ => {}
            }
        } else {
            let meta = metas.entry(section.clone()).or_default();
            match key.to_ascii_lowercase().as_str() {
                "context" => meta.context = value.to_owned(),
                "size" => meta.size = value.parse().ok(),
                "scale" => meta.scale = value.parse().ok(),
                "type" => meta.scalable = value.eq_ignore_ascii_case("scalable"),
                "minsize" if meta.size.is_none() => meta.size = value.parse().ok(),
                _ => {}
            }
        }
    }

    let dirs = listed
        .iter()
        .filter_map(|name| {
            let meta = metas.get(name)?;
            let path = theme_dir.join(name);
            if !path.is_dir() {
                return None;
            }
            Some(IconDir {
                path,
                applications: meta.context.eq_ignore_ascii_case("Applications"),
                scalable: meta.scalable,
                size: meta.size.unwrap_or(0),
                scale: meta.scale.unwrap_or(1),
            })
        })
        .collect();

    (dirs, inherits)
}

#[derive(Default)]
struct DirMeta {
    context: String,
    size: Option<u32>,
    scale: Option<u32>,
    scalable: bool,
}

fn split_list(value: &str) -> Vec<String> {
    value
        .split(',')
        .map(str::trim)
        .filter(|entry| !entry.is_empty())
        .map(str::to_owned)
        .collect()
}

/// Probe the directory layouts themes use when they ship no `index.theme`.
fn conventional_dirs(theme_dir: &Path) -> Vec<IconDir> {
    let mut dirs = Vec::new();

    for (size, pixels) in SIZES {
        for context in CONTEXTS {
            let applications = context == "apps";
            add_dir(
                &mut dirs,
                theme_dir.join(pixels).join(context),
                applications,
                false,
                size,
            );
            add_dir(
                &mut dirs,
                theme_dir.join(context).join(pixels),
                applications,
                false,
                size,
            );
        }
    }
    for context in CONTEXTS {
        let applications = context == "apps";
        add_dir(
            &mut dirs,
            theme_dir.join("scalable").join(context),
            applications,
            true,
            512,
        );
        add_dir(
            &mut dirs,
            theme_dir.join(context).join("scalable"),
            applications,
            true,
            512,
        );
    }
    for (size, pixels) in SIZES {
        add_dir(&mut dirs, theme_dir.join(pixels), true, false, size);
    }
    add_dir(&mut dirs, theme_dir.join("scalable"), true, true, 512);

    dirs
}

fn add_dir(
    dirs: &mut Vec<IconDir>,
    path: PathBuf,
    applications: bool,
    scalable: bool,
    size: u32,
) {
    if path.is_dir() {
        dirs.push(IconDir {
            path,
            applications,
            scalable,
            size,
            scale: 1,
        });
    }
}

/// Icon theme names configured on this machine, most authoritative first.
fn detected_themes() -> Vec<String> {
    let desktops = current_desktops();
    let mut themes = Vec::new();
    let matches = |needle: &str| desktops.iter().any(|desktop| desktop.contains(needle));

    // The desktop environment's own setting, when it is not stored in dconf.
    if matches("kde") {
        push_theme(&mut themes, kde_icon_theme());
    }
    if matches("xfce") {
        push_theme(&mut themes, xfce_icon_theme());
    }
    if matches("lxqt") {
        push_theme(&mut themes, lxqt_icon_theme());
    }

    // GTK's setting; on most desktops this mirrors the setting above.
    for file in gtk_settings_files() {
        push_theme(&mut themes, ini_value(&file, None, "gtk-icon-theme-name"));
    }

    for schema in gsettings_schemas(&desktops) {
        push_theme(&mut themes, gsettings_string(schema, "icon-theme"));
    }

    if !matches("kde") {
        push_theme(&mut themes, kde_icon_theme());
    }
    if !matches("xfce") {
        push_theme(&mut themes, xfce_icon_theme());
    }
    if !matches("lxqt") {
        push_theme(&mut themes, lxqt_icon_theme());
    }

    themes
}

fn push_theme(
    themes: &mut Vec<String>,
    theme: Option<String>,
) {
    let Some(theme) = theme else {
        return;
    };
    let theme = theme.trim();
    if theme.is_empty() || themes.iter().any(|known| known.eq_ignore_ascii_case(theme)) {
        return;
    }
    themes.push(theme.to_owned());
}

fn current_desktops() -> Vec<String> {
    env::var("XDG_CURRENT_DESKTOP")
        .unwrap_or_default()
        .to_lowercase()
        .split([':', ';'])
        .map(str::trim)
        .filter(|token| !token.is_empty())
        .map(str::to_owned)
        .collect()
}

/// dconf schemas holding an `icon-theme` key, the desktops' own first.
fn gsettings_schemas(desktops: &[String]) -> Vec<&'static str> {
    const SCHEMAS: [(&str, &str); 3] = [
        ("cinnamon", "org.cinnamon.desktop.interface"),
        ("mate", "org.mate.interface"),
        ("gnome", "org.gnome.desktop.interface"),
    ];

    let mut schemas = Vec::new();
    for desktop in desktops {
        for (token, schema) in SCHEMAS {
            if desktop.contains(token) && !schemas.contains(&schema) {
                schemas.push(schema);
            }
        }
    }
    for (_, schema) in SCHEMAS {
        if !schemas.contains(&schema) {
            schemas.push(schema);
        }
    }
    schemas
}

fn gsettings_string(
    schema: &str,
    key: &str,
) -> Option<String> {
    let output = Command::new("gsettings")
        .args(["get", schema, key])
        .stderr(Stdio::null())
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }

    let value = String::from_utf8_lossy(&output.stdout);
    let value = value.trim().trim_matches('\'').trim();
    if value.is_empty() || value == "''" {
        None
    } else {
        Some(value.to_owned())
    }
}

fn gtk_settings_files() -> Vec<PathBuf> {
    let mut files = Vec::new();
    if let Some(config) = dirs::config_dir() {
        files.push(config.join("gtk-3.0").join("settings.ini"));
        files.push(config.join("gtk-4.0").join("settings.ini"));
    }
    if let Some(home) = dirs::home_dir() {
        files.push(home.join(".gtkrc-2.0"));
    }
    files.retain(|file| file.is_file());
    files
}

fn kde_icon_theme() -> Option<String> {
    ini_value(
        &dirs::config_dir()?.join("kdeglobals"),
        Some("Icons"),
        "Theme",
    )
}

fn lxqt_icon_theme() -> Option<String> {
    ini_value(
        &dirs::config_dir()?.join("lxqt").join("lxqt.conf"),
        Some("General"),
        "icon_theme",
    )
}

fn xfce_icon_theme() -> Option<String> {
    let path = dirs::config_dir()?
        .join("xfce4")
        .join("xfconf")
        .join("xfce-perchannel-xml")
        .join("xsettings.xml");
    let content = fs::read_to_string(path).ok()?;

    for line in content.lines() {
        if !line.contains("Net/IconThemeName") {
            continue;
        }
        let value = line.split("value=\"").nth(1)?.split('"').next()?;
        if !value.is_empty() {
            return Some(value.to_owned());
        }
    }

    None
}

/// `key`'s value in `path`, under `section` when given; quotes are stripped
/// and `#`/`;` start comments.
fn ini_value(
    path: &Path,
    section: Option<&str>,
    key: &str,
) -> Option<String> {
    let content = fs::read_to_string(path).ok()?;
    let mut current = String::new();

    for line in content.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') || line.starts_with(';') {
            continue;
        }
        if let Some(name) = line
            .strip_prefix('[')
            .and_then(|rest| rest.strip_suffix(']'))
        {
            current = name.trim().to_owned();
            continue;
        }
        if let Some(section) = section {
            if !current.eq_ignore_ascii_case(section) {
                continue;
            }
        }
        if let Some((name, value)) = line.split_once('=') {
            if name.trim().eq_ignore_ascii_case(key) {
                let value = value.trim().trim_matches(['"', '\'']).trim();
                if !value.is_empty() {
                    return Some(value.to_owned());
                }
            }
        }
    }

    None
}

fn data_roots() -> &'static [PathBuf] {
    static ROOTS: OnceLock<Vec<PathBuf>> = OnceLock::new();
    ROOTS.get_or_init(|| {
        let mut roots = Vec::new();
        match env::var_os("XDG_DATA_HOME") {
            Some(data_home) if !data_home.is_empty() => roots.push(PathBuf::from(data_home)),
            _ => {
                if let Some(home) = dirs::home_dir() {
                    roots.push(home.join(".local").join("share"));
                }
            }
        }
        match env::var_os("XDG_DATA_DIRS") {
            Some(data_dirs) if !data_dirs.is_empty() => roots.extend(env::split_paths(&data_dirs)),
            _ => roots.extend([
                PathBuf::from("/usr/local/share"),
                PathBuf::from("/usr/share"),
            ]),
        }
        roots
    })
}

fn icon_roots() -> &'static [PathBuf] {
    static ROOTS: OnceLock<Vec<PathBuf>> = OnceLock::new();
    ROOTS.get_or_init(|| {
        let mut roots: Vec<PathBuf> = data_roots().iter().map(|root| root.join("icons")).collect();
        if let Some(home) = dirs::home_dir() {
            roots.push(home.join(".icons"));
        }
        roots
    })
}
