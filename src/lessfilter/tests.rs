use std::ffi::OsString;
use std::fs::{self, File};
use std::io::Write;
use std::path::{Path, PathBuf};

use super::{action::*, file_rule::*, rule_matcher::*, *};
use crate::abspath::AbsPath;
use crate::cli::paths::{current_exe, metadata_viewer_path, pager_path};
use cli_boilerplate_automation::vec_;
use tempfile::tempdir;

fn get_test_config() -> RulesConfig {
    let config_path = "src/lessfilter/tests/lessfilter.toml";
    let config_str = fs::read_to_string(config_path).unwrap();
    let lessfilter_config: LessfilterConfig = toml::from_str(&config_str).unwrap();
    lessfilter_config.rules
}

fn get_best_action<'a>(
    rules: &'a RuleMatcher<FileRule, ArrayVec<Action, 10>>,
    path: &Path,
) -> Option<&'a Action> {
    let apath = AbsPath::new(path.to_path_buf());
    let test = TestSettings {
        ..Default::default()
    };
    let data = FileData::new(apath, &test);
    rules.get_best_match(path, data).and_then(|arr| arr.first())
}

#[test]
fn test_config_loading() {
    let rules = get_test_config();
    assert!(!rules.preview.is_empty());
    assert!(!rules.edit.is_empty());
}

#[test]
fn test_directory_matching() {
    let rules = get_test_config();
    let dir = tempdir().unwrap();
    let path = dir.path();

    let action = get_best_action(&rules.preview, path).unwrap();
    assert_eq!(*action, Action::Directory);

    let progs = action.to_progs(path, Preset::Preview);
    assert_eq!(progs.0.len(), 1);
    let expected: Vec<OsString> = vec_![current_exe(), ":tool", "lz", ":u2", path];
    assert_eq!(progs.0[0], expected);
}

#[test]
fn test_text_file_matching() {
    let rules = get_test_config();
    let dir = tempdir().unwrap();
    let path = dir.path().join("test.txt");
    File::create(&path).unwrap().write_all(b"hello").unwrap();

    let action = get_best_action(&rules.preview, &path).unwrap();
    assert_eq!(*action, Action::Text);

    let progs = action.to_progs(&path, Preset::Preview);
    assert_eq!(progs.0.len(), 1);
    assert_eq!(progs.0[0], vec![pager_path(), &path]);

    // test edit preset
    let edit_action = get_best_action(&rules.edit, &path).unwrap();
    assert_eq!(*edit_action, Action::Text);
    let edit_progs = edit_action.to_progs(&path, Preset::Edit);
    assert!(!edit_progs.0.is_empty()); // This will depend on env vars
}

#[test]
fn test_rust_file_matching() {
    let rules = get_test_config();
    let dir = tempdir().unwrap();
    let path = dir.path().join("test.rs");
    File::create(&path)
        .unwrap()
        .write_all(b"fn main() {}")
        .unwrap();

    let action = get_best_action(&rules.preview, &path).unwrap();
    assert_eq!(*action, Action::Text);

    let progs = action.to_progs(&path, Preset::Preview);
    assert_eq!(progs.0.len(), 1);
    assert_eq!(progs.0[0], vec![pager_path(), &path]);
}

#[test]
fn test_image_file_matching() {
    let rules = get_test_config();
    let dir = tempdir().unwrap();
    let path = dir.path().join("test.png");
    // Create a minimal valid PNG header to ensure proper mime detection
    let png_header: [u8; 8] = [0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A];
    File::create(&path).unwrap().write_all(&png_header).unwrap();

    let action = get_best_action(&rules.preview, &path).unwrap();
    assert_eq!(*action, Action::Image);

    let progs = action.to_progs(&path, Preset::Preview);
    assert_eq!(progs.0.len(), 1);
    assert_eq!(progs.0[0][0], OsString::from("chafa"));

    // Test extended preset for multiple commands
    let extended_action = get_best_action(&rules.extended, &path).unwrap();
    let extended_progs = extended_action.to_progs(&path, Preset::Extended);
    assert_eq!(extended_progs.0.len(), 3);
    assert!(extended_progs.0[0].is_empty()); // header
    assert_eq!(extended_progs.0[1][0], OsString::from("chafa")); // image viewer
    assert_eq!(extended_progs.0[2][0], metadata_viewer_path()); // metadata
}

#[test]
fn test_archive_matching() {
    let rules = get_test_config();
    let dir = tempdir().unwrap();
    let path = dir.path().join("test.zip");
    let mut f = File::create(&path).unwrap();
    f.write_all(&[0x50, 0x4B, 0x03, 0x04]).unwrap(); // minimal zip magic number

    let action = get_best_action(&rules.preview, &path).unwrap();
    assert_eq!(*action, Action::Metadata);

    let progs = action.to_progs(&path, Preset::Preview);
    assert_eq!(progs.0.len(), 1);
    assert_eq!(progs.0[0], vec![metadata_viewer_path(), &path]);
}

#[test]
fn test_fallback_to_metadata() {
    let rules = get_test_config();
    let dir = tempdir().unwrap();
    let path = dir.path().join("some.binary");
    // Write some generic binary data that is not easily classified as text or other specific types
    File::create(&path)
        .unwrap()
        .write_all(b"\xDE\xAD\xBE\xEF\x00\x00\x00\x00")
        .unwrap();

    let action = get_best_action(&rules.preview, &path).unwrap();
    assert_eq!(*action, Action::Metadata);

    let progs = action.to_progs(&path, Preset::Preview);
    assert_eq!(progs.0.len(), 1);
    assert_eq!(progs.0[0], vec![metadata_viewer_path(), &path]);
}

// Temporary test to debug mime types
#[test]
fn debug_mime_types() {
    let dir = tempdir().unwrap();

    // Empty PNG file
    let empty_png_path = dir.path().join("empty.png");
    File::create(&empty_png_path).unwrap();
    let empty_png_data = FileData::new(
        AbsPath::new(empty_png_path.clone()),
        &TestSettings::default(),
    );
    println!("Empty PNG mime: {:?}", empty_png_data.mime); // Expected: image/png (from extension guess) or application/octet-stream (if infer fails)

    // Valid PNG file
    let valid_png_path = dir.path().join("valid.png");
    let png_header: [u8; 8] = [0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A];
    File::create(&valid_png_path)
        .unwrap()
        .write_all(&png_header)
        .unwrap();
    let valid_png_data = FileData::new(
        AbsPath::new(valid_png_path.clone()),
        &TestSettings::default(),
    );
    println!("Valid PNG mime: {:?}", valid_png_data.mime); // Expected: image/png

    // ELF header (binary)
    let elf_path = dir.path().join("elf.bin");
    File::create(&elf_path)
        .unwrap()
        .write_all(b"\x7FELF\x02\x01\x01\x00")
        .unwrap();
    let elf_data = FileData::new(AbsPath::new(elf_path.clone()), &TestSettings::default());
    println!("ELF mime: {:?}", elf_data.mime); // Expected: application/x-elf or application/octet-stream

    // Generic binary
    let generic_bin_path = dir.path().join("generic.bin");
    File::create(&generic_bin_path)
        .unwrap()
        .write_all(b"\xDE\xAD\xBE\xEF")
        .unwrap();
    let generic_bin_data = FileData::new(
        AbsPath::new(generic_bin_path.clone()),
        &TestSettings::default(),
    );
    println!("Generic binary mime: {:?}", generic_bin_data.mime); // Expected: application/octet-stream
}
