use std::ffi::OsString;
use std::fs::{self, File};
use std::io::Write;
use std::path::Path;

use super::{action::*, file_rule::*, *};
use crate::abspath::AbsPath;
use crate::cli::paths::{current_exe, text_renderer_path};
use crate::lessfilter::helpers::simple_metadata;
use crate::lessfilter::rule_matcher::Test;
use cba::vec_;
use fist_types::FileCategory;
use tempfile::tempdir;

const TEST_CONFIG: &str = "src/lessfilter/tests/lessfilter.toml";
const FILEFORMAT_CONFIG: &str = "src/lessfilter/tests/lessfilter.fileformat.toml";

fn load_config(config_path: &str) -> LessfilterConfig {
    let config_str = fs::read_to_string(config_path).unwrap();
    toml::from_str(&config_str).unwrap()
}

/// The action of the best-matching rule of `preset` for `path`, evaluated
/// with the config's own settings and categories.
fn best_action<'a>(
    cfg: &'a LessfilterConfig,
    preset: Preset,
    path: &Path,
) -> Option<&'a Action> {
    let data = FileData::new(
        AbsPath::new(path.to_path_buf()),
        &cfg.settings,
        &cfg.categories,
    );
    cfg.rules
        .get(preset)
        .get_best_match(path, data)
        .and_then(|arr| arr.rule.first())
}

fn write_png(path: &Path) {
    File::create(path)
        .unwrap()
        .write_all(&[0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A])
        .unwrap();
}

fn write_zip(path: &Path) {
    // a minimal empty zip: the End of Central Directory record, which is
    // what file-format's Zip signature (PK\x05\x06) accepts
    let mut eocd = b"PK\x05\x06".to_vec();
    eocd.extend_from_slice(&[0u8; 18]);
    File::create(path).unwrap().write_all(&eocd).unwrap();
}

// ---------------------------------------------------------------
// config-driven matching (lessfilter.toml, infer = "infer" mode)
// ---------------------------------------------------------------

#[test]
fn test_config_loading() {
    let cfg = load_config(TEST_CONFIG);
    assert!(!cfg.rules.preview.is_empty());
    assert!(!cfg.rules.edit.is_empty());

    let cfg = load_config(FILEFORMAT_CONFIG);
    assert!(!cfg.rules.preview.is_empty());
    assert!(cfg.categories.contains_key("raster"));
}

#[test]
fn test_directory_matching() {
    let cfg = load_config(TEST_CONFIG);
    let dir = tempdir().unwrap();
    let path = dir.path();

    let action = best_action(&cfg, Preset::Preview, path).unwrap();
    assert_eq!(*action, Action::Directory);

    let progs = action.to_progs(path, Preset::Preview);
    assert_eq!(progs.0.len(), 1);
    let expected: Vec<OsString> = vec_![: current_exe(), ":tool", "liza", ":u2", "--", path];
    assert_eq!(progs.0[0], expected);
}

#[test]
fn test_text_file_matching() {
    let cfg = load_config(TEST_CONFIG);
    let dir = tempdir().unwrap();
    let path = dir.path().join("test.txt");
    File::create(&path).unwrap().write_all(b"hello").unwrap();

    let action = best_action(&cfg, Preset::Preview, &path).unwrap();
    assert_eq!(*action, Action::Text);

    let progs = action.to_progs(&path, Preset::Preview);
    assert_eq!(progs.0.len(), 1);
    assert_eq!(
        progs.0[0],
        vec![text_renderer_path(), Path::new("--"), &path]
    );

    // the edit preset (used by `Advance`) matches the same file
    let edit_action = best_action(&cfg, Preset::Edit, &path).unwrap();
    assert_eq!(*edit_action, Action::Text);
    let edit_progs = edit_action.to_progs(&path, Preset::Edit);
    assert!(!edit_progs.0.is_empty()); // This will depend on env vars
}

#[test]
fn test_rust_file_matching() {
    let cfg = load_config(TEST_CONFIG);
    let dir = tempdir().unwrap();
    let path = dir.path().join("test.rs");
    File::create(&path)
        .unwrap()
        .write_all(b"fn main() {}")
        .unwrap();

    let action = best_action(&cfg, Preset::Preview, &path).unwrap();
    assert_eq!(*action, Action::Text);

    let progs = action.to_progs(&path, Preset::Preview);
    assert_eq!(progs.0.len(), 1);
    assert_eq!(
        progs.0[0],
        vec![text_renderer_path(), Path::new("--"), &path]
    );
}

#[test]
fn test_image_file_matching() {
    let cfg = load_config(TEST_CONFIG);
    let dir = tempdir().unwrap();
    let path = dir.path().join("test.png");
    write_png(&path);

    let action = best_action(&cfg, Preset::Preview, &path).unwrap();
    assert_eq!(*action, Action::Image);

    let progs = action.to_progs(&path, Preset::Preview);
    assert_eq!(progs.0.len(), 1);
    assert_eq!(progs.0[0][0], OsString::from("chafa"));

    // Test extended preset for multiple commands
    let extended_action = best_action(&cfg, Preset::Extended, &path).unwrap();
    let extended_progs = extended_action.to_progs(&path, Preset::Extended);
    assert_eq!(extended_progs.0.len(), 3);
    assert!(extended_progs.0[0].is_empty()); // header
    assert_eq!(extended_progs.0[1][0], OsString::from("chafa")); // image viewer
    assert_eq!(extended_progs.0[2], simple_metadata(&path)); // metadata
}

#[test]
fn test_application_matching() {
    let cfg = load_config(TEST_CONFIG);
    let dir = tempdir().unwrap();

    #[cfg(target_os = "macos")]
    let path = {
        let path = dir.path().join("Example.Application");
        fs::create_dir(&path).unwrap();
        path
    };

    #[cfg(target_os = "linux")]
    let path = {
        let path = dir.path().join("example.desktop");
        File::create(&path)
            .unwrap()
            .write_all(b"[Desktop Entry]\nType=Application\nName=Example\nExec=example\n")
            .unwrap();
        path
    };

    #[cfg(target_os = "windows")]
    let path = {
        let path = dir.path().join("example.exe");
        File::create(&path).unwrap();
        path
    };

    #[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
    let path = dir.path().join("example");

    #[cfg(any(target_os = "macos", target_os = "linux", target_os = "windows"))]
    {
        assert!(is_application_path(&path));

        let action = best_action(&cfg, Preset::Preview, &path).unwrap();
        assert_eq!(*action, Action::Application);
    }
}

#[test]
fn desktop_files_advance_to_the_editor() {
    // `Advance` runs the edit preset with the default preset appended (see
    // `handle`); on linux the apps pane entries are .desktop files, which
    // must resolve to Text (editor) rather than Application (launch).
    for config_path in [
        "assets/config/lessfilter.toml",
        "assets/config/lessfilter.dev.toml",
    ] {
        let cfg = load_config(config_path);
        let dir = tempdir().unwrap();
        let desktop = dir.path().join("example.desktop");
        File::create(&desktop)
            .unwrap()
            .write_all(b"[Desktop Entry]\nType=Application\nName=Example\nExec=example\n")
            .unwrap();

        // effective edit preset = edit rules + appended default rules
        let mut edit = cfg.rules.get(Preset::Edit).clone();
        let mut default = cfg.rules.get(Preset::Default).clone();
        edit.append(&mut default);

        let data = FileData::new(
            AbsPath::new(desktop.clone()),
            &cfg.settings,
            &cfg.categories,
        );
        let action = edit
            .get_best_match(&desktop, data)
            .and_then(|arr| arr.rule.first())
            .expect("a .desktop file must match the edit preset");
        assert_eq!(*action, Action::Text, "in {config_path}");
    }
}

#[test]
fn test_archive_matching() {
    let cfg = load_config(TEST_CONFIG);
    let dir = tempdir().unwrap();
    let path = dir.path().join("test.zip");
    write_zip(&path);

    let action = best_action(&cfg, Preset::Preview, &path).unwrap();
    assert_eq!(*action, Action::Metadata);

    let progs = action.to_progs(&path, Preset::Preview);
    assert_eq!(progs.0.len(), 1);
    assert_eq!(progs.0[0], simple_metadata(&path));
}

#[test]
fn test_fallback_to_metadata() {
    let cfg = load_config(TEST_CONFIG);
    let dir = tempdir().unwrap();
    let path = dir.path().join("some.binary");
    // Write some generic binary data that is not easily classified as text or other specific types
    File::create(&path)
        .unwrap()
        .write_all(b"\xDE\xAD\xBE\xEF\x00\x00\x00\x00")
        .unwrap();

    let action = best_action(&cfg, Preset::Preview, &path).unwrap();
    assert_eq!(*action, Action::Metadata);

    let progs = action.to_progs(&path, Preset::Preview);
    assert_eq!(progs.0.len(), 1);
    assert_eq!(progs.0[0], simple_metadata(&path));
}

#[test]
fn mime_detection_by_extension_and_magic() {
    let cfg = load_config(TEST_CONFIG);
    let dir = tempdir().unwrap();

    let mime_type = |path: &Path| {
        FileData::new(
            AbsPath::new(path.to_path_buf()),
            &cfg.settings,
            &cfg.categories,
        )
        .mime
        .mime
        .map(|m| m.type_().as_str().to_string())
    };

    // a real png header is detected from content
    let png = dir.path().join("magic.png");
    write_png(&png);
    assert_eq!(mime_type(&png).as_deref(), Some("image"));

    // an empty .png falls back to the extension guess
    let empty_png = dir.path().join("empty.png");
    File::create(&empty_png).unwrap();
    assert_eq!(mime_type(&empty_png).as_deref(), Some("image"));

    // plain text
    let txt = dir.path().join("note.txt");
    File::create(&txt).unwrap().write_all(b"hello").unwrap();
    assert_eq!(mime_type(&txt).as_deref(), Some("text"));

    // unknown binary content stays application/octet-stream
    let bin = dir.path().join("blob.bin");
    File::create(&bin)
        .unwrap()
        .write_all(b"\xDE\xAD\xBE\xEF")
        .unwrap();
    assert_eq!(mime_type(&bin).as_deref(), Some("application"));
}

// ---------------------------------------------------------------
// content-category matching (lessfilter.fileformat.toml)
// ---------------------------------------------------------------

#[test]
fn fileformat_mode_detects_content_categories() {
    let cfg = load_config(FILEFORMAT_CONFIG);
    let dir = tempdir().unwrap();

    let kind = |path: &Path| {
        FileData::new(
            AbsPath::new(path.to_path_buf()),
            &cfg.settings,
            &cfg.categories,
        )
        .mime
        .kind
    };

    let png = dir.path().join("photo.png");
    write_png(&png);
    assert_eq!(kind(&png), Some(FileCategory::Image));

    let zip = dir.path().join("bundle.zip");
    write_zip(&zip);
    assert_eq!(kind(&zip), Some(FileCategory::Compressed));

    let txt = dir.path().join("note.txt");
    File::create(&txt).unwrap().write_all(b"hello").unwrap();
    assert_eq!(kind(&txt), Some(FileCategory::Text));
}

#[test]
fn fileformat_config_chooses_by_score_and_category() {
    let cfg = load_config(FILEFORMAT_CONFIG);
    let dir = tempdir().unwrap();

    // png: the Image rule (40) outranks the custom cat:raster rule (30);
    // unknown action ids deserialize as custom actions
    let png = dir.path().join("photo.png");
    write_png(&png);
    assert_eq!(
        best_action(&cfg, Preset::Preview, &png),
        Some(&Action::Image)
    );

    // ...and the custom [categories] entry feeds the cat: rule directly
    let data = FileData::new(AbsPath::new(png.clone()), &cfg.settings, &cfg.categories);
    let raster = "cat:raster".parse::<FileRule>().unwrap();
    assert!(raster.passes(&png, &data));

    // zip: Compressed matches via the builtin category; the tie with the
    // catch-all Metadata rule is broken by rule order
    let zip = dir.path().join("bundle.zip");
    write_zip(&zip);
    assert_eq!(
        best_action(&cfg, Preset::Preview, &zip),
        Some(&Action::Custom("Compressed".into()))
    );

    // plain text: the Text rule ties the catch-all and wins by order
    let txt = dir.path().join("note.txt");
    File::create(&txt).unwrap().write_all(b"hello").unwrap();
    assert_eq!(
        best_action(&cfg, Preset::Preview, &txt),
        Some(&Action::Text)
    );

    // a directory matches the type:d rule
    assert_eq!(
        best_action(&cfg, Preset::Preview, dir.path()),
        Some(&Action::Directory)
    );
}
