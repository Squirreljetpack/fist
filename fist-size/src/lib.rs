use crossbeam_channel::{Sender, unbounded};
use dashmap::DashMap;
use rayon::prelude::*;
use std::collections::BTreeMap;
use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::thread::{self, JoinHandle};

enum QueueMessage {
    Process(PathBuf),
    Shutdown,
}

// ==========================================
// PathTrie Node and Structure Implementation
// ==========================================

#[derive(Default, Debug)]
struct PathTrieNode {
    active: bool,
    deferred: bool,
    active_descendants: usize,
    // BTreeMap handles the O(log N) lookups by the OsString component
    children: BTreeMap<OsString, PathTrieNode>,
}

#[derive(Default, Debug)]
struct PathTrie {
    root: PathTrieNode,
}

impl PathTrie {
    fn is_empty(&self) -> bool {
        self.root.active_descendants == 0 && !self.root.active
    }

    fn insert(
        &mut self,
        path: &Path,
    ) -> Option<PathBuf> {
        let comps: Vec<OsString> = path
            .components()
            .map(|c| c.as_os_str().to_os_string())
            .collect();

        let (became_active, _) = Self::insert_recursive(&mut self.root, &comps);

        if became_active {
            Some(path.to_path_buf())
        } else {
            None
        }
    }

    fn insert_recursive(
        node: &mut PathTrieNode,
        comps: &[OsString],
    ) -> (bool, bool) {
        if node.active || node.deferred {
            return (false, true);
        }

        if comps.is_empty() {
            if node.active_descendants > 0 {
                node.deferred = true;
                // A deferred node overrides previously deferred descendants.
                Self::clear_deferred_descendants(node);
                return (false, true);
            } else {
                node.active = true;
                return (true, true);
            }
        }

        let comp = &comps[0];

        let child = node.children.entry(comp.clone()).or_default();

        let (child_became_active, is_covered) = Self::insert_recursive(child, &comps[1..]);

        if child_became_active {
            node.active_descendants += 1;
        }

        (child_became_active, is_covered)
    }

    fn clear_deferred_descendants(node: &mut PathTrieNode) {
        for child in node.children.values_mut() {
            child.deferred = false;
            Self::clear_deferred_descendants(child);
        }
    }

    fn remove(
        &mut self,
        path: &Path,
    ) -> Option<PathBuf> {
        let comps: Vec<OsString> = path
            .components()
            .map(|c| c.as_os_str().to_os_string())
            .collect();

        let mut queued = None;
        let mut current_path = PathBuf::new();

        Self::remove_recursive(&mut self.root, &comps, &mut current_path, &mut queued);
        queued
    }

    /// Returns `true` if the parent should decrement its `active_descendants` count.
    fn remove_recursive(
        node: &mut PathTrieNode,
        comps: &[OsString],
        current_path: &mut PathBuf,
        queued: &mut Option<PathBuf>,
    ) -> bool {
        if comps.is_empty() {
            // GUARANTEE: The node being removed is always an active leaf node.
            node.active = false;
            return true; // Signal parent to decrement its count
        }

        let comp = &comps[0];
        current_path.push(comp);

        let mut propagate_decrement = false;

        if let Some(child) = node.children.get_mut(comp) {
            propagate_decrement = Self::remove_recursive(child, &comps[1..], current_path, queued);

            // Because active nodes are leaves, if it has no active state,
            // no deferred state, and 0 active descendants, it is perfectly dead.
            if !child.active && !child.deferred && child.active_descendants == 0 {
                node.children.remove(comp);
            }
        }

        current_path.pop();

        if propagate_decrement {
            node.active_descendants -= 1;

            if node.active_descendants == 0 && node.deferred {
                node.deferred = false;
                node.active = true;

                // At this exact moment, all children have naturally pruned themselves.
                // This node is now a strict leaf, ready for processing.
                *queued = Some(current_path.clone());

                return false; // Cascade absorbed. Stop propagating the decrement upward.
            }

            return true; // Keep propagating the decrement to higher ancestors
        }

        false
    }
}

// ==========================================
// Task Management Implementation
// ==========================================

#[derive(Default, Debug)]
struct TaskState {
    active: PathTrie,
}

struct TaskManager {
    state: Mutex<TaskState>,
    cvar: Condvar,
    sender: Sender<QueueMessage>,
    cancel_token: AtomicBool,
}

impl TaskManager {
    fn new(sender: Sender<QueueMessage>) -> Self {
        Self {
            state: Mutex::new(TaskState::default()),
            cvar: Condvar::new(),
            sender,
            cancel_token: AtomicBool::new(false),
        }
    }

    fn add(
        &self,
        path: PathBuf,
        worker_sizes: &Arc<DashMap<PathBuf, u64>>,
    ) {
        let mut state = self.state.lock().unwrap();

        if self.cancel_token.load(Ordering::Relaxed) {
            return;
        }

        if worker_sizes.contains_key(&path) {
            return;
        }

        if let Some(queued_path) = state.active.insert(&path) {
            let _ = self.sender.send(QueueMessage::Process(queued_path));
        }
    }

    fn cancel_and_wait(&self) {
        let mut state = self.state.lock().unwrap();
        self.cancel_token.store(true, Ordering::Relaxed);

        while !state.active.is_empty() {
            state = self.cvar.wait(state).unwrap();
        }

        self.cancel_token.store(false, Ordering::Relaxed);
    }
}

struct TaskGuard {
    manager: Arc<TaskManager>,
    path: PathBuf,
}

impl Drop for TaskGuard {
    fn drop(&mut self) {
        let mut state = self.manager.state.lock().unwrap();

        if let Some(queued_path) = state.active.remove(&self.path) {
            let _ = self.manager.sender.send(QueueMessage::Process(queued_path));
        }

        if state.active.is_empty() {
            self.manager.cvar.notify_all();
        }
    }
}

// ==========================================
// Main DirSizeCache Implementation
// ==========================================

pub struct DirSizeCache {
    sizes: Arc<DashMap<PathBuf, u64>>,
    sender: Sender<QueueMessage>,
    manager: Arc<TaskManager>,
    worker_handle: Option<JoinHandle<()>>,
}

impl Default for DirSizeCache {
    fn default() -> Self {
        Self::new()
    }
}

impl DirSizeCache {
    pub fn new() -> Self {
        let sizes = Arc::new(DashMap::with_shard_amount(128));
        let (sender, receiver) = unbounded::<QueueMessage>();
        let manager = Arc::new(TaskManager::new(sender.clone()));

        let worker_sizes = sizes.clone();
        let worker_manager = manager.clone();

        let handle = thread::spawn(move || {
            for message in receiver {
                match message {
                    QueueMessage::Process(path) => {
                        if worker_sizes.contains_key(&path) {
                            continue;
                        }
                        // we process files in the add mechanism, we only expect to process directories here

                        let sizes_ref = worker_sizes.clone();
                        let manager_ref = worker_manager.clone();

                        rayon::spawn(move || {
                            let _guard = TaskGuard {
                                manager: manager_ref.clone(),
                                path: path.clone(),
                            };
                            Self::compute_size(&path, &sizes_ref, &manager_ref.cancel_token);
                        });
                    }
                    QueueMessage::Shutdown => {
                        break;
                    }
                }
            }
        });

        Self {
            sizes,
            sender,
            manager,
            worker_handle: Some(handle),
        }
    }

    pub fn add<P: AsRef<Path>>(
        &self,
        path: P,
    ) {
        let path_ref = path.as_ref();

        if let Ok(m) = std::fs::symlink_metadata(path_ref)
            && !m.is_dir()
        {
            self.sizes.insert(path_ref.to_path_buf(), m.len());
            return;
        }

        self.manager.add(path_ref.to_path_buf(), &self.sizes);
    }

    pub fn get_path<P: AsRef<Path>>(
        &self,
        path: P,
    ) -> Option<u64> {
        self.sizes.get(path.as_ref()).map(|v| *v)
    }

    pub fn iter(&self) -> impl Iterator<Item = (PathBuf, u64)> + '_ {
        self.sizes
            .iter()
            .map(|entry| (entry.key().clone(), *entry.value()))
    }

    pub fn wait(&self) {
        let mut state = self.manager.state.lock().unwrap();
        while !state.active.is_empty() {
            state = self.manager.cvar.wait(state).unwrap();
        }
    }

    pub fn clear(&self) {
        self.manager.cancel_and_wait();
        self.sizes.clear();
    }

    fn compute_size(
        path: &Path,
        sizes: &DashMap<PathBuf, u64>,
        cancel_token: &AtomicBool,
    ) -> u64 {
        if cancel_token.load(Ordering::Relaxed) {
            return 0;
        }

        if let Some(cached) = sizes.get(path) {
            return *cached;
        }

        let entries: Vec<_> = match std::fs::read_dir(path) {
            Ok(dir) => dir.filter_map(Result::ok).collect(),
            Err(_) => return 0,
        };

        let mut total_size = 0;
        let mut sub_dirs = Vec::new();

        for entry in entries {
            if cancel_token.load(Ordering::Relaxed) {
                return 0;
            }

            if let Ok(ft) = entry.file_type() {
                if ft.is_dir() {
                    sub_dirs.push(entry.path());
                } else if let Ok(m) = entry.metadata() {
                    total_size += m.len();
                }
            }
        }

        let sub_dirs_size: u64 = sub_dirs
            .par_iter()
            .map(|dir| Self::compute_size(dir, sizes, cancel_token))
            .sum();

        let final_size = total_size + sub_dirs_size;

        if !cancel_token.load(Ordering::Relaxed) {
            sizes.insert(path.to_path_buf(), final_size);
        }

        final_size
    }
}

impl Drop for DirSizeCache {
    fn drop(&mut self) {
        self.manager.cancel_and_wait();

        let _ = self.sender.send(QueueMessage::Shutdown);
        if let Some(handle) = self.worker_handle.take() {
            let _ = handle.join();
        }
    }
}
// ==========================================
// Tests
// ==========================================

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::{self, File};
    use std::io::Write;
    use tempfile::TempDir;

    fn setup_test_dir() -> TempDir {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();

        let mut f1 = File::create(root.join("file1.txt")).unwrap();
        f1.write_all(&[0; 10]).unwrap();

        let dir_a = root.join("dir_a");
        fs::create_dir(&dir_a).unwrap();

        let mut f2 = File::create(dir_a.join("file2.txt")).unwrap();
        f2.write_all(&[0; 20]).unwrap();

        let dir_b = dir_a.join("dir_b");
        fs::create_dir(&dir_b).unwrap();

        let mut f3 = File::create(dir_b.join("file3.txt")).unwrap();
        f3.write_all(&[0; 30]).unwrap();

        dir
    }

    #[test]
    fn test_single_path_size() {
        let temp_dir = setup_test_dir();
        let cache = DirSizeCache::new();

        cache.add(temp_dir.path());
        cache.wait();

        assert_eq!(cache.get_path(temp_dir.path()), Some(60));
    }

    #[test]
    fn test_sub_directory_sizes_are_cached() {
        let temp_dir = setup_test_dir();
        let cache = DirSizeCache::new();

        cache.add(temp_dir.path());
        cache.wait();

        assert_eq!(cache.get_path(temp_dir.path()), Some(60));

        let dir_a = temp_dir.path().join("dir_a");
        assert_eq!(cache.get_path(&dir_a), Some(50));

        let dir_b = dir_a.join("dir_b");
        assert_eq!(cache.get_path(&dir_b), Some(30));
    }

    #[test]
    fn test_concurrent_stampede() {
        let temp_dir = setup_test_dir();
        let cache = DirSizeCache::new();

        for _ in 0..10 {
            cache.add(temp_dir.path().join("dir_a"));
            cache.add(temp_dir.path());
        }

        cache.wait();

        assert_eq!(cache.get_path(temp_dir.path()), Some(60));
        assert_eq!(cache.get_path(temp_dir.path().join("dir_a")), Some(50));
    }

    #[test]
    fn test_clear_functionality() {
        let temp_dir = setup_test_dir();
        let cache = DirSizeCache::new();

        cache.add(temp_dir.path());
        cache.wait();

        assert_eq!(cache.get_path(temp_dir.path()), Some(60));

        for _ in 0..50 {
            cache.add(temp_dir.path().join("dir_a"));
        }
        cache.clear();

        assert_eq!(cache.get_path(temp_dir.path()), None);

        cache.add(temp_dir.path());
        cache.wait();
        assert_eq!(cache.get_path(temp_dir.path()), None);
    }

    #[test]
    fn test_iter_empty() {
        let cache = DirSizeCache::new();
        assert_eq!(cache.iter().count(), 0);
    }

    #[test]
    fn test_iter_yields_computed_entries() {
        let temp_dir = setup_test_dir();
        let cache = DirSizeCache::new();

        cache.add(temp_dir.path());
        cache.wait();

        let entries: Vec<(PathBuf, u64)> = cache.iter().collect();
        let by_path: BTreeMap<PathBuf, u64> = entries.into_iter().collect();

        assert_eq!(by_path.get(temp_dir.path()).copied(), Some(60));
        assert_eq!(
            by_path.get(&temp_dir.path().join("dir_a")).copied(),
            Some(50)
        );
        assert_eq!(
            by_path
                .get(&temp_dir.path().join("dir_a").join("dir_b"))
                .copied(),
            Some(30)
        );
    }

    #[test]
    fn test_iter_after_clear() {
        let temp_dir = setup_test_dir();
        let cache = DirSizeCache::new();

        cache.add(temp_dir.path());
        cache.wait();
        assert!(cache.iter().count() > 0);

        cache.clear();
        assert_eq!(cache.iter().count(), 0);
    }

    #[test]
    fn test_iter_yields_explicitly_added_file() {
        let temp_dir = setup_test_dir();
        let cache = DirSizeCache::new();

        cache.add(temp_dir.path());
        let explicit_file = temp_dir.path().join("file1.txt");
        cache.add(&explicit_file);
        cache.wait();

        assert_eq!(cache.get_path(&explicit_file), Some(10));

        let entries: Vec<(PathBuf, u64)> = cache.iter().collect();
        let by_path: BTreeMap<PathBuf, u64> = entries.into_iter().collect();

        assert_eq!(by_path.get(&explicit_file).copied(), Some(10));
        let not_stored = temp_dir.path().join("dir_a").join("file2.txt");
        assert_eq!(by_path.get(&not_stored), None);
    }
}
