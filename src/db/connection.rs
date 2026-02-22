use crate::errors::DbError;
use sqlx::{Sqlite, SqlitePool, pool::PoolConnection};

/// Wraps an arc
#[derive(Clone, Debug)]
pub struct Pool {
    pub pool: SqlitePool,
}

pub struct Connection {
    pub conn: PoolConnection<Sqlite>,
    pub table_name: String,
}

#[allow(non_camel_case_types)]
#[derive(Debug, Copy, Clone, clap::ValueEnum, strum_macros::Display)]
#[strum(serialize_all = "lowercase")]
pub enum DbTable {
    apps,
    files,
    dirs,
}

impl Pool {
    pub async fn new(path: impl AsRef<std::path::Path>) -> Result<Self, DbError> {
        // get url
        let path = path.as_ref();
        #[cfg(not(test))] // in-memory
        {
            if !path.exists() {
                use cli_boilerplate_automation::bog::BogOkExt;
                std::fs::File::create(path)._wbog();
            }
        }

        let Some(url) = path.to_str().map(|s| format!("sqlite:{}", s)) else {
            return Err(DbError::InvalidPath(path.to_path_buf()));
        };

        log::debug!("db url: {url}");
        let pool = SqlitePool::connect(&url).await?;

        let ret = Self { pool };
        ret.init_tables().await?;
        Ok(ret)
    }

    async fn init_tables(&self) -> Result<(), DbError> {
        let mut conn = self.get_conn(DbTable::apps).await?;

        for table in [DbTable::apps, DbTable::dirs, DbTable::files] {
            conn.switch_table(table);

            let query = format!(
                r#"
            CREATE TABLE IF NOT EXISTS {} (
                name TEXT NOT NULL,
                path BLOB PRIMARY KEY NOT NULL,
                alias TEXT NOT NULL DEFAULT '',
                cmd TEXT NOT NULL DEFAULT '',
                atime INTEGER NOT NULL,
                count INTEGER NOT NULL DEFAULT 0
            )
            "#,
                table
            );
            sqlx::query(&query).execute(&mut *conn.conn).await?;
        }

        Ok(())
    }

    pub async fn get_conn(
        &self,
        table: DbTable,
    ) -> Result<Connection, DbError> {
        let conn = self.pool.acquire().await?;
        let ret = Connection {
            conn,
            table_name: table.to_string(),
        };

        log::debug!("db connected");
        Ok(ret)
    }
}
