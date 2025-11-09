//! Sync service implementation
//!
//! This module provides the core sync service that processes the sync queue
//! and sends files to MyList on AniDB.

use anidb_client_core::database::models::{SyncQueueItem, SyncStatus};
use anidb_client_core::database::repositories::anidb_result::AniDBResultRepository;
use anidb_client_core::database::repositories::sync_queue::{QueueStats, SyncQueueRepository};
use anidb_client_core::database::repositories::{FileRepository, HashRepository, Repository};
use anidb_client_core::error::{Error, IoError, ProtocolError, Result, ValidationError};
use anidb_client_core::protocol::client::ProtocolClient;
use anidb_client_core::protocol::messages::{
    Command, MyListAddCommand, MyListAddResponse, MyListDelCommand, MyListDelResponse, Response,
};
use anidb_client_core::security::credential_store::CredentialStore;
use anidb_client_core::security::fallback::EncryptedFileStore;
use log::{debug, error, info, warn};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Mutex;
use tokio::time::sleep;

/// Service trait for sync operations
#[async_trait::async_trait]
pub trait SyncService: Send + Sync {
    /// Process pending items from the sync queue
    async fn process_queue(&self, limit: usize) -> Result<ProcessResult>;

    /// Process a single sync item
    async fn process_item(&self, item: &SyncQueueItem) -> Result<ItemResult>;

    /// Get queue statistics
    async fn get_stats(&self) -> Result<QueueStats>;

    /// Cancel sync operations for file IDs
    #[allow(dead_code)]
    async fn cancel_by_file_ids(&self, file_ids: &[i64]) -> Result<u64>;

    /// Clear completed items older than the specified duration
    async fn clear_completed(&self, max_age: Duration) -> Result<u64>;
}

/// Result of processing the sync queue
#[derive(Debug, Clone)]
pub struct ProcessResult {
    /// Number of items processed
    pub processed: usize,
    /// Number of successful syncs
    pub succeeded: usize,
    /// Number of failed syncs
    pub failed: usize,
    /// Number of items already in list
    pub already_in_list: usize,
    /// Total processing time
    pub duration: Duration,
}

/// Result of processing a single sync item
#[derive(Debug, Clone)]
pub enum ItemResult {
    /// Successfully added to MyList
    Success { lid: u64 },
    /// Already in MyList
    AlreadyInList { lid: u64 },
    /// Failed with error
    Failed { error: String, can_retry: bool },
    /// Skipped (e.g., not ready)
    #[allow(dead_code)]
    Skipped,
}

/// Main sync service implementation
pub struct AniDBSyncService {
    /// Sync queue repository
    sync_repo: Arc<SyncQueueRepository>,
    /// File repository
    file_repo: Arc<FileRepository>,
    /// Hash repository
    hash_repo: Arc<HashRepository>,
    /// AniDB result repository (for cache updates)
    anidb_repo: Arc<AniDBResultRepository>,
    /// Protocol client for AniDB communication
    protocol_client: Arc<Mutex<ProtocolClient>>,
    /// Credential store
    credential_store: Arc<EncryptedFileStore>,
    /// Service configuration
    config: SyncServiceConfig,
}

/// Sync service configuration
#[derive(Debug, Clone)]
pub struct SyncServiceConfig {
    /// Maximum items to process per batch
    #[allow(dead_code)]
    pub batch_size: usize,
    /// Delay between operations (rate limiting)
    pub operation_delay: Duration,
    /// Initial retry delay
    pub initial_retry_delay: Duration,
    /// Maximum retry delay
    pub max_retry_delay: Duration,
    /// Enable verbose logging
    pub verbose: bool,
}

impl Default for SyncServiceConfig {
    fn default() -> Self {
        Self {
            batch_size: 10,
            operation_delay: Duration::from_millis(2100), // AniDB rate limit: 0.5 req/sec
            initial_retry_delay: Duration::from_secs(2),
            max_retry_delay: Duration::from_secs(60),
            verbose: false,
        }
    }
}

impl AniDBSyncService {
    /// Create a new sync service
    pub fn new(
        sync_repo: Arc<SyncQueueRepository>,
        file_repo: Arc<FileRepository>,
        hash_repo: Arc<HashRepository>,
        anidb_repo: Arc<AniDBResultRepository>,
        protocol_client: Arc<Mutex<ProtocolClient>>,
        credential_store: Arc<EncryptedFileStore>,
        config: SyncServiceConfig,
    ) -> Self {
        Self {
            sync_repo,
            file_repo,
            hash_repo,
            anidb_repo,
            protocol_client,
            credential_store,
            config,
        }
    }

    /// Calculate retry delay with exponential backoff
    fn calculate_retry_delay(&self, retry_count: i32) -> Duration {
        let delay_secs = self.config.initial_retry_delay.as_secs() * 2u64.pow(retry_count as u32);
        let max_secs = self.config.max_retry_delay.as_secs();
        Duration::from_secs(delay_secs.min(max_secs))
    }

    /// Process a MYLISTADD operation
    async fn process_mylist_add(&self, item: &SyncQueueItem) -> Result<ItemResult> {
        debug!("Processing MYLISTADD for file_id: {}", item.file_id);

        // Get file information
        let file = self
            .file_repo
            .find_by_id(item.file_id)
            .await?
            .ok_or_else(|| {
                Error::Io(IoError::file_not_found(std::path::Path::new(&format!(
                    "file_id:{}",
                    item.file_id
                ))))
            })?;

        // Get hash information for the file
        let hashes = self.hash_repo.find_by_file_id(item.file_id).await?;

        // Find ED2K hash (required for MyList)
        let ed2k_hash = hashes
            .iter()
            .find(|h| h.algorithm == "ed2k")
            .ok_or_else(|| {
                Error::Validation(ValidationError::missing_field(
                    "ED2K hash not found for file",
                ))
            })?;

        // Create MYLISTADD command
        let command = MyListAddCommand::by_hash(file.size as u64, &ed2k_hash.hash)
            .with_state(1) // 1 = on HDD
            .with_viewed(false);

        // Send command via protocol client
        let client = self.protocol_client.lock().await;

        // Ensure we're authenticated
        if !client.is_authenticated().await {
            // Try to authenticate using stored credentials
            let credentials = self
                .credential_store
                .retrieve("anidb", "default")
                .await
                .map_err(|e| {
                    Error::Validation(ValidationError::invalid_configuration(&format!(
                        "Failed to retrieve credentials: {e}"
                    )))
                })?;

            client
                .authenticate(
                    credentials.account.clone(),
                    credentials.secret.expose_secret().to_string(),
                )
                .await
                .map_err(|e| {
                    Error::Protocol(ProtocolError::other(format!("Authentication failed: {e}")))
                })?;
        }

        // Send the MYLISTADD command
        let response = client
            .send_command(Command::MyListAdd(command))
            .await
            .map_err(|e| {
                Error::Protocol(ProtocolError::other(format!(
                    "Failed to send MYLISTADD: {e}"
                )))
            })?;

        // Parse response based on response type
        let (code, message, fields) = match response {
            Response::Generic(ref r) => (r.code, r.message.clone(), r.fields.clone()),
            _ => {
                return Ok(ItemResult::Failed {
                    error: "Unexpected response type".to_string(),
                    can_retry: false,
                });
            }
        };

        let mylist_response = MyListAddResponse::parse(code, message, fields)?;

        // Process response
        if mylist_response.success() {
            let lid = mylist_response.lid.unwrap_or(0);
            info!(
                "Successfully added file {} to MyList (lid: {})",
                item.file_id, lid
            );

            // Update cache with mylist_lid
            if let Err(e) = self
                .anidb_repo
                .update_mylist_lid(item.file_id, Some(lid as i64))
                .await
            {
                warn!("Failed to update cache with mylist_lid: {}", e);
            }

            Ok(ItemResult::Success { lid })
        } else if mylist_response.already_in_list() {
            let lid = mylist_response.lid.unwrap_or(0);
            info!("File {} already in MyList (lid: {})", item.file_id, lid);

            // Update cache with mylist_lid
            if let Err(e) = self
                .anidb_repo
                .update_mylist_lid(item.file_id, Some(lid as i64))
                .await
            {
                warn!("Failed to update cache with mylist_lid: {}", e);
            }

            Ok(ItemResult::AlreadyInList { lid })
        } else if mylist_response.file_not_found() {
            error!("File {} not found in AniDB", item.file_id);
            Ok(ItemResult::Failed {
                error: "File not found in AniDB".to_string(),
                can_retry: false,
            })
        } else {
            warn!(
                "MYLISTADD failed for file {}: {}",
                item.file_id,
                mylist_response.status_message()
            );
            Ok(ItemResult::Failed {
                error: mylist_response.status_message(),
                can_retry: true,
            })
        }
    }

    /// Process a MYLISTDEL operation
    async fn process_mylist_del(&self, item: &SyncQueueItem) -> Result<ItemResult> {
        debug!("Processing MYLISTDEL for file_id: {}", item.file_id);

        // For now, we'll delete by file ID
        // In the future, we might want to track the MyList ID
        let command = MyListDelCommand::by_fid(item.file_id as u64);

        // Send command via protocol client
        let client = self.protocol_client.lock().await;

        // Ensure we're authenticated
        if !client.is_authenticated().await {
            let credentials = self
                .credential_store
                .retrieve("anidb", "default")
                .await
                .map_err(|e| {
                    Error::Validation(ValidationError::invalid_configuration(&format!(
                        "Failed to retrieve credentials: {e}"
                    )))
                })?;

            client
                .authenticate(
                    credentials.account.clone(),
                    credentials.secret.expose_secret().to_string(),
                )
                .await
                .map_err(|e| {
                    Error::Protocol(ProtocolError::other(format!("Authentication failed: {e}")))
                })?;
        }

        let response = client
            .send_command(Command::MyListDel(command))
            .await
            .map_err(|e| {
                Error::Protocol(ProtocolError::other(format!(
                    "Failed to send MYLISTDEL: {e}"
                )))
            })?;

        // Parse response based on response type
        let (code, message, fields) = match response {
            Response::Generic(ref r) => (r.code, r.message.clone(), r.fields.clone()),
            _ => {
                return Ok(ItemResult::Failed {
                    error: "Unexpected response type".to_string(),
                    can_retry: false,
                });
            }
        };

        let del_response = MyListDelResponse::parse(code, message, fields)?;

        if del_response.success() {
            info!("Successfully deleted file {} from MyList", item.file_id);
            Ok(ItemResult::Success { lid: 0 })
        } else {
            warn!(
                "MYLISTDEL failed for file {}: {}",
                item.file_id, del_response.message
            );
            Ok(ItemResult::Failed {
                error: del_response.message.to_string(),
                can_retry: true,
            })
        }
    }
}

#[async_trait::async_trait]
impl SyncService for AniDBSyncService {
    async fn process_queue(&self, limit: usize) -> Result<ProcessResult> {
        let start = std::time::Instant::now();
        let mut result = ProcessResult {
            processed: 0,
            succeeded: 0,
            failed: 0,
            already_in_list: 0,
            duration: Duration::default(),
        };

        // Get pending items from queue
        let items = self.sync_repo.find_ready(limit as i64).await?;

        if items.is_empty() {
            debug!("No items ready for processing");
            result.duration = start.elapsed();
            return Ok(result);
        }

        info!("Processing {} items from sync queue", items.len());

        for item in &items {
            // Update status to in-progress
            self.sync_repo
                .update_status(item.id, SyncStatus::InProgress, None)
                .await?;

            // Process the item
            match self.process_item(item).await {
                Ok(ItemResult::Success { lid }) => {
                    if self.config.verbose {
                        debug!("Item {} synced successfully (lid: {lid})", item.id);
                    }
                    result.succeeded += 1;
                    self.sync_repo
                        .update_status(item.id, SyncStatus::Completed, None)
                        .await?;
                }
                Ok(ItemResult::AlreadyInList { lid }) => {
                    if self.config.verbose {
                        debug!("Item {} already present (lid: {lid})", item.id);
                    }
                    result.already_in_list += 1;
                    self.sync_repo
                        .update_status(item.id, SyncStatus::Completed, Some("Already in MyList"))
                        .await?;
                }
                Ok(ItemResult::Failed { error, can_retry }) => {
                    result.failed += 1;

                    if can_retry && item.can_retry() {
                        // Schedule retry with exponential backoff
                        let delay = self.calculate_retry_delay(item.retry_count);
                        self.sync_repo
                            .retry(item.id, delay.as_millis() as i64)
                            .await?;
                        info!("Scheduled retry for item {} in {:?}", item.id, delay);
                    } else {
                        // Mark as permanently failed
                        self.sync_repo
                            .update_status(item.id, SyncStatus::Failed, Some(&error))
                            .await?;
                    }
                }
                Ok(ItemResult::Skipped) => {
                    // Reset to pending
                    self.sync_repo
                        .update_status(item.id, SyncStatus::Pending, None)
                        .await?;
                }
                Err(e) => {
                    error!("Error processing item {}: {}", item.id, e);
                    result.failed += 1;

                    // Mark as failed with error message
                    self.sync_repo
                        .update_status(item.id, SyncStatus::Failed, Some(&e.to_string()))
                        .await?;
                }
            }

            result.processed += 1;

            // Rate limiting
            if result.processed < items.len() {
                sleep(self.config.operation_delay).await;
            }
        }

        result.duration = start.elapsed();
        info!("Sync queue processing complete: {:?}", result);
        Ok(result)
    }

    async fn process_item(&self, item: &SyncQueueItem) -> Result<ItemResult> {
        if self.config.verbose {
            debug!("Processing sync item: {item:?}");
        }

        match item.operation.as_str() {
            "mylist_add" => self.process_mylist_add(item).await,
            "mylist_del" => self.process_mylist_del(item).await,
            _ => {
                warn!("Unknown sync operation: {}", item.operation);
                Ok(ItemResult::Failed {
                    error: format!("Unknown operation: {}", item.operation),
                    can_retry: false,
                })
            }
        }
    }

    async fn get_stats(&self) -> Result<QueueStats> {
        self.sync_repo.get_stats().await
    }

    #[allow(dead_code)]
    async fn cancel_by_file_ids(&self, file_ids: &[i64]) -> Result<u64> {
        self.sync_repo.cancel_by_file_ids(file_ids).await
    }

    async fn clear_completed(&self, max_age: Duration) -> Result<u64> {
        let max_age_ms = max_age.as_millis() as i64;
        self.sync_repo.clear_completed(max_age_ms).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use anidb_client_core::database::Database;
    use anidb_client_core::database::repositories::HashRepository;
    use anidb_client_core::protocol::ProtocolConfig;
    use tempfile::TempDir;

    async fn create_test_service() -> (AniDBSyncService, TempDir) {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("test.db");

        // Create database
        let db = Database::new(&db_path).await.unwrap();

        // Create repositories
        let sync_repo = Arc::new(SyncQueueRepository::new(db.pool().clone()));
        let file_repo = Arc::new(FileRepository::new(db.pool().clone()));
        let hash_repo = Arc::new(HashRepository::new(db.pool().clone()));
        let anidb_repo = Arc::new(AniDBResultRepository::new(db.pool().clone()));

        // Create protocol client
        let protocol_config = ProtocolConfig::default();
        let protocol_client = Arc::new(Mutex::new(
            ProtocolClient::new(protocol_config).await.unwrap(),
        ));

        // Create credential store
        let credential_store = Arc::new(EncryptedFileStore::new().await.unwrap());

        // Create service
        let config = SyncServiceConfig::default();
        let service = AniDBSyncService::new(
            sync_repo,
            file_repo,
            hash_repo,
            anidb_repo,
            protocol_client,
            credential_store,
            config,
        );

        (service, temp_dir)
    }

    #[tokio::test]
    async fn test_calculate_retry_delay() {
        let (service, _temp_dir) = create_test_service().await;

        // Test exponential backoff
        assert_eq!(service.calculate_retry_delay(0), Duration::from_secs(2));
        assert_eq!(service.calculate_retry_delay(1), Duration::from_secs(4));
        assert_eq!(service.calculate_retry_delay(2), Duration::from_secs(8));
        assert_eq!(service.calculate_retry_delay(3), Duration::from_secs(16));

        // Test max delay cap
        assert_eq!(service.calculate_retry_delay(10), Duration::from_secs(60));
    }

    #[tokio::test]
    async fn test_process_empty_queue() {
        let (service, _temp_dir) = create_test_service().await;

        let result = service.process_queue(10).await.unwrap();
        assert_eq!(result.processed, 0);
        assert_eq!(result.succeeded, 0);
        assert_eq!(result.failed, 0);
    }
}
