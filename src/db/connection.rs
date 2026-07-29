use crate::errors::DbError;
use sqlx::{
    Sqlite, SqlitePool,
    pool::PoolConnection,
    sqlite::{SqliteConnectOptions, SqlitePoolOptions},
};

/// Database connection pool.
///
///
///
#[derive(Clone, Debug)]
pub struct Pool {
    pub pool: SqlitePool,
    /// EMS decay constant. `None` for wall-clock atime mode, `Some(λ)` for EMS tick scoring.
    pub lambda: Option<f64>,
}

pub struct Connection {
    pub conn: PoolConnection<Sqlite>,
    /// Which DB table this connection is scoped to (apps/files/dirs).
    pub table: DbTable,
    /// EMS decay constant. `None` for wall-clock atime mode, `Some(λ)` for EMS tick scoring.
    pub lambda: Option<f64>,
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
    pub async fn new_from_cfg(cfg: &crate::config::Config) -> Result<Self, DbError> {
        let path = cfg.db_path();
        let existed = path.exists();
        let pool = Pool::new(&path, cfg.history.lambda).await?;

        // Seed default directories only if this is a fresh database
        if !existed {
            pool.seed_default_dirs(5).await?;
        }

        Ok(pool)
    }

    /// Seed the database with default system directories.
    ///
    /// `initial_count` is the visit count assigned to each seeded directory.
    pub async fn seed_default_dirs(
        &self,
        initial_count: i32,
    ) -> Result<(), DbError> {
        let default_dirs = crate::utils::path::default_directories();
        if default_dirs.is_empty() {
            return Ok(());
        }

        let mut conn = self.get_conn(DbTable::dirs).await?;
        for dir in default_dirs {
            if !dir.exists() {
                continue;
            }
            conn.bump_path(dir, initial_count).await?;
        }

        log::debug!("Seeded database with {} default directories", initial_count);
        Ok(())
    }

    /// Open (or create) the SQLite database at `path`, initialising tables on first use.
    ///
    /// `lambda`: `None` for wall-clock scoring, `Some(λ)` for EMS tick scoring (e.g. `Some(8e-3)`).
    pub async fn new(
        path: impl AsRef<std::path::Path>,
        lambda: Option<f64>,
    ) -> Result<Self, DbError> {
        // get url
        let path = path.as_ref();
        #[cfg(not(test))] // in-memory
        {
            if !path.exists() {
                use cba::bog::BogOkExt;
                std::fs::File::create(path)._wbog();
            }
        }

        let Some(url) = path.to_str().map(|s| format!("sqlite:{}", s)) else {
            return Err(DbError::InvalidPath(path.to_path_buf()));
        };

        log::debug!("db url: {url}");
        let options: SqliteConnectOptions = url.parse()?;
        let pool = SqlitePoolOptions::new()
            .connect_with(options)
            .await?;

        let ret = Self { pool, lambda };
        ret.init_tables().await?;
        Ok(ret)
    }

    async fn init_tables(&self) -> Result<(), DbError> {
        let mut conn = self.get_conn(DbTable::apps).await?;

        for table in [DbTable::apps, DbTable::dirs, DbTable::files] {
            conn.switch_table(table);

            let mut query = sqlx::QueryBuilder::new("CREATE TABLE IF NOT EXISTS ");
            query.push(table.to_string());
            query.push(
                " (
                name TEXT NOT NULL,
                path BLOB PRIMARY KEY NOT NULL,
                alias TEXT NOT NULL DEFAULT '',
                cmd TEXT NOT NULL DEFAULT '',
                atime INTEGER NOT NULL,
                count INTEGER NOT NULL DEFAULT 0,
                score REAL NOT NULL DEFAULT 1.0
            )",
            );
            query.build().execute(&mut *conn.conn).await?;
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
            table,
            lambda: self.lambda,
        };

        log::trace!("db connected");
        Ok(ret)
    }
}
