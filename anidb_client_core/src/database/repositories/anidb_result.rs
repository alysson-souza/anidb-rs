//! AniDB result repository implementation

use crate::Result;
use crate::database::models::{AniDBResult, time_utils};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use sqlx::{Row, SqlitePool};
use std::time::Duration;

use super::Repository;

/// Statistics for anime entries in the database
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnimeStats {
    pub anime_id: i64,
    pub anime_title: String,
    pub file_count: i64,
    pub total_size: i64,
    pub episode_count: i64,
    pub deprecated_count: i64,
    pub last_updated: i64,
}

/// Repository for AniDB result operations
pub struct AniDBResultRepository {
    pool: SqlitePool,
    default_cache_duration: Duration,
}

impl AniDBResultRepository {
    /// Create a new AniDB result repository
    pub fn new(pool: SqlitePool) -> Self {
        Self {
            pool,
            default_cache_duration: Duration::from_secs(7 * 24 * 60 * 60), // 7 days
        }
    }

    /// Create with custom cache duration
    pub fn with_cache_duration(pool: SqlitePool, cache_duration: Duration) -> Self {
        Self {
            pool,
            default_cache_duration: cache_duration,
        }
    }

    /// Find by ED2K hash and file size
    pub async fn find_by_hash_and_size(
        &self,
        ed2k_hash: &str,
        file_size: i64,
    ) -> Result<Option<AniDBResult>> {
        let row = sqlx::query(
            r#"
            SELECT id, file_id, ed2k_hash, file_size, anime_id, episode_id,
                   episode_number, anime_title, episode_title, group_name, group_short,
                   version, censored, deprecated, crc32_valid, file_type,
                   resolution, video_codec, audio_codec, source, quality,
                   fetched_at, expires_at, created_at, updated_at
            FROM anidb_results
            WHERE ed2k_hash = ? AND file_size = ?
            "#,
        )
        .bind(ed2k_hash)
        .bind(file_size)
        .fetch_optional(&self.pool)
        .await?;

        self.row_to_anidb_result(row).await
    }

    /// Find by file ID
    pub async fn find_by_file_id(&self, file_id: i64) -> Result<Option<AniDBResult>> {
        let row = sqlx::query(
            r#"
            SELECT id, file_id, ed2k_hash, file_size, anime_id, episode_id,
                   episode_number, anime_title, episode_title, group_name, group_short,
                   version, censored, deprecated, crc32_valid, file_type,
                   resolution, video_codec, audio_codec, source, quality,
                   fetched_at, expires_at, created_at, updated_at
            FROM anidb_results
            WHERE file_id = ?
            "#,
        )
        .bind(file_id)
        .fetch_optional(&self.pool)
        .await?;

        self.row_to_anidb_result(row).await
    }

    /// Find by anime ID
    pub async fn find_by_anime_id(&self, anime_id: i64) -> Result<Vec<AniDBResult>> {
        let rows = sqlx::query(
            r#"
            SELECT id, file_id, ed2k_hash, file_size, anime_id, episode_id,
                   episode_number, anime_title, episode_title, group_name, group_short,
                   version, censored, deprecated, crc32_valid, file_type,
                   resolution, video_codec, audio_codec, source, quality,
                   fetched_at, expires_at, created_at, updated_at
            FROM anidb_results
            WHERE anime_id = ?
            ORDER BY episode_number
            "#,
        )
        .bind(anime_id)
        .fetch_all(&self.pool)
        .await?;

        let mut results = Vec::with_capacity(rows.len());
        for row in rows {
            if let Some(result) = self.row_to_anidb_result(Some(row)).await? {
                results.push(result);
            }
        }

        Ok(results)
    }

    /// Find expired results
    pub async fn find_expired(&self, limit: i64) -> Result<Vec<AniDBResult>> {
        let now = time_utils::now_millis();

        let rows = sqlx::query(
            r#"
            SELECT id, file_id, ed2k_hash, file_size, anime_id, episode_id,
                   episode_number, anime_title, episode_title, group_name, group_short,
                   version, censored, deprecated, crc32_valid, file_type,
                   resolution, video_codec, audio_codec, source, quality,
                   fetched_at, expires_at, created_at, updated_at
            FROM anidb_results
            WHERE expires_at IS NOT NULL AND expires_at < ?
            LIMIT ?
            "#,
        )
        .bind(now)
        .bind(limit)
        .fetch_all(&self.pool)
        .await?;

        let mut results = Vec::with_capacity(rows.len());
        for row in rows {
            if let Some(result) = self.row_to_anidb_result(Some(row)).await? {
                results.push(result);
            }
        }

        Ok(results)
    }

    /// Delete expired results
    pub async fn delete_expired(&self) -> Result<u64> {
        let now = time_utils::now_millis();

        let result = sqlx::query(
            "DELETE FROM anidb_results WHERE expires_at IS NOT NULL AND expires_at < ?",
        )
        .bind(now)
        .execute(&self.pool)
        .await?;

        Ok(result.rows_affected())
    }

    /// Create or update a result
    pub async fn upsert(&self, result: &AniDBResult) -> Result<i64> {
        let expires_at = result
            .expires_at
            .unwrap_or_else(|| result.fetched_at + self.default_cache_duration.as_millis() as i64);

        let query_result = sqlx::query(
            r#"
            INSERT INTO anidb_results (
                file_id, ed2k_hash, file_size, anime_id, episode_id,
                episode_number, anime_title, episode_title, group_name, group_short,
                version, censored, deprecated, crc32_valid, file_type,
                resolution, video_codec, audio_codec, source, quality,
                fetched_at, expires_at, created_at, updated_at
            ) VALUES (
                ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?
            )
            ON CONFLICT(ed2k_hash, file_size) DO UPDATE SET
                file_id = excluded.file_id,
                anime_id = excluded.anime_id,
                episode_id = excluded.episode_id,
                episode_number = excluded.episode_number,
                anime_title = excluded.anime_title,
                episode_title = excluded.episode_title,
                group_name = excluded.group_name,
                group_short = excluded.group_short,
                version = excluded.version,
                censored = excluded.censored,
                deprecated = excluded.deprecated,
                crc32_valid = excluded.crc32_valid,
                file_type = excluded.file_type,
                resolution = excluded.resolution,
                video_codec = excluded.video_codec,
                audio_codec = excluded.audio_codec,
                source = excluded.source,
                quality = excluded.quality,
                fetched_at = excluded.fetched_at,
                expires_at = excluded.expires_at,
                updated_at = excluded.updated_at
            "#,
        )
        .bind(result.file_id)
        .bind(&result.ed2k_hash)
        .bind(result.file_size)
        .bind(result.anime_id)
        .bind(result.episode_id)
        .bind(&result.episode_number)
        .bind(&result.anime_title)
        .bind(&result.episode_title)
        .bind(&result.group_name)
        .bind(&result.group_short)
        .bind(result.version)
        .bind(result.censored)
        .bind(result.deprecated)
        .bind(result.crc32_valid)
        .bind(&result.file_type)
        .bind(&result.resolution)
        .bind(&result.video_codec)
        .bind(&result.audio_codec)
        .bind(&result.source)
        .bind(&result.quality)
        .bind(result.fetched_at)
        .bind(expires_at)
        .bind(result.created_at)
        .bind(result.updated_at)
        .execute(&self.pool)
        .await?;

        Ok(query_result.last_insert_rowid())
    }

    /// Batch insert AniDB results with optimized performance
    pub async fn batch_insert(&self, results: &[AniDBResult]) -> Result<Vec<i64>> {
        if results.is_empty() {
            return Ok(Vec::new());
        }

        let mut tx = self.pool.begin().await?;
        let mut ids = Vec::with_capacity(results.len());

        // Process in chunks to avoid SQL query size limits
        for chunk in results.chunks(500) {
            // Build multi-value insert query
            let mut query = String::from(
                r#"INSERT INTO anidb_results (
                    file_id, ed2k_hash, file_size, anime_id, episode_id,
                    episode_number, anime_title, episode_title, group_name, group_short,
                    version, censored, deprecated, crc32_valid, file_type,
                    resolution, video_codec, audio_codec, source, quality,
                    fetched_at, expires_at, created_at, updated_at
                ) VALUES "#,
            );

            let placeholders: Vec<String> = chunk
                .iter()
                .map(|_| {
                    "(?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)"
                        .to_string()
                })
                .collect();
            query.push_str(&placeholders.join(", "));
            query.push_str(" RETURNING id");

            // Bind all values
            let mut query_builder = sqlx::query(&query);
            for result in chunk {
                let expires_at = result.expires_at.unwrap_or_else(|| {
                    result.fetched_at + self.default_cache_duration.as_millis() as i64
                });

                query_builder = query_builder
                    .bind(result.file_id)
                    .bind(&result.ed2k_hash)
                    .bind(result.file_size)
                    .bind(result.anime_id)
                    .bind(result.episode_id)
                    .bind(&result.episode_number)
                    .bind(&result.anime_title)
                    .bind(&result.episode_title)
                    .bind(&result.group_name)
                    .bind(&result.group_short)
                    .bind(result.version)
                    .bind(result.censored)
                    .bind(result.deprecated)
                    .bind(result.crc32_valid)
                    .bind(&result.file_type)
                    .bind(&result.resolution)
                    .bind(&result.video_codec)
                    .bind(&result.audio_codec)
                    .bind(&result.source)
                    .bind(&result.quality)
                    .bind(result.fetched_at)
                    .bind(expires_at)
                    .bind(result.created_at)
                    .bind(result.updated_at);
            }

            // Execute and collect IDs
            let rows = query_builder.fetch_all(&mut *tx).await?;
            for row in rows {
                ids.push(row.try_get("id")?);
            }
        }

        tx.commit().await?;
        Ok(ids)
    }

    /// Batch update expiration times
    pub async fn batch_update_expiration(&self, updates: &[(i64, i64)]) -> Result<u64> {
        if updates.is_empty() {
            return Ok(0);
        }

        let mut tx = self.pool.begin().await?;
        let mut total_affected = 0u64;
        let now = time_utils::now_millis();

        // Use prepared statement for better performance
        let stmt = "UPDATE anidb_results SET expires_at = ?, updated_at = ? WHERE id = ?";

        for chunk in updates.chunks(100) {
            for &(id, expires_at) in chunk {
                let result = sqlx::query(stmt)
                    .bind(expires_at)
                    .bind(now)
                    .bind(id)
                    .execute(&mut *tx)
                    .await?;

                total_affected += result.rows_affected();
            }
        }

        tx.commit().await?;
        Ok(total_affected)
    }

    /// Batch mark results as deprecated
    pub async fn batch_mark_deprecated(&self, ed2k_hashes: &[String]) -> Result<u64> {
        if ed2k_hashes.is_empty() {
            return Ok(0);
        }

        let mut tx = self.pool.begin().await?;
        let mut total_affected = 0u64;
        let now = time_utils::now_millis();

        // Process in chunks to avoid query size limits
        for chunk in ed2k_hashes.chunks(500) {
            let placeholders = chunk.iter().map(|_| "?").collect::<Vec<_>>().join(",");
            let query = format!(
                "UPDATE anidb_results SET deprecated = TRUE, updated_at = ? WHERE ed2k_hash IN ({placeholders})"
            );

            let mut query_builder = sqlx::query(&query).bind(now);
            for hash in chunk {
                query_builder = query_builder.bind(hash);
            }

            let result = query_builder.execute(&mut *tx).await?;
            total_affected += result.rows_affected();
        }

        tx.commit().await?;
        Ok(total_affected)
    }

    /// Find unidentified files (files without anime_id)
    pub async fn find_unidentified_files(&self, limit: i64) -> Result<Vec<i64>> {
        let rows = sqlx::query(
            r#"
            SELECT DISTINCT file_id
            FROM anidb_results
            WHERE anime_id IS NULL
               OR anime_id = 0
            ORDER BY created_at DESC
            LIMIT ?
            "#,
        )
        .bind(limit)
        .fetch_all(&self.pool)
        .await?;

        let mut file_ids = Vec::with_capacity(rows.len());
        for row in rows {
            file_ids.push(row.try_get("file_id")?);
        }

        Ok(file_ids)
    }

    /// Get statistics for all anime in the database
    pub async fn get_anime_statistics(&self) -> Result<Vec<AnimeStats>> {
        let rows = sqlx::query(
            r#"
            SELECT 
                ar.anime_id,
                ar.anime_title,
                COUNT(DISTINCT ar.file_id) as file_count,
                SUM(ar.file_size) as total_size,
                COUNT(DISTINCT ar.episode_number) as episode_count,
                SUM(CASE WHEN ar.deprecated = TRUE THEN 1 ELSE 0 END) as deprecated_count,
                MAX(ar.updated_at) as last_updated
            FROM anidb_results ar
            WHERE ar.anime_id IS NOT NULL 
              AND ar.anime_id > 0
              AND ar.anime_title IS NOT NULL
            GROUP BY ar.anime_id, ar.anime_title
            ORDER BY file_count DESC
            "#,
        )
        .fetch_all(&self.pool)
        .await?;

        let mut stats = Vec::with_capacity(rows.len());
        for row in rows {
            stats.push(AnimeStats {
                anime_id: row.try_get("anime_id")?,
                anime_title: row.try_get("anime_title")?,
                file_count: row.try_get("file_count")?,
                total_size: row.try_get("total_size")?,
                episode_count: row.try_get("episode_count")?,
                deprecated_count: row.try_get("deprecated_count")?,
                last_updated: row.try_get("last_updated")?,
            });
        }

        Ok(stats)
    }

    /// Convert a database row to AniDBResult
    async fn row_to_anidb_result(
        &self,
        row: Option<sqlx::sqlite::SqliteRow>,
    ) -> Result<Option<AniDBResult>> {
        if let Some(row) = row {
            Ok(Some(AniDBResult {
                id: row.try_get("id")?,
                file_id: row.try_get("file_id")?,
                ed2k_hash: row.try_get("ed2k_hash")?,
                file_size: row.try_get("file_size")?,
                anime_id: row.try_get("anime_id")?,
                episode_id: row.try_get("episode_id")?,
                episode_number: row.try_get("episode_number")?,
                anime_title: row.try_get("anime_title")?,
                episode_title: row.try_get("episode_title")?,
                group_name: row.try_get("group_name")?,
                group_short: row.try_get("group_short")?,
                version: row.try_get("version")?,
                censored: row.try_get("censored")?,
                deprecated: row.try_get("deprecated")?,
                crc32_valid: row.try_get("crc32_valid")?,
                file_type: row.try_get("file_type")?,
                resolution: row.try_get("resolution")?,
                video_codec: row.try_get("video_codec")?,
                audio_codec: row.try_get("audio_codec")?,
                source: row.try_get("source")?,
                quality: row.try_get("quality")?,
                fetched_at: row.try_get("fetched_at")?,
                expires_at: row.try_get("expires_at")?,
                created_at: row.try_get("created_at")?,
                updated_at: row.try_get("updated_at")?,
            }))
        } else {
            Ok(None)
        }
    }
}

#[async_trait]
impl Repository<AniDBResult> for AniDBResultRepository {
    async fn create(&self, result: &AniDBResult) -> Result<i64> {
        self.upsert(result).await
    }

    async fn find_by_id(&self, id: i64) -> Result<Option<AniDBResult>> {
        let row = sqlx::query(
            r#"
            SELECT id, file_id, ed2k_hash, file_size, anime_id, episode_id,
                   episode_number, anime_title, episode_title, group_name, group_short,
                   version, censored, deprecated, crc32_valid, file_type,
                   resolution, video_codec, audio_codec, source, quality,
                   fetched_at, expires_at, created_at, updated_at
            FROM anidb_results
            WHERE id = ?
            "#,
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await?;

        self.row_to_anidb_result(row).await
    }

    async fn update(&self, result: &AniDBResult) -> Result<()> {
        sqlx::query(
            r#"
            UPDATE anidb_results SET
                file_id = ?, ed2k_hash = ?, file_size = ?, anime_id = ?, episode_id = ?,
                episode_number = ?, anime_title = ?, episode_title = ?, group_name = ?, 
                group_short = ?, version = ?, censored = ?, deprecated = ?, crc32_valid = ?, 
                file_type = ?, resolution = ?, video_codec = ?, audio_codec = ?, source = ?, 
                quality = ?, fetched_at = ?, expires_at = ?, updated_at = ?
            WHERE id = ?
            "#,
        )
        .bind(result.file_id)
        .bind(&result.ed2k_hash)
        .bind(result.file_size)
        .bind(result.anime_id)
        .bind(result.episode_id)
        .bind(&result.episode_number)
        .bind(&result.anime_title)
        .bind(&result.episode_title)
        .bind(&result.group_name)
        .bind(&result.group_short)
        .bind(result.version)
        .bind(result.censored)
        .bind(result.deprecated)
        .bind(result.crc32_valid)
        .bind(&result.file_type)
        .bind(&result.resolution)
        .bind(&result.video_codec)
        .bind(&result.audio_codec)
        .bind(&result.source)
        .bind(&result.quality)
        .bind(result.fetched_at)
        .bind(result.expires_at)
        .bind(result.updated_at)
        .bind(result.id)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    async fn delete(&self, id: i64) -> Result<()> {
        sqlx::query("DELETE FROM anidb_results WHERE id = ?")
            .bind(id)
            .execute(&self.pool)
            .await?;

        Ok(())
    }

    async fn count(&self) -> Result<i64> {
        let count = sqlx::query_scalar("SELECT COUNT(*) FROM anidb_results")
            .fetch_one(&self.pool)
            .await?;

        Ok(count)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::database::models::{File, FileStatus};
    use crate::database::repositories::FileRepository;
    use tempfile::TempDir;

    async fn create_test_repo() -> (AniDBResultRepository, FileRepository, TempDir) {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("test.db");

        // Create the database using the Database struct which handles migrations
        let db = crate::database::Database::new(&db_path).await.unwrap();

        let anidb_repo = AniDBResultRepository::new(db.pool().clone());
        let file_repo = FileRepository::new(db.pool().clone());
        (anidb_repo, file_repo, temp_dir)
    }

    #[tokio::test]
    async fn test_anidb_result_crud() {
        let (anidb_repo, file_repo, _temp_dir) = create_test_repo().await;

        // Create a file first
        let file = File {
            id: 0,
            path: "/test/anime.mkv".to_string(),
            size: 1024 * 1024 * 100, // 100MB
            modified_time: time_utils::now_millis(),
            inode: None,
            status: FileStatus::Processed,
            last_checked: time_utils::now_millis(),
            created_at: time_utils::now_millis(),
            updated_at: time_utils::now_millis(),
        };
        let file_id = file_repo.create(&file).await.unwrap();

        // Create AniDB result
        let result = AniDBResult {
            id: 0,
            file_id,
            ed2k_hash: "a1b2c3d4e5f6".to_string(),
            file_size: file.size,
            anime_id: Some(12345),
            episode_id: Some(67890),
            episode_number: Some("01".to_string()),
            anime_title: Some("Test Anime".to_string()),
            episode_title: Some("First Episode".to_string()),
            group_name: Some("TestGroup".to_string()),
            group_short: Some("TG".to_string()),
            version: Some(1),
            censored: Some(false),
            deprecated: Some(false),
            crc32_valid: Some(true),
            file_type: Some("mkv".to_string()),
            resolution: Some("1920x1080".to_string()),
            video_codec: Some("h264".to_string()),
            audio_codec: Some("aac".to_string()),
            source: Some("www".to_string()),
            quality: Some("high".to_string()),
            fetched_at: time_utils::now_millis(),
            expires_at: None,
            created_at: time_utils::now_millis(),
            updated_at: time_utils::now_millis(),
        };

        // Create
        let id = anidb_repo.create(&result).await.unwrap();
        assert!(id > 0);

        // Find by hash and size
        let found = anidb_repo
            .find_by_hash_and_size(&result.ed2k_hash, result.file_size)
            .await
            .unwrap();
        assert!(found.is_some());
        let found = found.unwrap();
        assert_eq!(found.anime_title, result.anime_title);

        // Find by file ID
        let found = anidb_repo.find_by_file_id(file_id).await.unwrap();
        assert!(found.is_some());

        // Find by anime ID
        let results = anidb_repo.find_by_anime_id(12345).await.unwrap();
        assert_eq!(results.len(), 1);
    }

    #[tokio::test]
    async fn test_expired_results() {
        let (anidb_repo, file_repo, _temp_dir) = create_test_repo().await;

        // Create a file
        let file = File {
            id: 0,
            path: "/test/expired.mkv".to_string(),
            size: 1024,
            modified_time: time_utils::now_millis(),
            inode: None,
            status: FileStatus::Processed,
            last_checked: time_utils::now_millis(),
            created_at: time_utils::now_millis(),
            updated_at: time_utils::now_millis(),
        };
        let file_id = file_repo.create(&file).await.unwrap();

        // Create expired result
        let result = AniDBResult {
            id: 0,
            file_id,
            ed2k_hash: "expired123".to_string(),
            file_size: file.size,
            anime_id: Some(99999),
            episode_id: None,
            episode_number: None,
            anime_title: None,
            episode_title: None,
            group_name: None,
            group_short: None,
            version: None,
            censored: None,
            deprecated: None,
            crc32_valid: None,
            file_type: None,
            resolution: None,
            video_codec: None,
            audio_codec: None,
            source: None,
            quality: None,
            fetched_at: time_utils::now_millis() - 1000000,
            expires_at: Some(time_utils::now_millis() - 1000), // Expired
            created_at: time_utils::now_millis(),
            updated_at: time_utils::now_millis(),
        };

        anidb_repo.create(&result).await.unwrap();

        // Find expired
        let expired = anidb_repo.find_expired(10).await.unwrap();
        assert_eq!(expired.len(), 1);
        assert!(expired[0].is_expired());

        // Delete expired
        let deleted = anidb_repo.delete_expired().await.unwrap();
        assert_eq!(deleted, 1);

        // Verify deleted
        let expired = anidb_repo.find_expired(10).await.unwrap();
        assert_eq!(expired.len(), 0);
    }

    #[tokio::test]
    async fn test_batch_insert() {
        let (anidb_repo, file_repo, _temp_dir) = create_test_repo().await;

        // Create files first
        let mut file_ids = Vec::new();
        for i in 0..10 {
            let file = File {
                id: 0,
                path: format!("/test/batch_{i}.mkv"),
                size: 1024 * 1024 * (i + 1) as i64,
                modified_time: time_utils::now_millis(),
                inode: None,
                status: FileStatus::Processed,
                last_checked: time_utils::now_millis(),
                created_at: time_utils::now_millis(),
                updated_at: time_utils::now_millis(),
            };
            let file_id = file_repo.create(&file).await.unwrap();
            file_ids.push(file_id);
        }

        // Create batch of results
        let mut results = Vec::new();
        for (i, &file_id) in file_ids.iter().enumerate() {
            results.push(AniDBResult {
                id: 0,
                file_id,
                ed2k_hash: format!("batch_hash_{i}"),
                file_size: 1024 * 1024 * (i + 1) as i64,
                anime_id: Some(1000 + i as i64),
                episode_id: Some(2000 + i as i64),
                episode_number: Some(format!("{:02}", i + 1)),
                anime_title: Some(format!("Batch Anime {i}")),
                episode_title: Some(format!("Episode {}", i + 1)),
                group_name: Some("BatchGroup".to_string()),
                group_short: Some("BG".to_string()),
                version: Some(1),
                censored: Some(false),
                deprecated: Some(false),
                crc32_valid: Some(true),
                file_type: Some("mkv".to_string()),
                resolution: Some("1920x1080".to_string()),
                video_codec: Some("h264".to_string()),
                audio_codec: Some("aac".to_string()),
                source: Some("www".to_string()),
                quality: Some("high".to_string()),
                fetched_at: time_utils::now_millis(),
                expires_at: None,
                created_at: time_utils::now_millis(),
                updated_at: time_utils::now_millis(),
            });
        }

        // Batch insert
        let ids = anidb_repo.batch_insert(&results).await.unwrap();
        assert_eq!(ids.len(), 10);

        // Verify all inserted
        let count = anidb_repo.count().await.unwrap();
        assert_eq!(count, 10);

        // Verify data integrity
        for (i, &id) in ids.iter().enumerate() {
            let found = anidb_repo.find_by_id(id).await.unwrap().unwrap();
            assert_eq!(found.ed2k_hash, format!("batch_hash_{i}"));
            assert_eq!(found.anime_title, Some(format!("Batch Anime {i}")));
        }
    }

    #[tokio::test]
    async fn test_batch_update_expiration() {
        let (anidb_repo, file_repo, _temp_dir) = create_test_repo().await;

        // Create test data
        let file = File {
            id: 0,
            path: "/test/expire_update.mkv".to_string(),
            size: 1024,
            modified_time: time_utils::now_millis(),
            inode: None,
            status: FileStatus::Processed,
            last_checked: time_utils::now_millis(),
            created_at: time_utils::now_millis(),
            updated_at: time_utils::now_millis(),
        };
        let file_id = file_repo.create(&file).await.unwrap();

        let mut ids = Vec::new();
        for i in 0..5 {
            let result = AniDBResult {
                id: 0,
                file_id,
                ed2k_hash: format!("expire_{i}"),
                file_size: 1024,
                anime_id: Some(1000 + i),
                episode_id: None,
                episode_number: None,
                anime_title: Some(format!("Anime {i}")),
                episode_title: None,
                group_name: None,
                group_short: None,
                version: None,
                censored: None,
                deprecated: None,
                crc32_valid: None,
                file_type: None,
                resolution: None,
                video_codec: None,
                audio_codec: None,
                source: None,
                quality: None,
                fetched_at: time_utils::now_millis(),
                expires_at: None,
                created_at: time_utils::now_millis(),
                updated_at: time_utils::now_millis(),
            };
            let id = anidb_repo.create(&result).await.unwrap();
            ids.push(id);
        }

        // Prepare updates
        let new_expiry = time_utils::now_millis() + 86400000; // +1 day
        let updates: Vec<(i64, i64)> = ids.iter().map(|&id| (id, new_expiry)).collect();

        // Batch update
        let affected = anidb_repo.batch_update_expiration(&updates).await.unwrap();
        assert_eq!(affected, 5);

        // Verify updates
        for &id in &ids {
            let result = anidb_repo.find_by_id(id).await.unwrap().unwrap();
            assert_eq!(result.expires_at, Some(new_expiry));
        }
    }

    #[tokio::test]
    async fn test_batch_mark_deprecated() {
        let (anidb_repo, file_repo, _temp_dir) = create_test_repo().await;

        // Create test data
        let file = File {
            id: 0,
            path: "/test/deprecate.mkv".to_string(),
            size: 1024,
            modified_time: time_utils::now_millis(),
            inode: None,
            status: FileStatus::Processed,
            last_checked: time_utils::now_millis(),
            created_at: time_utils::now_millis(),
            updated_at: time_utils::now_millis(),
        };
        let file_id = file_repo.create(&file).await.unwrap();

        let mut hashes = Vec::new();
        for i in 0..5 {
            let result = AniDBResult {
                id: 0,
                file_id,
                ed2k_hash: format!("deprecate_{i}"),
                file_size: 1024,
                anime_id: Some(1000 + i),
                episode_id: None,
                episode_number: None,
                anime_title: Some(format!("Anime {i}")),
                episode_title: None,
                group_name: None,
                group_short: None,
                version: None,
                censored: None,
                deprecated: Some(false),
                crc32_valid: None,
                file_type: None,
                resolution: None,
                video_codec: None,
                audio_codec: None,
                source: None,
                quality: None,
                fetched_at: time_utils::now_millis(),
                expires_at: None,
                created_at: time_utils::now_millis(),
                updated_at: time_utils::now_millis(),
            };
            anidb_repo.create(&result).await.unwrap();
            hashes.push(result.ed2k_hash);
        }

        // Batch mark as deprecated
        let affected = anidb_repo.batch_mark_deprecated(&hashes).await.unwrap();
        assert_eq!(affected, 5);

        // Verify updates
        for hash in &hashes {
            let result = anidb_repo
                .find_by_hash_and_size(hash, 1024)
                .await
                .unwrap()
                .unwrap();
            assert_eq!(result.deprecated, Some(true));
        }
    }

    #[tokio::test]
    async fn test_find_unidentified_files() {
        let (anidb_repo, file_repo, _temp_dir) = create_test_repo().await;

        // Create files
        let mut file_ids = Vec::new();
        for i in 0..5 {
            let file = File {
                id: 0,
                path: format!("/test/unidentified_{i}.mkv"),
                size: 1024,
                modified_time: time_utils::now_millis(),
                inode: None,
                status: FileStatus::Processed,
                last_checked: time_utils::now_millis(),
                created_at: time_utils::now_millis(),
                updated_at: time_utils::now_millis(),
            };
            let file_id = file_repo.create(&file).await.unwrap();
            file_ids.push(file_id);
        }

        // Create results - some identified, some not
        for (i, &file_id) in file_ids.iter().enumerate() {
            let result = AniDBResult {
                id: 0,
                file_id,
                ed2k_hash: format!("unidentified_{i}"),
                file_size: 1024,
                anime_id: if i < 2 { None } else { Some(1000 + i as i64) },
                episode_id: None,
                episode_number: None,
                anime_title: None,
                episode_title: None,
                group_name: None,
                group_short: None,
                version: None,
                censored: None,
                deprecated: None,
                crc32_valid: None,
                file_type: None,
                resolution: None,
                video_codec: None,
                audio_codec: None,
                source: None,
                quality: None,
                fetched_at: time_utils::now_millis(),
                expires_at: None,
                created_at: time_utils::now_millis() - (i as i64 * 1000), // Different creation times
                updated_at: time_utils::now_millis(),
            };
            anidb_repo.create(&result).await.unwrap();
        }

        // Find unidentified
        let unidentified = anidb_repo.find_unidentified_files(10).await.unwrap();
        assert_eq!(unidentified.len(), 2);
        assert_eq!(unidentified[0], file_ids[0]); // Most recent first
        assert_eq!(unidentified[1], file_ids[1]);
    }

    #[tokio::test]
    async fn test_get_anime_statistics() {
        let (anidb_repo, file_repo, _temp_dir) = create_test_repo().await;

        // Create files for different anime
        let anime_data = vec![
            (1001, "Anime One", 3),
            (1002, "Anime Two", 2),
            (1003, "Anime Three", 1),
        ];

        for (anime_id, anime_title, episode_count) in &anime_data {
            for ep in 0..*episode_count {
                let file = File {
                    id: 0,
                    path: format!("/test/anime_{anime_id}_ep_{ep}.mkv"),
                    size: 1024 * 1024 * 100 * (ep + 1) as i64,
                    modified_time: time_utils::now_millis(),
                    inode: None,
                    status: FileStatus::Processed,
                    last_checked: time_utils::now_millis(),
                    created_at: time_utils::now_millis(),
                    updated_at: time_utils::now_millis(),
                };
                let file_id = file_repo.create(&file).await.unwrap();

                let result = AniDBResult {
                    id: 0,
                    file_id,
                    ed2k_hash: format!("hash_{anime_id}_{ep}"),
                    file_size: file.size,
                    anime_id: Some(*anime_id),
                    episode_id: Some(2000 + ep as i64),
                    episode_number: Some(format!("{:02}", ep + 1)),
                    anime_title: Some(anime_title.to_string()),
                    episode_title: Some(format!("Episode {}", ep + 1)),
                    group_name: Some("TestGroup".to_string()),
                    group_short: Some("TG".to_string()),
                    version: Some(1),
                    censored: Some(false),
                    deprecated: Some(ep == 0 && *anime_id == 1001), // One deprecated
                    crc32_valid: Some(true),
                    file_type: Some("mkv".to_string()),
                    resolution: Some("1920x1080".to_string()),
                    video_codec: Some("h264".to_string()),
                    audio_codec: Some("aac".to_string()),
                    source: Some("www".to_string()),
                    quality: Some("high".to_string()),
                    fetched_at: time_utils::now_millis(),
                    expires_at: None,
                    created_at: time_utils::now_millis(),
                    updated_at: time_utils::now_millis(),
                };
                anidb_repo.create(&result).await.unwrap();
            }
        }

        // Get statistics
        let stats = anidb_repo.get_anime_statistics().await.unwrap();
        assert_eq!(stats.len(), 3);

        // Check ordering (by file count desc)
        assert_eq!(stats[0].anime_id, 1001);
        assert_eq!(stats[0].anime_title, "Anime One");
        assert_eq!(stats[0].file_count, 3);
        assert_eq!(stats[0].episode_count, 3);
        assert_eq!(stats[0].deprecated_count, 1);
        assert_eq!(stats[0].total_size, 1024 * 1024 * (100 + 200 + 300));

        assert_eq!(stats[1].anime_id, 1002);
        assert_eq!(stats[1].file_count, 2);
        assert_eq!(stats[1].deprecated_count, 0);

        assert_eq!(stats[2].anime_id, 1003);
        assert_eq!(stats[2].file_count, 1);
    }
}
