mod entry;
pub use entry::*;
mod connection;
pub use connection::*;
mod crud;
mod display;
pub use display::*;

pub mod zoxide;
pub use fist_types::filters::DbSortOrder;

use crate::{abspath::AbsPath, errors::DbError, run::state::TASKS};
use cba::{bait::ResultExt, bath::PathExt};

pub type Epoch = i64;

impl Pool {
    /// Spawn a background task that records a directory or file visit.
    /// `folder`: `true` for dirs table, `false` for files.
    pub fn bump_path(
        self,
        folder: bool,
        path: AbsPath,
    ) {
        TASKS::spawn(async move {
            let table = if folder {
                DbTable::dirs
            } else {
                DbTable::files
            };

            match self.get_conn(table).await {
                Ok(mut conn) => {
                    if let Err(e) = conn.bump_path(path, 1).await {
                        log::error!("Error bumping entry: {}", e);
                    }
                }
                Err(e) => {
                    log::error!("Error getting connection: {}", e);
                }
            }
        });
    }

    /// Spawn a background task that sets an alias for a path, creating the entry if needed.
    pub fn set_path_alias(
        self,
        path: AbsPath,
        alias: String,
        table: DbTable,
    ) {
        TASKS::spawn(async move {
            match self.get_conn(table).await {
                Ok(mut conn) => {
                    if let Err(e) = conn.bump_path(path.clone(), 1).await {
                        log::error!("Error bumping entry: {}", e);
                        return;
                    }

                    // Update the alias
                    if let Err(e) = conn.set_alias(&path, &alias).await {
                        log::error!("Error setting alias: {}", e);
                    }
                }
                Err(e) => {
                    log::error!("Error getting connection: {}", e);
                }
            }
        });
    }
}

impl Connection {
    pub async fn push_files_and_folders(
        &mut self,
        paths: impl IntoIterator<Item = AbsPath>,
    ) -> Result<(), DbError> {
        // Delegate to the new function with an amount of 1
        self.bump_files_and_folders_n(paths, 1).await
    }

    pub async fn bump_files_and_folders_n(
        &mut self,
        paths: impl IntoIterator<Item = AbsPath>,
        count: i32,
    ) -> Result<(), DbError> {
        let mut files = Vec::new();
        let mut dirs = Vec::new();

        for path in paths {
            if path.is_dir() {
                dirs.push(path);
            } else if path.is_file() {
                files.push(path);
            }
            // else {
            //     dbog!(
            //         "Could not determine filetype of '{}'",
            //         path.to_string_lossy()
            //     )
            // }
        }

        self.switch_table(DbTable::dirs);
        for f in dirs {
            self.bump_path(f, count).await?;
            // todo: maybe should also update the abspath?
        }

        self.switch_table(DbTable::files);
        for f in files {
            self.bump_path(f, count).await?;
        }

        Ok(())
    }

    /// Bump an existing entry (or insert a new one if missing).
    ///
    /// Delegates to [`Connection::bump_entry`] which branches on `use_atime`.
    /// `count` is clamped to `≥ -(entry.count)` so the count never goes negative.
    pub async fn bump_path(
        &mut self,
        path: AbsPath,
        count: i32,
    ) -> Result<(), DbError> {
        // name doesn't really do anything
        let name = path.basename();

        match self.get_entry(&path).await? {
            Some(e) => {
                let count = count.max(-(e.count.abs()));
                self.bump_entry(&e, count).await
            }
            // This variant is only called on actual paths since apps are prepopulated
            None => {
                let canonical = path
                    .canonicalize()
                    .ok()
                    .and_then(|c| (c.as_path() != path.as_path()).then_some(c));
                let mut entry = Entry::new(name, path);
                if let Some(p) = canonical {
                    entry.cmd = p.into()
                }
                self.set_entry(&entry).await
            }
        }
    }

    pub async fn get_entries(
        &mut self,
        sort: DbSortOrder,
        config: &zoxide::HistoryConfig,
        table: DbTable,
    ) -> Result<Vec<Entry>, DbError> {
        let mut entries = self.get_entries_range(0, 0, sort).await.elog()?;
        entries.retain(|e| config.filter(e, table));

        if matches!(sort, DbSortOrder::frecency) && entries.len() > config.prune_max {
            self.prune_tail(&entries, config.prune_max, config.prune_min)
                .await
                ._elog();
            entries.truncate(config.prune_min);
        }

        Ok(entries)
    }

    /// Prune entries beyond `prune_min` when total count exceeds `prune_max`.
    /// Requires (and is usually called when) entries are already sorted by frecency (best first).
    /// Returns the number of rows removed.
    pub async fn prune_tail(
        &mut self,
        entries: &[Entry],
        prune_max: usize,
        prune_min: usize,
    ) -> Result<u64, DbError> {
        if entries.len() > prune_max {
            let to_remove: Vec<_> = entries[prune_min..]
                .iter()
                .map(|e| e.path.clone())
                .collect();
            self.remove_entries(&to_remove).await
        } else {
            Ok(0)
        }
    }

    // remove all entries whose canonical path is contained in targets
    pub async fn remove_paths(
        &mut self,
        targets: &[AbsPath],
    ) -> Result<usize, DbError> {
        let mut removed = 0;

        let entries: Vec<Entry> = self.get_entries_range(0, 0, DbSortOrder::none).await?;

        for target in targets {
            for entry in &entries {
                let matches = entry
                    .path
                    .canonicalize()
                    .map(|p| p == target.as_path())
                    .unwrap_or(entry.path == *target);

                if matches {
                    self.delete_entry(&entry.path).await?;
                    removed += 1;
                }
            }
        }

        Ok(removed)
    }
}
