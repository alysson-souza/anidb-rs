//! File repository implementation

use crate::Result;
use crate::database::models::{File, FileStatus, time_utils};
use async_trait::async_trait;
use sqlx::{Row, SqlitePool};
use std::path::Path;

use super::Repository;

/// Repository for file operations
pub struct FileRepository {
    pool: SqlitePool,
}

impl FileRepository {
    /// Create a new file repository
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    /// Find a file by path
    pub async fn find_by_path(&self, path: &Path) -> Result<Option<File>> {
        let path_str = path.to_string_lossy();

        let row = sqlx::query(
            r#"
            SELECT id, path, size, modified_time, inode, status, 
                   last_checked, created_at, updated_at
            FROM files
            WHERE path = ?
            "#,
        )
        .bind(path_str.as_ref())
        .fetch_optional(&self.pool)
        .await?;

        if let Some(row) = row {
            Ok(Some(File {
                id: row.try_get("id")?,
                path: row.try_get("path")?,
                size: row.try_get("size")?,
                modified_time: row.try_get("modified_time")?,
                inode: row.try_get("inode")?,
                status: row.try_get("status")?,
                last_checked: row.try_get("last_checked")?,
                created_at: row.try_get("created_at")?,
                updated_at: row.try_get("updated_at")?,
            }))
        } else {
            Ok(None)
        }
    }

    /// Find files by status
    pub async fn find_by_status(&self, status: FileStatus, limit: i64) -> Result<Vec<File>> {
        let rows = sqlx::query(
            r#"
            SELECT id, path, size, modified_time, inode, status, 
                   last_checked, created_at, updated_at
            FROM files
            WHERE status = ?
            ORDER BY last_checked ASC
            LIMIT ?
            "#,
        )
        .bind(status)
        .bind(limit)
        .fetch_all(&self.pool)
        .await?;

        let mut files = Vec::with_capacity(rows.len());
        for row in rows {
            files.push(File {
                id: row.try_get("id")?,
                path: row.try_get("path")?,
                size: row.try_get("size")?,
                modified_time: row.try_get("modified_time")?,
                inode: row.try_get("inode")?,
                status: row.try_get("status")?,
                last_checked: row.try_get("last_checked")?,
                created_at: row.try_get("created_at")?,
                updated_at: row.try_get("updated_at")?,
            });
        }

        Ok(files)
    }

    /// Update file status
    pub async fn update_status(&self, file_id: i64, status: FileStatus) -> Result<()> {
        let now = time_utils::now_millis();

        sqlx::query(
            r#"
            UPDATE files 
            SET status = ?, updated_at = ?
            WHERE id = ?
            "#,
        )
        .bind(status)
        .bind(now)
        .bind(file_id)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    /// Update file metadata
    pub async fn update_metadata(
        &self,
        file_id: i64,
        size: i64,
        modified_time: i64,
        inode: Option<i64>,
    ) -> Result<()> {
        let now = time_utils::now_millis();

        sqlx::query(
            r#"
            UPDATE files 
            SET size = ?, modified_time = ?, inode = ?, 
                last_checked = ?, updated_at = ?
            WHERE id = ?
            "#,
        )
        .bind(size)
        .bind(modified_time)
        .bind(inode)
        .bind(now)
        .bind(now)
        .bind(file_id)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    /// Mark files as deleted if they no longer exist
    pub async fn mark_deleted(&self, paths: &[String]) -> Result<u64> {
        if paths.is_empty() {
            return Ok(0);
        }

        let now = time_utils::now_millis();
        let placeholders = paths.iter().map(|_| "?").collect::<Vec<_>>().join(",");
        let query =
            format!("UPDATE files SET status = ?, updated_at = ? WHERE path IN ({placeholders})");

        let mut query = sqlx::query(&query).bind(FileStatus::Deleted).bind(now);

        for path in paths {
            query = query.bind(path);
        }

        let result = query.execute(&self.pool).await?;
        Ok(result.rows_affected())
    }

    /// Get files that need to be checked
    pub async fn get_files_to_check(&self, limit: i64, max_age_ms: i64) -> Result<Vec<File>> {
        let cutoff_time = time_utils::now_millis() - max_age_ms;

        let rows = sqlx::query(
            r#"
            SELECT id, path, size, modified_time, inode, status, 
                   last_checked, created_at, updated_at
            FROM files
            WHERE status != ? AND last_checked < ?
            ORDER BY last_checked ASC
            LIMIT ?
            "#,
        )
        .bind(FileStatus::Deleted)
        .bind(cutoff_time)
        .bind(limit)
        .fetch_all(&self.pool)
        .await?;

        let mut files = Vec::with_capacity(rows.len());
        for row in rows {
            files.push(File {
                id: row.try_get("id")?,
                path: row.try_get("path")?,
                size: row.try_get("size")?,
                modified_time: row.try_get("modified_time")?,
                inode: row.try_get("inode")?,
                status: row.try_get("status")?,
                last_checked: row.try_get("last_checked")?,
                created_at: row.try_get("created_at")?,
                updated_at: row.try_get("updated_at")?,
            });
        }

        Ok(files)
    }

    /// Batch insert files with optimized performance
    pub async fn batch_insert(&self, files: &[File]) -> Result<Vec<i64>> {
        if files.is_empty() {
            return Ok(Vec::new());
        }

        let mut tx = self.pool.begin().await?;
        let mut ids = Vec::with_capacity(files.len());

        // Process in chunks to avoid SQL query size limits
        for chunk in files.chunks(500) {
            // Build multi-value insert query
            let mut query = String::from(
                "INSERT INTO files (path, size, modified_time, inode, status, last_checked, created_at, updated_at) VALUES ",
            );

            let placeholders: Vec<String> = chunk
                .iter()
                .map(|_| "(?, ?, ?, ?, ?, ?, ?, ?)".to_string())
                .collect();
            query.push_str(&placeholders.join(", "));
            query.push_str(" RETURNING id");

            // Bind all values
            let mut query_builder = sqlx::query(&query);
            for file in chunk {
                query_builder = query_builder
                    .bind(&file.path)
                    .bind(file.size)
                    .bind(file.modified_time)
                    .bind(file.inode)
                    .bind(file.status)
                    .bind(file.last_checked)
                    .bind(file.created_at)
                    .bind(file.updated_at);
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

    /// Batch update file metadata
    pub async fn batch_update_metadata(
        &self,
        updates: &[(i64, i64, i64, Option<i64>)], // (id, size, modified_time, inode)
    ) -> Result<u64> {
        if updates.is_empty() {
            return Ok(0);
        }

        let mut tx = self.pool.begin().await?;
        let mut total_affected = 0u64;
        let now = time_utils::now_millis();

        // Use prepared statement for better performance
        let stmt = "UPDATE files SET size = ?, modified_time = ?, inode = ?, last_checked = ?, updated_at = ? WHERE id = ?";

        for chunk in updates.chunks(100) {
            for &(id, size, modified_time, inode) in chunk {
                let result = sqlx::query(stmt)
                    .bind(size)
                    .bind(modified_time)
                    .bind(inode)
                    .bind(now)
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

    /// Find files needing hash calculation
    pub async fn find_files_without_hashes(&self, limit: i64) -> Result<Vec<File>> {
        let rows = sqlx::query(
            r#"
            SELECT f.id, f.path, f.size, f.modified_time, f.inode, f.status, 
                   f.last_checked, f.created_at, f.updated_at
            FROM files f
            LEFT JOIN hashes h ON f.id = h.file_id AND h.algorithm = 'ed2k'
            WHERE h.id IS NULL 
              AND f.status NOT IN ('deleted', 'error')
              AND f.size > 0
            ORDER BY f.created_at DESC
            LIMIT ?
            "#,
        )
        .bind(limit)
        .fetch_all(&self.pool)
        .await?;

        let mut files = Vec::with_capacity(rows.len());
        for row in rows {
            files.push(File {
                id: row.try_get("id")?,
                path: row.try_get("path")?,
                size: row.try_get("size")?,
                modified_time: row.try_get("modified_time")?,
                inode: row.try_get("inode")?,
                status: row.try_get("status")?,
                last_checked: row.try_get("last_checked")?,
                created_at: row.try_get("created_at")?,
                updated_at: row.try_get("updated_at")?,
            });
        }

        Ok(files)
    }

    /// Delete files by IDs
    pub async fn batch_delete(&self, ids: &[i64]) -> Result<u64> {
        if ids.is_empty() {
            return Ok(0);
        }

        let mut tx = self.pool.begin().await?;
        let mut total_deleted = 0u64;

        // Delete in chunks to avoid query size limits
        for chunk in ids.chunks(500) {
            let placeholders = chunk.iter().map(|_| "?").collect::<Vec<_>>().join(",");
            let query = format!("DELETE FROM files WHERE id IN ({placeholders})");

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

    /// Update file status in batch
    pub async fn batch_update_status(&self, updates: &[(i64, FileStatus)]) -> Result<u64> {
        if updates.is_empty() {
            return Ok(0);
        }

        let mut tx = self.pool.begin().await?;
        let mut total_affected = 0u64;
        let now = time_utils::now_millis();

        // Group by status for more efficient updates
        let mut status_groups: std::collections::HashMap<FileStatus, Vec<i64>> =
            std::collections::HashMap::new();

        for &(id, status) in updates {
            status_groups.entry(status).or_default().push(id);
        }

        // Update each status group
        for (status, ids) in status_groups {
            for chunk in ids.chunks(500) {
                let placeholders = chunk.iter().map(|_| "?").collect::<Vec<_>>().join(",");
                let query = format!(
                    "UPDATE files SET status = ?, updated_at = ? WHERE id IN ({placeholders})"
                );

                let mut query_builder = sqlx::query(&query).bind(status).bind(now);

                for &id in chunk {
                    query_builder = query_builder.bind(id);
                }

                let result = query_builder.execute(&mut *tx).await?;
                total_affected += result.rows_affected();
            }
        }

        tx.commit().await?;
        Ok(total_affected)
    }
}

#[async_trait]
impl Repository<File> for FileRepository {
    async fn create(&self, file: &File) -> Result<i64> {
        let result = sqlx::query(
            r#"
            INSERT INTO files (path, size, modified_time, inode, status, 
                             last_checked, created_at, updated_at)
            VALUES (?, ?, ?, ?, ?, ?, ?, ?)
            "#,
        )
        .bind(&file.path)
        .bind(file.size)
        .bind(file.modified_time)
        .bind(file.inode)
        .bind(file.status)
        .bind(file.last_checked)
        .bind(file.created_at)
        .bind(file.updated_at)
        .execute(&self.pool)
        .await?;

        Ok(result.last_insert_rowid())
    }

    async fn find_by_id(&self, id: i64) -> Result<Option<File>> {
        let row = sqlx::query(
            r#"
            SELECT id, path, size, modified_time, inode, status, 
                   last_checked, created_at, updated_at
            FROM files
            WHERE id = ?
            "#,
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await?;

        if let Some(row) = row {
            Ok(Some(File {
                id: row.try_get("id")?,
                path: row.try_get("path")?,
                size: row.try_get("size")?,
                modified_time: row.try_get("modified_time")?,
                inode: row.try_get("inode")?,
                status: row.try_get("status")?,
                last_checked: row.try_get("last_checked")?,
                created_at: row.try_get("created_at")?,
                updated_at: row.try_get("updated_at")?,
            }))
        } else {
            Ok(None)
        }
    }

    async fn update(&self, file: &File) -> Result<()> {
        sqlx::query(
            r#"
            UPDATE files 
            SET path = ?, size = ?, modified_time = ?, inode = ?, 
                status = ?, last_checked = ?, updated_at = ?
            WHERE id = ?
            "#,
        )
        .bind(&file.path)
        .bind(file.size)
        .bind(file.modified_time)
        .bind(file.inode)
        .bind(file.status)
        .bind(file.last_checked)
        .bind(file.updated_at)
        .bind(file.id)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    async fn delete(&self, id: i64) -> Result<()> {
        sqlx::query("DELETE FROM files WHERE id = ?")
            .bind(id)
            .execute(&self.pool)
            .await?;

        Ok(())
    }

    async fn count(&self) -> Result<i64> {
        let count = sqlx::query_scalar("SELECT COUNT(*) FROM files")
            .fetch_one(&self.pool)
            .await?;

        Ok(count)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    async fn create_test_repo() -> (FileRepository, TempDir) {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("test.db");

        // Create the database using the Database struct which handles migrations
        let db = crate::database::Database::new(&db_path).await.unwrap();

        let repo = FileRepository::new(db.pool().clone());
        (repo, temp_dir)
    }

    #[tokio::test]
    async fn test_file_crud() {
        let (repo, _temp_dir) = create_test_repo().await;

        let file = File {
            id: 0,
            path: "/test/file.txt".to_string(),
            size: 1024,
            modified_time: time_utils::now_millis(),
            inode: Some(12345),
            status: FileStatus::Pending,
            last_checked: time_utils::now_millis(),
            created_at: time_utils::now_millis(),
            updated_at: time_utils::now_millis(),
        };

        // Create
        let id = repo.create(&file).await.unwrap();
        assert!(id > 0);

        // Find by ID
        let found = repo.find_by_id(id).await.unwrap();
        assert!(found.is_some());
        let found = found.unwrap();
        assert_eq!(found.path, file.path);
        assert_eq!(found.size, file.size);

        // Find by path
        let found = repo
            .find_by_path(Path::new("/test/file.txt"))
            .await
            .unwrap();
        assert!(found.is_some());

        // Update
        let mut updated = found.unwrap();
        updated.status = FileStatus::Processed;
        repo.update(&updated).await.unwrap();

        // Verify update
        let found = repo.find_by_id(id).await.unwrap().unwrap();
        assert_eq!(found.status, FileStatus::Processed);

        // Delete
        repo.delete(id).await.unwrap();
        let found = repo.find_by_id(id).await.unwrap();
        assert!(found.is_none());
    }

    #[tokio::test]
    async fn test_find_by_status() {
        let (repo, _temp_dir) = create_test_repo().await;

        // Create files with different statuses
        for i in 0..5 {
            let file = File {
                id: 0,
                path: format!("/test/file{i}.txt"),
                size: 1024,
                modified_time: time_utils::now_millis(),
                inode: None,
                status: if i % 2 == 0 {
                    FileStatus::Pending
                } else {
                    FileStatus::Processed
                },
                last_checked: time_utils::now_millis() - i * 1000,
                created_at: time_utils::now_millis(),
                updated_at: time_utils::now_millis(),
            };
            repo.create(&file).await.unwrap();
        }

        // Find pending files
        let pending = repo.find_by_status(FileStatus::Pending, 10).await.unwrap();
        assert_eq!(pending.len(), 3);

        // Should be ordered by last_checked ASC (oldest first)
        assert!(pending[0].last_checked < pending[1].last_checked);
    }

    #[tokio::test]
    async fn test_batch_insert() {
        let (repo, _temp_dir) = create_test_repo().await;

        // Create test files
        let mut files = Vec::new();
        for i in 0..100 {
            files.push(File {
                id: 0,
                path: format!("/test/batch/file_{i}.txt"),
                size: 1024 * i,
                modified_time: time_utils::now_millis(),
                inode: Some(i),
                status: FileStatus::Pending,
                last_checked: time_utils::now_millis(),
                created_at: time_utils::now_millis(),
                updated_at: time_utils::now_millis(),
            });
        }

        // Batch insert
        let ids = repo.batch_insert(&files).await.unwrap();
        assert_eq!(ids.len(), 100);

        // Verify all inserted
        let count = repo.count().await.unwrap();
        assert_eq!(count, 100);

        // Verify individual files
        for (i, id) in ids.iter().enumerate() {
            let file = repo.find_by_id(*id).await.unwrap().unwrap();
            assert_eq!(file.path, format!("/test/batch/file_{i}.txt"));
            assert_eq!(file.size, 1024 * i as i64);
        }
    }

    #[tokio::test]
    async fn test_batch_update_metadata() {
        let (repo, _temp_dir) = create_test_repo().await;

        // Create test files
        let mut file_ids = Vec::new();
        for i in 0..50 {
            let file = File {
                id: 0,
                path: format!("/test/update_{i}.txt"),
                size: 1024,
                modified_time: time_utils::now_millis() - 10000,
                inode: None,
                status: FileStatus::Pending,
                last_checked: time_utils::now_millis() - 10000,
                created_at: time_utils::now_millis(),
                updated_at: time_utils::now_millis(),
            };
            let id = repo.create(&file).await.unwrap();
            file_ids.push(id);
        }

        // Prepare updates
        let updates: Vec<_> = file_ids
            .iter()
            .map(|&id| (id, 2048, time_utils::now_millis(), Some(id * 100)))
            .collect();

        // Batch update
        let affected = repo.batch_update_metadata(&updates).await.unwrap();
        assert_eq!(affected, 50);

        // Verify updates
        for &id in &file_ids {
            let file = repo.find_by_id(id).await.unwrap().unwrap();
            assert_eq!(file.size, 2048);
            assert_eq!(file.inode, Some(id * 100));
            assert!(file.last_checked > time_utils::now_millis() - 1000);
        }
    }

    #[tokio::test]
    async fn test_find_files_without_hashes() {
        let (repo, _temp_dir) = create_test_repo().await;

        // Create files
        for i in 0..10 {
            let file = File {
                id: 0,
                path: format!("/test/no_hash_{i}.txt"),
                size: if i == 5 { 0 } else { 1024 }, // One file with size 0
                modified_time: time_utils::now_millis(),
                inode: None,
                status: if i == 6 {
                    FileStatus::Deleted
                } else if i == 7 {
                    FileStatus::Error
                } else {
                    FileStatus::Pending
                },
                last_checked: time_utils::now_millis(),
                created_at: time_utils::now_millis() - i * 1000,
                updated_at: time_utils::now_millis(),
            };
            repo.create(&file).await.unwrap();
        }

        // Find files without hashes (should exclude size=0, deleted, and error status)
        let files = repo.find_files_without_hashes(20).await.unwrap();
        assert_eq!(files.len(), 7); // 10 - 1 (size=0) - 1 (deleted) - 1 (error)

        // Should be ordered by created_at DESC (newest first)
        assert!(files[0].created_at > files[1].created_at);
    }

    #[tokio::test]
    async fn test_batch_delete() {
        let (repo, _temp_dir) = create_test_repo().await;

        // Create files
        let mut ids = Vec::new();
        for i in 0..20 {
            let file = File {
                id: 0,
                path: format!("/test/delete_{i}.txt"),
                size: 1024,
                modified_time: time_utils::now_millis(),
                inode: None,
                status: FileStatus::Pending,
                last_checked: time_utils::now_millis(),
                created_at: time_utils::now_millis(),
                updated_at: time_utils::now_millis(),
            };
            let id = repo.create(&file).await.unwrap();
            ids.push(id);
        }

        // Delete half of them
        let to_delete = ids[0..10].to_vec();
        let deleted = repo.batch_delete(&to_delete).await.unwrap();
        assert_eq!(deleted, 10);

        // Verify deleted
        for id in &to_delete {
            assert!(repo.find_by_id(*id).await.unwrap().is_none());
        }

        // Verify remaining
        for id in &ids[10..] {
            assert!(repo.find_by_id(*id).await.unwrap().is_some());
        }

        // Total count should be 10
        assert_eq!(repo.count().await.unwrap(), 10);
    }

    #[tokio::test]
    async fn test_batch_update_status() {
        let (repo, _temp_dir) = create_test_repo().await;

        // Create files
        let mut file_ids = Vec::new();
        for i in 0..30 {
            let file = File {
                id: 0,
                path: format!("/test/status_{i}.txt"),
                size: 1024,
                modified_time: time_utils::now_millis(),
                inode: None,
                status: FileStatus::Pending,
                last_checked: time_utils::now_millis(),
                created_at: time_utils::now_millis(),
                updated_at: time_utils::now_millis() - 10000,
            };
            let id = repo.create(&file).await.unwrap();
            file_ids.push(id);
        }

        // Prepare status updates
        let mut updates = Vec::new();
        for (i, &id) in file_ids.iter().enumerate() {
            let status = match i % 3 {
                0 => FileStatus::Processing,
                1 => FileStatus::Processed,
                _ => FileStatus::Error,
            };
            updates.push((id, status));
        }

        // Batch update
        let affected = repo.batch_update_status(&updates).await.unwrap();
        assert_eq!(affected, 30);

        // Verify updates
        for (i, &id) in file_ids.iter().enumerate() {
            let file = repo.find_by_id(id).await.unwrap().unwrap();
            let expected_status = match i % 3 {
                0 => FileStatus::Processing,
                1 => FileStatus::Processed,
                _ => FileStatus::Error,
            };
            assert_eq!(file.status, expected_status);
            assert!(file.updated_at > time_utils::now_millis() - 1000);
        }
    }
}
