use std::path::{Path, PathBuf};

use cba::bog::BogOkExt;

use crate::cli::paths::tmp_dir;

#[cfg(target_os = "linux")]
use super::xdg_icon;

/// Path of the icon to preview for `path`: the first resolvable candidate the
/// image preview pipeline can decode. Formats it cannot read (xpm, icns) are
/// skipped in favour of lower-priority candidates, so `None` means the
/// caller should fall back to a text preview instead of an empty pane.
pub fn application_icon_path(path: &Path) -> Option<PathBuf> {
    application_icon_candidates(path)
        .into_iter()
        .find(|candidate| previewable_image(candidate))
}

/// Candidate icons for `path`, most preferred first.
fn application_icon_candidates(path: &Path) -> Vec<PathBuf> {
    cfg_if::cfg_if! {
        if #[cfg(target_os = "macos")] {
            macos_application_icon_path(path).into_iter().collect()
        } else if #[cfg(target_os = "linux")] {
            linux_application_icon_candidates(path)
        } else {
            sidecar_icon_candidates(path)
        }
    }
}

/// Whether `path` names an image the preview pipeline can show, svg included.
fn previewable_image(path: &Path) -> bool {
    matchmaker::preview::image::PreviewFormat::from_path(path).is_some()
}

/// Whether an icon file can be resolved for `path` without materializing it.
/// On macOS this checks the bundle's `.icns`; elsewhere it reports whether any
/// candidate exists, whether or not the preview pipeline can decode it.
pub fn application_has_icon(path: &Path) -> bool {
    cfg_if::cfg_if! {
        if #[cfg(target_os = "macos")] {
            macos_application_icon_file(path).is_some()
        } else if #[cfg(target_os = "linux")] {
            !linux_application_icon_candidates(path).is_empty()
        } else {
            !sidecar_icon_candidates(path).is_empty()
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
fn macos_extract_icon_appkit(
    app_path: &Path,
    cache_path: &Path,
) -> Option<()> {
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
fn linux_application_icon_candidates(path: &Path) -> Vec<PathBuf> {
    if path
        .file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| name.ends_with(".AppImage"))
    {
        return sidecar_icon_candidates(path);
    }

    let mut candidates = Vec::new();
    if let Some(icon) = xdg_icon::desktop_icon_name(path) {
        let icon_path = Path::new(&icon);
        if icon_path.is_absolute() {
            if icon_path.is_file() {
                candidates.push(icon_path.to_path_buf());
            }
        } else if icon_path.components().count() > 1 {
            // A relative path resolves against the desktop file's directory,
            // falling back to the process directory.
            if let Some(parent) = path.parent() {
                let relative = parent.join(icon_path);
                if relative.is_file() {
                    candidates.push(relative);
                }
            }
            if icon_path.is_file() {
                candidates.push(icon_path.to_path_buf());
            }
        }
        candidates.extend(xdg_icon::resolve(&icon));
    }
    candidates.extend(sidecar_icon_candidates(path));
    candidates
}

#[cfg(any(target_os = "windows", not(any(target_os = "macos"))))]
#[cfg(not(target_os = "macos"))]
fn sidecar_icon_candidates(path: &Path) -> Vec<PathBuf> {
    ["png", "svg", "xpm", "ico"]
        .into_iter()
        .map(|extension| path.with_extension(extension))
        .filter(|candidate| candidate.is_file())
        .collect()
}
