//! MyList repository implementation

use crate::Result;
use crate::database::models::{MyListEntry, time_utils};
use async_trait::async_trait;
use sqlx::{Row, SqlitePool};

use super::Repository;

/// Repository for MyList operations
pub struct MyListRepository {
    pool: SqlitePool,
}

impl MyListRepository {
    /// Create a new MyList repository
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    /// Find by file ID
    pub async fn find_by_file_id(&self, file_id: i64) -> Result<Option<MyListEntry>> {
        let row = sqlx::query(
            r#"
            SELECT id, file_id, mylist_id, state, filestate, viewed,
                   viewdate, storage, source, other, created_at, updated_at
            FROM mylist_cache
            WHERE file_id = ?
            "#,
        )
        .bind(file_id)
        .fetch_optional(&self.pool)
        .await?;

        self.row_to_mylist_entry(row).await
    }

    /// Find by MyList ID
    pub async fn find_by_mylist_id(&self, mylist_id: i64) -> Result<Option<MyListEntry>> {
        let row = sqlx::query(
            r#"
            SELECT id, file_id, mylist_id, state, filestate, viewed,
                   viewdate, storage, source, other, created_at, updated_at
            FROM mylist_cache
            WHERE mylist_id = ?
            "#,
        )
        .bind(mylist_id)
        .fetch_optional(&self.pool)
        .await?;

        self.row_to_mylist_entry(row).await
    }

    /// Find all viewed entries
    pub async fn find_viewed(&self, limit: i64) -> Result<Vec<MyListEntry>> {
        let rows = sqlx::query(
            r#"
            SELECT id, file_id, mylist_id, state, filestate, viewed,
                   viewdate, storage, source, other, created_at, updated_at
            FROM mylist_cache
            WHERE viewed = TRUE
            ORDER BY viewdate DESC
            LIMIT ?
            "#,
        )
        .bind(limit)
        .fetch_all(&self.pool)
        .await?;

        let mut entries = Vec::with_capacity(rows.len());
        for row in rows {
            if let Some(entry) = self.row_to_mylist_entry(Some(row)).await? {
                entries.push(entry);
            }
        }

        Ok(entries)
    }

    /// Find by state
    pub async fn find_by_state(&self, state: i32, limit: i64) -> Result<Vec<MyListEntry>> {
        let rows = sqlx::query(
            r#"
            SELECT id, file_id, mylist_id, state, filestate, viewed,
                   viewdate, storage, source, other, created_at, updated_at
            FROM mylist_cache
            WHERE state = ?
            ORDER BY updated_at DESC
            LIMIT ?
            "#,
        )
        .bind(state)
        .bind(limit)
        .fetch_all(&self.pool)
        .await?;

        let mut entries = Vec::with_capacity(rows.len());
        for row in rows {
            if let Some(entry) = self.row_to_mylist_entry(Some(row)).await? {
                entries.push(entry);
            }
        }

        Ok(entries)
    }

    /// Update viewed status
    pub async fn update_viewed(
        &self,
        file_id: i64,
        viewed: bool,
        viewdate: Option<i64>,
    ) -> Result<()> {
        let now = time_utils::now_millis();

        sqlx::query(
            r#"
            UPDATE mylist_cache 
            SET viewed = ?, viewdate = ?, updated_at = ?
            WHERE file_id = ?
            "#,
        )
        .bind(viewed)
        .bind(viewdate)
        .bind(now)
        .bind(file_id)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    /// Create or update an entry
    pub async fn upsert(&self, entry: &MyListEntry) -> Result<i64> {
        let result = sqlx::query(
            r#"
            INSERT INTO mylist_cache (
                file_id, mylist_id, state, filestate, viewed,
                viewdate, storage, source, other, created_at, updated_at
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            ON CONFLICT(file_id) DO UPDATE SET
                mylist_id = excluded.mylist_id,
                state = excluded.state,
                filestate = excluded.filestate,
                viewed = excluded.viewed,
                viewdate = excluded.viewdate,
                storage = excluded.storage,
                source = excluded.source,
                other = excluded.other,
                updated_at = excluded.updated_at
            "#,
        )
        .bind(entry.file_id)
        .bind(entry.mylist_id)
        .bind(entry.state)
        .bind(entry.filestate)
        .bind(entry.viewed)
        .bind(entry.viewdate)
        .bind(&entry.storage)
        .bind(&entry.source)
        .bind(&entry.other)
        .bind(entry.created_at)
        .bind(entry.updated_at)
        .execute(&self.pool)
        .await?;

        Ok(result.last_insert_rowid())
    }

    /// Get statistics
    pub async fn get_stats(&self) -> Result<MyListStats> {
        let total_count = sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM mylist_cache")
            .fetch_one(&self.pool)
            .await?;

        let viewed_count =
            sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM mylist_cache WHERE viewed = TRUE")
                .fetch_one(&self.pool)
                .await?;

        let state_counts = sqlx::query(
            r#"
            SELECT state, COUNT(*) as count
            FROM mylist_cache
            GROUP BY state
            "#,
        )
        .fetch_all(&self.pool)
        .await?;

        let mut state_distribution = Vec::new();
        for row in state_counts {
            let state: i32 = row.try_get("state")?;
            let count: i64 = row.try_get("count")?;
            state_distribution.push((state, count as u64));
        }

        Ok(MyListStats {
            total_entries: total_count as u64,
            viewed_entries: viewed_count as u64,
            state_distribution,
        })
    }

    /// Convert a database row to MyListEntry
    async fn row_to_mylist_entry(
        &self,
        row: Option<sqlx::sqlite::SqliteRow>,
    ) -> Result<Option<MyListEntry>> {
        if let Some(row) = row {
            Ok(Some(MyListEntry {
                id: row.try_get("id")?,
                file_id: row.try_get("file_id")?,
                mylist_id: row.try_get("mylist_id")?,
                state: row.try_get("state")?,
                filestate: row.try_get("filestate")?,
                viewed: row.try_get("viewed")?,
                viewdate: row.try_get("viewdate")?,
                storage: row.try_get("storage")?,
                source: row.try_get("source")?,
                other: row.try_get("other")?,
                created_at: row.try_get("created_at")?,
                updated_at: row.try_get("updated_at")?,
            }))
        } else {
            Ok(None)
        }
    }
}

#[async_trait]
impl Repository<MyListEntry> for MyListRepository {
    async fn create(&self, entry: &MyListEntry) -> Result<i64> {
        self.upsert(entry).await
    }

    async fn find_by_id(&self, id: i64) -> Result<Option<MyListEntry>> {
        let row = sqlx::query(
            r#"
            SELECT id, file_id, mylist_id, state, filestate, viewed,
                   viewdate, storage, source, other, created_at, updated_at
            FROM mylist_cache
            WHERE id = ?
            "#,
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await?;

        self.row_to_mylist_entry(row).await
    }

    async fn update(&self, entry: &MyListEntry) -> Result<()> {
        sqlx::query(
            r#"
            UPDATE mylist_cache SET
                file_id = ?, mylist_id = ?, state = ?, filestate = ?,
                viewed = ?, viewdate = ?, storage = ?, source = ?,
                other = ?, updated_at = ?
            WHERE id = ?
            "#,
        )
        .bind(entry.file_id)
        .bind(entry.mylist_id)
        .bind(entry.state)
        .bind(entry.filestate)
        .bind(entry.viewed)
        .bind(entry.viewdate)
        .bind(&entry.storage)
        .bind(&entry.source)
        .bind(&entry.other)
        .bind(entry.updated_at)
        .bind(entry.id)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    async fn delete(&self, id: i64) -> Result<()> {
        sqlx::query("DELETE FROM mylist_cache WHERE id = ?")
            .bind(id)
            .execute(&self.pool)
            .await?;

        Ok(())
    }

    async fn count(&self) -> Result<i64> {
        let count = sqlx::query_scalar("SELECT COUNT(*) FROM mylist_cache")
            .fetch_one(&self.pool)
            .await?;

        Ok(count)
    }
}

/// MyList statistics
#[derive(Debug, Clone, Default)]
pub struct MyListStats {
    pub total_entries: u64,
    pub viewed_entries: u64,
    pub state_distribution: Vec<(i32, u64)>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::database::models::{File, FileStatus};
    use crate::database::repositories::FileRepository;
    use tempfile::TempDir;

    async fn create_test_repo() -> (MyListRepository, FileRepository, TempDir) {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("test.db");

        // Create the database using the Database struct which handles migrations
        let db = crate::database::Database::new(&db_path).await.unwrap();

        let mylist_repo = MyListRepository::new(db.pool().clone());
        let file_repo = FileRepository::new(db.pool().clone());
        (mylist_repo, file_repo, temp_dir)
    }

    #[tokio::test]
    async fn test_mylist_crud() {
        let (mylist_repo, file_repo, _temp_dir) = create_test_repo().await;

        // Create a file first
        let file = File {
            id: 0,
            path: "/test/anime.mkv".to_string(),
            size: 1024,
            modified_time: time_utils::now_millis(),
            inode: None,
            status: FileStatus::Processed,
            last_checked: time_utils::now_millis(),
            created_at: time_utils::now_millis(),
            updated_at: time_utils::now_millis(),
        };
        let file_id = file_repo.create(&file).await.unwrap();

        // Create MyList entry
        let entry = MyListEntry {
            id: 0,
            file_id,
            mylist_id: 12345,
            state: 1, // OnHDD
            filestate: 0,
            viewed: false,
            viewdate: None,
            storage: Some("HDD1".to_string()),
            source: Some("torrent".to_string()),
            other: None,
            created_at: time_utils::now_millis(),
            updated_at: time_utils::now_millis(),
        };

        // Create
        let id = mylist_repo.create(&entry).await.unwrap();
        assert!(id > 0);

        // Find by file ID
        let found = mylist_repo.find_by_file_id(file_id).await.unwrap();
        assert!(found.is_some());
        let found = found.unwrap();
        assert_eq!(found.mylist_id, entry.mylist_id);
        assert_eq!(found.storage, entry.storage);

        // Update viewed status
        mylist_repo
            .update_viewed(file_id, true, Some(time_utils::now_millis()))
            .await
            .unwrap();

        // Verify update
        let found = mylist_repo.find_by_file_id(file_id).await.unwrap().unwrap();
        assert!(found.viewed);
        assert!(found.viewdate.is_some());

        // Find viewed entries
        let viewed = mylist_repo.find_viewed(10).await.unwrap();
        assert_eq!(viewed.len(), 1);
    }

    #[tokio::test]
    async fn test_mylist_stats() {
        let (mylist_repo, file_repo, _temp_dir) = create_test_repo().await;

        // Create multiple entries
        for i in 0..5 {
            let file = File {
                id: 0,
                path: format!("/test/anime{i}.mkv"),
                size: 1024,
                modified_time: time_utils::now_millis(),
                inode: None,
                status: FileStatus::Processed,
                last_checked: time_utils::now_millis(),
                created_at: time_utils::now_millis(),
                updated_at: time_utils::now_millis(),
            };
            let file_id = file_repo.create(&file).await.unwrap();

            let entry = MyListEntry {
                id: 0,
                file_id,
                mylist_id: 10000 + i,
                state: if i % 2 == 0 { 1 } else { 2 }, // Alternate states
                filestate: 0,
                viewed: i % 3 == 0, // Some viewed
                viewdate: if i % 3 == 0 {
                    Some(time_utils::now_millis())
                } else {
                    None
                },
                storage: None,
                source: None,
                other: None,
                created_at: time_utils::now_millis(),
                updated_at: time_utils::now_millis(),
            };

            mylist_repo.create(&entry).await.unwrap();
        }

        // Get stats
        let stats = mylist_repo.get_stats().await.unwrap();
        assert_eq!(stats.total_entries, 5);
        assert_eq!(stats.viewed_entries, 2); // 0 and 3 are viewed
        assert_eq!(stats.state_distribution.len(), 2);
    }
}
