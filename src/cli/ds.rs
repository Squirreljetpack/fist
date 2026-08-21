//! `fs :tool ds` — disk usage: compute directory sizes concurrently and
//! print them.
//!
//! With a single INPUT, a tree of that input is printed. With multiple
//! INPUTs, a skeleton is printed instead: a single tree rooted at the
//! inputs' common ancestor, with one branch per input. Every input
//! branch is realized as a full tree, with `-d`, `-F`, and `-m` applied
//! to each realized tree; intermediate skeleton nodes are printed
//! without a size.

use std::{
    collections::HashMap,
    ffi::OsString,
    path::{Component, Path, PathBuf},
};

use cba::{bath::PathExt, ebog};
use clap::Parser;
use fist_size::DirSizeCache;

use crate::{cli::paths::current_exe, display::human_size, errors::CliError};

#[derive(Parser, Debug)]
#[command(about = "Compute directory sizes concurrently and print them")]
pub struct DsArgs {
    #[arg(value_name = "PATH")]
    pub inputs: Vec<PathBuf>,

    /// Output paths to print sizes for.
    #[arg(short, long, value_name = "PATH")]
    pub output: Vec<PathBuf>,

    /// Maximum tree depth.
    #[arg(
        short,
        long,
        value_name = "N",
        default_value_t = 2,
        conflicts_with = "output"
    )]
    pub depth: usize,

    /// Hide files in tree output.
    #[arg(short = 'F', long = "hide-files")]
    pub hide_files: bool,

    /// Minimum percentage of parent dir size for an entry to be shown.
    #[arg(
        short,
        long,
        value_name = "PERCENT",
        default_value_t = 1.0,
        value_parser = validate_percent
    )]
    pub min_percent: f64,

    /// Show sizes in binary instead of decimal units.
    #[arg(short, long)]
    pub binary: bool,
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

/// Entry point for `fs :tool ds`.
pub fn handle(mut args: Vec<OsString>) -> Result<(), CliError> {
    let path = current_exe().basename();
    args.insert(0, format!("{path} :tool diskspace").into());

    let args = DsArgs::parse_from(args);
    run(args)
}

fn run(args: DsArgs) -> Result<(), CliError> {
    // Default to ["."] if no inputs were given.
    let inputs: Vec<PathBuf> = if args.inputs.is_empty() {
        vec![PathBuf::from(".")]
    } else {
        args.inputs
    };

    for input in &inputs {
        if !input.exists() {
            ebog!("error: path does not exist: {}", input.display());
            return Err(CliError::Handled);
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

    let decimal = !args.binary;

    if !args.output.is_empty() {
        // Size-printing mode: one line per -o. `get_path` returns None
        // for paths that weren't covered by an INPUT (or that were
        // files which weren't explicitly added), so the "None" branch
        // is reachable and is what the user expects.
        for output in &args.output {
            match cache.get_path(output) {
                Some(size) => println!("{}: {}", output.display(), human_size(size, decimal)),
                None => println!("{}: None", output.display()),
            }
        }
    } else if inputs.len() > 1 {
        // Skeleton mode: one tree linking the inputs from their common
        // ancestor, with each input branch realized as a full tree.
        print_skeleton(
            &build_skeleton(
                &inputs,
                &cache,
                args.depth,
                !args.hide_files,
                args.min_percent,
            ),
            decimal,
        );
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
            decimal,
        );
    }

    Ok(())
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

    // When depth is not reached, each dir is read for files to add as
    // leaf nodes (unless `-F` is set). The SAME condition gates both
    // subdir recursion and file reading. `0` is the special "unlimited"
    // sentinel; any other value is the inclusive max depth (a node at
    // depth == max_depth is terminal).
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
        // time. They're not stored in the cache. Entries that already
        // appear as cache children (explicitly `add()`ed files, which
        // carry their size in the cache) are skipped so a file input
        // isn't listed twice in its parent's branch.
        if show_files && let Ok(entries) = std::fs::read_dir(path) {
            let files: Vec<(String, u64)> = entries
                .filter_map(Result::ok)
                .filter_map(|entry| {
                    let entry_path = entry.path();
                    let meta = std::fs::symlink_metadata(&entry_path).ok()?;
                    if meta.is_dir() {
                        return None;
                    }
                    let file_name = entry_path
                        .file_name()
                        .map(|n| n.to_string_lossy().into_owned())
                        .unwrap_or_else(|| entry_path.to_string_lossy().into_owned());
                    Some((file_name, meta.len()))
                })
                .collect();
            for (file_name, size) in files {
                if !children.iter().any(|c| c.name == file_name) {
                    children.push(TreeNode {
                        name: file_name,
                        size,
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
fn print_tree(roots: &[TreeNode], decimal: bool) {
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
fn print_subtree(node: &TreeNode, prefix: &str, decimal: bool) {
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

/// A node in the multi-input skeleton tree. Inputs and realized nodes
/// carry their computed size; intermediate nodes (path components
/// between the common ancestor and the inputs) are printed without a
/// size.
#[derive(Debug, PartialEq)]
struct SkeletonNode {
    name: String,
    size: Option<u64>,
    children: Vec<SkeletonNode>,
}

/// Builds a single tree linking every input to the common ancestor of
/// all inputs. The path components in between become bare intermediate
/// nodes; each input branch is realized as a full tree beneath its
/// leaf, honoring `max_depth`, `show_files`, and `min_percent` exactly
/// as in single-input tree mode.
fn build_skeleton(
    inputs: &[PathBuf],
    cache: &DirSizeCache,
    max_depth: usize,
    show_files: bool,
    min_percent: f64,
) -> SkeletonNode {
    // Absolute, lexically normalized forms keep component comparison
    // meaningful even when the inputs mix relative and absolute paths
    // and use `..` components.
    let abs: Vec<PathBuf> = inputs.iter().map(|p| lexical_absolute(p)).collect();

    let ancestor = common_ancestor(&abs);

    let root_name = ancestor
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| ancestor.to_string_lossy().into_owned());

    // When the common ancestor is itself an input, the root carries
    // that input's size.
    let root_size = inputs
        .iter()
        .zip(&abs)
        .find(|(_, abs_input)| *abs_input == &ancestor)
        .and_then(|(input, _)| cache.get_path(input));

    let mut root = SkeletonNode {
        name: root_name,
        size: root_size,
        children: Vec::new(),
    };

    for (input, abs_input) in inputs.iter().zip(&abs) {
        let chain: Vec<OsString> = abs_input
            .components()
            .skip(ancestor.components().count())
            .map(|c| c.as_os_str().to_os_string())
            .collect();

        // Realize the input's own subtree exactly as single-input tree
        // mode would, then hang it beneath the input's skeleton leaf.
        let realized = build_tree(
            std::slice::from_ref(input),
            cache,
            max_depth,
            show_files,
            min_percent,
        );
        let realized_children = realized
            .first()
            .map(|root| tree_to_skeleton(&root.children))
            .unwrap_or_default();

        insert_chain(&mut root, &chain, cache.get_path(input), realized_children);
    }

    sort_skeleton(&mut root);
    root
}

/// Converts a realized (sized) tree into skeleton nodes so it can be
/// hung beneath the skeleton spine. Every realized node has a concrete
/// size, so all converted nodes carry `Some`.
fn tree_to_skeleton(nodes: &[TreeNode]) -> Vec<SkeletonNode> {
    nodes
        .iter()
        .map(|n| SkeletonNode {
            name: n.name.clone(),
            size: Some(n.size),
            children: tree_to_skeleton(&n.children),
        })
        .collect()
}

/// Absolute, lexically normalized form of `path` (`CurDir` components
/// dropped, `ParentDir` components resolved against their prefix). No
/// filesystem access — symlinks are not resolved.
fn lexical_absolute(path: &Path) -> PathBuf {
    let abs = std::path::absolute(path).unwrap_or_else(|_| path.to_path_buf());

    let mut normalized = PathBuf::new();
    for comp in abs.components() {
        match comp {
            Component::CurDir => {}
            Component::ParentDir => {
                if !normalized.pop() {
                    normalized.push(comp.as_os_str());
                }
            }
            other => normalized.push(other.as_os_str()),
        }
    }
    normalized
}

/// The common ancestor (longest shared component prefix) of absolute
/// paths.
fn common_ancestor(abs_paths: &[PathBuf]) -> PathBuf {
    let comp_lists: Vec<Vec<OsString>> = abs_paths
        .iter()
        .map(|p| {
            p.components()
                .map(|c| c.as_os_str().to_os_string())
                .collect()
        })
        .collect();

    let mut ancestor = PathBuf::new();
    for (i, comp) in comp_lists[0].iter().enumerate() {
        if comp_lists[1..]
            .iter()
            .any(|comps| comps.get(i) != Some(comp))
        {
            break;
        }
        ancestor.push(comp);
    }
    ancestor
}

/// Inserts a path chain (input components relative to the skeleton root)
/// into the skeleton tree; the final node receives the input's size and
/// its realized subtree.
fn insert_chain(
    node: &mut SkeletonNode,
    chain: &[OsString],
    size: Option<u64>,
    realized_children: Vec<SkeletonNode>,
) {
    if chain.is_empty() {
        node.size = size;
        merge_children(&mut node.children, realized_children);
        return;
    }

    let name = chain[0].to_string_lossy().into_owned();
    if let Some(existing) = node.children.iter_mut().find(|c| c.name == name) {
        insert_chain(existing, &chain[1..], size, realized_children);
    } else {
        let mut child = SkeletonNode {
            name,
            size: None,
            children: Vec::new(),
        };
        insert_chain(&mut child, &chain[1..], size, realized_children);
        node.children.push(child);
    }
}

/// Merges `additions` into `children` by name, recursing into matching
/// nodes. An input whose path descends from another input realizes its
/// subtree onto a node the outer input's realization already produced;
/// merging keeps such overlapping branches from duplicating.
fn merge_children(children: &mut Vec<SkeletonNode>, additions: Vec<SkeletonNode>) {
    for addition in additions {
        if let Some(existing) = children.iter_mut().find(|c| c.name == addition.name) {
            existing.size = existing.size.or(addition.size);
            merge_children(&mut existing.children, addition.children);
        } else {
            children.push(addition);
        }
    }
}

/// Sorts skeleton nodes largest-first at every level. Bare intermediate
/// nodes rank by the largest size in their subtree, so branches carrying
/// big inputs float to the top.
fn sort_skeleton(node: &mut SkeletonNode) {
    for child in &mut node.children {
        sort_skeleton(child);
    }
    node.children.sort_by(|a, b| {
        subtree_max(b)
            .cmp(&subtree_max(a))
            .then(a.name.cmp(&b.name))
    });
}

fn subtree_max(node: &SkeletonNode) -> u64 {
    node.size
        .unwrap_or(0)
        .max(node.children.iter().map(subtree_max).max().unwrap_or(0))
}

fn print_skeleton(root: &SkeletonNode, decimal: bool) {
    match root.size {
        Some(size) => println!("{} ({})", root.name, human_size(size, decimal)),
        None => println!("{}", root.name),
    }
    print_skeleton_children(root, "", decimal);
}

fn print_skeleton_children(node: &SkeletonNode, prefix: &str, decimal: bool) {
    for (i, child) in node.children.iter().enumerate() {
        let is_last = i == node.children.len() - 1;
        let connector = if is_last { "└── " } else { "├── " };
        let extension = if is_last { "    " } else { "│   " };

        let label = match child.size {
            Some(size) => format!("{} ({})", child.name, human_size(size, decimal)),
            None => child.name.clone(),
        };
        println!("{prefix}{connector}{label}");
        print_skeleton_children(child, &format!("{prefix}{extension}"), decimal);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::{self, File};
    use std::io::Write;

    /// Helper to set up a directory like:
    /// root/
    /// ├── file1.txt (100 bytes)
    /// ├── file2.txt (50 bytes)
    /// └── sub/
    ///     └── file3.txt (30 bytes)
    /// Total: 180 bytes for root, 30 for sub.
    fn setup_test_dir(root: &Path) {
        let mut f1 = File::create(root.join("file1.txt")).unwrap();
        f1.write_all(&[0; 100]).unwrap();

        let mut f2 = File::create(root.join("file2.txt")).unwrap();
        f2.write_all(&[0; 50]).unwrap();

        let sub = root.join("sub");
        fs::create_dir(&sub).unwrap();
        let mut f3 = File::create(sub.join("file3.txt")).unwrap();
        f3.write_all(&[0; 30]).unwrap();
    }

    #[test]
    fn test_build_tree_includes_files_sorted_by_size() {
        let temp_dir = tempfile::tempdir().unwrap();
        setup_test_dir(temp_dir.path());
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
        let temp_dir = tempfile::tempdir().unwrap();
        setup_test_dir(temp_dir.path());
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
        let temp_dir = tempfile::tempdir().unwrap();
        setup_test_dir(temp_dir.path());
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
    fn test_min_percent_filters_small_descendants() {
        // root/big.txt (1000), root/small.txt (10),
        // root/sub/huge.txt (990), root/sub/tiny.txt (10).
        let temp_dir = tempfile::tempdir().unwrap();
        let root = temp_dir.path();

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

        let cache = DirSizeCache::new();
        cache.add(temp_dir.path());
        cache.wait();

        // With min_percent=2, tiny.txt (1% of sub) is dropped; huge.txt stays.
        let tree = build_tree(&[temp_dir.path().to_path_buf()], &cache, 0, true, 2.0);

        let root = &tree[0];
        let sub = root.children.iter().find(|c| c.name == "sub").unwrap();
        assert_eq!(sub.children.len(), 1);
        assert_eq!(sub.children[0].name, "huge.txt");
        assert_eq!(sub.children[0].size, 990);
    }

    #[test]
    fn test_skeleton_merges_inputs_from_common_ancestor() {
        // root/
        // ├── a/      (file: 100 bytes)
        // └── x/
        //     └── b/  (file: 30 bytes)
        // Inputs root/a and root/x/b -> skeleton rooted at root with
        // branches a and x -> b.
        let temp_dir = tempfile::tempdir().unwrap();
        let root = temp_dir.path();

        let a = root.join("a");
        fs::create_dir(&a).unwrap();
        let mut fa = File::create(a.join("file.txt")).unwrap();
        fa.write_all(&[0; 100]).unwrap();

        let x = root.join("x");
        fs::create_dir(&x).unwrap();
        let b = x.join("b");
        fs::create_dir(&b).unwrap();
        let mut fb = File::create(b.join("file.txt")).unwrap();
        fb.write_all(&[0; 30]).unwrap();

        let cache = DirSizeCache::new();
        cache.add(&a);
        cache.add(&b);
        cache.wait();

        let skeleton = build_skeleton(&[a.clone(), b.clone()], &cache, 0, true, 0.0);

        assert_eq!(skeleton.name, root.file_name().unwrap().to_str().unwrap());
        assert_eq!(skeleton.size, None);

        // Sorted largest first: branch `a` (100) before branch `x` (30).
        assert_eq!(skeleton.children.len(), 2);
        assert_eq!(skeleton.children[0].name, "a");
        assert_eq!(skeleton.children[0].size, Some(100));
        // Realized: a's file.txt hangs beneath the input leaf.
        assert_eq!(skeleton.children[0].children.len(), 1);
        assert_eq!(skeleton.children[0].children[0].name, "file.txt");
        assert_eq!(skeleton.children[0].children[0].size, Some(100));

        assert_eq!(skeleton.children[1].name, "x");
        assert_eq!(skeleton.children[1].size, None);
        assert_eq!(skeleton.children[1].children.len(), 1);
        assert_eq!(skeleton.children[1].children[0].name, "b");
        assert_eq!(skeleton.children[1].children[0].size, Some(30));
        // b is realized too: its file.txt is a leaf beneath it.
        assert_eq!(skeleton.children[1].children[0].children.len(), 1);
        assert_eq!(
            skeleton.children[1].children[0].children[0].name,
            "file.txt"
        );
        assert_eq!(skeleton.children[1].children[0].children[0].size, Some(30));
    }

    #[test]
    fn test_skeleton_input_that_is_the_ancestor_carries_root_size() {
        // Inputs root and root/a: the common ancestor is root itself, so
        // the skeleton root carries root's size and has a single branch.
        let temp_dir = tempfile::tempdir().unwrap();
        let root = temp_dir.path();

        let a = root.join("a");
        fs::create_dir(&a).unwrap();
        let mut fa = File::create(a.join("file.txt")).unwrap();
        fa.write_all(&[0; 10]).unwrap();

        let cache = DirSizeCache::new();
        cache.add(root);
        cache.add(&a);
        cache.wait();

        let skeleton = build_skeleton(&[root.to_path_buf(), a.clone()], &cache, 0, true, 0.0);

        assert_eq!(skeleton.size, Some(10));
        assert_eq!(skeleton.children.len(), 1);
        assert_eq!(skeleton.children[0].name, "a");
        assert_eq!(skeleton.children[0].size, Some(10));
        // The root input is realized: a's file.txt hangs beneath it.
        assert_eq!(skeleton.children[0].children.len(), 1);
        assert_eq!(skeleton.children[0].children[0].name, "file.txt");
    }

    #[test]
    fn test_skeleton_file_inputs() {
        // Two files in different dirs become leaves of the skeleton.
        let temp_dir = tempfile::tempdir().unwrap();
        let root = temp_dir.path();

        let a = root.join("a");
        fs::create_dir(&a).unwrap();
        let fa = a.join("f1.txt");
        let mut f1 = File::create(&fa).unwrap();
        f1.write_all(&[0; 10]).unwrap();

        let b = root.join("b");
        fs::create_dir(&b).unwrap();
        let fb = b.join("f2.txt");
        let mut f2 = File::create(&fb).unwrap();
        f2.write_all(&[0; 20]).unwrap();

        let cache = DirSizeCache::new();
        cache.add(&fa);
        cache.add(&fb);
        cache.wait();

        let skeleton = build_skeleton(&[fa.clone(), fb.clone()], &cache, 0, true, 0.0);

        assert_eq!(skeleton.children.len(), 2);
        // f2 (20) sorts before f1 (10).
        assert_eq!(skeleton.children[0].name, "b");
        assert_eq!(skeleton.children[0].children[0].name, "f2.txt");
        assert_eq!(skeleton.children[0].children[0].size, Some(20));
        assert_eq!(skeleton.children[1].name, "a");
        assert_eq!(skeleton.children[1].children[0].name, "f1.txt");
        assert_eq!(skeleton.children[1].children[0].size, Some(10));
    }

    #[test]
    fn test_skeleton_realized_branch_matches_single_input_tree() {
        // The realized branch for an input must be identical to the
        // single-input tree for that input under the same flags.
        let temp_dir = tempfile::tempdir().unwrap();
        let root = temp_dir.path();
        let a = root.join("a");
        fs::create_dir(&a).unwrap();
        setup_test_dir(&a);
        let b = root.join("b");
        fs::create_dir(&b).unwrap();

        let cache = DirSizeCache::new();
        cache.add(&a);
        cache.add(&b);
        cache.wait();

        let single = build_tree(&[a.clone()], &cache, 2, true, 0.0);
        let skeleton = build_skeleton(&[a.clone(), b.clone()], &cache, 2, true, 0.0);

        let branch = skeleton.children.iter().find(|c| c.name == "a").unwrap();
        assert_eq!(branch.size, Some(single[0].size));
        assert_eq!(branch.children, tree_to_skeleton(&single[0].children));
    }

    #[test]
    fn test_skeleton_realization_respects_depth() {
        let temp_dir = tempfile::tempdir().unwrap();
        let root = temp_dir.path();
        let a = root.join("a");
        fs::create_dir(&a).unwrap();
        setup_test_dir(&a);
        let b = root.join("b");
        fs::create_dir(&b).unwrap();

        let cache = DirSizeCache::new();
        cache.add(&a);
        cache.add(&b);
        cache.wait();

        let skeleton = build_skeleton(&[a.clone(), b.clone()], &cache, 1, true, 0.0);

        let branch = skeleton.children.iter().find(|c| c.name == "a").unwrap();
        // depth=1: a's own children only; sub is terminal (no file3).
        assert_eq!(branch.children.len(), 3);
        let sub = branch.children.iter().find(|c| c.name == "sub").unwrap();
        assert!(sub.children.is_empty());
    }

    #[test]
    fn test_skeleton_realization_hides_files() {
        let temp_dir = tempfile::tempdir().unwrap();
        let root = temp_dir.path();
        let a = root.join("a");
        fs::create_dir(&a).unwrap();
        setup_test_dir(&a);
        let b = root.join("b");
        fs::create_dir(&b).unwrap();

        let cache = DirSizeCache::new();
        cache.add(&a);
        cache.add(&b);
        cache.wait();

        let skeleton = build_skeleton(&[a.clone(), b.clone()], &cache, 0, false, 0.0);

        let branch = skeleton.children.iter().find(|c| c.name == "a").unwrap();
        // Only `sub` - no file leaves in the realized branch.
        assert_eq!(branch.children.len(), 1);
        assert_eq!(branch.children[0].name, "sub");
        assert!(branch.children[0].children.is_empty());
    }

    #[test]
    fn test_skeleton_min_percent_filters_realized_branches() {
        let temp_dir = tempfile::tempdir().unwrap();
        let root = temp_dir.path();
        let a = root.join("a");
        fs::create_dir(&a).unwrap();
        let mut big = File::create(a.join("big.txt")).unwrap();
        big.write_all(&[0; 1000]).unwrap();
        let mut small = File::create(a.join("small.txt")).unwrap();
        small.write_all(&[0; 10]).unwrap();
        let b = root.join("b");
        fs::create_dir(&b).unwrap();

        let cache = DirSizeCache::new();
        cache.add(&a);
        cache.add(&b);
        cache.wait();

        // min_percent=50: small.txt (10 of 1010) is dropped from a's branch.
        let skeleton = build_skeleton(&[a.clone(), b.clone()], &cache, 0, true, 50.0);

        let branch = skeleton.children.iter().find(|c| c.name == "a").unwrap();
        assert_eq!(branch.children.len(), 1);
        assert_eq!(branch.children[0].name, "big.txt");
        assert_eq!(branch.children[0].size, Some(1000));
    }

    #[test]
    fn test_skeleton_nested_inputs_merge_without_duplicates() {
        // Inputs root/a and root/a/b: `b` is both a realized child of
        // `a` and an input of its own; its branch must appear once with
        // merged children.
        let temp_dir = tempfile::tempdir().unwrap();
        let root = temp_dir.path();
        let a = root.join("a");
        fs::create_dir(&a).unwrap();
        let mut big = File::create(a.join("big.txt")).unwrap();
        big.write_all(&[0; 100]).unwrap();
        let b = a.join("b");
        fs::create_dir(&b).unwrap();
        let mut small = File::create(b.join("small.txt")).unwrap();
        small.write_all(&[0; 30]).unwrap();

        let cache = DirSizeCache::new();
        cache.add(&a);
        cache.add(&b);
        cache.wait();

        let skeleton = build_skeleton(&[a.clone(), b.clone()], &cache, 0, true, 0.0);

        // The common ancestor of root/a and root/a/b is root/a itself,
        // so the skeleton root carries a's size and its realized
        // children: big.txt and the merged `b` branch -> [small.txt].
        assert_eq!(skeleton.name, "a");
        assert_eq!(skeleton.size, Some(130));
        assert_eq!(skeleton.children.len(), 2);
        assert_eq!(skeleton.children[0].name, "big.txt");
        assert_eq!(skeleton.children[0].size, Some(100));
        let b_node = skeleton.children.iter().find(|c| c.name == "b").unwrap();
        assert_eq!(b_node.size, Some(30));
        assert_eq!(b_node.children.len(), 1);
        assert_eq!(b_node.children[0].name, "small.txt");
        assert_eq!(b_node.children[0].size, Some(30));
    }

    /// Helper: root/dir/file.txt (42 bytes) plus root/dir/sub, with
    /// `dir` and `file.txt` both used as inputs.
    fn setup_file_input_in_dir(root: &Path) -> (PathBuf, PathBuf, DirSizeCache) {
        let dir = root.join("dir");
        fs::create_dir(&dir).unwrap();
        let f = dir.join("file.txt");
        let mut file = File::create(&f).unwrap();
        file.write_all(&[0; 42]).unwrap();
        fs::create_dir(dir.join("sub")).unwrap();

        let cache = DirSizeCache::new();
        cache.add(&dir);
        cache.add(&f);
        cache.wait();
        (dir, f, cache)
    }

    #[test]
    fn test_file_input_inside_dir_input_appears_once() {
        // `file.txt` is both a cache child of `dir` and a file on
        // disk; the tree must list it once, not twice.
        let temp_dir = tempfile::tempdir().unwrap();
        let (dir, f, cache) = setup_file_input_in_dir(temp_dir.path());

        let tree = build_tree(&[dir.clone(), f], &cache, 0, true, 0.0);
        assert_eq!(tree.len(), 2);

        let dir_node = tree.iter().find(|n| n.name == "dir").unwrap();
        assert_eq!(dir_node.children.len(), 2);
        let file_children: Vec<_> = dir_node
            .children
            .iter()
            .filter(|c| c.name == "file.txt")
            .collect();
        assert_eq!(file_children.len(), 1);
        assert_eq!(file_children[0].size, 42);
    }

    #[test]
    fn test_skeleton_file_input_inside_dir_input_appears_once() {
        // Same setup through the skeleton: `file.txt`'s chain
        // (dir/file.txt) merges into `dir`'s realized branch, which
        // already carries the cache child — it must not duplicate.
        let temp_dir = tempfile::tempdir().unwrap();
        let (dir, f, cache) = setup_file_input_in_dir(temp_dir.path());

        let skeleton = build_skeleton(&[dir.clone(), f], &cache, 0, true, 0.0);

        // The common ancestor of dir and dir/file.txt is dir itself, so
        // the skeleton root carries dir's size; `file.txt`'s chain
        // merges into its realized branch, which already carries the
        // cache child — it must not duplicate.
        assert_eq!(skeleton.name, "dir");
        assert_eq!(skeleton.size, Some(42));
        assert_eq!(skeleton.children.len(), 2);
        assert_eq!(skeleton.children[0].name, "file.txt");
        assert_eq!(skeleton.children[0].size, Some(42));
        assert_eq!(skeleton.children[1].name, "sub");
        assert_eq!(skeleton.children[0].children.len(), 0);
    }
}
