//! Database module for SQLite-based data management
//!
//! This module provides data storage functionality for the AniDB client,
//! including file tracking, hash caching, AniDB results storage, and synchronization queuing.

pub mod migrations;
pub mod models;
pub mod repositories;
pub mod schema;

use crate::{
    Error, Result,
    error::{InternalError, IoError},
};
use sqlx::SqlitePool;
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use std::path::Path;
use std::str::FromStr;

// Re-export commonly used types
pub use models::{
    AniDBResult, File, FileStatus, Hash, MyListEntry, MyListStatus, SchemaVersion, SyncQueueItem,
    SyncStatus,
};
pub use repositories::{
    AniDBResultRepository, AnimeStats, FileRepository, HashRepository, MyListRepository,
    QueueStats, Repository, SyncQueueRepository,
};

/// Database connection manager with connection pooling
pub struct Database {
    pool: SqlitePool,
}

impl Database {
    /// Create a new database connection with migrations
    pub async fn new(db_path: &Path) -> Result<Self> {
        // Ensure parent directory exists
        if let Some(parent) = db_path.parent() {
            tokio::fs::create_dir_all(parent)
                .await
                .map_err(|e| Error::Io(IoError::from_std(e).with_path(db_path)))?;
        }

        // Build connection options with WAL mode for better concurrency
        let connect_options =
            SqliteConnectOptions::from_str(&format!("sqlite://{}", db_path.display()))?
                .create_if_missing(true)
                .journal_mode(sqlx::sqlite::SqliteJournalMode::Wal)
                .foreign_keys(true);

        // Create connection pool
        let pool = SqlitePoolOptions::new()
            .min_connections(5)
            .max_connections(10)
            .connect_with(connect_options)
            .await
            .map_err(|e| {
                Error::Internal(InternalError::assertion(format!(
                    "Failed to connect to database: {e}"
                )))
            })?;

        let db = Self { pool };

        // Run migrations
        db.migrate().await?;

        Ok(db)
    }

    /// Get a reference to the connection pool
    pub fn pool(&self) -> &SqlitePool {
        &self.pool
    }

    /// Run database migrations
    async fn migrate(&self) -> Result<()> {
        migrations::run_migrations(&self.pool).await
    }

    /// Get database statistics
    pub async fn stats(&self) -> Result<DatabaseStats> {
        let file_count = sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM files")
            .fetch_one(&self.pool)
            .await?;

        let hash_count = sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM hashes")
            .fetch_one(&self.pool)
            .await?;

        let anidb_result_count = sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM anidb_results")
            .fetch_one(&self.pool)
            .await?;

        let mylist_count = sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM mylist_cache")
            .fetch_one(&self.pool)
            .await?;

        let sync_queue_count = sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM sync_queue")
            .fetch_one(&self.pool)
            .await?;

        Ok(DatabaseStats {
            file_count: file_count as u64,
            hash_count: hash_count as u64,
            anidb_result_count: anidb_result_count as u64,
            mylist_count: mylist_count as u64,
            sync_queue_count: sync_queue_count as u64,
        })
    }

    /// Clear all data from the database (for testing)
    #[cfg(test)]
    pub async fn clear_all(&self) -> Result<()> {
        sqlx::query("DELETE FROM sync_queue")
            .execute(&self.pool)
            .await?;
        sqlx::query("DELETE FROM mylist_cache")
            .execute(&self.pool)
            .await?;
        sqlx::query("DELETE FROM anidb_results")
            .execute(&self.pool)
            .await?;
        sqlx::query("DELETE FROM hashes")
            .execute(&self.pool)
            .await?;
        sqlx::query("DELETE FROM files").execute(&self.pool).await?;
        Ok(())
    }
}

/// Database statistics
#[derive(Debug, Clone, Default)]
pub struct DatabaseStats {
    pub file_count: u64,
    pub hash_count: u64,
    pub anidb_result_count: u64,
    pub mylist_count: u64,
    pub sync_queue_count: u64,
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    async fn create_test_db() -> (Database, TempDir) {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("test.db");
        let db = Database::new(&db_path).await.unwrap();
        (db, temp_dir)
    }

    #[tokio::test]
    async fn test_database_creation() {
        let (_db, _temp_dir) = create_test_db().await;
        // Database should be created successfully
    }

    #[tokio::test]
    async fn test_database_stats() {
        let (db, _temp_dir) = create_test_db().await;
        let stats = db.stats().await.unwrap();
        assert_eq!(stats.file_count, 0);
        assert_eq!(stats.hash_count, 0);
        assert_eq!(stats.anidb_result_count, 0);
        assert_eq!(stats.mylist_count, 0);
        assert_eq!(stats.sync_queue_count, 0);
    }
}
