//! Sync queue repository implementation

use crate::Result;
use crate::database::models::{SyncQueueItem, SyncStatus, time_utils};
use async_trait::async_trait;
use sqlx::{Row, SqlitePool};

use super::Repository;

/// Repository for sync queue operations
pub struct SyncQueueRepository {
    pool: SqlitePool,
}

impl SyncQueueRepository {
    /// Create a new sync queue repository
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    /// Find next items ready for processing
    pub async fn find_ready(&self, limit: i64) -> Result<Vec<SyncQueueItem>> {
        let now = time_utils::now_millis();

        let rows = sqlx::query(
            r#"
            SELECT id, file_id, operation, priority, status, retry_count,
                   max_retries, error_message, scheduled_at, last_attempt_at,
                   created_at, updated_at
            FROM sync_queue
            WHERE status = ? AND scheduled_at <= ?
            ORDER BY priority DESC, scheduled_at ASC
            LIMIT ?
            "#,
        )
        .bind(SyncStatus::Pending)
        .bind(now)
        .bind(limit)
        .fetch_all(&self.pool)
        .await?;

        let mut items = Vec::with_capacity(rows.len());
        for row in rows {
            if let Some(item) = self.row_to_sync_queue_item(Some(row)).await? {
                items.push(item);
            }
        }

        Ok(items)
    }

    /// Find items by file ID
    pub async fn find_by_file_id(&self, file_id: i64) -> Result<Vec<SyncQueueItem>> {
        let rows = sqlx::query(
            r#"
            SELECT id, file_id, operation, priority, status, retry_count,
                   max_retries, error_message, scheduled_at, last_attempt_at,
                   created_at, updated_at
            FROM sync_queue
            WHERE file_id = ?
            ORDER BY created_at DESC
            "#,
        )
        .bind(file_id)
        .fetch_all(&self.pool)
        .await?;

        let mut items = Vec::with_capacity(rows.len());
        for row in rows {
            if let Some(item) = self.row_to_sync_queue_item(Some(row)).await? {
                items.push(item);
            }
        }

        Ok(items)
    }

    /// Find items by status
    pub async fn find_by_status(
        &self,
        status: SyncStatus,
        limit: i64,
    ) -> Result<Vec<SyncQueueItem>> {
        let rows = sqlx::query(
            r#"
            SELECT id, file_id, operation, priority, status, retry_count,
                   max_retries, error_message, scheduled_at, last_attempt_at,
                   created_at, updated_at
            FROM sync_queue
            WHERE status = ?
            ORDER BY updated_at DESC
            LIMIT ?
            "#,
        )
        .bind(status)
        .bind(limit)
        .fetch_all(&self.pool)
        .await?;

        let mut items = Vec::with_capacity(rows.len());
        for row in rows {
            if let Some(item) = self.row_to_sync_queue_item(Some(row)).await? {
                items.push(item);
            }
        }

        Ok(items)
    }

    /// Find failed items that can be retried
    pub async fn find_retriable(&self, limit: i64) -> Result<Vec<SyncQueueItem>> {
        let rows = sqlx::query(
            r#"
            SELECT id, file_id, operation, priority, status, retry_count,
                   max_retries, error_message, scheduled_at, last_attempt_at,
                   created_at, updated_at
            FROM sync_queue
            WHERE status = ? AND retry_count < max_retries
            ORDER BY priority DESC, last_attempt_at ASC
            LIMIT ?
            "#,
        )
        .bind(SyncStatus::Failed)
        .bind(limit)
        .fetch_all(&self.pool)
        .await?;

        let mut items = Vec::with_capacity(rows.len());
        for row in rows {
            if let Some(item) = self.row_to_sync_queue_item(Some(row)).await? {
                items.push(item);
            }
        }

        Ok(items)
    }

    /// Update item status
    pub async fn update_status(
        &self,
        id: i64,
        status: SyncStatus,
        error_message: Option<&str>,
    ) -> Result<()> {
        let now = time_utils::now_millis();

        sqlx::query(
            r#"
            UPDATE sync_queue 
            SET status = ?, error_message = ?, last_attempt_at = ?, updated_at = ?
            WHERE id = ?
            "#,
        )
        .bind(status)
        .bind(error_message)
        .bind(now)
        .bind(now)
        .bind(id)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    /// Increment retry count and reschedule
    pub async fn retry(&self, id: i64, delay_ms: i64) -> Result<()> {
        let now = time_utils::now_millis();
        let scheduled_at = now + delay_ms;

        sqlx::query(
            r#"
            UPDATE sync_queue 
            SET retry_count = retry_count + 1,
                scheduled_at = ?,
                status = ?,
                updated_at = ?
            WHERE id = ?
            "#,
        )
        .bind(scheduled_at)
        .bind(SyncStatus::Pending)
        .bind(now)
        .bind(id)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    /// Enqueue a new operation
    pub async fn enqueue(&self, file_id: i64, operation: &str, priority: i32) -> Result<i64> {
        let now = time_utils::now_millis();

        let item = SyncQueueItem {
            id: 0,
            file_id,
            operation: operation.to_string(),
            priority,
            status: SyncStatus::Pending,
            retry_count: 0,
            max_retries: 3,
            error_message: None,
            scheduled_at: now,
            last_attempt_at: None,
            created_at: now,
            updated_at: now,
        };

        self.create(&item).await
    }

    /// Clear all items from sync queue unconditionally
    pub async fn clear_all(&self) -> Result<u64> {
        let result = sqlx::query("DELETE FROM sync_queue")
            .execute(&self.pool)
            .await?;

        Ok(result.rows_affected())
    }

    /// Batch enqueue operations
    pub async fn batch_enqueue(&self, items: &[(i64, String, i32)]) -> Result<Vec<i64>> {
        if items.is_empty() {
            return Ok(Vec::new());
        }

        let mut tx = self.pool.begin().await?;
        let mut ids = Vec::with_capacity(items.len());
        let now = time_utils::now_millis();

        // Process in chunks to avoid SQL query size limits
        for chunk in items.chunks(500) {
            // Build multi-value insert query
            let mut query = String::from(
                r#"INSERT INTO sync_queue (
                    file_id, operation, priority, status, retry_count,
                    max_retries, error_message, scheduled_at, last_attempt_at,
                    created_at, updated_at
                ) VALUES "#,
            );

            let placeholders: Vec<String> = chunk
                .iter()
                .map(|_| "(?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)".to_string())
                .collect();
            query.push_str(&placeholders.join(", "));
            query.push_str(" RETURNING id");

            // Bind all values
            let mut query_builder = sqlx::query(&query);
            for (file_id, operation, priority) in chunk {
                query_builder = query_builder
                    .bind(file_id)
                    .bind(operation)
                    .bind(priority)
                    .bind(SyncStatus::Pending)
                    .bind(0) // retry_count
                    .bind(3) // max_retries
                    .bind(None::<String>) // error_message
                    .bind(now) // scheduled_at
                    .bind(None::<i64>) // last_attempt_at
                    .bind(now) // created_at
                    .bind(now); // updated_at
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

    /// Batch update status for multiple items
    pub async fn batch_update_status(
        &self,
        updates: &[(i64, SyncStatus, Option<String>)],
    ) -> Result<u64> {
        if updates.is_empty() {
            return Ok(0);
        }

        let mut tx = self.pool.begin().await?;
        let mut total_affected = 0u64;
        let now = time_utils::now_millis();

        // Use prepared statement for better performance
        let stmt = "UPDATE sync_queue SET status = ?, error_message = ?, last_attempt_at = ?, updated_at = ? WHERE id = ?";

        for chunk in updates.chunks(100) {
            for (id, status, error_message) in chunk {
                let result = sqlx::query(stmt)
                    .bind(status)
                    .bind(error_message.as_deref())
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

    /// Batch retry multiple items with delay
    pub async fn batch_retry(&self, ids: &[i64], delay_ms: i64) -> Result<u64> {
        if ids.is_empty() {
            return Ok(0);
        }

        let mut tx = self.pool.begin().await?;
        let mut total_affected = 0u64;
        let now = time_utils::now_millis();
        let scheduled_at = now + delay_ms;

        // Process in chunks to avoid query size limits
        for chunk in ids.chunks(500) {
            let placeholders = chunk.iter().map(|_| "?").collect::<Vec<_>>().join(",");
            let query = format!(
                r#"UPDATE sync_queue 
                   SET retry_count = retry_count + 1,
                       scheduled_at = ?,
                       status = ?,
                       updated_at = ?
                   WHERE id IN ({placeholders}) AND status = ?"#
            );

            let mut query_builder = sqlx::query(&query)
                .bind(scheduled_at)
                .bind(SyncStatus::Pending)
                .bind(now);

            for &id in chunk {
                query_builder = query_builder.bind(id);
            }

            query_builder = query_builder.bind(SyncStatus::Failed);

            let result = query_builder.execute(&mut *tx).await?;
            total_affected += result.rows_affected();
        }

        tx.commit().await?;
        Ok(total_affected)
    }

    /// Cancel sync operations by file IDs
    pub async fn cancel_by_file_ids(&self, file_ids: &[i64]) -> Result<u64> {
        if file_ids.is_empty() {
            return Ok(0);
        }

        let mut tx = self.pool.begin().await?;
        let mut total_affected = 0u64;

        // Process in chunks to avoid query size limits
        for chunk in file_ids.chunks(500) {
            let placeholders = chunk.iter().map(|_| "?").collect::<Vec<_>>().join(",");
            let query = format!(
                "DELETE FROM sync_queue WHERE file_id IN ({placeholders}) AND status IN (?, ?)"
            );

            let mut query_builder = sqlx::query(&query);
            for &file_id in chunk {
                query_builder = query_builder.bind(file_id);
            }

            query_builder = query_builder
                .bind(SyncStatus::Pending)
                .bind(SyncStatus::Failed);

            let result = query_builder.execute(&mut *tx).await?;
            total_affected += result.rows_affected();
        }

        tx.commit().await?;
        Ok(total_affected)
    }

    /// Get file history - all sync operations for a specific file
    pub async fn get_file_history(&self, file_id: i64, limit: i64) -> Result<Vec<SyncQueueItem>> {
        let rows = sqlx::query(
            r#"
            SELECT id, file_id, operation, priority, status, retry_count,
                   max_retries, error_message, scheduled_at, last_attempt_at,
                   created_at, updated_at
            FROM sync_queue
            WHERE file_id = ?
            ORDER BY created_at DESC
            LIMIT ?
            "#,
        )
        .bind(file_id)
        .bind(limit)
        .fetch_all(&self.pool)
        .await?;

        let mut items = Vec::with_capacity(rows.len());
        for row in rows {
            if let Some(item) = self.row_to_sync_queue_item(Some(row)).await? {
                items.push(item);
            }
        }

        Ok(items)
    }

    /// Get queue statistics
    pub async fn get_stats(&self) -> Result<QueueStats> {
        let status_counts = sqlx::query(
            r#"
            SELECT status, COUNT(*) as count
            FROM sync_queue
            GROUP BY status
            "#,
        )
        .fetch_all(&self.pool)
        .await?;

        let mut stats = QueueStats::default();

        for row in status_counts {
            let status: SyncStatus = row.try_get("status")?;
            let count: i64 = row.try_get("count")?;

            match status {
                SyncStatus::Pending => stats.pending_count = count as u64,
                SyncStatus::InProgress => stats.in_progress_count = count as u64,
                SyncStatus::Completed => stats.completed_count = count as u64,
                SyncStatus::Failed => stats.failed_count = count as u64,
            }
        }

        // Get retry stats
        let retry_stats = sqlx::query(
            r#"
            SELECT COUNT(*) as retriable_count
            FROM sync_queue
            WHERE status = ? AND retry_count < max_retries
            "#,
        )
        .bind(SyncStatus::Failed)
        .fetch_one(&self.pool)
        .await?;

        stats.retriable_count = retry_stats.try_get::<i64, _>("retriable_count")? as u64;

        Ok(stats)
    }

    /// Convert a database row to SyncQueueItem
    async fn row_to_sync_queue_item(
        &self,
        row: Option<sqlx::sqlite::SqliteRow>,
    ) -> Result<Option<SyncQueueItem>> {
        if let Some(row) = row {
            Ok(Some(SyncQueueItem {
                id: row.try_get("id")?,
                file_id: row.try_get("file_id")?,
                operation: row.try_get("operation")?,
                priority: row.try_get("priority")?,
                status: row.try_get("status")?,
                retry_count: row.try_get("retry_count")?,
                max_retries: row.try_get("max_retries")?,
                error_message: row.try_get("error_message")?,
                scheduled_at: row.try_get("scheduled_at")?,
                last_attempt_at: row.try_get("last_attempt_at")?,
                created_at: row.try_get("created_at")?,
                updated_at: row.try_get("updated_at")?,
            }))
        } else {
            Ok(None)
        }
    }
}

#[async_trait]
impl Repository<SyncQueueItem> for SyncQueueRepository {
    async fn create(&self, item: &SyncQueueItem) -> Result<i64> {
        let result = sqlx::query(
            r#"
            INSERT INTO sync_queue (
                file_id, operation, priority, status, retry_count,
                max_retries, error_message, scheduled_at, last_attempt_at,
                created_at, updated_at
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            "#,
        )
        .bind(item.file_id)
        .bind(&item.operation)
        .bind(item.priority)
        .bind(item.status)
        .bind(item.retry_count)
        .bind(item.max_retries)
        .bind(&item.error_message)
        .bind(item.scheduled_at)
        .bind(item.last_attempt_at)
        .bind(item.created_at)
        .bind(item.updated_at)
        .execute(&self.pool)
        .await?;

        Ok(result.last_insert_rowid())
    }

    async fn find_by_id(&self, id: i64) -> Result<Option<SyncQueueItem>> {
        let row = sqlx::query(
            r#"
            SELECT id, file_id, operation, priority, status, retry_count,
                   max_retries, error_message, scheduled_at, last_attempt_at,
                   created_at, updated_at
            FROM sync_queue
            WHERE id = ?
            "#,
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await?;

        self.row_to_sync_queue_item(row).await
    }

    async fn update(&self, item: &SyncQueueItem) -> Result<()> {
        sqlx::query(
            r#"
            UPDATE sync_queue SET
                file_id = ?, operation = ?, priority = ?, status = ?,
                retry_count = ?, max_retries = ?, error_message = ?,
                scheduled_at = ?, last_attempt_at = ?, updated_at = ?
            WHERE id = ?
            "#,
        )
        .bind(item.file_id)
        .bind(&item.operation)
        .bind(item.priority)
        .bind(item.status)
        .bind(item.retry_count)
        .bind(item.max_retries)
        .bind(&item.error_message)
        .bind(item.scheduled_at)
        .bind(item.last_attempt_at)
        .bind(item.updated_at)
        .bind(item.id)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    async fn delete(&self, id: i64) -> Result<()> {
        sqlx::query("DELETE FROM sync_queue WHERE id = ?")
            .bind(id)
            .execute(&self.pool)
            .await?;

        Ok(())
    }

    async fn count(&self) -> Result<i64> {
        let count = sqlx::query_scalar("SELECT COUNT(*) FROM sync_queue")
            .fetch_one(&self.pool)
            .await?;

        Ok(count)
    }
}

#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct QueueStats {
    pub pending_count: u64,
    pub in_progress_count: u64,
    pub completed_count: u64,
    pub failed_count: u64,
    pub retriable_count: u64,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::database::models::{File, FileStatus};
    use crate::database::repositories::FileRepository;
    use tempfile::TempDir;

    async fn create_test_repo() -> (SyncQueueRepository, FileRepository, TempDir) {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("test.db");

        // Create the database using the Database struct which handles migrations
        let db = crate::database::Database::new(&db_path).await.unwrap();

        let sync_repo = SyncQueueRepository::new(db.pool().clone());
        let file_repo = FileRepository::new(db.pool().clone());
        (sync_repo, file_repo, temp_dir)
    }

    #[tokio::test]
    async fn test_sync_queue_crud() {
        let (sync_repo, file_repo, _temp_dir) = create_test_repo().await;

        // Create a file first
        let file = File {
            id: 0,
            path: "/test/file.txt".to_string(),
            size: 1024,
            modified_time: time_utils::now_millis(),
            inode: None,
            status: FileStatus::Processed,
            last_checked: time_utils::now_millis(),
            created_at: time_utils::now_millis(),
            updated_at: time_utils::now_millis(),
        };
        let file_id = file_repo.create(&file).await.unwrap();

        // Enqueue operation
        let id = sync_repo.enqueue(file_id, "mylist_add", 5).await.unwrap();
        assert!(id > 0);

        // Find by ID
        let item = sync_repo.find_by_id(id).await.unwrap().unwrap();
        assert_eq!(item.operation, "mylist_add");
        assert_eq!(item.priority, 5);
        assert_eq!(item.status, SyncStatus::Pending);

        // Find ready items
        let ready = sync_repo.find_ready(10).await.unwrap();
        assert_eq!(ready.len(), 1);
        assert!(ready[0].is_ready());

        // Update status
        sync_repo
            .update_status(id, SyncStatus::InProgress, None)
            .await
            .unwrap();

        // Verify status update
        let item = sync_repo.find_by_id(id).await.unwrap().unwrap();
        assert_eq!(item.status, SyncStatus::InProgress);

        // Simulate failure
        sync_repo
            .update_status(id, SyncStatus::Failed, Some("Network error"))
            .await
            .unwrap();

        // Find retriable
        let retriable = sync_repo.find_retriable(10).await.unwrap();
        assert_eq!(retriable.len(), 1);
        assert!(retriable[0].can_retry());

        // Retry with delay
        sync_repo.retry(id, 5000).await.unwrap();

        // Verify retry
        let item = sync_repo.find_by_id(id).await.unwrap().unwrap();
        assert_eq!(item.status, SyncStatus::Pending);
        assert_eq!(item.retry_count, 1);
    }

    #[tokio::test]
    async fn test_queue_stats() {
        let (sync_repo, file_repo, _temp_dir) = create_test_repo().await;

        // Create multiple items with different statuses
        for i in 0..10 {
            let file = File {
                id: 0,
                path: format!("/test/file{i}.txt"),
                size: 1024,
                modified_time: time_utils::now_millis(),
                inode: None,
                status: FileStatus::Processed,
                last_checked: time_utils::now_millis(),
                created_at: time_utils::now_millis(),
                updated_at: time_utils::now_millis(),
            };
            let file_id = file_repo.create(&file).await.unwrap();

            let status = match i % 4 {
                0 => SyncStatus::Pending,
                1 => SyncStatus::InProgress,
                2 => SyncStatus::Completed,
                _ => SyncStatus::Failed,
            };

            let item = SyncQueueItem {
                id: 0,
                file_id,
                operation: "test_op".to_string(),
                priority: i,
                status,
                retry_count: if status == SyncStatus::Failed { 1 } else { 0 },
                max_retries: 3,
                error_message: None,
                scheduled_at: time_utils::now_millis(),
                last_attempt_at: None,
                created_at: time_utils::now_millis(),
                updated_at: time_utils::now_millis(),
            };

            sync_repo.create(&item).await.unwrap();
        }

        // Get stats
        let stats = sync_repo.get_stats().await.unwrap();
        assert_eq!(stats.pending_count, 3); // 0, 4, 8
        assert_eq!(stats.in_progress_count, 3); // 1, 5, 9  
        assert_eq!(stats.completed_count, 2); // 2, 6
        assert_eq!(stats.failed_count, 2); // 3, 7
        assert_eq!(stats.retriable_count, 2); // All failed items can be retried
    }

    #[tokio::test]
    async fn test_batch_enqueue() {
        let (sync_repo, file_repo, _temp_dir) = create_test_repo().await;

        // Create files first
        let mut file_ids = Vec::new();
        for i in 0..10 {
            let file = File {
                id: 0,
                path: format!("/test/batch_{i}.mkv"),
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

        // Prepare batch operations
        let operations: Vec<(i64, String, i32)> = file_ids
            .iter()
            .enumerate()
            .map(|(i, &file_id)| {
                {
                    let op_num = i % 3;
                    (
                        file_id,
                        format!("operation_{op_num}"),
                        (i * 10) as i32, // priority
                    )
                }
            })
            .collect();

        // Batch enqueue
        let ids = sync_repo.batch_enqueue(&operations).await.unwrap();
        assert_eq!(ids.len(), 10);

        // Verify all enqueued
        let count = sync_repo.count().await.unwrap();
        assert_eq!(count, 10);

        // Verify data integrity
        for (i, &id) in ids.iter().enumerate() {
            let item = sync_repo.find_by_id(id).await.unwrap().unwrap();
            let expected_op = i % 3;
            assert_eq!(item.operation, format!("operation_{expected_op}"));
            assert_eq!(item.priority, (i * 10) as i32);
            assert_eq!(item.status, SyncStatus::Pending);
            assert_eq!(item.retry_count, 0);
            assert_eq!(item.max_retries, 3);
        }
    }

    #[tokio::test]
    async fn test_batch_update_status() {
        let (sync_repo, file_repo, _temp_dir) = create_test_repo().await;

        // Create test data
        let file = File {
            id: 0,
            path: "/test/status_update.mkv".to_string(),
            size: 1024,
            modified_time: time_utils::now_millis(),
            inode: None,
            status: FileStatus::Processed,
            last_checked: time_utils::now_millis(),
            created_at: time_utils::now_millis(),
            updated_at: time_utils::now_millis(),
        };
        let file_id = file_repo.create(&file).await.unwrap();

        // Create multiple sync items
        let mut ids = Vec::new();
        for i in 0..5 {
            let id = sync_repo
                .enqueue(file_id, &format!("op_{i}"), i)
                .await
                .unwrap();
            ids.push(id);
        }

        // Prepare status updates
        let updates: Vec<(i64, SyncStatus, Option<String>)> = ids
            .iter()
            .enumerate()
            .map(|(i, &id)| match i % 3 {
                0 => (id, SyncStatus::InProgress, None),
                1 => (id, SyncStatus::Completed, None),
                _ => (id, SyncStatus::Failed, Some(format!("Error {i}"))),
            })
            .collect();

        // Batch update
        let affected = sync_repo.batch_update_status(&updates).await.unwrap();
        assert_eq!(affected, 5);

        // Verify updates
        for (i, &id) in ids.iter().enumerate() {
            let item = sync_repo.find_by_id(id).await.unwrap().unwrap();
            match i % 3 {
                0 => {
                    assert_eq!(item.status, SyncStatus::InProgress);
                    assert!(item.error_message.is_none());
                }
                1 => {
                    assert_eq!(item.status, SyncStatus::Completed);
                    assert!(item.error_message.is_none());
                }
                _ => {
                    assert_eq!(item.status, SyncStatus::Failed);
                    assert_eq!(item.error_message, Some(format!("Error {i}")));
                }
            }
        }
    }

    #[tokio::test]
    async fn test_batch_retry() {
        let (sync_repo, file_repo, _temp_dir) = create_test_repo().await;

        // Create test data
        let file = File {
            id: 0,
            path: "/test/retry.mkv".to_string(),
            size: 1024,
            modified_time: time_utils::now_millis(),
            inode: None,
            status: FileStatus::Processed,
            last_checked: time_utils::now_millis(),
            created_at: time_utils::now_millis(),
            updated_at: time_utils::now_millis(),
        };
        let file_id = file_repo.create(&file).await.unwrap();

        // Create failed items
        let mut ids = Vec::new();
        for i in 0..5 {
            let item = SyncQueueItem {
                id: 0,
                file_id,
                operation: format!("retry_op_{i}"),
                priority: i,
                status: SyncStatus::Failed,
                retry_count: 1,
                max_retries: 3,
                error_message: Some("Initial failure".to_string()),
                scheduled_at: time_utils::now_millis(),
                last_attempt_at: Some(time_utils::now_millis()),
                created_at: time_utils::now_millis(),
                updated_at: time_utils::now_millis(),
            };
            let id = sync_repo.create(&item).await.unwrap();
            ids.push(id);
        }

        // Batch retry with 5 second delay
        let delay_ms = 5000;
        let affected = sync_repo.batch_retry(&ids, delay_ms).await.unwrap();
        assert_eq!(affected, 5);

        // Verify retries
        for &id in &ids {
            let item = sync_repo.find_by_id(id).await.unwrap().unwrap();
            assert_eq!(item.status, SyncStatus::Pending);
            assert_eq!(item.retry_count, 2);
            assert!(item.scheduled_at > time_utils::now_millis());
        }
    }

    #[tokio::test]
    async fn test_cancel_by_file_ids() {
        let (sync_repo, file_repo, _temp_dir) = create_test_repo().await;

        // Create multiple files
        let mut file_ids = Vec::new();
        for i in 0..5 {
            let file = File {
                id: 0,
                path: format!("/test/cancel_{i}.mkv"),
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

            // Create various sync items
            for status in &[
                SyncStatus::Pending,
                SyncStatus::Failed,
                SyncStatus::InProgress,
                SyncStatus::Completed,
            ] {
                let item = SyncQueueItem {
                    id: 0,
                    file_id,
                    operation: format!("op_{i}_{status:?}"),
                    priority: i,
                    status: *status,
                    retry_count: 0,
                    max_retries: 3,
                    error_message: None,
                    scheduled_at: time_utils::now_millis(),
                    last_attempt_at: None,
                    created_at: time_utils::now_millis(),
                    updated_at: time_utils::now_millis(),
                };
                sync_repo.create(&item).await.unwrap();
            }
        }

        // Cancel by file IDs (should only cancel Pending and Failed)
        let cancelled = sync_repo.cancel_by_file_ids(&file_ids[..3]).await.unwrap();
        assert_eq!(cancelled, 6); // 2 statuses (Pending, Failed) × 3 files

        // Verify remaining items
        for (i, &file_id) in file_ids.iter().enumerate() {
            let items = sync_repo.find_by_file_id(file_id).await.unwrap();
            if i < 3 {
                // First 3 files should only have InProgress and Completed
                assert_eq!(items.len(), 2);
                for item in items {
                    assert!(
                        item.status == SyncStatus::InProgress
                            || item.status == SyncStatus::Completed
                    );
                }
            } else {
                // Last 2 files should have all 4 statuses
                assert_eq!(items.len(), 4);
            }
        }
    }

    #[tokio::test]
    async fn test_get_file_history() {
        let (sync_repo, file_repo, _temp_dir) = create_test_repo().await;

        // Create a file
        let file = File {
            id: 0,
            path: "/test/history.mkv".to_string(),
            size: 1024,
            modified_time: time_utils::now_millis(),
            inode: None,
            status: FileStatus::Processed,
            last_checked: time_utils::now_millis(),
            created_at: time_utils::now_millis(),
            updated_at: time_utils::now_millis(),
        };
        let file_id = file_repo.create(&file).await.unwrap();

        // Create history of operations
        let operations = [
            ("mylist_add", SyncStatus::Completed),
            ("metadata_update", SyncStatus::Failed),
            ("mylist_update", SyncStatus::Pending),
            ("mylist_delete", SyncStatus::InProgress),
        ];

        let mut created_times = Vec::new();
        for (i, (op, status)) in operations.iter().enumerate() {
            let created_at = time_utils::now_millis() - (1000 * (operations.len() - i) as i64);
            created_times.push(created_at);

            let item = SyncQueueItem {
                id: 0,
                file_id,
                operation: op.to_string(),
                priority: i as i32,
                status: *status,
                retry_count: 0,
                max_retries: 3,
                error_message: if *status == SyncStatus::Failed {
                    Some("Test error".to_string())
                } else {
                    None
                },
                scheduled_at: created_at,
                last_attempt_at: None,
                created_at,
                updated_at: created_at,
            };
            sync_repo.create(&item).await.unwrap();
        }

        // Get file history
        let history = sync_repo.get_file_history(file_id, 10).await.unwrap();
        assert_eq!(history.len(), 4);

        // Verify ordering (newest first)
        assert_eq!(history[0].operation, "mylist_delete");
        assert_eq!(history[1].operation, "mylist_update");
        assert_eq!(history[2].operation, "metadata_update");
        assert_eq!(history[3].operation, "mylist_add");

        // Test limit
        let limited_history = sync_repo.get_file_history(file_id, 2).await.unwrap();
        assert_eq!(limited_history.len(), 2);
        assert_eq!(limited_history[0].operation, "mylist_delete");
        assert_eq!(limited_history[1].operation, "mylist_update");
    }
}
