#![allow(clippy::doc_markdown)]

//! SQLite persistence layer.
//!
//! Uses non-macro `sqlx::query` everywhere so the build never depends on a
//! live database or an offline-mode metadata directory. Trade-off: the
//! compile-time SQL check is on us — covered by the per-module test suite.

pub mod audit_log;
pub mod participants;
pub mod rooms;

use std::path::Path;

use sqlx::sqlite::{
    SqliteAutoVacuum, SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions, SqliteSynchronous,
};
use sqlx::{Pool, Sqlite};

#[derive(Debug, thiserror::Error)]
pub enum DbError {
    #[error("sqlx: {0}")]
    Sqlx(#[from] sqlx::Error),

    #[error("migrate: {0}")]
    Migrate(#[from] sqlx::migrate::MigrateError),

    #[error("io: {0}")]
    Io(#[from] std::io::Error),
}

/// Embedded migration set — `migrations/` at the workspace root.
pub static MIGRATIONS: sqlx::migrate::Migrator = sqlx::migrate!("../../migrations");

#[derive(Debug, Clone)]
pub struct Db {
    pub pool: Pool<Sqlite>,
}

impl Db {
    /// Open (or create) the SQLite file at `path` and apply migrations.
    ///
    /// WAL journal mode + NORMAL synchronous + foreign_keys ON + temp_store MEMORY
    /// per the plan. Newly-created files are chmod 0600 on unix.
    ///
    /// # Errors
    ///
    /// Returns [`DbError`] on filesystem / sqlx / migration failures.
    pub async fn open(path: &Path) -> Result<Self, DbError> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let existed = path.exists();

        let opts = SqliteConnectOptions::new()
            .filename(path)
            .create_if_missing(true)
            .journal_mode(SqliteJournalMode::Wal)
            .synchronous(SqliteSynchronous::Normal)
            .foreign_keys(true)
            .auto_vacuum(SqliteAutoVacuum::Incremental);

        let pool = SqlitePoolOptions::new()
            .max_connections(8)
            .connect_with(opts)
            .await?;

        if !existed {
            chmod_db_0600(path)?;
        }

        let db = Self { pool };
        db.migrate().await?;
        Ok(db)
    }

    /// In-memory variant for tests.
    ///
    /// # Errors
    ///
    /// Returns [`DbError`] on sqlx failures.
    pub async fn open_in_memory() -> Result<Self, DbError> {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await?;
        let db = Self { pool };
        db.migrate().await?;
        Ok(db)
    }

    /// Apply all pending migrations. Idempotent.
    ///
    /// # Errors
    ///
    /// Returns [`DbError::Migrate`] on schema mismatch.
    pub async fn migrate(&self) -> Result<(), DbError> {
        MIGRATIONS.run(&self.pool).await?;
        Ok(())
    }
}

#[cfg(unix)]
fn chmod_db_0600(path: &Path) -> std::io::Result<()> {
    use std::os::unix::fs::PermissionsExt;
    let mut perms = std::fs::metadata(path)?.permissions();
    perms.set_mode(0o600);
    std::fs::set_permissions(path, perms)
}

#[cfg(not(unix))]
fn chmod_db_0600(_path: &Path) -> std::io::Result<()> {
    Ok(())
}

#[cfg(test)]
#[allow(clippy::disallowed_methods)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn in_memory_opens_and_migrates() {
        let db = Db::open_in_memory().await.expect("open");
        // The migrator records itself in `_sqlx_migrations` — querying it
        // here proves the migration ran.
        let row: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM _sqlx_migrations")
            .fetch_one(&db.pool)
            .await
            .expect("count");
        assert!(row.0 >= 1);
    }

    #[tokio::test]
    async fn migrate_is_idempotent() {
        let db = Db::open_in_memory().await.expect("open");
        db.migrate().await.expect("second migrate");
    }

    #[tokio::test]
    async fn schema_has_three_tables() {
        let db = Db::open_in_memory().await.expect("open");
        let names: Vec<(String,)> =
            sqlx::query_as("SELECT name FROM sqlite_master WHERE type = 'table' ORDER BY name")
                .fetch_all(&db.pool)
                .await
                .expect("query");
        let names: Vec<String> = names.into_iter().map(|r| r.0).collect();
        for table in ["rooms", "participants", "audit_log"] {
            assert!(names.iter().any(|n| n == table), "missing table {table}");
        }
    }
}
