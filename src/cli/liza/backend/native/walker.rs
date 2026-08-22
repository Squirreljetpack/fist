use std::{
    fs::{self, Metadata},
    os::unix::fs::PermissionsExt,
    path::Path,
    time::SystemTime,
};

use chrono::{DateTime, Local};
use ignore::{DirEntry, WalkBuilder};

use crate::display::human_size;

use super::{
    entry::{FileEntry, FileMetadata},
    tree::TreeNode,
};
use crate::cli::liza::config::{LizaConfig, ViewMode};

pub fn build_file_entry(name: String, path: &Path, is_dir: bool, config: &LizaConfig) -> FileEntry {
    if !needs_metadata(config) {
        return FileEntry::new(name, is_dir);
    }

    let meta = fs::symlink_metadata(path).ok();
    let file_meta = extract_metadata(meta.as_ref(), is_dir, config);
    FileEntry::with_metadata(name, is_dir, file_meta)
}

fn needs_metadata(config: &LizaConfig) -> bool {
    config.show_clean_long
        || config.show_extensive
        || config.show_octal
        || config.show_time
        || config.show_mtime
        || config.show_size
        || config.git_status
        || config.view_mode == Some(ViewMode::Dirs)
}

fn format_symbolic_permissions(mode: u32, is_dir: bool) -> String {
    let d = if is_dir { 'd' } else { '-' };
    let r1 = if mode & 0o400 != 0 { 'r' } else { '-' };
    let w1 = if mode & 0o200 != 0 { 'w' } else { '-' };
    let x1 = if mode & 0o100 != 0 { 'x' } else { '-' };
    let r2 = if mode & 0o040 != 0 { 'r' } else { '-' };
    let w2 = if mode & 0o020 != 0 { 'w' } else { '-' };
    let x2 = if mode & 0o010 != 0 { 'x' } else { '-' };
    let r3 = if mode & 0o004 != 0 { 'r' } else { '-' };
    let w3 = if mode & 0o002 != 0 { 'w' } else { '-' };
    let x3 = if mode & 0o001 != 0 { 'x' } else { '-' };
    format!("{d}{r1}{w1}{x1}{r2}{w2}{x2}{r3}{w3}{x3}")
}

fn extract_metadata(
    meta: Option<&Metadata>,
    is_dir: bool,
    config: &LizaConfig,
) -> FileMetadata {
    let mut file_meta = FileMetadata::default();

    if let Some(m) = meta {
        let mode = m.permissions().mode() & 0o7777;
        if config.show_octal {
            file_meta.permissions = Some(format!("{mode:04o}"));
        } else if !config.no_permissions || config.show_clean_long || config.show_extensive {
            file_meta.permissions = Some(format_symbolic_permissions(mode, is_dir));
        }

        if !config.no_filesize
            || config.show_size
            || config.show_clean_long
            || config.show_extensive
            || config.view_mode == Some(ViewMode::Dirs)
        {
            file_meta.size = Some(human_size(m.len(), true));
        }

        if config.show_time {
            if let Ok(created) = m.created() {
                let dt: DateTime<Local> = DateTime::from(created);
                file_meta.btime = Some(dt.format("%Y-%m-%d %H:%M").to_string());
            }
            if let Ok(modified) = m.modified() {
                let dt: DateTime<Local> = DateTime::from(modified);
                file_meta.mtime = Some(dt.format("%Y-%m-%d %H:%M").to_string());
            }
            if let Ok(accessed) = m.accessed() {
                let dt: DateTime<Local> = DateTime::from(accessed);
                file_meta.atime = Some(dt.format("%Y-%m-%d %H:%M").to_string());
            }
        } else if !config.no_time
            || config.show_mtime
            || config.show_clean_long
            || config.show_extensive
        {
            if let Ok(modified) = m.modified() {
                let dt: DateTime<Local> = DateTime::from(modified);
                file_meta.mtime = Some(dt.format("%Y-%m-%d %H:%M").to_string());
            }
        }

        if config.show_extensive {
            file_meta.extra = Some(if is_dir { "dir".into() } else { "file".into() });
        }
    }

    file_meta
}

pub fn collect_dir_entries(dir: &Path, config: &LizaConfig) -> Vec<DirEntry> {
    let mut builder = WalkBuilder::new(dir);
    builder
        .max_depth(Some(1))
        .hidden(!config.all)
        .git_ignore(config.git_ignore)
        .git_global(config.git_ignore)
        .git_exclude(config.git_ignore)
        .require_git(false);

    let mut entries: Vec<DirEntry> = builder
        .build()
        .filter_map(|e| e.ok())
        .filter(|e| e.path() != dir)
        .filter(|entry| {
            if config.view_mode == Some(ViewMode::Dirs) {
                entry.file_type().map(|t| t.is_dir()).unwrap_or(false)
            } else {
                true
            }
        })
        .collect();

    sort_entries(&mut entries, config);
    entries
}

fn sort_entries(entries: &mut [DirEntry], config: &LizaConfig) {
    if config.view_mode == Some(ViewMode::Recent) {
        entries.sort_by(|a, b| {
            let time_a = a
                .metadata()
                .ok()
                .and_then(|m| m.accessed().ok())
                .unwrap_or(SystemTime::UNIX_EPOCH);
            let time_b = b
                .metadata()
                .ok()
                .and_then(|m| m.accessed().ok())
                .unwrap_or(SystemTime::UNIX_EPOCH);
            time_b.cmp(&time_a)
        });
    } else if config.show_mtime {
        entries.sort_by(|a, b| {
            let time_a = a
                .metadata()
                .ok()
                .and_then(|m| m.modified().ok())
                .unwrap_or(SystemTime::UNIX_EPOCH);
            let time_b = b
                .metadata()
                .ok()
                .and_then(|m| m.modified().ok())
                .unwrap_or(SystemTime::UNIX_EPOCH);
            time_b.cmp(&time_a)
        });
    } else {
        entries.sort_by(|a, b| {
            let a_is_dir = a.file_type().map(|t| t.is_dir()).unwrap_or(false);
            let b_is_dir = b.file_type().map(|t| t.is_dir()).unwrap_or(false);

            match (a_is_dir, b_is_dir) {
                (true, false) => std::cmp::Ordering::Less,
                (false, true) => std::cmp::Ordering::Greater,
                _ => a.file_name().cmp(b.file_name()),
            }
        });
    }
}

pub fn build_tree_node(
    path: &Path,
    name: String,
    current_depth: usize,
    max_depth: usize,
    config: &LizaConfig,
) -> TreeNode<FileEntry> {
    let is_dir = path.is_dir();
    let entry = build_file_entry(name, path, is_dir, config);
    let mut node = TreeNode::new(entry);

    if is_dir && current_depth < max_depth {
        let child_entries = collect_dir_entries(path, config);
        for child in child_entries {
            let child_path = child.path();
            let child_name = child.file_name().to_string_lossy().to_string();
            let child_node = build_tree_node(
                child_path,
                child_name,
                current_depth + 1,
                max_depth,
                config,
            );
            node.children.push(child_node);
        }
    }

    node
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::File;

    #[test]
    fn test_tree_node_building() {
        let temp = tempfile::tempdir().unwrap();
        let sub = temp.path().join("subdir");
        fs::create_dir(&sub).unwrap();
        File::create(sub.join("file.txt")).unwrap();

        let mut config = LizaConfig::default();
        config.all = true;
        let node = build_tree_node(temp.path(), "root".into(), 0, 2, &config);

        assert_eq!(node.value.name, "root");
        assert_eq!(node.children.len(), 1);
        assert_eq!(node.children[0].value.name, "subdir");
        assert_eq!(node.children[0].children.len(), 1);
        assert_eq!(node.children[0].children[0].value.name, "file.txt");
    }

    #[test]
    fn test_file_entry_metadata_generation() {
        let temp = tempfile::tempdir().unwrap();
        let file_path = temp.path().join("hello.sh");
        File::create(&file_path).unwrap();

        let mut config = LizaConfig::default();
        config.show_size = true;
        config.show_mtime = true;

        let entry = build_file_entry("hello.sh".into(), &file_path, false, &config);
        assert!(entry.metadata.is_some());
        let meta = entry.metadata.as_ref().unwrap();
        assert!(meta.size.is_some());
        assert!(meta.mtime.is_some());
        assert!(entry.to_string().starts_with('('));
        assert!(entry.to_string().ends_with("hello.sh"));
    }

    #[test]
    fn test_gitignore_filtering() {
        use std::io::Write;
        let temp = tempfile::tempdir().unwrap();
        let gitignore = temp.path().join(".gitignore");
        let mut f = File::create(&gitignore).unwrap();
        writeln!(f, "ignored.txt").unwrap();
        File::create(temp.path().join("ignored.txt")).unwrap();
        File::create(temp.path().join("visible.txt")).unwrap();

        let mut config = LizaConfig::default();
        config.all = true;
        config.git_ignore = true;

        let entries = collect_dir_entries(temp.path(), &config);
        let names: Vec<String> = entries.iter().map(|e| e.file_name().to_string_lossy().to_string()).collect();
        assert!(names.contains(&"visible.txt".to_string()));
        assert!(!names.contains(&"ignored.txt".to_string()));
    }

    #[test]
    fn test_sba_metadata_generation() {
        use crate::cli::liza::lexer::parse_liza_args;
        use std::ffi::OsString;

        let temp = tempfile::tempdir().unwrap();
        let file_path = temp.path().join("hello.txt");
        File::create(&file_path).unwrap();

        let config = parse_liza_args(&[OsString::from(":sba")]);
        let entry = build_file_entry("hello.txt".into(), &file_path, false, &config);
        assert!(entry.metadata.is_some());
        let meta = entry.metadata.as_ref().unwrap();
        assert!(meta.permissions.is_some()); // octal permissions
        assert!(meta.size.is_some());        // file size
        assert!(meta.mtime.is_some());       // mtime from :b
    }

    #[test]
    fn test_show_time_metadata_generation() {
        use crate::cli::liza::lexer::parse_liza_args;
        use std::ffi::OsString;

        let temp = tempfile::tempdir().unwrap();
        let file_path = temp.path().join("hello.txt");
        File::create(&file_path).unwrap();

        let config = parse_liza_args(&[OsString::from(":t")]);
        let entry = build_file_entry("hello.txt".into(), &file_path, false, &config);
        assert!(entry.metadata.is_some());
        let meta = entry.metadata.as_ref().unwrap();
        assert!(meta.mtime.is_some());
        assert!(meta.atime.is_some());
    }
}
