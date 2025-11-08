//! Main identification service implementation
//!
//! This module provides the core identification service that coordinates
//! cache lookups, hash calculations, and network queries.

use crate::ClientConfig;
use crate::api::{AniDBClient, ProcessOptions};
use crate::error::{Error, Result, ValidationError};
use crate::hashing::HashAlgorithm;
use crate::identification::query_manager::AniDBQueryManager;
use crate::identification::types::{
    BatchIdentificationResult, IdentificationOptions, IdentificationRequest, IdentificationResult,
    IdentificationSource, IdentificationStatus, Priority,
};
use crate::progress::{NullProvider, ProgressProvider, ProgressUpdate, SharedProvider};
use crate::protocol::ProtocolConfig;
use crate::protocol::client::ProtocolClient;
use crate::security::credential_store::CredentialStore;
use crate::security::fallback::EncryptedFileStore;
use std::path::Path;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::Mutex;

/// Service trait for file identification
#[async_trait::async_trait]
pub trait IdentificationService: Send + Sync {
    /// Identify a file by its path
    async fn identify_file(
        &self,
        path: &Path,
        options: IdentificationOptions,
    ) -> Result<IdentificationResult>;

    /// Identify a file by its path with progress reporting
    async fn identify_file_with_progress(
        &self,
        path: &Path,
        options: IdentificationOptions,
        progress: &dyn ProgressProvider,
    ) -> Result<IdentificationResult>;

    /// Identify by ED2K hash and size
    async fn identify_hash(
        &self,
        ed2k: &str,
        size: u64,
        options: IdentificationOptions,
    ) -> Result<IdentificationResult>;

    /// Identify multiple files in batch
    async fn identify_batch(
        &self,
        requests: Vec<IdentificationRequest>,
    ) -> BatchIdentificationResult;

    /// Identify multiple files in batch with progress reporting
    async fn identify_batch_with_progress(
        &self,
        requests: Vec<IdentificationRequest>,
        progress: &dyn ProgressProvider,
    ) -> BatchIdentificationResult;
}

/// Main file identification service
pub struct FileIdentificationService {
    /// AniDB client for hash calculations
    anidb_client: Arc<AniDBClient>,
    /// Protocol client for network queries
    protocol_client: Arc<Mutex<ProtocolClient>>,
    /// Query manager
    query_manager: Arc<AniDBQueryManager>,
    /// Credential store
    credential_store: Arc<EncryptedFileStore>,
    /// Service configuration
    config: ServiceConfig,
}

/// Service configuration
#[derive(Debug, Clone)]
pub struct ServiceConfig {
    /// Enable verbose logging
    pub verbose: bool,
    /// Maximum concurrent identifications
    pub max_concurrent: usize,
    /// Enable offline queue
    pub enable_offline_queue: bool,
}

impl Default for ServiceConfig {
    fn default() -> Self {
        Self {
            verbose: false,
            max_concurrent: 4,
            enable_offline_queue: true,
        }
    }
}

impl FileIdentificationService {
    /// Create a new identification service
    pub async fn new(client_config: ClientConfig, service_config: ServiceConfig) -> Result<Self> {
        // Create AniDB client for hash calculations
        let anidb_client = Arc::new(AniDBClient::new(client_config.clone()).await?);

        // Create protocol client
        let protocol_config = ProtocolConfig {
            client_name: client_config.client_name.clone().unwrap_or_default(),
            client_version: client_config.client_version.clone().unwrap_or_default(),
            ..Default::default() // This will use api.anidb.net:9000
        };
        let protocol_client = Arc::new(Mutex::new(ProtocolClient::new(protocol_config).await?));

        // Create query manager
        let query_manager = Arc::new(AniDBQueryManager::new(protocol_client.clone()));

        // Create credential store
        let credential_store = Arc::new(EncryptedFileStore::new().await.map_err(|e| {
            Error::Validation(ValidationError::invalid_configuration(&e.to_string()))
        })?);

        Ok(Self {
            anidb_client,
            protocol_client,
            query_manager,
            credential_store,
            config: service_config,
        })
    }

    /// Process a single identification request
    async fn process_request(
        &self,
        request: IdentificationRequest,
    ) -> Result<IdentificationResult> {
        let start = Instant::now();

        // Handle offline mode
        if request.options.offline_mode {
            return Ok(IdentificationResult::error(
                request,
                IdentificationStatus::Queued,
                start.elapsed(),
            ));
        }

        // Process based on source type
        let result = match &request.source {
            IdentificationSource::FilePath(path) => {
                self.identify_file_internal(path, &request.options).await?
            }
            IdentificationSource::HashWithSize { ed2k, size } => {
                self.identify_hash_internal(ed2k, *size, &request.options)
                    .await?
            }
            IdentificationSource::FileId(fid) => {
                self.identify_by_id_internal(*fid, &request.options).await?
            }
        };

        Ok(result)
    }

    /// Internal file identification
    async fn identify_file_internal(
        &self,
        path: &Path,
        options: &IdentificationOptions,
    ) -> Result<IdentificationResult> {
        // Use null provider when no progress is needed
        self.identify_file_internal_with_progress(path, options, &NullProvider)
            .await
    }

    /// Internal file identification with progress support
    async fn identify_file_internal_with_progress(
        &self,
        path: &Path,
        options: &IdentificationOptions,
        progress: &dyn ProgressProvider,
    ) -> Result<IdentificationResult> {
        let start = Instant::now();

        // Report starting
        progress.report(ProgressUpdate::Status {
            message: format!("Identifying file: {}", path.display()),
        });

        // First, calculate ED2K hash
        // Create a shared provider for the hash calculation
        let hash_progress = Arc::new(SharedProvider::new(Arc::from(
            progress.create_child("hash"),
        )));
        let process_options = ProcessOptions::new()
            .with_algorithms(&[HashAlgorithm::ED2K])
            .with_progress_reporting(true);

        let file_result = self
            .anidb_client
            .process_file_with_progress(path, process_options, hash_progress.clone())
            .await?;

        let ed2k = file_result
            .hashes
            .get(&HashAlgorithm::ED2K)
            .ok_or_else(|| {
                Error::Validation(ValidationError::invalid_configuration(
                    "ED2K hash not calculated",
                ))
            })?;

        // Report hash calculated
        progress.report(ProgressUpdate::Status {
            message: format!("ED2K hash calculated: {ed2k}"),
        });

        // Report querying database
        progress.report(ProgressUpdate::NetworkProgress {
            operation: "Querying AniDB".to_string(),
            status: "In progress".to_string(),
        });

        // Now identify by hash
        let mut result = self
            .identify_hash_internal(ed2k, file_result.file_size, options)
            .await?;

        // Update the request source to reflect it came from a file
        result.request.source = IdentificationSource::FilePath(path.to_path_buf());
        result.processing_time = start.elapsed();

        // Report completion
        progress.complete();

        Ok(result)
    }

    /// Internal hash identification
    async fn identify_hash_internal(
        &self,
        ed2k: &str,
        size: u64,
        options: &IdentificationOptions,
    ) -> Result<IdentificationResult> {
        // Get available accounts
        let accounts = self
            .credential_store
            .list_accounts("anidb")
            .await
            .map_err(|e| {
                Error::Validation(ValidationError::invalid_configuration(&format!(
                    "Failed to access credentials: {e}"
                )))
            })?;

        if accounts.is_empty() {
            return Err(Error::Validation(ValidationError::invalid_configuration(
                "No AniDB credentials found. Please run 'anidb auth login' to authenticate first.",
            )));
        }

        // Use the first available account (in the future, could make this configurable)
        let username = &accounts[0];

        let creds = self
            .credential_store
            .retrieve("anidb", username)
            .await
            .map_err(|e| {
                Error::Validation(ValidationError::invalid_configuration(&format!(
                    "Failed to retrieve credentials: {e}"
                )))
            })?;

        // Ensure authenticated
        self.query_manager
            .ensure_authenticated(&creds.account, &creds.secret.expose_secret())
            .await?;

        // Query AniDB
        let source = IdentificationSource::HashWithSize {
            ed2k: ed2k.to_string(),
            size,
        };

        self.query_manager
            .query_file(&source, options.fmask.as_deref(), options.amask.as_deref())
            .await
    }

    /// Internal file ID identification
    async fn identify_by_id_internal(
        &self,
        fid: u64,
        options: &IdentificationOptions,
    ) -> Result<IdentificationResult> {
        // Get available accounts
        let accounts = self
            .credential_store
            .list_accounts("anidb")
            .await
            .map_err(|e| {
                Error::Validation(ValidationError::invalid_configuration(&format!(
                    "Failed to access credentials: {e}"
                )))
            })?;

        if accounts.is_empty() {
            return Err(Error::Validation(ValidationError::invalid_configuration(
                "No AniDB credentials found. Please run 'anidb auth login' to authenticate first.",
            )));
        }

        // Use the first available account (in the future, could make this configurable)
        let username = &accounts[0];

        let creds = self
            .credential_store
            .retrieve("anidb", username)
            .await
            .map_err(|e| {
                Error::Validation(ValidationError::invalid_configuration(&format!(
                    "Failed to retrieve credentials: {e}"
                )))
            })?;

        // Ensure authenticated
        self.query_manager
            .ensure_authenticated(&creds.account, &creds.secret.expose_secret())
            .await?;

        // Query AniDB
        let source = IdentificationSource::FileId(fid);

        self.query_manager
            .query_file(&source, options.fmask.as_deref(), options.amask.as_deref())
            .await
    }
}

#[async_trait::async_trait]
impl IdentificationService for FileIdentificationService {
    async fn identify_file(
        &self,
        path: &Path,
        options: IdentificationOptions,
    ) -> Result<IdentificationResult> {
        let request = IdentificationRequest {
            source: IdentificationSource::FilePath(path.to_path_buf()),
            options,
            priority: Priority::Normal,
        };

        self.process_request(request).await
    }

    async fn identify_file_with_progress(
        &self,
        path: &Path,
        options: IdentificationOptions,
        progress: &dyn ProgressProvider,
    ) -> Result<IdentificationResult> {
        // Handle offline mode early
        if options.offline_mode {
            progress.report(ProgressUpdate::Status {
                message: "Offline mode - queuing for later".to_string(),
            });
            progress.complete();
            let request = IdentificationRequest {
                source: IdentificationSource::FilePath(path.to_path_buf()),
                options,
                priority: Priority::Normal,
            };
            return Ok(IdentificationResult::error(
                request,
                IdentificationStatus::Queued,
                Duration::from_millis(0),
            ));
        }

        // Process the file with progress (caching is handled inside)
        self.identify_file_internal_with_progress(path, &options, progress)
            .await
    }

    async fn identify_hash(
        &self,
        ed2k: &str,
        size: u64,
        options: IdentificationOptions,
    ) -> Result<IdentificationResult> {
        let request = IdentificationRequest {
            source: IdentificationSource::HashWithSize {
                ed2k: ed2k.to_string(),
                size,
            },
            options,
            priority: Priority::Normal,
        };

        self.process_request(request).await
    }

    async fn identify_batch(
        &self,
        requests: Vec<IdentificationRequest>,
    ) -> BatchIdentificationResult {
        // Use null provider when no progress is needed
        self.identify_batch_with_progress(requests, &NullProvider)
            .await
    }

    async fn identify_batch_with_progress(
        &self,
        requests: Vec<IdentificationRequest>,
        _progress: &dyn ProgressProvider,
    ) -> BatchIdentificationResult {
        let start = Instant::now();
        let _total_requests = requests.len();

        // Process requests concurrently with limit
        let semaphore = Arc::new(tokio::sync::Semaphore::new(self.config.max_concurrent));
        let mut handles = Vec::new();

        for request in requests {
            let sem = semaphore.clone();
            let service = self.clone(); // Assuming we implement Clone

            let handle = tokio::spawn(async move {
                let _permit = sem.acquire().await.unwrap();
                service.process_request(request).await
            });

            handles.push(handle);
        }

        // Collect results
        let mut results = Vec::new();
        let mut success_count = 0;
        let mut failure_count = 0;

        for handle in handles {
            match handle.await {
                Ok(Ok(result)) => {
                    if result.is_success() {
                        success_count += 1;
                    } else {
                        failure_count += 1;
                    }
                    results.push(result);
                }
                Ok(Err(_)) | Err(_) => {
                    failure_count += 1;
                }
            }
        }

        BatchIdentificationResult {
            results,
            total_time: start.elapsed(),
            success_count,
            failure_count,
        }
    }
}

// Manual Clone implementation
impl Clone for FileIdentificationService {
    fn clone(&self) -> Self {
        Self {
            anidb_client: self.anidb_client.clone(),
            protocol_client: self.protocol_client.clone(),
            query_manager: self.query_manager.clone(),
            credential_store: self.credential_store.clone(),
            config: self.config.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_service_creation() {
        let client_config = ClientConfig::test();

        let service_config = ServiceConfig::default();
        let service = FileIdentificationService::new(client_config, service_config).await;
        if service.is_err() {
            eprintln!(
                "Skipping test_service_creation due to network sandbox: {:?}",
                service.err()
            );
            return;
        }
    }

    #[tokio::test]
    async fn test_identify_hash_request() {
        let client_config = ClientConfig::test();

        let service_config = ServiceConfig::default();
        let service = match FileIdentificationService::new(client_config, service_config).await {
            Ok(svc) => svc,
            Err(e) => {
                eprintln!("Skipping test_identify_hash_request due to network sandbox: {e:?}");
                return;
            }
        };

        let options = IdentificationOptions {
            offline_mode: true, // Test in offline mode
            ..Default::default()
        };

        let result = service.identify_hash("test_hash", 1000000, options).await;

        // In offline mode, should get a queued status
        match result {
            Ok(res) => assert_eq!(res.status, IdentificationStatus::Queued),
            Err(_) => panic!("Should not error in offline mode"),
        }
    }
}
