//! Database migration system
//!
//! This module handles database schema migrations, ensuring the database
//! is always at the correct version.

use crate::{Error, Result, error::InternalError};
use sqlx::SqlitePool;
use std::time::{SystemTime, UNIX_EPOCH};

use super::schema::{MIGRATION_FROM_HASH_CACHE, SCHEMA_V1, SCHEMA_V2};

/// Run all necessary migrations
pub async fn run_migrations(pool: &SqlitePool) -> Result<()> {
    // Get current schema version
    let current_version = get_current_version(pool).await?;

    // Apply migrations in order
    if current_version < 1 {
        apply_migration(pool, 1, SCHEMA_V1).await?;

        // Check if we need to migrate from existing hash_cache table
        if table_exists(pool, "hash_cache").await? {
            migrate_from_hash_cache(pool).await?;
        }
    }

    // Apply v2 migration for performance profiles
    if current_version < 2 {
        apply_migration(pool, 2, SCHEMA_V2).await?;
    }

    Ok(())
}

/// Get the current schema version from the database
async fn get_current_version(pool: &SqlitePool) -> Result<i32> {
    // First check if schema_version table exists
    let table_exists = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='schema_version'",
    )
    .fetch_one(pool)
    .await?;

    if table_exists == 0 {
        return Ok(0);
    }

    // Get the highest version
    let version = sqlx::query_scalar::<_, Option<i32>>("SELECT MAX(version) FROM schema_version")
        .fetch_one(pool)
        .await?;

    Ok(version.unwrap_or(0))
}

/// Apply a single migration
async fn apply_migration(pool: &SqlitePool, version: i32, sql: &str) -> Result<()> {
    // Start a transaction
    let mut tx = pool.begin().await.map_err(|e| {
        Error::Internal(InternalError::assertion(format!(
            "Failed to start migration transaction: {e}"
        )))
    })?;

    // Execute the migration SQL
    sqlx::raw_sql(sql).execute(&mut *tx).await.map_err(|e| {
        Error::Internal(InternalError::assertion(format!(
            "Failed to apply migration {version}: {e}"
        )))
    })?;

    // Record the migration
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64;

    sqlx::query("INSERT INTO schema_version (version, applied_at) VALUES (?, ?)")
        .bind(version)
        .bind(now)
        .execute(&mut *tx)
        .await
        .map_err(|e| {
            Error::Internal(InternalError::assertion(format!(
                "Failed to record migration {version}: {e}"
            )))
        })?;

    // Commit the transaction
    tx.commit().await.map_err(|e| {
        Error::Internal(InternalError::assertion(format!(
            "Failed to commit migration {version}: {e}"
        )))
    })?;

    Ok(())
}

/// Check if a table exists
async fn table_exists(pool: &SqlitePool, table_name: &str) -> Result<bool> {
    let count = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name=?",
    )
    .bind(table_name)
    .fetch_one(pool)
    .await?;

    Ok(count > 0)
}

/// Migrate data from existing hash_cache table
async fn migrate_from_hash_cache(pool: &SqlitePool) -> Result<()> {
    sqlx::raw_sql(MIGRATION_FROM_HASH_CACHE)
        .execute(pool)
        .await
        .map_err(|e| {
            Error::Internal(InternalError::assertion(format!(
                "Failed to migrate from hash_cache: {e}"
            )))
        })?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::super::schema::CURRENT_SCHEMA_VERSION;
    use super::*;
    use std::str::FromStr;
    use tempfile::TempDir;

    async fn create_test_pool() -> (SqlitePool, TempDir) {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("test.db");

        // Use the same connection options as in Database::new
        let connect_options = sqlx::sqlite::SqliteConnectOptions::from_str(&format!(
            "sqlite://{}",
            db_path.display()
        ))
        .unwrap()
        .create_if_missing(true)
        .journal_mode(sqlx::sqlite::SqliteJournalMode::Wal)
        .foreign_keys(true);

        let pool = sqlx::sqlite::SqlitePoolOptions::new()
            .min_connections(1)
            .max_connections(5)
            .connect_with(connect_options)
            .await
            .unwrap();

        (pool, temp_dir)
    }

    #[tokio::test]
    async fn test_migrations_fresh_database() {
        let (pool, _temp_dir) = create_test_pool().await;

        // Should start at version 0
        let version = get_current_version(&pool).await.unwrap();
        assert_eq!(version, 0);

        // Run migrations
        run_migrations(&pool).await.unwrap();

        // Should be at current version
        let version = get_current_version(&pool).await.unwrap();
        assert_eq!(version, CURRENT_SCHEMA_VERSION);

        // Tables should exist
        assert!(table_exists(&pool, "files").await.unwrap());
        assert!(table_exists(&pool, "hashes").await.unwrap());
        assert!(table_exists(&pool, "anidb_results").await.unwrap());
        assert!(table_exists(&pool, "mylist_cache").await.unwrap());
        assert!(table_exists(&pool, "sync_queue").await.unwrap());
    }

    #[tokio::test]
    async fn test_migrations_idempotent() {
        let (pool, _temp_dir) = create_test_pool().await;

        // Run migrations twice
        run_migrations(&pool).await.unwrap();
        run_migrations(&pool).await.unwrap();

        // Should still be at current version
        let version = get_current_version(&pool).await.unwrap();
        assert_eq!(version, CURRENT_SCHEMA_VERSION);
    }

    #[tokio::test]
    async fn test_migrate_from_hash_cache() {
        let (pool, _temp_dir) = create_test_pool().await;

        // Create old hash_cache table
        sqlx::raw_sql(
            r#"
            CREATE TABLE hash_cache (
                id INTEGER PRIMARY KEY,
                file_path TEXT NOT NULL,
                algorithm TEXT NOT NULL,
                hash TEXT NOT NULL,
                file_size INTEGER NOT NULL,
                file_modified_time INTEGER NOT NULL,
                file_inode INTEGER,
                hash_duration_ms INTEGER NOT NULL,
                created_at INTEGER NOT NULL,
                accessed_at INTEGER NOT NULL,
                access_count INTEGER NOT NULL DEFAULT 1
            )
            "#,
        )
        .execute(&pool)
        .await
        .unwrap();

        // Insert test data
        sqlx::query(
            "INSERT INTO hash_cache (file_path, algorithm, hash, file_size, file_modified_time, hash_duration_ms, created_at, accessed_at) VALUES (?, ?, ?, ?, ?, ?, ?, ?)"
        )
        .bind("/test/file.txt")
        .bind("ED2K")
        .bind("abcdef123456")
        .bind(1024i64)
        .bind(1000000i64)
        .bind(100i64)
        .bind(1000000i64)
        .bind(1000000i64)
        .execute(&pool)
        .await
        .unwrap();

        // Run migrations
        run_migrations(&pool).await.unwrap();

        // Check data was migrated
        let file_count = sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM files")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(file_count, 1);

        let hash_count = sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM hashes")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(hash_count, 1);
    }
}
