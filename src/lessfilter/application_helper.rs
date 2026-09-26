use std::path::{Path, PathBuf};

use cba::bog::BogOkExt;

use crate::cli::paths::tmp_dir;

pub fn application_icon_path(path: &Path) -> Option<PathBuf> {
    cfg_if::cfg_if! {
        if #[cfg(target_os = "macos")] {
            macos_application_icon_path(path)
        } else if #[cfg(target_os = "linux")] {
            linux_application_icon_path(path)
        } else {
            sidecar_icon_path(path)
        }
    }
}

/// Whether an icon file can be resolved for `path` without materializing it.
/// On macOS this checks the bundle's `.icns`; elsewhere it is equivalent to
/// [`application_icon_path`].
pub fn application_has_icon(path: &Path) -> bool {
    cfg_if::cfg_if! {
        if #[cfg(target_os = "macos")] {
            macos_application_icon_file(path).is_some()
        } else if #[cfg(target_os = "linux")] {
            linux_application_icon_path(path).is_some()
        } else {
            sidecar_icon_path(path).is_some()
        }
    }
}

#[allow(dead_code)]
fn application_icon_cache_path(path: &Path) -> PathBuf {
    let app_path = path.to_string_lossy();
    let sanitized: String = app_path
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect();

    tmp_dir()
        .__ebog()
        .join("application-icons")
        .join(format!("{sanitized}.png"))
}

#[cfg(target_os = "macos")]
fn macos_extract_icon_appkit(app_path: &Path, cache_path: &Path) -> Option<()> {
    use objc2_app_kit::{NSBitmapImageFileType, NSBitmapImageRep, NSWorkspace};
    use objc2_foundation::{NSDictionary, NSString};

    let path_str = app_path.to_str()?;
    let ns_path = NSString::from_str(path_str);
    let ws = NSWorkspace::sharedWorkspace();
    let img = ws.iconForFile(&ns_path);
    let tiff_data = img.TIFFRepresentation()?;
    let reps = NSBitmapImageRep::imageRepsWithData(&tiff_data);
    if reps.is_empty() {
        return None;
    }
    let empty_dict = NSDictionary::new();
    let png_data = unsafe {
        NSBitmapImageRep::representationOfImageRepsInArray_usingType_properties(
            &reps,
            NSBitmapImageFileType::PNG,
            &empty_dict,
        )?
    };

    if let Some(parent) = cache_path.parent() {
        std::fs::create_dir_all(parent).ok()?;
    }
    std::fs::write(cache_path, png_data.to_vec()).ok()?;
    Some(())
}

#[cfg(target_os = "macos")]
fn macos_application_icon_file(path: &Path) -> Option<PathBuf> {
    use std::process::{Command, Stdio};

    let info = path.join("Contents").join("Info");
    let output = Command::new("defaults")
        .arg("read")
        .arg(&info)
        .arg("CFBundleIconFile")
        .stderr(Stdio::null())
        .output()
        .ok();

    if let Some(output) = output {
        if output.status.success() {
            let mut icon_file = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if !icon_file.is_empty() {
                if !icon_file.contains('.') {
                    icon_file.push_str(".icns");
                }
                let icon_path = path.join("Contents").join("Resources").join(icon_file);
                if icon_path.is_file() {
                    return Some(icon_path);
                }
            }
        }
    }

    // Modern macOS apps bundle icons in Assets.car (e.g. CFBundleIconName = AppIcon)
    let resources = path.join("Contents").join("Resources");
    let assets_car = resources.join("Assets.car");
    if assets_car.is_file() {
        return Some(assets_car);
    }

    // Default icon fallback file if present
    let default_icns = resources.join("AppIcon.icns");
    if default_icns.is_file() {
        return Some(default_icns);
    }

    None
}

#[cfg(target_os = "macos")]
fn macos_application_icon_path(path: &Path) -> Option<PathBuf> {
    use std::process::{Command, Stdio};

    let icon_path = macos_application_icon_file(path)?;

    let cache_path = application_icon_cache_path(path);
    if cache_path.is_file() {
        return Some(cache_path);
    }

    if let Some(parent) = cache_path.parent() {
        std::fs::create_dir_all(parent).ok()?;
    }

    // If it's a standalone .icns, try sips first
    if icon_path.extension().and_then(|ext| ext.to_str()) == Some("icns") {
        let status = Command::new("sips")
            .args(["-s", "format", "png"])
            .arg(&icon_path)
            .arg("--out")
            .arg(&cache_path)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .ok();

        if status.is_some_and(|s| s.success()) && cache_path.is_file() {
            return Some(cache_path);
        }
    }

    // For Assets.car or if sips failed, extract natively via AppKit
    if macos_extract_icon_appkit(path, &cache_path).is_some() && cache_path.is_file() {
        return Some(cache_path);
    }

    None
}

#[cfg(target_os = "linux")]
fn linux_application_icon_path(path: &Path) -> Option<PathBuf> {
    if path
        .file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| name.ends_with(".AppImage"))
    {
        return sidecar_icon_path(path);
    }

    let icon = desktop_icon_name(path)?;
    let icon_path = PathBuf::from(&icon);
    if icon_path.is_absolute() && icon_path.is_file() {
        return Some(icon_path);
    }
    if icon_path.components().count() > 1 && icon_path.is_file() {
        return Some(icon_path);
    }

    resolve_xdg_icon(&icon).or_else(|| sidecar_icon_path(path))
}

#[cfg(target_os = "linux")]
fn desktop_icon_name(path: &Path) -> Option<String> {
    let content = std::fs::read_to_string(path).ok()?;
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

#[cfg(target_os = "linux")]
fn resolve_xdg_icon(icon: &str) -> Option<PathBuf> {
    let icon = icon.strip_suffix(".png").unwrap_or(icon);
    let icon = icon.strip_suffix(".svg").unwrap_or(icon);
    let icon = icon.strip_suffix(".xpm").unwrap_or(icon);

    let mut roots = Vec::new();
    if let Some(data_home) = std::env::var_os("XDG_DATA_HOME") {
        roots.push(PathBuf::from(data_home));
    } else if let Some(home) = dirs::home_dir() {
        roots.push(home.join(".local").join("share"));
    }

    if let Some(data_dirs) = std::env::var_os("XDG_DATA_DIRS") {
        roots.extend(std::env::split_paths(&data_dirs));
    } else {
        roots.push(PathBuf::from("/usr/local/share"));
        roots.push(PathBuf::from("/usr/share"));
    }

    let exts = ["png", "svg", "xpm"];
    let sizes = [
        "512x512", "256x256", "128x128", "96x96", "64x64", "48x48", "32x32", "scalable",
    ];

    for root in &roots {
        for ext in exts {
            let candidate = root.join("pixmaps").join(format!("{icon}.{ext}"));
            if candidate.is_file() {
                return Some(candidate);
            }
        }

        for theme in ["hicolor", "Adwaita"] {
            for size in sizes {
                for ext in exts {
                    let candidate = root
                        .join("icons")
                        .join(theme)
                        .join(size)
                        .join("apps")
                        .join(format!("{icon}.{ext}"));
                    if candidate.is_file() {
                        return Some(candidate);
                    }
                }
            }
        }
    }

    None
}

#[cfg(any(target_os = "windows", not(any(target_os = "macos"))))]
fn sidecar_icon_path(path: &Path) -> Option<PathBuf> {
    for ext in ["png", "svg", "xpm", "ico"] {
        let candidate = path.with_extension(ext);
        if candidate.is_file() {
            return Some(candidate);
        }
    }

    None
}

