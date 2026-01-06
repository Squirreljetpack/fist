mod entry;
pub use entry::*;
mod connection;
pub use connection::*;
mod crud;
mod display;
pub use display::*;

pub mod zoxide;
pub use crate::filters::DbSortOrder;

use crate::{abspath::AbsPath, errors::DbError};
use cli_boilerplate_automation::{bait::ResultExt, bath::PathExt};

pub type Epoch = i64;

impl Pool {
    pub fn bump(
        &self,
        folder: bool,
        path: AbsPath,
    ) {
        let pool = self.clone();

        tokio::spawn(async move {
            let table = if folder {
                DbTable::dirs
            } else {
                DbTable::files
            };

            match pool.get_conn(table).await {
                Ok(mut conn) => {
                    if let Err(e) = conn.bump_entry(path, 1).await {
                        log::error!("Error bumping entry: {}", e);
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
        let mut files = Vec::new();
        let mut dirs = Vec::new();

        for path in paths {
            if path.is_dir() {
                dirs.push(path);
            } else {
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
            self.bump_entry(f, 1).await?;
        }
        self.switch_table(DbTable::files);
        for f in files {
            self.bump_entry(f, 1).await?;
        }
        Ok(())
    }

    pub async fn bump_entry(
        &mut self,
        path: AbsPath,
        count: i32,
    ) -> Result<(), DbError> {
        let name = path.basename();

        match self.get_entry(&path).await? {
            Some(e) => {
                let count = count.max(-(e.count.abs()));
                self.bump(&path, count).await
            }
            None => {
                let entry = Entry::new(name, path);
                self.set_entry(&entry).await
            }
        }
    }

    pub async fn get_entries(
        &mut self,
        sort: DbSortOrder,
        filter: &zoxide::DbFilter,
    ) -> Result<Vec<Entry>, DbError> {
        let mut remove = vec![];
        let mut entries = self.get_entries_range(0, 0, sort).await.elog()?;
        entries.retain(|e| match filter.filter(&e.path, e.atime) {
            None => {
                remove.push(e.path.clone());
                false
            }
            Some(true) => true,
            _ => false,
        });

        if matches!(sort, DbSortOrder::frecency) {
            entries.sort_by_key(|e| std::cmp::Reverse(filter.score(e)));
        }
        self.remove_entries(&remove).await._elog();
        Ok(entries)
    }
}
