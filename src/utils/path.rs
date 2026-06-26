use cba::{bath::PathExt, bog::BogOkExt};
use indexmap::IndexSet;
use std::{
    collections::HashSet,
    env::current_dir,
    path::{Path, PathBuf},
};

use crate::abspath::AbsPath;

pub fn paths_base<P>(paths: impl IntoIterator<Item = P>) -> PathBuf
where
    P: AsRef<std::path::Path>,
{
    let mut iter = paths
        .into_iter()
        .map(|p| p.as_ref().abs(current_dir().__ebog()))
        .peekable();

    let first = match iter.peek() {
        Some(p) => p.clone(),
        None => return PathBuf::new(),
    };

    let mut components: Vec<_> = first.components().collect();

    for path in iter {
        let mut i = 0;
        for (a, b) in components.iter().zip(path.components()) {
            if a != &b {
                break;
            }
            i += 1;
        }
        components.truncate(i);
        if components.is_empty() {
            break;
        }
    }

    components.iter().collect()
}

/// Resolve a symlink chain from `path`, returning each target in order.
///
/// Returns `[]` when `path` is not a symlink. Walks until it hits a non-symlink,
/// revisits an already-seen target (cycle), or reaches `MAX_DEPTH` (POSIX `MAXSYMLINKS`).
pub fn follow_symlink_chain(path: &Path) -> Vec<AbsPath> {
    const MAX_DEPTH: usize = 40; // POSIX MAXSYMLINKS
    let mut chain = Vec::new();
    let mut seen: HashSet<AbsPath> = HashSet::new();
    let mut current = path.to_path_buf();

    for _ in 0..MAX_DEPTH {
        match std::fs::read_link(&current) {
            Ok(target) => {
                let resolved = if target.is_absolute() {
                    target
                } else if let Some(parent) = current.parent() {
                    parent.join(&target)
                } else {
                    target
                };
                let base = current.parent().unwrap_or_else(|| Path::new(""));
                let abs = AbsPath::new_unchecked(resolved.abs(base));
                if !seen.insert(abs.clone()) {
                    break;
                }
                chain.push(abs);
                current = resolved;
            }
            Err(_) => break,
        }
    }

    chain
}

/// Expand symlinks in `paths` into the deletion queue used by `--follow`.
///
/// The input itself is always inserted first; non-symlinks pass through unchanged.
/// `recursive = false` replaces each symlink with its first chain target;
/// `recursive = true` walks every target in chain order. Results are deduplicated
/// and stable-ordered via [`IndexSet`]; a chain that hits an already-queued target
/// is truncated there (cycle break).
pub fn expand_follow(
    paths: impl IntoIterator<Item = PathBuf>,
    recursive: bool,
) -> Vec<PathBuf> {
    let mut queue: IndexSet<PathBuf> = IndexSet::new();

    for p in paths {
        if !p.is_symlink() {
            queue.insert(p);
            continue;
        }

        let chain = follow_symlink_chain(&p);
        queue.insert(p);
        if chain.is_empty() {
            continue;
        }

        if recursive {
            for target in &chain {
                if !queue.insert(target.into()) {
                    // Already represented further down the queue; the rest of the
                    // chain after this target has been added by an earlier input.
                    break;
                }
            }
        } else {
            queue.insert(chain.into_iter().next().unwrap().into());
        }
    }

    queue.into_iter().collect()
}

#[cfg(unix)]
#[cfg(test)]
mod tests {
    use super::*;

    // -----------------------------------------------------------------------
    // paths_base
    // -----------------------------------------------------------------------

    /// No inputs → empty path (the identity element for the operation).
    #[test]
    fn paths_base_empty() {
        let paths: [PathBuf; 0] = [];
        assert_eq!(paths_base(paths), PathBuf::new());
    }

    /// A single path is its own common prefix. The function abs-ifies inputs
    /// before comparing components, so we normalise the expected value the
    /// same way rather than comparing against a bare literal.
    #[test]
    fn paths_base_single() {
        let cwd = current_dir().unwrap();
        let input = PathBuf::from("/a/b/c");
        let result = paths_base([input.clone()]);
        let expected = input.abs(&cwd).normalize();
        assert_eq!(result, expected);
    }

    /// The common prefix stops at the last component shared by *all* paths.
    /// "/a/b/c/d", "/a/b/c/e", "/a/b/c/f/g" → "/a/b/c".
    #[test]
    fn paths_base_common_prefix() {
        let cwd = current_dir().unwrap();
        let paths = [
            PathBuf::from("/a/b/c/d"),
            PathBuf::from("/a/b/c/e"),
            PathBuf::from("/a/b/c/f/g"),
        ];
        let result = paths_base(&paths);
        let expected = PathBuf::from("/a/b/c").abs(&cwd).normalize();
        assert_eq!(result, expected);
    }

    /// Identical inputs share every component, so the result equals the input.
    #[test]
    fn paths_base_identical_inputs() {
        let cwd = current_dir().unwrap();
        let p = PathBuf::from("/x/y/z");
        let result = paths_base([p.clone(), p.clone()]);
        let expected = p.abs(&cwd).normalize();
        assert_eq!(result, expected);
    }

    /// Paths that diverge at the root share only "/", so the result is "/".
    /// Absolute paths abs-ify to themselves, so no cwd adjustment needed here.
    #[test]
    fn paths_base_no_common_prefix() {
        let result = paths_base([PathBuf::from("/a/b"), PathBuf::from("/c/d")]);
        assert_eq!(result, PathBuf::from("/"));
    }

    // -----------------------------------------------------------------------
    // follow_symlink_chain
    // -----------------------------------------------------------------------

    /// A regular file is not a symlink, so the chain must be empty.
    #[test]
    fn follow_chain_regular_file_returns_empty() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("regular_file");
        std::fs::write(&file, b"hello").unwrap();

        assert!(follow_symlink_chain(&file).is_empty());
    }

    /// A single symlink yields a one-element chain containing its resolved target.
    #[test]
    fn follow_chain_single_hop() {
        let dir = tempfile::tempdir().unwrap();
        let target = dir.path().join("target");
        let link = dir.path().join("link");
        std::fs::write(&target, b"data").unwrap();
        std::os::unix::fs::symlink(&target, &link).unwrap();

        let chain = follow_symlink_chain(&link);

        assert_eq!(chain.len(), 1);
        assert_eq!(
            <AbsPath as AsRef<Path>>::as_ref(&chain[0]),
            target.abs(dir.path()).normalize().as_path(),
        );
    }

    /// A chain of two symlinks (link2 → link1 → target) produces two entries
    /// in traversal order: the intermediate link first, the final target second.
    #[test]
    fn follow_chain_multi_hop_order() {
        let dir = tempfile::tempdir().unwrap();
        let target = dir.path().join("real.txt");
        let link1 = dir.path().join("a");
        let link2 = dir.path().join("b");
        std::fs::write(&target, b"data").unwrap();
        std::os::unix::fs::symlink(&target, &link1).unwrap();
        std::os::unix::fs::symlink(&link1, &link2).unwrap();

        let chain = follow_symlink_chain(&link2);

        let abs = |p: &Path| p.abs(dir.path()).normalize();
        assert_eq!(chain.len(), 2);
        // First resolution: link2 → link1 (the intermediate symlink).
        assert_eq!(<AbsPath as AsRef<Path>>::as_ref(&chain[0]), abs(&link1));
        // Second resolution: link1 → target (the real file).
        assert_eq!(<AbsPath as AsRef<Path>>::as_ref(&chain[1]), abs(&target));
    }

    /// Absolute symlink targets are resolved directly without consulting the
    /// parent directory of the link.
    #[test]
    fn follow_chain_absolute_symlink_target() {
        let dir = tempfile::tempdir().unwrap();
        let target = dir.path().join("abs_target");
        std::fs::write(&target, b"abs").unwrap();
        let link = dir.path().join("abs_link");
        std::os::unix::fs::symlink(&target, &link).unwrap();

        let chain = follow_symlink_chain(&link);

        assert_eq!(chain.len(), 1);
        assert_eq!(
            <AbsPath as AsRef<Path>>::as_ref(&chain[0]),
            target.abs(dir.path()).normalize(),
        );
    }

    /// Relative symlink targets are resolved relative to the directory that
    /// contains the link, not relative to the current working directory.
    #[test]
    fn follow_chain_relative_symlink_target() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("file"), b"rel").unwrap();
        let link = dir.path().join("rel_link");
        std::os::unix::fs::symlink("file", &link).unwrap();

        let chain = follow_symlink_chain(&link);

        assert_eq!(chain.len(), 1);
        assert_eq!(
            <AbsPath as AsRef<Path>>::as_ref(&chain[0]),
            dir.path().join("file").abs(dir.path()).normalize(),
        );
    }

    /// A two-node cycle (a → b → a) is detected on the third read: after
    /// pushing b then a, the next attempt would re-read a which is already
    /// in `seen`, so the walk stops. Result: exactly two entries [b, a].
    #[test]
    fn follow_chain_two_node_cycle() {
        let dir = tempfile::tempdir().unwrap();
        let a = dir.path().join("cycle_a");
        let b = dir.path().join("cycle_b");
        std::os::unix::fs::symlink(&b, &a).unwrap();
        std::os::unix::fs::symlink(&a, &b).unwrap();

        let chain = follow_symlink_chain(&a);

        let abs = |p: &Path| p.abs(dir.path()).normalize();
        assert_eq!(chain.len(), 2);
        assert_eq!(<AbsPath as AsRef<Path>>::as_ref(&chain[0]), abs(&b));
        assert_eq!(<AbsPath as AsRef<Path>>::as_ref(&chain[1]), abs(&a));
    }

    /// A longer cycle a → d → a → … also truncates as soon as a repeated
    /// target is seen. Walk: start=a, read→d (push d), read→a (push a),
    /// read→d (d already in `seen` → break). Result: [d, a].
    #[test]
    fn follow_chain_longer_cycle_truncates_at_repeat() {
        let dir = tempfile::tempdir().unwrap();
        let a = dir.path().join("a");
        let b = dir.path().join("b");
        let c = dir.path().join("c");
        let d = dir.path().join("d");
        // a → d → a (two-node cycle embedded in a four-node graph)
        // b → c → b (separate cycle, never reached from a)
        std::os::unix::fs::symlink(&d, &a).unwrap();
        std::os::unix::fs::symlink(&c, &b).unwrap();
        std::os::unix::fs::symlink(&b, &c).unwrap();
        std::os::unix::fs::symlink(&a, &d).unwrap();

        let chain = follow_symlink_chain(&a);

        // Trace: a→d (push d), d→a (push a), a→d (d seen → stop).
        assert_eq!(chain.len(), 2);
    }

    /// A dangling symlink (target does not exist) still has readable link
    /// metadata. `read_link` succeeds, so the unresolved target path is
    /// pushed onto the chain before the next `read_link` call fails.
    /// Result: exactly one entry.
    #[test]
    fn follow_chain_broken_symlink_yields_one_entry() {
        let dir = tempfile::tempdir().unwrap();
        let link = dir.path().join("broken");
        std::os::unix::fs::symlink("nowhere", &link).unwrap();

        let chain = follow_symlink_chain(&link);

        assert_eq!(chain.len(), 1);
    }

    // -----------------------------------------------------------------------
    // expand_follow
    // -----------------------------------------------------------------------

    /// A non-symlink passes through unchanged and no extra entries are added.
    #[test]
    fn expand_follow_non_symlink_passes_through() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("plain.txt");
        std::fs::write(&file, b"data").unwrap();

        let result = expand_follow([file.clone()], true);

        assert_eq!(result, vec![file]);
    }

    /// With `recursive = false`, a symlink is followed exactly one step:
    /// the output is [input_link, immediate_target].
    #[test]
    fn expand_follow_non_recursive_adds_only_first_target() {
        let dir = tempfile::tempdir().unwrap();
        let target = dir.path().join("target");
        let link = dir.path().join("link");
        std::fs::write(&target, b"data").unwrap();
        std::os::unix::fs::symlink(&target, &link).unwrap();

        let result = expand_follow([link.clone()], false);

        assert_eq!(result.len(), 2);
        assert_eq!(result[0], link);
        assert_eq!(result[1], target.abs(dir.path()).normalize());
    }

    /// With `recursive = true`, every step of the chain is included in order:
    /// [input, chain[0], chain[1], …].
    #[test]
    fn expand_follow_recursive_includes_full_chain() {
        let dir = tempfile::tempdir().unwrap();
        let target = dir.path().join("real.txt");
        std::fs::write(&target, b"x").unwrap();
        let link1 = dir.path().join("a");
        let link2 = dir.path().join("b");
        std::os::unix::fs::symlink(&target, &link1).unwrap();
        std::os::unix::fs::symlink(&link1, &link2).unwrap();

        let result = expand_follow([link2.clone()], true);

        let abs = |p: &Path| p.abs(dir.path()).normalize();
        assert_eq!(result.len(), 3);
        assert_eq!(result[0], link2);
        assert_eq!(result[1], abs(&link1));
        assert_eq!(result[2], abs(&target));
    }

    /// Mixed input (plain file + symlink): non-symlinks go in first, then the
    /// symlink and its single target follow in input order.
    #[test]
    fn expand_follow_mixed_inputs_preserves_order() {
        let dir = tempfile::tempdir().unwrap();
        let plain = dir.path().join("plain");
        let target = dir.path().join("t");
        let link = dir.path().join("link");
        std::fs::write(&plain, b"x").unwrap();
        std::fs::write(&target, b"t").unwrap();
        std::os::unix::fs::symlink(&target, &link).unwrap();

        let result = expand_follow([plain.clone(), link.clone()], false);

        assert_eq!(result.len(), 3);
        assert_eq!(result[0], plain);
        assert_eq!(result[1], link);
        assert_eq!(result[2], target.abs(dir.path()).normalize());
    }

    /// Duplicate inputs and targets are collapsed: the first occurrence wins
    /// and subsequent duplicates are silently dropped.
    #[test]
    fn expand_follow_deduplicates_inputs_and_targets() {
        let dir = tempfile::tempdir().unwrap();
        let target = dir.path().join("target");
        std::fs::write(&target, b"d").unwrap();
        let link = dir.path().join("x");
        std::os::unix::fs::symlink(&target, &link).unwrap();

        // [link, link, target]: link expands to [link, target]; second link
        // and the standalone target are already queued.
        let result = expand_follow([link.clone(), link.clone(), target.clone()], false);

        assert_eq!(result.len(), 2);
        assert_eq!(result[0], link);
        assert_eq!(result[1], target.abs(dir.path()).normalize());
    }

    /// When two cyclic symlinks are both given as inputs, each one is queued
    /// exactly once. The queue for a→b→a plus b→a→b resolves to just [a, b].
    #[test]
    fn expand_follow_cyclic_inputs_each_queued_once() {
        let dir = tempfile::tempdir().unwrap();
        let a = dir.path().join("cycle_a");
        let b = dir.path().join("cycle_b");
        std::os::unix::fs::symlink(&b, &a).unwrap();
        std::os::unix::fs::symlink(&a, &b).unwrap();

        let result = expand_follow([a.clone(), b.clone()], true);

        // a's chain is [b, a]; b is new so it's pushed, then a is already
        // in the queue so the chain walk stops. b's chain is [a, b]; both
        // are already in the queue so nothing new is added.
        let abs = |p: &Path| p.abs(dir.path()).normalize();
        assert_eq!(result.len(), 2);
        assert_eq!(result[0], abs(&a));
        assert_eq!(result[1], abs(&b));
    }

    /// An empty iterator produces an empty result without panicking.
    #[test]
    fn expand_follow_empty_input() {
        let result = expand_follow(Vec::<PathBuf>::new(), true);
        assert!(result.is_empty());
    }
}
