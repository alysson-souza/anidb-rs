//! SQLite-based implementation of the HashCache trait
//!
//! This module provides persistent caching of hash results using SQLite,
//! with automatic cache invalidation based on file modification times.

// Note: This implementation needs to be updated to match the current HashCache trait
use anidb_client_core::hashing::{HashAlgorithm, HashResult};
use anidb_client_core::{
    Error, Result,
    error::{InternalError, IoError},
};
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use sqlx::{Row, SqlitePool};
use std::path::Path;
use std::str::FromStr;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

/// SQLite-based hash cache implementation
pub struct SqliteHashCache {
    pool: SqlitePool,
}

impl SqliteHashCache {
    /// Create a new SQLite-based hash cache
    #[allow(dead_code)]
    pub async fn new(db_path: &Path) -> Result<Self> {
        // Ensure parent directory exists
        if let Some(parent) = db_path.parent() {
            tokio::fs::create_dir_all(parent)
                .await
                .map_err(|e| Error::Io(IoError::from_std(e).with_path(db_path)))?;
        }

        // Build connection options
        let connect_options =
            SqliteConnectOptions::from_str(&format!("sqlite://{}", db_path.display()))?
                .create_if_missing(true);

        // Create connection pool
        let pool = SqlitePoolOptions::new()
            .max_connections(5)
            .connect_with(connect_options)
            .await
            .map_err(|e| {
                Error::Internal(InternalError::assertion(format!(
                    "Failed to connect to cache database: {e}"
                )))
            })?;

        // Initialize database schema
        Self::initialize_schema(&pool).await?;

        Ok(Self { pool })
    }

    /// Initialize the database schema
    #[allow(dead_code)]
    async fn initialize_schema(pool: &SqlitePool) -> Result<()> {
        let schema = r#"
            CREATE TABLE IF NOT EXISTS hash_cache (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                file_path TEXT NOT NULL,
                algorithm TEXT NOT NULL,
                hash TEXT NOT NULL,
                file_size INTEGER NOT NULL,
                file_modified_time INTEGER NOT NULL,
                file_inode INTEGER,
                hash_duration_ms INTEGER NOT NULL,
                created_at INTEGER NOT NULL,
                accessed_at INTEGER NOT NULL,
                access_count INTEGER NOT NULL DEFAULT 1,
                UNIQUE(file_path, algorithm)
            );

            CREATE INDEX IF NOT EXISTS idx_file_path_algorithm 
                ON hash_cache(file_path, algorithm);

            CREATE INDEX IF NOT EXISTS idx_accessed_at 
                ON hash_cache(accessed_at);
        "#;

        sqlx::raw_sql(schema).execute(pool).await.map_err(|e| {
            Error::Internal(InternalError::assertion(format!(
                "Failed to initialize cache schema: {e}"
            )))
        })?;

        Ok(())
    }

    /// Convert SystemTime to Unix timestamp (milliseconds)
    fn system_time_to_millis(time: SystemTime) -> i64 {
        time.duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as i64
    }

    /// Convert Unix timestamp (milliseconds) to SystemTime
    #[allow(dead_code)]
    fn millis_to_system_time(millis: i64) -> SystemTime {
        UNIX_EPOCH + Duration::from_millis(millis as u64)
    }

    /// Clean up old cache entries (optional maintenance task)
    #[allow(dead_code)]
    pub async fn cleanup(&self, max_age: Duration) -> Result<u64> {
        let cutoff_time = SystemTime::now().checked_sub(max_age).unwrap_or(UNIX_EPOCH);
        let cutoff_millis = Self::system_time_to_millis(cutoff_time);

        let result = sqlx::query("DELETE FROM hash_cache WHERE accessed_at < ?")
            .bind(cutoff_millis)
            .execute(&self.pool)
            .await
            .map_err(|e| {
                Error::Internal(InternalError::assertion(format!(
                    "Failed to cleanup old entries: {e}"
                )))
            })?;

        Ok(result.rows_affected())
    }

    /// Get cache statistics
    #[allow(dead_code)]
    pub async fn stats(&self) -> Result<CacheStats> {
        let row = sqlx::query(
            r#"
            SELECT 
                COUNT(*) as total_entries,
                SUM(access_count) as total_accesses,
                AVG(access_count) as avg_accesses
            FROM hash_cache
            "#,
        )
        .fetch_one(&self.pool)
        .await
        .map_err(|e| {
            Error::Internal(InternalError::assertion(format!(
                "Failed to get cache stats: {e}"
            )))
        })?;

        let total_entries: Option<i64> = row.try_get("total_entries").ok();
        let total_accesses: Option<i64> = row.try_get("total_accesses").ok();
        let avg_accesses: Option<f64> = row.try_get("avg_accesses").ok();

        Ok(CacheStats {
            total_entries: total_entries.unwrap_or(0) as u64,
            total_accesses: total_accesses.unwrap_or(0) as u64,
            average_accesses: avg_accesses.unwrap_or(0.0),
        })
    }
}

// FileMetadata is no longer available from core library
#[derive(Debug, Clone)]
struct FileMetadata {
    size: u64,
    modified_time: SystemTime,
    #[cfg(unix)]
    inode: u64,
}

// TODO: Update to match current HashCache trait in traits.rs
// This implementation uses the old trait interface which is incompatible
#[allow(dead_code)]
impl SqliteHashCache {
    async fn store(
        &self,
        file_path: &Path,
        algorithm: HashAlgorithm,
        hash_result: &HashResult,
        file_metadata: FileMetadata,
    ) -> Result<()> {
        let path_str = file_path.to_string_lossy();
        let algorithm_str = format!("{algorithm:?}");
        let modified_millis = Self::system_time_to_millis(file_metadata.modified_time);
        let now_millis = Self::system_time_to_millis(SystemTime::now());
        let duration_ms = hash_result.duration.as_millis() as i64;

        #[cfg(unix)]
        let inode = Some(file_metadata.inode as i64);
        #[cfg(not(unix))]
        let inode: Option<i64> = None;

        // Upsert the cache entry
        sqlx::query(
            r#"
            INSERT INTO hash_cache (
                file_path, algorithm, hash, file_size, file_modified_time,
                file_inode, hash_duration_ms, created_at, accessed_at, access_count
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, 1)
            ON CONFLICT(file_path, algorithm) DO UPDATE SET
                hash = excluded.hash,
                file_size = excluded.file_size,
                file_modified_time = excluded.file_modified_time,
                file_inode = excluded.file_inode,
                hash_duration_ms = excluded.hash_duration_ms,
                accessed_at = excluded.accessed_at,
                access_count = access_count + 1
            "#,
        )
        .bind(path_str)
        .bind(algorithm_str)
        .bind(&hash_result.hash)
        .bind(file_metadata.size as i64)
        .bind(modified_millis)
        .bind(inode)
        .bind(duration_ms)
        .bind(now_millis)
        .bind(now_millis)
        .execute(&self.pool)
        .await
        .map_err(|e| {
            Error::Internal(InternalError::assertion(format!(
                "Failed to store hash in cache: {e}"
            )))
        })?;

        Ok(())
    }

    async fn get(
        &self,
        file_path: &Path,
        algorithm: HashAlgorithm,
        current_metadata: &FileMetadata,
    ) -> Result<Option<HashResult>> {
        let path_str = file_path.to_string_lossy();
        let algorithm_str = format!("{algorithm:?}");
        let current_modified_millis = Self::system_time_to_millis(current_metadata.modified_time);

        // Query for cached entry
        let row = sqlx::query(
            r#"
            SELECT 
                hash, file_size, file_modified_time, file_inode, hash_duration_ms
            FROM hash_cache 
            WHERE file_path = ? AND algorithm = ?
            "#,
        )
        .bind(&path_str)
        .bind(&algorithm_str)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| {
            Error::Internal(InternalError::assertion(format!(
                "Failed to query cache: {e}"
            )))
        })?;

        if let Some(row) = row {
            // Check if file has been modified
            let cached_size: i64 = row.try_get("file_size")?;
            let cached_modified_millis: i64 = row.try_get("file_modified_time")?;
            let hash: String = row.try_get("hash")?;
            let hash_duration_ms: i64 = row.try_get("hash_duration_ms")?;

            #[cfg(unix)]
            let cached_inode: Option<i64> = row.try_get("file_inode").ok();

            // Validate cache entry
            if cached_size as u64 != current_metadata.size {
                // File size changed, invalidate
                return Ok(None);
            }

            // Allow small time differences (within 1 second) to handle filesystem precision
            let time_diff = (current_modified_millis - cached_modified_millis).abs();
            if time_diff > 1000 {
                // Modification time differs by more than 1 second, invalidate
                return Ok(None);
            }

            #[cfg(unix)]
            if let Some(cached_inode) = cached_inode
                && cached_inode as u64 != current_metadata.inode
            {
                // Inode changed, file was replaced, invalidate
                return Ok(None);
            }

            // Update access time and count
            let now_millis = Self::system_time_to_millis(SystemTime::now());
            sqlx::query(
                r#"
                UPDATE hash_cache 
                SET accessed_at = ?, access_count = access_count + 1
                WHERE file_path = ? AND algorithm = ?
                "#,
            )
            .bind(now_millis)
            .bind(&path_str)
            .bind(&algorithm_str)
            .execute(&self.pool)
            .await
            .map_err(|e| {
                Error::Internal(InternalError::assertion(format!(
                    "Failed to update cache access: {e}"
                )))
            })?;

            // Return the cached result
            Ok(Some(HashResult {
                algorithm,
                hash,
                input_size: cached_size as u64,
                duration: Duration::from_millis(hash_duration_ms as u64),
            }))
        } else {
            Ok(None)
        }
    }

    async fn invalidate(&self, file_path: &Path) -> Result<()> {
        let path_str = file_path.to_string_lossy();

        sqlx::query("DELETE FROM hash_cache WHERE file_path = ?")
            .bind(&path_str)
            .execute(&self.pool)
            .await
            .map_err(|e| {
                Error::Internal(InternalError::assertion(format!(
                    "Failed to invalidate cache entries: {e}"
                )))
            })?;

        Ok(())
    }

    async fn clear(&self) -> Result<()> {
        sqlx::query("DELETE FROM hash_cache")
            .execute(&self.pool)
            .await
            .map_err(|e| {
                Error::Internal(InternalError::assertion(format!(
                    "Failed to clear cache: {e}"
                )))
            })?;

        Ok(())
    }
}

/// Cache statistics
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct CacheStats {
    pub total_entries: u64,
    pub total_accesses: u64,
    pub average_accesses: f64,
}
