//! macOS backend: disk images (`.dmg`, `.iso`, `.img`, `.cdr`, `.toast`) via
//! `hdiutil`.
//!
//! `hdiutil attach` mounts synchronously — it returns only once the filesystem
//! is available — and, without `-mountpoint`, chooses the mount point itself
//! (`/Volumes/<volume name>`, macOS resolving a name that is taken). The
//! `-plist` form of its report names those mount points, and `hdiutil info
//! -plist` reports the same for images that are already attached, whoever
//! attached them.
//!
//! One image may hold more than one mountable volume: `attach` mounts all of
//! them and the report lists one `mount-point` per volume, in report order.
//! That list is returned whole, so the caller decides which volume to enter.
//!
//! Images that prompt on stdin (encrypted images, license agreements) are not
//! supported: stdin is closed for every `hdiutil` invocation, so a prompt fails
//! instead of eating terminal input, and the attach error is reported.

use std::path::{Path, PathBuf};

#[cfg(target_os = "macos")]
use cba::broc::has;

/// Extensions [`supported`] accepts; `hdiutil` treats all of them as disk
/// images and mounts them if the filesystem inside is one macOS knows.
#[cfg(any(target_os = "macos", test))]
fn is_image_extension(path: &Path) -> bool {
    const EXTENSIONS: [&str; 6] = ["dmg", "iso", "img", "image", "cdr", "toast"];

    path.extension().is_some_and(|ext| {
        EXTENSIONS
            .iter()
            .any(|known| ext.eq_ignore_ascii_case(known))
    })
}

/// Whether `path` is a disk image this backend can mount: a known extension and
/// `hdiutil` on `PATH`.
#[cfg(target_os = "macos")]
pub fn supported(path: &Path) -> bool {
    is_image_extension(path) && has("hdiutil")
}

/// Attaches `source` read-only and returns the mount points `hdiutil` chose,
/// in report order.
#[cfg(target_os = "macos")]
pub fn mount(source: &Path) -> Result<Vec<PathBuf>, String> {
    let output = hdiutil()
        .args(["attach", "-readonly", "-nobrowse", "-noautoopen", "-plist"])
        .arg(source)
        .output()
        .map_err(|err| format!("hdiutil: {err}"))?;

    if !output.status.success() {
        return Err(failure(&output));
    }

    let report = String::from_utf8_lossy(&output.stdout);
    let points = mount_points_from_plist(&report);
    if points.is_empty() {
        return Err("hdiutil mounted nothing".to_string());
    }
    Ok(points)
}

/// Where `source` is already mounted, if it is: the answer covers images this
/// process never attached, which would otherwise attach a second copy.
#[cfg(target_os = "macos")]
pub fn attached_at(source: &Path) -> Option<Vec<PathBuf>> {
    let output = hdiutil().args(["info", "-plist"]).output().ok()?;
    if !output.status.success() {
        return None;
    }

    let report = String::from_utf8_lossy(&output.stdout);
    mount_points_from_info(&report, source)
}

/// Detaches the filesystem mounted at `mount_point`.
#[cfg(target_os = "macos")]
pub fn detach(mount_point: &Path) -> Result<(), String> {
    let output = hdiutil()
        .args(["detach", "-quiet"])
        .arg(mount_point)
        .output()
        .map_err(|err| format!("hdiutil: {err}"))?;

    if output.status.success() {
        Ok(())
    } else {
        Err(failure(&output))
    }
}

/// `hdiutil` with stdin closed, so an image needing a passphrase or a license
/// agreement fails rather than prompting into the terminal.
#[cfg(target_os = "macos")]
fn hdiutil() -> std::process::Command {
    let mut hdiutil = std::process::Command::new("hdiutil");
    hdiutil.stdin(std::process::Stdio::null());
    hdiutil
}

/// `hdiutil`'s first non-empty diagnostic line.
#[cfg(target_os = "macos")]
fn failure(output: &std::process::Output) -> String {
    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);
    stderr
        .lines()
        .chain(stdout.lines())
        .map(str::trim)
        .find(|line| !line.is_empty())
        .unwrap_or("hdiutil failed")
        .to_string()
}

/// Every `mount-point` in `hdiutil attach`'s report, in report order: one per
/// mounted volume, so a single-volume image yields one.
#[cfg(any(target_os = "macos", test))]
fn mount_points_from_plist(report: &str) -> Vec<PathBuf> {
    report
        .split("<key>mount-point</key>")
        .skip(1)
        .filter_map(plist_string)
        .map(PathBuf::from)
        .collect()
}

/// The mount points of the already-attached `source` in `hdiutil info`'s
/// report. The entries of one image precede the next `image-path`, so the
/// `mount-point`s after a matching `image-path` belong to that image.
#[cfg(any(target_os = "macos", test))]
fn mount_points_from_info(
    report: &str,
    source: &Path,
) -> Option<Vec<PathBuf>> {
    for image in report.split("<key>image-path</key>").skip(1) {
        let Some(path) = plist_string(image) else {
            continue;
        };
        if same_image(Path::new(&path), source) {
            let points = mount_points_from_plist(image);
            return (!points.is_empty()).then_some(points);
        }
    }
    None
}

/// Whether a reported image path names `source`, directly or resolved.
#[cfg(any(target_os = "macos", test))]
fn same_image(
    reported: &Path,
    source: &Path,
) -> bool {
    reported == source
        || matches!(
            (reported.canonicalize(), source.canonicalize()),
            (Ok(reported), Ok(source)) if reported == source
        )
}

/// First `<string>` value in a plist fragment, entity-decoded.
#[cfg(any(target_os = "macos", test))]
fn plist_string(fragment: &str) -> Option<String> {
    let (_, tail) = fragment.split_once("<string>")?;
    let (value, _) = tail.split_once("</string>")?;
    Some(unescape_xml(value))
}

/// Decodes the XML entity references a plist uses in string values. `&amp;` is
/// decoded last so that `&amp;lt;` becomes `&lt;` text rather than `<`.
#[cfg(any(target_os = "macos", test))]
fn unescape_xml(value: &str) -> String {
    if !value.contains('&') {
        return value.to_string();
    }
    value
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&apos;", "'")
        .replace("&amp;", "&")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn attach_report_lists_every_volume() {
        let report = r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
	<key>system-entities</key>
	<array>
		<dict>
			<key>dev-entry</key>
			<string>/dev/disk4s1</string>
			<key>potentially-mountable</key>
			<true/>
			<key>mount-point</key>
			<string>/Volumes/My &amp; Your Install</string>
		</dict>
		<dict>
			<key>dev-entry</key>
			<string>/dev/disk4s2</string>
			<key>mount-point</key>
			<string>/Volumes/Second &lt;Volume&gt;</string>
		</dict>
	</array>
</dict>
</plist>
"#;

        assert_eq!(
            mount_points_from_plist(report),
            vec![
                PathBuf::from("/Volumes/My & Your Install"),
                PathBuf::from("/Volumes/Second <Volume>"),
            ]
        );
    }

    #[test]
    fn attach_report_without_a_mounted_volume_is_empty() {
        let report =
            r#"<plist version="1.0"><dict><key>system-entities</key><array/></dict></plist>"#;

        assert!(mount_points_from_plist(report).is_empty());
    }

    #[test]
    fn attached_image_resolves_to_all_its_volumes() {
        let report = r#"<plist version="1.0">
<dict>
	<key>images</key>
	<array>
		<dict>
			<key>image-path</key>
			<string>/Users/me/other.dmg</string>
			<key>system-entities</key>
			<array>
				<dict>
					<key>mount-point</key>
					<string>/Volumes/Other</string>
				</dict>
			</array>
		</dict>
		<dict>
			<key>image-path</key>
			<string>/Users/me/Two &amp; More.dmg</string>
			<key>system-entities</key>
			<array>
				<dict>
					<key>dev-entry</key>
					<string>/dev/disk5s1</string>
					<key>mount-point</key>
					<string>/Volumes/First</string>
				</dict>
				<dict>
					<key>dev-entry</key>
					<string>/dev/disk5s2</string>
					<key>mount-point</key>
					<string>/Volumes/Second</string>
				</dict>
			</array>
		</dict>
		<dict>
			<key>image-path</key>
			<string>/Users/me/not-mounted.dmg</string>
			<key>system-entities</key>
			<array>
				<dict>
					<key>dev-entry</key>
					<string>/dev/disk6</string>
				</dict>
			</array>
		</dict>
	</array>
</dict>
</plist>
"#;

        assert_eq!(
            mount_points_from_info(report, Path::new("/Users/me/Two & More.dmg")),
            Some(vec![
                PathBuf::from("/Volumes/First"),
                PathBuf::from("/Volumes/Second"),
            ])
        );
        assert_eq!(
            mount_points_from_info(report, Path::new("/Users/me/not-mounted.dmg")),
            None
        );
        assert_eq!(
            mount_points_from_info(report, Path::new("/Users/me/absent.dmg")),
            None
        );
    }

    #[test]
    fn image_extensions_are_recognized_case_insensitively() {
        assert!(is_image_extension(Path::new("A.dmg")));
        assert!(is_image_extension(Path::new("A.ISO")));
        assert!(is_image_extension(Path::new("disc.toast")));
        assert!(!is_image_extension(Path::new("A.zip")));
        assert!(!is_image_extension(Path::new("README")));
    }

    #[test]
    fn xml_entities_decode_ampersands_last() {
        assert_eq!(unescape_xml("a &amp;lt; b"), "a &lt; b");
        assert_eq!(unescape_xml("plain"), "plain");
    }
}
