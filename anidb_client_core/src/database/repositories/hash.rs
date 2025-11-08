//! Hash repository implementation

use crate::Result;
use crate::database::models::Hash;
use crate::hashing::HashAlgorithm;
use async_trait::async_trait;
use sqlx::{Row, SqlitePool};

use super::Repository;

/// Repository for hash operations
pub struct HashRepository {
    pool: SqlitePool,
}

impl HashRepository {
    /// Create a new hash repository
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    /// Find hashes by file ID
    pub async fn find_by_file_id(&self, file_id: i64) -> Result<Vec<Hash>> {
        let rows = sqlx::query(
            r#"
            SELECT id, file_id, algorithm, hash, duration_ms, created_at
            FROM hashes
            WHERE file_id = ?
            "#,
        )
        .bind(file_id)
        .fetch_all(&self.pool)
        .await?;

        let mut hashes = Vec::with_capacity(rows.len());
        for row in rows {
            hashes.push(Hash {
                id: row.try_get("id")?,
                file_id: row.try_get("file_id")?,
                algorithm: row.try_get("algorithm")?,
                hash: row.try_get("hash")?,
                duration_ms: row.try_get("duration_ms")?,
                created_at: row.try_get("created_at")?,
            });
        }

        Ok(hashes)
    }

    /// Find a specific hash by file ID and algorithm
    pub async fn find_by_file_and_algorithm(
        &self,
        file_id: i64,
        algorithm: HashAlgorithm,
    ) -> Result<Option<Hash>> {
        let algorithm_str = format!("{algorithm:?}");

        let row = sqlx::query(
            r#"
            SELECT id, file_id, algorithm, hash, duration_ms, created_at
            FROM hashes
            WHERE file_id = ? AND algorithm = ?
            "#,
        )
        .bind(file_id)
        .bind(&algorithm_str)
        .fetch_optional(&self.pool)
        .await?;

        if let Some(row) = row {
            Ok(Some(Hash {
                id: row.try_get("id")?,
                file_id: row.try_get("file_id")?,
                algorithm: row.try_get("algorithm")?,
                hash: row.try_get("hash")?,
                duration_ms: row.try_get("duration_ms")?,
                created_at: row.try_get("created_at")?,
            }))
        } else {
            Ok(None)
        }
    }

    /// Create or update a hash
    pub async fn upsert(&self, hash: &Hash) -> Result<i64> {
        let result = sqlx::query(
            r#"
            INSERT INTO hashes (file_id, algorithm, hash, duration_ms, created_at)
            VALUES (?, ?, ?, ?, ?)
            ON CONFLICT(file_id, algorithm) DO UPDATE SET
                hash = excluded.hash,
                duration_ms = excluded.duration_ms,
                created_at = excluded.created_at
            "#,
        )
        .bind(hash.file_id)
        .bind(&hash.algorithm)
        .bind(&hash.hash)
        .bind(hash.duration_ms)
        .bind(hash.created_at)
        .execute(&self.pool)
        .await?;

        Ok(result.last_insert_rowid())
    }

    /// Delete all hashes for a file
    pub async fn delete_by_file_id(&self, file_id: i64) -> Result<u64> {
        let result = sqlx::query("DELETE FROM hashes WHERE file_id = ?")
            .bind(file_id)
            .execute(&self.pool)
            .await?;

        Ok(result.rows_affected())
    }

    /// Get hash statistics
    pub async fn get_stats(&self) -> Result<HashStats> {
        let row = sqlx::query(
            r#"
            SELECT 
                COUNT(DISTINCT file_id) as file_count,
                COUNT(*) as total_hashes,
                AVG(duration_ms) as avg_duration_ms,
                algorithm,
                COUNT(*) as count
            FROM hashes
            GROUP BY algorithm
            "#,
        )
        .fetch_all(&self.pool)
        .await?;

        let mut stats = HashStats::default();

        for row in row {
            let algorithm: String = row.try_get("algorithm")?;
            let count: i64 = row.try_get("count")?;
            stats.algorithm_counts.push((algorithm, count as u64));

            if stats.file_count == 0 {
                stats.file_count = row.try_get::<i64, _>("file_count")? as u64;
                stats.total_hashes = row.try_get::<i64, _>("total_hashes")? as u64;
                stats.avg_duration_ms = row.try_get::<f64, _>("avg_duration_ms").ok();
            }
        }

        Ok(stats)
    }

    /// Batch insert hashes
    pub async fn batch_insert(&self, hashes: &[Hash]) -> Result<()> {
        if hashes.is_empty() {
            return Ok(());
        }

        let mut tx = self.pool.begin().await?;

        for chunk in hashes.chunks(500) {
            let mut query = String::from(
                "INSERT INTO hashes (file_id, algorithm, hash, duration_ms, created_at) VALUES ",
            );

            let placeholders: Vec<String> = chunk
                .iter()
                .map(|_| "(?, ?, ?, ?, ?)".to_string())
                .collect();
            query.push_str(&placeholders.join(", "));

            let mut query_builder = sqlx::query(&query);
            for hash in chunk {
                query_builder = query_builder
                    .bind(hash.file_id)
                    .bind(&hash.algorithm)
                    .bind(&hash.hash)
                    .bind(hash.duration_ms)
                    .bind(hash.created_at);
            }

            query_builder.execute(&mut *tx).await?;
        }

        tx.commit().await?;
        Ok(())
    }

    /// Find files with specific ED2K hash
    pub async fn find_files_by_ed2k(&self, ed2k_hash: &str) -> Result<Vec<i64>> {
        let rows = sqlx::query(
            r#"
            SELECT DISTINCT file_id 
            FROM hashes 
            WHERE algorithm = ? AND hash = ?
            ORDER BY file_id
            "#,
        )
        .bind(HashAlgorithm::ED2K.to_string())
        .bind(ed2k_hash)
        .fetch_all(&self.pool)
        .await?;

        let mut file_ids = Vec::with_capacity(rows.len());
        for row in rows {
            file_ids.push(row.try_get("file_id")?);
        }

        Ok(file_ids)
    }

    /// Get hash calculation statistics
    pub async fn get_hash_stats(&self) -> Result<HashStatsExtended> {
        let row = sqlx::query(
            r#"
            SELECT 
                COUNT(*) as total_hashes,
                COUNT(DISTINCT file_id) as unique_files,
                AVG(duration_ms) as avg_duration_ms,
                MIN(duration_ms) as min_duration_ms,
                MAX(duration_ms) as max_duration_ms
            FROM hashes
            "#,
        )
        .fetch_one(&self.pool)
        .await?;

        let algorithm_stats = sqlx::query(
            r#"
            SELECT 
                algorithm,
                COUNT(*) as count,
                AVG(duration_ms) as avg_duration_ms
            FROM hashes
            GROUP BY algorithm
            "#,
        )
        .fetch_all(&self.pool)
        .await?;

        let mut stats_by_algorithm = std::collections::HashMap::new();
        for row in algorithm_stats {
            let algorithm: String = row.try_get("algorithm")?;
            let count: i64 = row.try_get("count")?;
            let avg_duration: f64 = row.try_get("avg_duration_ms")?;

            if let Ok(algo) = algorithm.parse::<HashAlgorithm>() {
                stats_by_algorithm.insert(
                    algo,
                    AlgorithmStats {
                        count: count as u64,
                        avg_duration_ms: avg_duration,
                    },
                );
            }
        }

        Ok(HashStatsExtended {
            total_hashes: row.try_get::<i64, _>("total_hashes")? as u64,
            unique_files: row.try_get::<i64, _>("unique_files")? as u64,
            avg_duration_ms: row.try_get("avg_duration_ms")?,
            min_duration_ms: row.try_get::<i64, _>("min_duration_ms")? as u64,
            max_duration_ms: row.try_get::<i64, _>("max_duration_ms")? as u64,
            stats_by_algorithm,
        })
    }

    /// Delete hashes by file IDs
    pub async fn delete_by_file_ids(&self, file_ids: &[i64]) -> Result<u64> {
        if file_ids.is_empty() {
            return Ok(0);
        }

        let mut tx = self.pool.begin().await?;
        let mut total_deleted = 0u64;

        for chunk in file_ids.chunks(500) {
            let placeholders = chunk.iter().map(|_| "?").collect::<Vec<_>>().join(",");
            let query = format!("DELETE FROM hashes WHERE file_id IN ({placeholders})");

            let mut query_builder = sqlx::query(&query);
            for &id in chunk {
                query_builder = query_builder.bind(id);
            }

            let result = query_builder.execute(&mut *tx).await?;
            total_deleted += result.rows_affected();
        }

        tx.commit().await?;
        Ok(total_deleted)
    }

    /// Find duplicate files based on ED2K hash
    pub async fn find_duplicate_groups(&self, min_group_size: i64) -> Result<Vec<DuplicateGroup>> {
        let rows = sqlx::query(
            r#"
            SELECT h.hash, COUNT(DISTINCT h.file_id) as file_count, SUM(f.size) as total_size
            FROM hashes h
            JOIN files f ON h.file_id = f.id
            WHERE h.algorithm = ? AND f.status != ?
            GROUP BY h.hash
            HAVING COUNT(DISTINCT h.file_id) >= ?
            ORDER BY total_size DESC
            "#,
        )
        .bind(HashAlgorithm::ED2K.to_string())
        .bind("deleted")
        .bind(min_group_size)
        .fetch_all(&self.pool)
        .await?;

        let mut groups = Vec::with_capacity(rows.len());
        for row in rows {
            let hash: String = row.try_get("hash")?;
            let file_count: i64 = row.try_get("file_count")?;
            let total_size: i64 = row.try_get("total_size")?;

            // Get file IDs for this hash
            let file_ids = self.find_files_by_ed2k(&hash).await?;

            groups.push(DuplicateGroup {
                ed2k_hash: hash,
                file_ids,
                file_count: file_count as u64,
                total_size: total_size as u64,
            });
        }

        Ok(groups)
    }
}

#[async_trait]
impl Repository<Hash> for HashRepository {
    async fn create(&self, hash: &Hash) -> Result<i64> {
        let result = sqlx::query(
            r#"
            INSERT INTO hashes (file_id, algorithm, hash, duration_ms, created_at)
            VALUES (?, ?, ?, ?, ?)
            "#,
        )
        .bind(hash.file_id)
        .bind(&hash.algorithm)
        .bind(&hash.hash)
        .bind(hash.duration_ms)
        .bind(hash.created_at)
        .execute(&self.pool)
        .await?;

        Ok(result.last_insert_rowid())
    }

    async fn find_by_id(&self, id: i64) -> Result<Option<Hash>> {
        let row = sqlx::query(
            r#"
            SELECT id, file_id, algorithm, hash, duration_ms, created_at
            FROM hashes
            WHERE id = ?
            "#,
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await?;

        if let Some(row) = row {
            Ok(Some(Hash {
                id: row.try_get("id")?,
                file_id: row.try_get("file_id")?,
                algorithm: row.try_get("algorithm")?,
                hash: row.try_get("hash")?,
                duration_ms: row.try_get("duration_ms")?,
                created_at: row.try_get("created_at")?,
            }))
        } else {
            Ok(None)
        }
    }

    async fn update(&self, hash: &Hash) -> Result<()> {
        sqlx::query(
            r#"
            UPDATE hashes 
            SET file_id = ?, algorithm = ?, hash = ?, duration_ms = ?
            WHERE id = ?
            "#,
        )
        .bind(hash.file_id)
        .bind(&hash.algorithm)
        .bind(&hash.hash)
        .bind(hash.duration_ms)
        .bind(hash.id)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    async fn delete(&self, id: i64) -> Result<()> {
        sqlx::query("DELETE FROM hashes WHERE id = ?")
            .bind(id)
            .execute(&self.pool)
            .await?;

        Ok(())
    }

    async fn count(&self) -> Result<i64> {
        let count = sqlx::query_scalar("SELECT COUNT(*) FROM hashes")
            .fetch_one(&self.pool)
            .await?;

        Ok(count)
    }
}

/// Hash statistics
#[derive(Debug, Clone, Default)]
pub struct HashStats {
    pub file_count: u64,
    pub total_hashes: u64,
    pub avg_duration_ms: Option<f64>,
    pub algorithm_counts: Vec<(String, u64)>,
}

/// Extended hash calculation statistics
#[derive(Debug, Clone)]
pub struct HashStatsExtended {
    pub total_hashes: u64,
    pub unique_files: u64,
    pub avg_duration_ms: f64,
    pub min_duration_ms: u64,
    pub max_duration_ms: u64,
    pub stats_by_algorithm: std::collections::HashMap<HashAlgorithm, AlgorithmStats>,
}

#[derive(Debug, Clone)]
pub struct AlgorithmStats {
    pub count: u64,
    pub avg_duration_ms: f64,
}

#[derive(Debug, Clone)]
pub struct DuplicateGroup {
    pub ed2k_hash: String,
    pub file_ids: Vec<i64>,
    pub file_count: u64,
    pub total_size: u64,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::database::models::{File, FileStatus, time_utils};
    use crate::database::repositories::FileRepository;
    use tempfile::TempDir;

    async fn create_test_repo() -> (HashRepository, FileRepository, TempDir) {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("test.db");

        // Create the database using the Database struct which handles migrations
        let db = crate::database::Database::new(&db_path).await.unwrap();

        let hash_repo = HashRepository::new(db.pool().clone());
        let file_repo = FileRepository::new(db.pool().clone());
        (hash_repo, file_repo, temp_dir)
    }

    #[tokio::test]
    async fn test_hash_crud() {
        let (hash_repo, file_repo, _temp_dir) = create_test_repo().await;

        // Create a file first
        let file = File {
            id: 0,
            path: "/test/file.txt".to_string(),
            size: 1024,
            modified_time: time_utils::now_millis(),
            inode: None,
            status: FileStatus::Pending,
            last_checked: time_utils::now_millis(),
            created_at: time_utils::now_millis(),
            updated_at: time_utils::now_millis(),
        };
        let file_id = file_repo.create(&file).await.unwrap();

        // Create hash
        let hash = Hash {
            id: 0,
            file_id,
            algorithm: "ED2K".to_string(),
            hash: "abcdef123456".to_string(),
            duration_ms: 100,
            created_at: time_utils::now_millis(),
        };

        let id = hash_repo.create(&hash).await.unwrap();
        assert!(id > 0);

        // Find by ID
        let found = hash_repo.find_by_id(id).await.unwrap();
        assert!(found.is_some());
        let found = found.unwrap();
        assert_eq!(found.hash, hash.hash);

        // Find by file ID
        let hashes = hash_repo.find_by_file_id(file_id).await.unwrap();
        assert_eq!(hashes.len(), 1);

        // Find by file and algorithm
        let found = hash_repo
            .find_by_file_and_algorithm(file_id, HashAlgorithm::ED2K)
            .await
            .unwrap();
        assert!(found.is_some());

        // Upsert
        let updated_hash = Hash {
            id: 0,
            file_id,
            algorithm: "ED2K".to_string(),
            hash: "updated_hash".to_string(),
            duration_ms: 200,
            created_at: time_utils::now_millis(),
        };
        hash_repo.upsert(&updated_hash).await.unwrap();

        // Verify update
        let found = hash_repo
            .find_by_file_and_algorithm(file_id, HashAlgorithm::ED2K)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(found.hash, "updated_hash");
        assert_eq!(found.duration_ms, 200);
    }

    #[tokio::test]
    async fn test_batch_insert() {
        let (hash_repo, file_repo, _temp_dir) = create_test_repo().await;

        // Create multiple files
        let mut file_ids = Vec::new();
        for i in 0..10 {
            let file = File {
                id: 0,
                path: format!("/test/batch_hash_{i}.txt"),
                size: 1024,
                modified_time: time_utils::now_millis(),
                inode: None,
                status: FileStatus::Pending,
                last_checked: time_utils::now_millis(),
                created_at: time_utils::now_millis(),
                updated_at: time_utils::now_millis(),
            };
            let id = file_repo.create(&file).await.unwrap();
            file_ids.push(id);
        }

        // Create hashes for batch insert
        let mut hashes = Vec::new();
        for (i, &file_id) in file_ids.iter().enumerate() {
            // Add multiple algorithms per file
            hashes.push(Hash {
                id: 0,
                file_id,
                algorithm: HashAlgorithm::ED2K.to_string(),
                hash: format!("ed2k_hash_{i}"),
                duration_ms: 100 + i as i64,
                created_at: time_utils::now_millis(),
            });
            hashes.push(Hash {
                id: 0,
                file_id,
                algorithm: HashAlgorithm::CRC32.to_string(),
                hash: format!("crc32_hash_{i}"),
                duration_ms: 50 + i as i64,
                created_at: time_utils::now_millis(),
            });
        }

        // Batch insert
        hash_repo.batch_insert(&hashes).await.unwrap();

        // Verify all inserted
        let count = hash_repo.count().await.unwrap();
        assert_eq!(count, 20); // 10 files * 2 algorithms

        // Verify individual hashes
        for &file_id in &file_ids {
            let file_hashes = hash_repo.find_by_file_id(file_id).await.unwrap();
            assert_eq!(file_hashes.len(), 2);
        }
    }

    #[tokio::test]
    async fn test_find_files_by_ed2k() {
        let (hash_repo, file_repo, _temp_dir) = create_test_repo().await;

        // Create files with same ED2K hash (duplicates)
        let ed2k_hash = "31d6cfe0d16ae931b73c59d7e0c089c0";
        let mut file_ids = Vec::new();

        for i in 0..3 {
            let file = File {
                id: 0,
                path: format!("/test/dup_{i}.txt"),
                size: 1024,
                modified_time: time_utils::now_millis(),
                inode: None,
                status: FileStatus::Pending,
                last_checked: time_utils::now_millis(),
                created_at: time_utils::now_millis(),
                updated_at: time_utils::now_millis(),
            };
            let file_id = file_repo.create(&file).await.unwrap();
            file_ids.push(file_id);

            let hash = Hash {
                id: 0,
                file_id,
                algorithm: HashAlgorithm::ED2K.to_string(),
                hash: ed2k_hash.to_string(),
                duration_ms: 100,
                created_at: time_utils::now_millis(),
            };
            hash_repo.create(&hash).await.unwrap();
        }

        // Find all files with this hash
        let found_ids = hash_repo.find_files_by_ed2k(ed2k_hash).await.unwrap();
        assert_eq!(found_ids.len(), 3);
        assert_eq!(found_ids, file_ids);
    }
}
