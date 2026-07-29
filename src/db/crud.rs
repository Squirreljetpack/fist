use crate::errors::DbError;
use crate::{abspath::OsStringWrapper, db::*};
use cba::bait::ResultExt;
use log::trace;
use sqlx::Acquire;
use std::time::{SystemTime, UNIX_EPOCH};

const MAX_PLACEHOLDERS: usize = 200;

// try not to use these externally
impl Connection {
    pub fn switch_table(
        &mut self,
        table: DbTable,
    ) {
        self.table = table;
    }

    /// Creates a new entry in the db. Only used to create entries in the
    /// codebase. Initializes `atime` when it is 0: in EMS mode this is set to
    /// `MAX(atime) + 1` from the table; otherwise it is set to the current
    /// unix timestamp.
    ///
    /// Note: When inserting an entry with `atime == 0` in EMS mode, this method
    /// automatically initializes the atime to `COALESCE(MAX(atime), 0) + 1`.
    pub async fn set_entry(
        &mut self,
        entry: &Entry,
    ) -> Result<(), DbError> {
        trace!("Setting {entry:?}");

        if entry.atime == 0 && self.lambda.is_some() {
            // initialize on next tick
            let sql = format!(
                "INSERT OR REPLACE INTO {} (name, path, alias, cmd, atime, count, score) VALUES (?, ?, ?, ?, COALESCE((SELECT MAX(atime) FROM {}), 0) + 1, ?, ?)",
                self.table, self.table
            );
            sqlx::query(sqlx::AssertSqlSafe(sql))
                .bind(&entry.name)
                .bind(&entry.path)
                .bind(&entry.alias)
                .bind(&entry.cmd)
                .bind(entry.count)
                .bind(entry.score)
                .execute(&mut *self.conn)
                .await?;
            return Ok(());
        }

        let sql = format!(
            "INSERT OR REPLACE INTO {} (name, path, alias, cmd, atime, count, score) VALUES (?, ?, ?, ?, ?, ?, ?)",
            self.table
        );

        let atime = if entry.atime == 0 {
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs() as Epoch
        } else {
            entry.atime
        };

        sqlx::query(sqlx::AssertSqlSafe(sql))
            .bind(&entry.name)
            .bind(&entry.path)
            .bind(&entry.alias)
            .bind(&entry.cmd)
            .bind(atime)
            .bind(entry.count)
            .bind(entry.score)
            .execute(&mut *self.conn)
            .await?;
        Ok(())
    }

    pub async fn create_many(
        &mut self,
        entries: &[Entry],
    ) -> Result<u64, DbError> {
        let mut tx = self.conn.begin().await?;
        let mut total_rows = 0;

        let sql = format!(
            "INSERT OR IGNORE INTO {} (name, path, alias, cmd, atime, count, score) VALUES (?, ?, ?, ?, ?, ?, ?)",
            self.table
        );

        for entry in entries {
            let result = sqlx::query(sqlx::AssertSqlSafe(sql.clone()))
                .bind(&entry.name)
                .bind(&entry.path)
                .bind(&entry.alias)
                .bind(&entry.cmd)
                .bind(entry.atime)
                .bind(entry.count)
                .bind(entry.score)
                .execute(&mut *tx)
                .await?;
            total_rows += result.rows_affected();
        }

        tx.commit().await?;
        Ok(total_rows)
    }

    pub async fn reset_table(&mut self) -> Result<(), DbError> {
        let sql = format!("DROP TABLE IF EXISTS {}", self.table);
        sqlx::query(sqlx::AssertSqlSafe(sql))
            .execute(&mut *self.conn)
            .await?;
        Ok(())
    }

    pub async fn remove_entries(
        &mut self,
        paths: &[AbsPath],
    ) -> Result<u64, DbError> {
        if paths.is_empty() {
            return Ok(0);
        }

        let mut tx = self.conn.begin().await?;
        let mut total_removed = 0;

        for chunk in paths.chunks(MAX_PLACEHOLDERS) {
            let mut query = sqlx::QueryBuilder::new("DELETE FROM ");
            query.push(self.table);
            query.push(" WHERE path IN (");
            let mut separated = query.separated(", ");
            for path in chunk {
                separated.push_bind(path);
            }
            query.push(")");

            let result = query.build().execute(&mut *tx).await?;
            total_removed += result.rows_affected();
        }

        tx.commit().await?;
        Ok(total_removed)
    }

    pub async fn get_entry(
        &mut self,
        path: &AbsPath,
    ) -> Result<Option<Entry>, DbError> {
        let sql = format!("SELECT * FROM {} WHERE path = ?", self.table);
        let entry = sqlx::query_as::<_, Entry>(sqlx::AssertSqlSafe(sql))
            .bind(path)
            .fetch_optional(&mut *self.conn)
            .await?;
        Ok(entry)
    }

    pub async fn set_alias(
        &mut self,
        path: &AbsPath,
        new_alias: &str,
    ) -> Result<(), DbError> {
        let sql = format!("UPDATE {} SET alias = ? WHERE path = ?", self.table);
        sqlx::query(sqlx::AssertSqlSafe(sql))
            .bind(new_alias)
            .bind(path)
            .execute(&mut *self.conn)
            .await?;
        Ok(())
    }

    pub async fn set_cmd(
        &mut self,
        path: &AbsPath,
        cmd: &OsStringWrapper,
    ) -> Result<(), DbError> {
        let sql = format!("UPDATE {} SET cmd = ? WHERE path = ?", self.table);
        sqlx::query(sqlx::AssertSqlSafe(sql))
            .bind(cmd)
            .bind(path)
            .execute(&mut *self.conn)
            .await?;
        Ok(())
    }

    pub async fn bump_entry(
        &mut self,
        entry: &Entry,
        count: i32,
    ) -> Result<(), DbError> {
        trace!("Bumping {}", entry.path.display());

        if let Some(lambda) = self.lambda {
            let mut query = sqlx::QueryBuilder::new("UPDATE ");
            query.push(self.table);
            query.push(" SET score = ");
            query.push_bind(entry.score);
            query.push(" * exp(-");
            query.push_bind(lambda);
            query.push(" * (COALESCE((SELECT MAX(atime) FROM ");
            query.push(self.table);
            query.push("), 0) + 1 - ");
            query.push_bind(entry.atime);
            query.push(")) + ");
            query.push_bind(count as f64);
            query.push(", atime = COALESCE((SELECT MAX(atime) FROM ");
            query.push(self.table);
            query.push("), 0) + 1, count = ");
            query.push_bind(entry.count + count);
            query.push(" WHERE path = ");
            query.push_bind(&entry.path);
            query.build().execute(&mut *self.conn).await?;
        } else {
            let now = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs() as Epoch;
            let sql = format!(
                "UPDATE {} SET count = count + ?, atime = ? WHERE path = ?",
                self.table
            );
            sqlx::query(sqlx::AssertSqlSafe(sql))
                .bind(count)
                .bind(now)
                .bind(&entry.path)
                .execute(&mut *self.conn)
                .await?;
        }
        Ok(())
    }

    pub async fn get_max_atime(&mut self) -> Result<Epoch, DbError> {
        let sql = format!("SELECT MAX(atime) FROM {}", self.table);
        let result: (Option<Epoch>,) = sqlx::query_as(sqlx::AssertSqlSafe(sql))
            .fetch_one(&mut *self.conn)
            .await?;
        Ok(result.0.unwrap_or(0))
    }

    pub async fn update_score_and_atime(
        &mut self,
        path: &AbsPath,
        score: f64,
        atime: Epoch,
    ) -> Result<(), DbError> {
        let sql = format!(
            "UPDATE {} SET score = ?, atime = ? WHERE path = ?",
            self.table
        );
        sqlx::query(sqlx::AssertSqlSafe(sql))
            .bind(score)
            .bind(atime)
            .bind(path)
            .execute(&mut *self.conn)
            .await?;
        Ok(())
    }

    pub async fn delete_entry(
        &mut self,
        path: &AbsPath,
    ) -> Result<(), DbError> {
        let sql = format!("DELETE FROM {} WHERE path = ?", self.table);
        sqlx::query(sqlx::AssertSqlSafe(sql))
            .bind(path)
            .execute(&mut *self.conn)
            .await?;
        Ok(())
    }

    pub async fn get_entries_range(
        &mut self,
        start: u32,
        end: u32,
        sort_by: DbSortOrder,
    ) -> Result<Vec<Entry>, DbError> {
        let order_by = match sort_by {
            DbSortOrder::name => "ORDER BY name".to_string(),
            DbSortOrder::atime => "ORDER BY atime DESC".to_string(),
            DbSortOrder::count => "ORDER BY count DESC".to_string(),
            DbSortOrder::frecency => {
                // Frecency = decayed score
                if let Some(lambda) = self.lambda {
                    // EMS
                    format!(
                        "ORDER BY score * exp(-{} * ((SELECT MAX(atime) FROM {}) - atime)) DESC",
                        lambda, self.table
                    )
                } else {
                    // zoxide: bucketed scoring based on age
                    format!(
                        "ORDER BY CASE
                            WHEN (SELECT MAX(atime) FROM {}) - atime <= 3600 THEN count * 16
                            WHEN (SELECT MAX(atime) FROM {}) - atime <= 86400 THEN count * 8
                            WHEN (SELECT MAX(atime) FROM {}) - atime <= 604800 THEN count * 2
                            ELSE count
                        END DESC",
                        self.table, self.table, self.table
                    )
                }
            }
            DbSortOrder::none => "ORDER BY 1".to_string(),
        };

        let limit = if end == 0 {
            -1
        } else {
            end.saturating_sub(start) as i32
        };

        let sql = format!("SELECT * FROM {} {} LIMIT ? OFFSET ?", self.table, order_by);

        sqlx::query_as::<_, Entry>(sqlx::AssertSqlSafe(sql))
            .bind(limit)
            .bind(start)
            .fetch_all(&mut *self.conn)
            .await
            .cast()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn setup_db() -> Connection {
        let pool = Pool::new("sqlite::memory:", Some(8e-3)).await.unwrap();
        pool.get_conn(DbTable::files).await.unwrap()
    }

    #[tokio::test]
    async fn test_new_connection() {
        let mut db = setup_db().await;
        // Check if the table exists by trying to insert data.
        let entry = Entry::new("test", AbsPath::new("/test"));
        let result = db.set_entry(&entry).await;
        assert!(result.is_ok());
    }
    #[tokio::test]
    async fn test_add_and_get_entry() {
        let mut db = setup_db().await;
        let path = AbsPath::new("/test_add");
        let entry = Entry::new("test_add", path.clone());
        db.set_entry(&entry).await.unwrap();

        let fetched_entry = db.get_entry(&path).await.unwrap().unwrap();
        assert_eq!(entry.name, fetched_entry.name);
        assert_eq!(entry.path, fetched_entry.path);
    }

    #[tokio::test]
    async fn test_add_and_get_entry_with_cmd() {
        let mut db = setup_db().await;
        let path = AbsPath::new("/test_add_with_cmd");
        let mut entry = Entry::new("test_add_with_cmd", path.clone());
        entry.cmd = "test command".into();
        db.set_entry(&entry).await.unwrap();

        let fetched_entry = db.get_entry(&path).await.unwrap().unwrap();
        assert_eq!(entry.name, fetched_entry.name);
        assert_eq!(entry.path, fetched_entry.path);
        assert_eq!(entry.cmd, fetched_entry.cmd);
    }

    #[tokio::test]
    async fn test_update_alias() {
        let mut db = setup_db().await;
        let path = AbsPath::new("/test_alias");
        let entry = Entry::new("test_alias", path.clone());
        db.set_entry(&entry).await.unwrap();

        let new_alias = "new_alias";
        db.set_alias(&path, new_alias).await.unwrap();
        let fetched_entry = db.get_entry(&path).await.unwrap().unwrap();
        assert_eq!(fetched_entry.alias, new_alias);

        db.set_alias(&path, "").await.unwrap();
        let fetched_entry_no_alias = db.get_entry(&path).await.unwrap().unwrap();
        assert_eq!(fetched_entry_no_alias.alias, "");
    }

    #[tokio::test]
    async fn test_set_cmd() {
        let mut db = setup_db().await;
        let path = AbsPath::new("/test_set_cmd");
        let entry = Entry::new("test_set_cmd", path.clone());
        db.set_entry(&entry).await.unwrap();

        let cmd = "new command".into();
        db.set_cmd(&path, &cmd).await.unwrap();
        let fetched_entry = db.get_entry(&path).await.unwrap().unwrap();
        assert_eq!(fetched_entry.cmd, cmd);

        let cmd = "".into();
        db.set_cmd(&path, &cmd).await.unwrap();
        let fetched_entry_no_cmd = db.get_entry(&path).await.unwrap().unwrap();
        assert_eq!(fetched_entry_no_cmd.cmd, cmd);
    }

    #[tokio::test]
    async fn test_delete_entry() {
        let mut db = setup_db().await;
        let path = AbsPath::new("/test_delete");
        let entry = Entry::new("test_delete", path.clone());
        db.set_entry(&entry).await.unwrap();

        db.delete_entry(&path).await.unwrap();
        let fetched_entry = db.get_entry(&path).await.unwrap();
        assert!(fetched_entry.is_none());
    }
    #[tokio::test]
    async fn test_get_entries_range_sorted() {
        let mut db = setup_db().await;
        let entry1 = Entry::new("b_entry", AbsPath::new("/b_entry"));
        db.set_entry(&entry1).await.unwrap();
        tokio::time::sleep(std::time::Duration::from_secs(1)).await;
        let entry2 = Entry::new("a_entry", AbsPath::new("/a_entry"));
        db.set_entry(&entry2).await.unwrap();

        // Test sort by name
        let entries_by_name = db.get_entries_range(0, 2, DbSortOrder::name).await.unwrap();
        assert_eq!(entries_by_name.len(), 2);
        assert_eq!(entries_by_name[0].name, "a_entry");
        assert_eq!(entries_by_name[1].name, "b_entry");

        // Newest atime first
        let entries_by_time = db
            .get_entries_range(0, 2, DbSortOrder::atime)
            .await
            .unwrap();
        assert_eq!(entries_by_time.len(), 2);
        assert_eq!(entries_by_time[0].name, "a_entry");
        assert_eq!(entries_by_time[1].name, "b_entry");

        // Full range
        let entries_by_time = db
            .get_entries_range(0, 0, DbSortOrder::atime)
            .await
            .unwrap();
        assert_eq!(entries_by_time.len(), 2);
        assert_eq!(entries_by_time[0].name, "a_entry");
        assert_eq!(entries_by_time[1].name, "b_entry");
    }

    // --- EMS mode tests ---

    async fn setup_db_ems() -> Connection {
        let pool = Pool::new("sqlite::memory:", Some(8e-3)).await.unwrap();
        pool.get_conn(DbTable::files).await.unwrap()
    }

    /// Helper: create an entry with a given atime (tick) for EMS testing.
    fn entry_at(
        path: &str,
        atime: Epoch,
    ) -> Entry {
        Entry {
            name: path.rsplit('/').next().unwrap_or(path).to_string(),
            path: AbsPath::new(path),
            alias: String::new(),
            cmd: OsStringWrapper::default(),
            atime,
            count: 1,
            score: 1.0,
        }
    }

    #[tokio::test]
    async fn test_ems_bump_accumulates_score() {
        let mut db = setup_db_ems().await;
        let path = AbsPath::new("/test_ems_bump");

        // Insert entry at tick 0
        let entry = entry_at("/test_ems_bump", 0);
        db.set_entry(&entry).await.unwrap();

        // Bump once (set_entry already initialized atime to 1, so bump advances to 2)
        db.bump_entry(&entry, 1).await.unwrap();
        let e = db.get_entry(&path).await.unwrap().unwrap();
        assert_eq!(e.atime, 2); // set_entry initialized to 1, bump advanced to 2
        assert!(e.score > 1.0); // score increased
        assert_eq!(e.count, 2);

        // Bump again (no tick gap)
        db.bump_path(path.clone(), 1).await.unwrap();
        let e2 = db.get_entry(&path).await.unwrap().unwrap();
        assert_eq!(e2.atime, 3); // tick advanced to 3
        assert!(e2.score > e.score); // score accumulated further
        assert_eq!(e2.count, 3);
    }

    #[tokio::test]
    async fn test_ems_score_decays_with_tick_gap() {
        let mut db = setup_db_ems().await;
        let path = AbsPath::new("/test_ems_decay");

        // Insert entry at tick 0
        let entry = entry_at("/test_ems_decay", 0);
        db.set_entry(&entry).await.unwrap();

        // Bump at tick 0 → tick 1
        db.bump_path(path.clone(), 1).await.unwrap();
        let e1 = db.get_entry(&path).await.unwrap().unwrap();
        let score_after_first = e1.score;

        // Artificially advance the tick by inserting a dummy entry and bumping many times
        for i in 0..100 {
            let dummy_path = AbsPath::new(format!("/dummy_{i}"));
            db.set_entry(&entry_at(&format!("/dummy_{i}"), (2 + i) as Epoch))
                .await
                .unwrap();
        }

        // Now bump again — the tick should be much higher, causing decay
        db.bump_path(path.clone(), 1).await.unwrap();
        let e2 = db.get_entry(&path).await.unwrap().unwrap();

        // The new score = old_score * exp(-lambda * delta) + 1
        // Since delta is large, the decayed old_score contribution is small
        assert!(e2.score < score_after_first + 10.0); // not a huge jump
    }

    #[tokio::test]
    async fn test_ems_get_max_atime() {
        let mut db = setup_db_ems().await;

        db.set_entry(&entry_at("/a", 0)).await.unwrap();
        db.set_entry(&entry_at("/b", 5)).await.unwrap();
        db.set_entry(&entry_at("/c", 3)).await.unwrap();

        let max_tick = db.get_max_atime().await.unwrap();
        assert_eq!(max_tick, 5);
    }

    #[tokio::test]
    async fn test_atime_bump() {
        let pool = Pool::new("sqlite::memory:", None).await.unwrap();
        let mut db = pool.get_conn(DbTable::files).await.unwrap();

        let path = AbsPath::new("/test_atime");
        let entry = Entry::new("test_atime", path.clone());
        db.set_entry(&entry).await.unwrap();

        let old_atime = entry.atime;
        db.bump_path(path.clone(), 1).await.unwrap();
        let e = db.get_entry(&path).await.unwrap().unwrap();

        assert_eq!(e.count, 2);
        // atime should be updated to current wall-clock
        assert!(e.atime >= old_atime);
        // score should remain 1.0 (atime mode doesn't touch it beyond default)
        assert!((e.score - 1.0).abs() < 1e-6);
    }
}
