//! Database model definitions
//!
//! This module contains all data structures that map to database tables.

use serde::{Deserialize, Serialize};
use std::time::SystemTime;

/// File status enum
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "TEXT")]
pub enum FileStatus {
    #[sqlx(rename = "pending")]
    Pending,
    #[sqlx(rename = "processing")]
    Processing,
    #[sqlx(rename = "processed")]
    Processed,
    #[sqlx(rename = "error")]
    Error,
    #[sqlx(rename = "deleted")]
    Deleted,
}

/// Sync operation status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "TEXT")]
pub enum SyncStatus {
    #[sqlx(rename = "pending")]
    Pending,
    #[sqlx(rename = "in_progress")]
    InProgress,
    #[sqlx(rename = "completed")]
    Completed,
    #[sqlx(rename = "failed")]
    Failed,
}

/// MyList entry status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MyListStatus {
    Unknown = 0,
    OnHDD = 1,
    OnCD = 2,
    Deleted = 3,
}

/// File record
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct File {
    pub id: i64,
    pub path: String,
    pub size: i64,
    pub modified_time: i64,
    pub inode: Option<i64>,
    pub status: FileStatus,
    pub last_checked: i64,
    pub created_at: i64,
    pub updated_at: i64,
}

impl File {
    /// Convert modified_time to SystemTime
    pub fn modified_time_as_system_time(&self) -> SystemTime {
        SystemTime::UNIX_EPOCH + std::time::Duration::from_millis(self.modified_time as u64)
    }
}

/// Hash record
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Hash {
    pub id: i64,
    pub file_id: i64,
    pub algorithm: String,
    pub hash: String,
    pub duration_ms: i64,
    pub created_at: i64,
}

/// AniDB identification result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AniDBResult {
    pub id: i64,
    pub file_id: i64,
    pub ed2k_hash: String,
    pub file_size: i64,
    pub anime_id: Option<i64>,
    pub episode_id: Option<i64>,
    pub episode_number: Option<String>,
    pub anime_title: Option<String>,
    pub episode_title: Option<String>,
    pub group_name: Option<String>,
    pub group_short: Option<String>,
    pub version: Option<i32>,
    pub censored: Option<bool>,
    pub deprecated: Option<bool>,
    pub crc32_valid: Option<bool>,
    pub file_type: Option<String>,
    pub resolution: Option<String>,
    pub video_codec: Option<String>,
    pub audio_codec: Option<String>,
    pub source: Option<String>,
    pub quality: Option<String>,
    pub mylist_lid: Option<i64>,
    pub fetched_at: i64,
    pub expires_at: Option<i64>,
    pub created_at: i64,
    pub updated_at: i64,
}

impl AniDBResult {
    /// Check if the result has expired
    pub fn is_expired(&self) -> bool {
        if let Some(expires_at) = self.expires_at {
            let now = SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis() as i64;
            now > expires_at
        } else {
            false
        }
    }
}

/// MyList cache entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MyListEntry {
    pub id: i64,
    pub file_id: i64,
    pub mylist_id: i64,
    pub state: i32,
    pub filestate: i32,
    pub viewed: bool,
    pub viewdate: Option<i64>,
    pub storage: Option<String>,
    pub source: Option<String>,
    pub other: Option<String>,
    pub created_at: i64,
    pub updated_at: i64,
}

/// Sync queue item
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncQueueItem {
    pub id: i64,
    pub file_id: i64,
    pub operation: String,
    pub priority: i32,
    pub status: SyncStatus,
    pub retry_count: i32,
    pub max_retries: i32,
    pub error_message: Option<String>,
    pub scheduled_at: i64,
    pub last_attempt_at: Option<i64>,
    pub created_at: i64,
    pub updated_at: i64,
}

impl SyncQueueItem {
    /// Check if the item can be retried
    pub fn can_retry(&self) -> bool {
        self.status == SyncStatus::Failed && self.retry_count < self.max_retries
    }

    /// Check if the item is ready to be processed
    pub fn is_ready(&self) -> bool {
        let now = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as i64;
        self.status == SyncStatus::Pending && self.scheduled_at <= now
    }
}

/// Schema version record
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SchemaVersion {
    pub version: i32,
    pub applied_at: i64,
}

/// Helper functions for time conversion
pub mod time_utils {
    use std::time::{Duration, SystemTime, UNIX_EPOCH};

    /// Convert SystemTime to milliseconds since Unix epoch
    pub fn system_time_to_millis(time: SystemTime) -> i64 {
        time.duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as i64
    }

    /// Convert milliseconds since Unix epoch to SystemTime
    pub fn millis_to_system_time(millis: i64) -> SystemTime {
        UNIX_EPOCH + Duration::from_millis(millis as u64)
    }

    /// Get current time as milliseconds since Unix epoch
    pub fn now_millis() -> i64 {
        system_time_to_millis(SystemTime::now())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_file_status_serialization() {
        assert_eq!(
            serde_json::to_string(&FileStatus::Pending).unwrap(),
            "\"Pending\""
        );
        assert_eq!(
            serde_json::to_string(&FileStatus::Processed).unwrap(),
            "\"Processed\""
        );
    }

    #[test]
    fn test_anidb_result_expiration() {
        let now = time_utils::now_millis();

        let mut result = AniDBResult {
            id: 1,
            file_id: 1,
            ed2k_hash: "test".to_string(),
            file_size: 1024,
            anime_id: Some(1),
            episode_id: Some(1),
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
            fetched_at: now,
            expires_at: Some(now + 1000),
            mylist_lid: None,
            created_at: now,
            updated_at: now,
        };

        // Not expired
        assert!(!result.is_expired());

        // Expired
        result.expires_at = Some(now - 1000);
        assert!(result.is_expired());

        // No expiration
        result.expires_at = None;
        assert!(!result.is_expired());
    }

    #[test]
    fn test_sync_queue_item_retry() {
        let item = SyncQueueItem {
            id: 1,
            file_id: 1,
            operation: "test".to_string(),
            priority: 0,
            status: SyncStatus::Failed,
            retry_count: 1,
            max_retries: 3,
            error_message: None,
            scheduled_at: 0,
            last_attempt_at: None,
            created_at: 0,
            updated_at: 0,
        };

        assert!(item.can_retry());

        let mut item2 = item.clone();
        item2.retry_count = 3;
        assert!(!item2.can_retry());

        let mut item3 = item.clone();
        item3.status = SyncStatus::Completed;
        assert!(!item3.can_retry());
    }
}
