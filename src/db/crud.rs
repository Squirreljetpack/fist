use crate::db::*;
use crate::errors::DbError;
use cli_boilerplate_automation::bait::ResultExt;
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
        let table_name = table.to_string();
        self.table_name = table_name;
    }

    pub async fn set_entry(
        &mut self,
        entry: &Entry,
    ) -> Result<(), DbError> {
        trace!("Setting {entry:?}");
        let query = format!(
            "INSERT OR REPLACE INTO {} (name, path, alias, cmd, atime, count) VALUES (?, ?, ?, ?, ?, ?)",
            self.table_name
        );

        // todo:
        sqlx::query(&query)
            .bind(&entry.name)
            .bind(&entry.path)
            .bind(&entry.alias)
            .bind(&entry.cmd)
            .bind(entry.atime)
            .bind(entry.count)
            .execute(&mut *self.conn)
            .await?;
        Ok(())
    }

    pub async fn create_many(
        &mut self,
        entries: &[Entry],
    ) -> Result<u64, DbError> {
        let query = format!(
            "INSERT OR IGNORE INTO {} (name, path, alias, cmd, atime, count)
         VALUES (?, ?, ?, ?, ?, ?)",
            self.table_name
        );

        let mut tx = self.conn.begin().await?;
        let mut total_rows = 0;

        for entry in entries {
            let result = sqlx::query(&query)
                .bind(&entry.name)
                .bind(&entry.path)
                .bind(&entry.alias)
                .bind(&entry.cmd)
                .bind(entry.atime)
                .bind(entry.count)
                .execute(&mut *tx)
                .await?;

            total_rows += result.rows_affected();
        }

        tx.commit().await?;
        Ok(total_rows)
    }

    pub async fn reset_table(&mut self) -> Result<(), DbError> {
        let query = format!("DROP TABLE IF EXISTS {}", self.table_name);

        sqlx::query(&query).execute(&mut *self.conn).await?;

        Ok(())
    }

    pub async fn remove_entries(
        &mut self,
        paths: &[AbsPath],
    ) -> Result<(), DbError> {
        if paths.is_empty() {
            return Ok(());
        }

        let mut tx = self.conn.begin().await?;

        for chunk in paths.chunks(MAX_PLACEHOLDERS) {
            let placeholders = std::iter::repeat_n("?", chunk.len())
                .collect::<Vec<_>>()
                .join(",");

            let query = format!(
                "DELETE FROM {} WHERE path IN ({})",
                self.table_name, placeholders
            );

            let mut q = sqlx::query(&query);
            for path in chunk {
                q = q.bind(path);
            }

            q.execute(&mut *tx).await?;
        }

        tx.commit().await?;
        Ok(())
    }

    pub async fn get_entry(
        &mut self,
        path: &AbsPath,
    ) -> Result<Option<Entry>, DbError> {
        let query = format!("SELECT * FROM {} WHERE path = ?", self.table_name);
        let entry = sqlx::query_as::<_, Entry>(&query)
            .bind(path)
            .fetch_optional(&mut *self.conn)
            .await?;
        Ok(entry)
    }

    pub async fn update_alias(
        &mut self,
        path: &AbsPath,
        new_alias: &str,
    ) -> Result<(), DbError> {
        let query = format!("UPDATE {} SET alias = ? WHERE path = ?", self.table_name);
        sqlx::query(&query)
            .bind(new_alias)
            .bind(path)
            .execute(&mut *self.conn)
            .await?;
        Ok(())
    }

    pub async fn set_cmd(
        &mut self,
        path: &AbsPath,
        entry: &Entry,
    ) -> Result<(), DbError> {
        let query = format!("UPDATE {} SET cmd = ? WHERE path = ?", self.table_name);
        sqlx::query(&query)
            .bind(&entry.cmd)
            .bind(path)
            .execute(&mut *self.conn)
            .await?;
        Ok(())
    }

    pub async fn update_atime_impl(
        &mut self,
        path: &AbsPath,
        new_time: Epoch,
    ) -> Result<(), DbError> {
        let query = format!("UPDATE {} SET atime = ? WHERE path = ?", self.table_name);
        sqlx::query(&query)
            .bind(new_time)
            .bind(path)
            .execute(&mut *self.conn)
            .await?;
        Ok(())
    }

    pub async fn update_atime(
        &mut self,
        path: &AbsPath,
    ) -> Result<(), DbError> {
        self.update_atime_impl(
            path,
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs() as Epoch,
        )
        .await
    }

    pub async fn increment(
        &mut self,
        path: &AbsPath,
        count: i32,
    ) -> Result<(), DbError> {
        let query = format!(
            "UPDATE {} SET count = count + ? WHERE path = ?",
            self.table_name
        );

        sqlx::query(&query)
            .bind(count)
            .bind(path)
            .execute(&mut *self.conn)
            .await?;

        Ok(())
    }

    /// Bump an entry: increment count and update atime
    pub async fn bump(
        &mut self,
        path: &AbsPath,
        count: i32,
    ) -> Result<(), DbError> {
        trace!("Bumping {}", path.display());

        self.increment(path, count).await?;
        self.update_atime(path).await?;
        Ok(())
    }

    pub async fn delete_entry(
        &mut self,
        path: &AbsPath,
    ) -> Result<(), DbError> {
        let query = format!("DELETE FROM {} WHERE path = ?", self.table_name);
        sqlx::query(&query)
            .bind(path)
            .execute(&mut *self.conn)
            .await?;
        Ok(())
    }

    /// Frecency and none => unsorted
    pub async fn get_entries_range(
        &mut self,
        start: u32,
        end: u32,
        sort_by: DbSortOrder,
    ) -> Result<Vec<Entry>, DbError> {
        let order_by = match sort_by {
            DbSortOrder::name => "ORDER BY name",
            DbSortOrder::atime => "ORDER BY atime DESC",
            DbSortOrder::count => "ORDER BY count DESC",
            DbSortOrder::frecency | DbSortOrder::none => "ORDER BY 1",
        };

        let query = format!(
            "SELECT * FROM {} {} LIMIT ? OFFSET ?",
            self.table_name, order_by
        );

        let limit = if end == 0 {
            -1
        } else {
            end.saturating_sub(start) as i32
        };

        sqlx::query_as::<_, Entry>(&query)
            .bind(limit) // no offset without limit
            .bind(start)
            .bind(limit)
            .fetch_all(&mut *self.conn)
            .await
            .cast()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn setup_db() -> Connection {
        let pool = Pool::new("sqlite::memory:").await.unwrap();
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
        entry.cmd = "test command".to_string();
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
        db.update_alias(&path, new_alias).await.unwrap();
        let fetched_entry = db.get_entry(&path).await.unwrap().unwrap();
        assert_eq!(fetched_entry.alias, new_alias);

        db.update_alias(&path, "").await.unwrap();
        let fetched_entry_no_alias = db.get_entry(&path).await.unwrap().unwrap();
        assert_eq!(fetched_entry_no_alias.alias, "");
    }

    #[tokio::test]
    async fn test_set_cmd() {
        let mut db = setup_db().await;
        let path = AbsPath::new("/test_set_cmd");
        let mut entry = Entry::new("test_set_cmd", path.clone());
        db.set_entry(&entry).await.unwrap();

        entry.cmd = "new command".to_string();
        db.set_cmd(&path, &entry).await.unwrap();
        let fetched_entry = db.get_entry(&path).await.unwrap().unwrap();
        assert_eq!(fetched_entry.cmd, "new command");

        entry.cmd = "".to_string();
        db.set_cmd(&path, &entry).await.unwrap();
        let fetched_entry_no_cmd = db.get_entry(&path).await.unwrap().unwrap();
        assert_eq!(fetched_entry_no_cmd.cmd, "");
    }

    #[tokio::test]
    async fn test_increment_count() {
        let mut db = setup_db().await;
        let path = AbsPath::new("/test_count");
        let entry = Entry::new("test_count", path.clone());
        db.set_entry(&entry).await.unwrap();

        db.increment(&path, 1).await.unwrap();
        let fetched_entry = db.get_entry(&path).await.unwrap().unwrap();
        assert_eq!(fetched_entry.count, 2);
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
}
