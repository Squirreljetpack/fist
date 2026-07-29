//! `fist-size` binary: computes directory sizes concurrently.
//!
//! Usage:
//!   fist-size [INPUT...] [-o OUTPUT]... [-d DEPTH] [-F]
//!
//! - INPUTs are paths to queue up for sizing. Defaults to `["."]` if
//!   none are given.
//! - If any `-o OUTPUT` is given, the size of each OUTPUT is printed
//!   (one per line, `path: size` or `path: None` if it isn't in the
//!   cache). `-o` is mutually exclusive with `-d`.
//! - If no `-o` is given, a tree of the INPUTs is printed, with each
//!   entry's size shown next to its name. By default, files are
//!   included as leaf nodes (read from the filesystem at print time,
//!   not from the cache); pass `-F` to hide them. `-d N` limits the
//!   tree depth to N levels of descendants below each INPUT. `-d 0`
//!   means unlimited; the default is 2.
//! - `-m PERCENT` (default 0.1) hides any entry whose size is below
//!   `PERCENT%` of its parent dir's size, including the immediate
//!   children of each INPUT. The INPUT path itself is always shown
//!   (it's a tree root and is never compared against a parent).
//! - `-b` prints sizes in binary (1024-based) units (KiB/MiB/GiB)
//!   instead of the default decimal (1000-based) units (KB/MB/GB).
//! - Items are always sorted largest-to-smallest before printing
//!   (with name as a stable tiebreaker for deterministic output).
//!
//! Examples:
//!   `fist-size`                              (tree of ".")
//!   `fist-size src`                          (tree of src/, depth 2)
//!   `fist-size src -d 5`                    (tree of src/, depth 5)
//!   `fist-size src -d 0`                     (tree of src/, unlimited)
//!   `fist-size src -F`                      (tree of src/, files hidden)
//!   `fist-size src -m 5`                    (hide entries < 5% of parent)
//!   `fist-size src -b`                      (show sizes in KiB/MiB/GiB)
//!   `fist-size src -o out1 -o out2`         (sizes of out1 and out2)

use clap::Parser;
use fist_size::DirSizeCache;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

#[derive(Parser, Debug)]
#[command(
    name = "fist-size",
    version,
    about = "Compute directory sizes concurrently and print them"
)]
struct Args {
    /// Paths to compute sizes for (defaults to ".")
    #[arg(value_name = "INPUT")]
    inputs: Vec<PathBuf>,

    /// Output paths to print sizes for
    #[arg(short, long, value_name = "PATH")]
    output: Vec<PathBuf>,

    /// Maximum tree depth
    #[arg(
        short,
        long,
        value_name = "N",
        default_value_t = 2,
        conflicts_with = "output"
    )]
    depth: usize,

    /// Hide files in tree output
    #[arg(short = 'F', long = "hide-files")]
    hide_files: bool,

    /// Minimum percentage of parent dir size for an entry to be shown
    #[arg(
        short,
        long,
        value_name = "PERCENT",
        default_value_t = 1.0,
        value_parser = validate_percent
    )]
    min_percent: f64,

    /// Show sizes in binary instead of
    /// decimal units.
    #[arg(short, long)]
    binary: bool,
}

fn validate_percent(s: &str) -> Result<f64, String> {
    let val: f64 = s
        .parse()
        .map_err(|_| format!("`{s}` isn't a valid floating-point number"))?;

    if (0.0..=100.0).contains(&val) {
        Ok(val)
    } else {
        Err(format!("Percent must be between 0.0 and 100.0, got {val}"))
    }
}

/// Formats a byte count as a human-readable size, with a `decimal`
/// flag selecting the unit system:
/// - `decimal = true`  -> 1000-based SI units (B, KB, MB, GB, TB, PB)
/// - `decimal = false` -> 1024-based IEC units (B, KiB, MiB, GiB, TiB, PiB)
fn human_size(
    bytes: u64,
    decimal: bool,
) -> String {
    const DECIMAL_UNITS: [&str; 6] = ["B", "KB", "MB", "GB", "TB", "PB"];
    const BINARY_UNITS: [&str; 6] = ["B", "KiB", "MiB", "GiB", "TiB", "PiB"];

    if bytes == 0 {
        return "0 B".to_string();
    }

    let base = if decimal { 1000.0 } else { 1024.0 };
    let units = if decimal { DECIMAL_UNITS } else { BINARY_UNITS };

    let mut size = bytes as f64;
    let mut unit_idx = 0;
    while size >= base && unit_idx < units.len() - 1 {
        size /= base;
        unit_idx += 1;
    }

    if unit_idx == 0 {
        format!("{bytes} {}", units[0])
    } else {
        format!("{size:.1} {}", units[unit_idx])
    }
}

/// A node in the directory tree: a name, its computed size, and any
/// children (subdirs from the cache plus, optionally, files read from
/// the filesystem).
#[derive(Debug)]
struct TreeNode {
    name: String,
    size: u64,
    children: Vec<TreeNode>,
}

/// Builds a forest (a `Vec` of trees) rooted at `root_paths`.
///
/// The cache only stores directory sizes (plus any file that was
/// explicitly passed to `DirSizeCache::add`). For each directory in the
/// tree, `build_node` pulls its subdirectories from a pre-built
/// `parent -> children` map (O(1) lookup, single pass over the cache)
/// and, when `show_files` is true, reads the directory on disk to
/// pick up its files as leaf nodes. Files are NEVER inserted into the
/// cache from this path - they're only sized here at print time.
///
/// Items are sorted largest-to-smallest at every level.
fn build_tree(
    root_paths: &[PathBuf],
    cache: &DirSizeCache,
    max_depth: usize,
    show_files: bool,
    min_percent: f64,
) -> Vec<TreeNode> {
    // Pre-build a parent -> subdirs map and a path -> size lookup from
    // the cache in one pass. We do this up front so the recursive
    // `build_node` below doesn't re-scan the cache for every node.
    let entries: Vec<(PathBuf, u64)> = cache.iter().collect();
    let size_by_path: HashMap<PathBuf, u64> =
        entries.iter().map(|(p, s)| (p.clone(), *s)).collect();

    let mut children_of: HashMap<PathBuf, Vec<PathBuf>> = HashMap::new();
    for (path, _) in &entries {
        if let Some(parent) = path.parent() {
            children_of
                .entry(parent.to_path_buf())
                .or_default()
                .push(path.clone());
        }
    }

    let mut roots: Vec<TreeNode> = Vec::new();
    for root in root_paths {
        if let Some(node) = build_node(
            root,
            0,
            max_depth,
            show_files,
            min_percent,
            &size_by_path,
            &children_of,
        ) {
            roots.push(node);
        }
    }
    // Sort roots largest first, with name as a tiebreaker so the output
    // is deterministic across runs.
    roots.sort_by(|a, b| b.size.cmp(&a.size).then(a.name.cmp(&b.name)));
    roots
}

fn build_node(
    path: &Path,
    depth: usize,
    max_depth: usize,
    show_files: bool,
    min_percent: f64,
    size_by_path: &HashMap<PathBuf, u64>,
    children_of: &HashMap<PathBuf, Vec<PathBuf>>,
) -> Option<TreeNode> {
    // We can only build a node for a path that's in the cache. Files
    // that weren't explicitly `add()`'d won't be here, but that's
    // intentional - they're not roots, they're leaves attached to a
    // directory at print time below.
    let size = *size_by_path.get(path)?;

    // `file_name` returns None for things like `/` or `.`; fall back to
    // the full path so the tree still has a printable label.
    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.to_string_lossy().into_owned());

    let mut children: Vec<TreeNode> = Vec::new();

    // Per the spec: "when depth is not reached, we have to read each
    // dir for files to add as leaf nodes" (and `-F` not set). So the
    // SAME condition gates both subdir recursion and file reading.
    // `0` is the special "unlimited" sentinel; any other value is the
    // inclusive max depth (a node at depth == max_depth is terminal).
    let can_recurse = max_depth == 0 || depth < max_depth;

    if can_recurse {
        // Subdirs come from the cache via the pre-built map.
        if let Some(subdirs) = children_of.get(path) {
            for sub in subdirs {
                if let Some(child) = build_node(
                    sub,
                    depth + 1,
                    max_depth,
                    show_files,
                    min_percent,
                    size_by_path,
                    children_of,
                ) {
                    children.push(child);
                }
            }
        }

        // Files come from the filesystem, lazily and only at print
        // time. They're not stored in the cache.
        if show_files && let Ok(entries) = std::fs::read_dir(path) {
            for entry in entries.filter_map(Result::ok) {
                let entry_path = entry.path();
                if let Ok(meta) = std::fs::symlink_metadata(&entry_path)
                    && !meta.is_dir()
                {
                    let file_name = entry_path
                        .file_name()
                        .map(|n| n.to_string_lossy().into_owned())
                        .unwrap_or_else(|| entry_path.to_string_lossy().into_owned());
                    children.push(TreeNode {
                        name: file_name,
                        size: meta.len(),
                        children: Vec::new(),
                    });
                }
            }
        }
    }

    // Apply the -m threshold: drop any child whose size is less than
    // `min_percent%` of the current node's size. The threshold applies
    // to all children at every depth, including the immediate children
    // of each INPUT (the "root" inputs themselves are never filtered -
    // they have no parent to compare against, since `build_tree` adds
    // them unconditionally). We skip the retain when `min_percent == 0`
    // since the comparison would be `>= 0` and always true.
    if min_percent > 0.0 {
        let parent_size = size as f64;
        children.retain(|c| (c.size as f64) * 100.0 >= parent_size * min_percent);
    }

    // Sort children largest first, with name as a stable tiebreaker so
    // ties (e.g. two 0-byte files) come out deterministically.
    children.sort_by(|a, b| b.size.cmp(&a.size).then(a.name.cmp(&b.name)));

    Some(TreeNode {
        name,
        size,
        children,
    })
}

/// Prints each root and its descendants, with a blank line between
/// distinct trees. `decimal` selects the unit system for the size
/// column (true = KB/MB/GB, false = KiB/MiB/GiB).
fn print_tree(
    roots: &[TreeNode],
    decimal: bool,
) {
    for (i, root) in roots.iter().enumerate() {
        if i > 0 {
            println!();
        }
        println!("{} ({})", root.name, human_size(root.size, decimal));
        print_subtree(root, "", decimal);
    }
}

/// Prints `node`'s children and their descendants. The node itself is
/// assumed to have already been printed by the caller; `prefix` is the
/// indent + vertical-bar continuation that comes before each child's
/// line (empty for the children of a root).
fn print_subtree(
    node: &TreeNode,
    prefix: &str,
    decimal: bool,
) {
    for (i, child) in node.children.iter().enumerate() {
        let is_last = i == node.children.len() - 1;
        let connector = if is_last { "└── " } else { "├── " };
        // When this child is the last one, the verticals above its own
        // descendants stop - we replace `│` with a space.
        let extension = if is_last { "    " } else { "│   " };

        println!(
            "{}{}{} ({})",
            prefix,
            connector,
            child.name,
            human_size(child.size, decimal)
        );
        print_subtree(child, &format!("{prefix}{extension}"), decimal);
    }
}

fn main() -> ExitCode {
    let args = Args::parse();

    // Default to ["."] if no inputs were given.
    let inputs: Vec<PathBuf> = if args.inputs.is_empty() {
        vec![PathBuf::from(".")]
    } else {
        args.inputs
    };

    for input in &inputs {
        if !input.exists() {
            eprintln!("error: path does not exist: {}", input.display());
            return ExitCode::FAILURE;
        }
    }

    let cache = DirSizeCache::new();

    // Queue every input root before waiting once at the end, so sizing
    // across all of them happens concurrently rather than one root fully
    // finishing before the next starts. `add` itself recurses into
    // subdirectories (via `compute_size`), so this alone populates the
    // cache for everything underneath each root too.
    for input in &inputs {
        cache.add(input);
    }

    cache.wait();

    if !args.output.is_empty() {
        // Size-printing mode: one line per -o. `get_path` returns None
        // for paths that weren't covered by an INPUT (or that were
        // files which weren't explicitly added), so the "None" branch
        // is reachable and is what the user expects.
        for output in &args.output {
            match cache.get_path(output) {
                Some(size) => println!("{}: {}", output.display(), human_size(size, !args.binary)),
                None => println!("{}: None", output.display()),
            }
        }
    } else {
        // Tree mode. `build_tree` consumes `cache.iter()` and does all
        // file I/O for the `-F`-not-set case here, lazily as it walks.
        print_tree(
            &build_tree(
                &inputs,
                &cache,
                args.depth,
                !args.hide_files,
                args.min_percent,
            ),
            !args.binary,
        );
    }

    ExitCode::SUCCESS
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::{self, File};
    use std::io::Write;
    use tempfile::TempDir;

    /// Helper to set up a directory like:
    /// root/
    /// ├── file1.txt (100 bytes)
    /// ├── file2.txt (50 bytes)
    /// └── sub/
    ///     └── file3.txt (30 bytes)
    /// Total: 180 bytes for root, 30 for sub.
    fn setup_test_dir() -> TempDir {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();

        let mut f1 = File::create(root.join("file1.txt")).unwrap();
        f1.write_all(&[0; 100]).unwrap();

        let mut f2 = File::create(root.join("file2.txt")).unwrap();
        f2.write_all(&[0; 50]).unwrap();

        let sub = root.join("sub");
        fs::create_dir(&sub).unwrap();
        let mut f3 = File::create(sub.join("file3.txt")).unwrap();
        f3.write_all(&[0; 30]).unwrap();

        dir
    }

    #[test]
    fn test_build_tree_includes_files_sorted_by_size() {
        let temp_dir = setup_test_dir();
        let cache = DirSizeCache::new();
        cache.add(temp_dir.path());
        cache.wait();

        // depth=0 means unlimited; min_percent=0.0 disables the filter.
        let tree = build_tree(&[temp_dir.path().to_path_buf()], &cache, 0, true, 0.0);

        assert_eq!(tree.len(), 1);
        let root = &tree[0];
        assert_eq!(root.size, 180);

        // 3 children sorted largest first: file1 (100), file2 (50), sub (30).
        assert_eq!(root.children.len(), 3);
        assert_eq!(root.children[0].name, "file1.txt");
        assert_eq!(root.children[0].size, 100);
        assert_eq!(root.children[1].name, "file2.txt");
        assert_eq!(root.children[1].size, 50);
        assert_eq!(root.children[2].name, "sub");
        assert_eq!(root.children[2].size, 30);

        // sub has its one file as a leaf.
        let sub = &root.children[2];
        assert_eq!(sub.children.len(), 1);
        assert_eq!(sub.children[0].name, "file3.txt");
        assert_eq!(sub.children[0].size, 30);
    }

    #[test]
    fn test_build_tree_hide_files_flag() {
        let temp_dir = setup_test_dir();
        let cache = DirSizeCache::new();
        cache.add(temp_dir.path());
        cache.wait();

        let tree = build_tree(&[temp_dir.path().to_path_buf()], &cache, 0, false, 0.0);

        let root = &tree[0];
        // Only `sub` - no file1.txt / file2.txt leaves.
        assert_eq!(root.children.len(), 1);
        assert_eq!(root.children[0].name, "sub");
        // sub itself also has no leaves when files are hidden.
        assert!(root.children[0].children.is_empty());
    }

    #[test]
    fn test_build_tree_respects_depth() {
        let temp_dir = setup_test_dir();
        let cache = DirSizeCache::new();
        cache.add(temp_dir.path());
        cache.wait();

        // depth=1: root + one level of children, no grandchildren.
        let tree = build_tree(&[temp_dir.path().to_path_buf()], &cache, 1, true, 0.0);

        let root = &tree[0];
        assert_eq!(root.children.len(), 3);
        // sub is at depth 1 (= max), so it has no children at all.
        let sub = root.children.iter().find(|c| c.name == "sub").unwrap();
        assert!(sub.children.is_empty());
    }

    #[test]
    fn test_depth_zero_is_unlimited() {
        // depth=0 is the special "unlimited" sentinel: the full tree is
        // walked regardless of nesting depth.
        let temp_dir = setup_test_dir();
        let cache = DirSizeCache::new();
        cache.add(temp_dir.path());
        cache.wait();

        let tree = build_tree(&[temp_dir.path().to_path_buf()], &cache, 0, true, 0.0);

        let root = &tree[0];
        // 3 top-level children (file1, file2, sub), and sub has its
        // one file as a leaf - i.e. we recursed all the way down.
        assert_eq!(root.children.len(), 3);
        let sub = root.children.iter().find(|c| c.name == "sub").unwrap();
        assert_eq!(sub.children.len(), 1);
    }

    #[test]
    fn test_build_tree_explicitly_added_file_is_a_root() {
        // A file passed to `add` is in the cache, so it should appear
        // as a tree root - distinct from files read at print time
        // (which only ever show up as leaves of a directory). The
        // hide_files flag does NOT affect roots, only the files read
        // out of subdirectories.
        let temp_dir = setup_test_dir();
        let cache = DirSizeCache::new();
        cache.add(temp_dir.path());
        let explicit_file = temp_dir.path().join("file1.txt");
        cache.add(&explicit_file);
        cache.wait();

        let inputs = vec![explicit_file.clone()];
        let tree = build_tree(&inputs, &cache, 0, false, 0.0);

        // The explicit file is the only root and has no children.
        assert_eq!(tree.len(), 1);
        assert_eq!(tree[0].name, "file1.txt");
        assert_eq!(tree[0].size, 100);
        assert!(tree[0].children.is_empty());
    }

    /// Sets up a dir like:
    /// root/
    /// ├── big.txt  (1000 bytes)
    /// ├── small.txt (10 bytes)
    /// └── sub/
    ///     ├── huge.txt  (990 bytes)
    ///     └── tiny.txt   (10 bytes)
    /// Total: 2010 bytes for root, 1000 bytes for sub.
    fn setup_threshold_dir() -> TempDir {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();

        let mut big = File::create(root.join("big.txt")).unwrap();
        big.write_all(&vec![0; 1000]).unwrap();

        let mut small = File::create(root.join("small.txt")).unwrap();
        small.write_all(&[0; 10]).unwrap();

        let sub = root.join("sub");
        fs::create_dir(&sub).unwrap();
        let mut huge = File::create(sub.join("huge.txt")).unwrap();
        huge.write_all(&vec![0; 990]).unwrap();
        let mut tiny = File::create(sub.join("tiny.txt")).unwrap();
        tiny.write_all(&[0; 10]).unwrap();

        dir
    }

    #[test]
    fn test_min_percent_filters_small_descendants() {
        // sub's children: huge.txt (990/1000 = 99%) and tiny.txt (10/1000 = 1%).
        // With min_percent=2, only huge.txt survives the filter; tiny.txt is dropped.
        let temp_dir = setup_threshold_dir();
        let cache = DirSizeCache::new();
        cache.add(temp_dir.path());
        cache.wait();

        let tree = build_tree(&[temp_dir.path().to_path_buf()], &cache, 0, true, 2.0);

        let root = &tree[0];
        // sub survives the top-level filter because 1000/2010 = 49.8% > 2%.
        // (The threshold applies at all levels now, not just depth >= 1.)
        let sub = root.children.iter().find(|c| c.name == "sub").unwrap();
        // Only huge.txt passes the 2% threshold inside sub; tiny.txt is dropped.
        assert_eq!(sub.children.len(), 1);
        assert_eq!(sub.children[0].name, "huge.txt");
        assert_eq!(sub.children[0].size, 990);
    }

    #[test]
    fn test_min_percent_applies_at_top_level() {
        // The threshold is relative to the parent dir, and applies at
        // every level - including the immediate children of the INPUT.
        // setup_threshold_dir() creates a root with:
        //   big.txt   (1000 / 2010 = 49.8%) - fails 50% threshold
        //   small.txt (  10 / 2010 = 0.5%)  - fails
        //   sub/      (1000 / 2010 = 49.8%) - fails
        // So at min_percent=50, no top-level child passes and the root
        // ends up with an empty children list.
        let temp_dir = setup_threshold_dir();
        let cache = DirSizeCache::new();
        cache.add(temp_dir.path());
        cache.wait();

        let tree = build_tree(&[temp_dir.path().to_path_buf()], &cache, 0, true, 50.0);

        let root = &tree[0];
        // Root itself is still a tree root (it's an INPUT, never filtered).
        assert_eq!(
            root.name,
            temp_dir.path().file_name().unwrap().to_str().unwrap()
        );
        // But all 3 top-level children fail the 50% threshold.
        assert!(root.children.is_empty());
    }

    #[test]
    fn test_min_percent_input_root_always_shown() {
        // Even at min_percent=100, the INPUT path itself is still a
        // tree root - it has no parent to be compared against, so the
        // filter can't hide it. Children may all be filtered out, but
        // the root survives.
        let temp_dir = setup_threshold_dir();
        let cache = DirSizeCache::new();
        cache.add(temp_dir.path());
        cache.wait();

        let tree = build_tree(&[temp_dir.path().to_path_buf()], &cache, 0, true, 100.0);

        // Exactly one tree root (the input), empty children.
        assert_eq!(tree.len(), 1);
        assert!(tree[0].children.is_empty());
    }

    #[test]
    fn test_human_size_decimal_and_binary() {
        // Decimal (1000-based): 999 stays as B, 1000 -> 1.0 KB, 1500 -> 1.5 KB,
        // 1024 -> 1.0 KB (NOT 1.0 KiB), zero -> "0 B".
        assert_eq!(human_size(0, true), "0 B");
        assert_eq!(human_size(999, true), "999 B");
        assert_eq!(human_size(1000, true), "1.0 KB");
        assert_eq!(human_size(1500, true), "1.5 KB");
        assert_eq!(human_size(1024, true), "1.0 KB");

        // Binary (1024-based): 1023 -> 1023 B, 1024 -> 1.0 KiB,
        // 1536 -> 1.5 KiB, 1000 -> 1000 B (NOT 1.0 KB).
        assert_eq!(human_size(1023, false), "1023 B");
        assert_eq!(human_size(1024, false), "1.0 KiB");
        assert_eq!(human_size(1536, false), "1.5 KiB");
        assert_eq!(human_size(1000, false), "1000 B");
    }
}
