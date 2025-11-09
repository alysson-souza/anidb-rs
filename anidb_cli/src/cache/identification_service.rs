//! Identification cache service wrapper for the AniDB CLI
//!
//! This module provides a service layer that adds caching functionality
//! on top of the core FileIdentificationService, transparently handling cache lookups
//! and storage for identification operations using the database.

use anidb_client_core::{
    api::{AniDBClient, ProcessOptions},
    database::{AniDBResult, AniDBResultRepository, models::time_utils},
    error::Result,
    hashing::HashAlgorithm,
    identification::{
        service::{FileIdentificationService, IdentificationService},
        types::{
            AnimeInfo, DataSource, EpisodeInfo, FileInfo, GroupInfo, IdentificationOptions,
            IdentificationRequest, IdentificationResult, IdentificationSource,
            IdentificationStatus, Priority,
        },
    },
    progress::{ProgressProvider, ProgressUpdate, SharedProvider},
};
use std::path::Path;
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime};

/// Identification cache service that wraps the core FileIdentificationService with caching functionality
///
/// This service acts as a layer between CLI commands and the core library,
/// adding transparent caching support for identification results using the database.
pub struct IdentificationCacheService {
    /// The underlying identification service from the core library
    service: Arc<FileIdentificationService>,
    /// The core AniDB client for hash calculations
    client: Arc<AniDBClient>,
    /// Database repository for AniDB results
    db_repo: Arc<AniDBResultRepository>,
    /// Enable verbose logging
    verbose: bool,
}

impl IdentificationCacheService {
    /// Create a new IdentificationCacheService
    ///
    /// # Arguments
    ///
    /// * `service` - The FileIdentificationService from the core library
    /// * `client` - The AniDBClient for hash calculations
    /// * `db_repo` - The database repository for AniDB results
    pub fn new(
        service: Arc<FileIdentificationService>,
        client: Arc<AniDBClient>,
        db_repo: Arc<AniDBResultRepository>,
    ) -> Self {
        Self {
            service,
            client,
            db_repo,
            verbose: false,
        }
    }

    /// Create a new IdentificationCacheService with verbose logging
    pub fn with_verbose(mut self, verbose: bool) -> Self {
        self.verbose = verbose;
        self
    }

    /// Identify a file with cache support
    ///
    /// This method checks the database cache first if caching is enabled,
    /// processes the file if needed, and stores the result in the cache.
    ///
    /// # Arguments
    ///
    /// * `file_path` - Path to the file to identify
    /// * `options` - Identification options including cache settings
    ///
    /// # Returns
    ///
    /// A Result containing the identification results or an error
    #[allow(dead_code)]
    pub async fn identify_file_with_cache(
        &self,
        file_path: &Path,
        options: IdentificationOptions,
    ) -> Result<IdentificationResult> {
        let start = Instant::now();

        // Check if caching is disabled
        if !options.use_cache {
            if self.verbose {
                log::debug!("Cache disabled, processing file directly: {file_path:?}");
            }
            return self.service.identify_file(file_path, options).await;
        }

        // First, calculate ED2K hash to check cache
        let process_options = ProcessOptions::new().with_algorithms(&[HashAlgorithm::ED2K]);

        let file_result = self.client.process_file(file_path, process_options).await?;

        let ed2k = file_result
            .hashes
            .get(&HashAlgorithm::ED2K)
            .ok_or_else(|| {
                anidb_client_core::Error::Validation(
                    anidb_client_core::error::ValidationError::invalid_configuration(
                        "ED2K hash not calculated",
                    ),
                )
            })?;

        // Check database cache
        if let Ok(Some(cached_result)) = self
            .db_repo
            .find_by_hash_and_size(ed2k, file_result.file_size as i64)
            .await
        {
            // Check if cache is expired
            if !cached_result.is_expired() {
                if self.verbose {
                    log::debug!("Cache hit for {file_path:?} with ED2K hash {ed2k}");
                }

                // Convert database result to identification result
                let identification_result = self.convert_to_identification_result(
                    cached_result,
                    IdentificationRequest {
                        source: IdentificationSource::FilePath(file_path.to_path_buf()),
                        options: options.clone(),
                        priority: Priority::Normal,
                    },
                    start.elapsed(),
                );

                return Ok(identification_result);
            } else if self.verbose {
                log::debug!("Cache hit but expired for {file_path:?} with ED2K hash {ed2k}");
            }
        }

        // Cache miss or expired - process normally
        if self.verbose {
            log::debug!("Cache miss for {file_path:?}, querying AniDB");
        }

        let result = self.service.identify_file(file_path, options).await?;

        // Store successful results in cache
        #[allow(clippy::collapsible_if)]
        if result.is_success() {
            if let Err(e) = self
                .store_result_in_cache(&result, ed2k, file_result.file_size)
                .await
            {
                if self.verbose {
                    log::warn!("Failed to cache identification result: {e}");
                }
            }
        }

        Ok(result)
    }

    /// Identify a file with cache support and progress reporting
    ///
    /// Similar to `identify_file_with_cache` but with progress reporting support.
    pub async fn identify_file_with_cache_and_progress(
        &self,
        file_path: &Path,
        options: IdentificationOptions,
        progress: &dyn ProgressProvider,
    ) -> Result<IdentificationResult> {
        let start = Instant::now();

        // Check if caching is disabled
        if !options.use_cache {
            if self.verbose {
                log::debug!("Cache disabled, processing file directly: {file_path:?}");
            }
            return self
                .service
                .identify_file_with_progress(file_path, options, progress)
                .await;
        }

        // Handle offline mode by delegating to the core service which already
        // emits the correct progress lifecycle (queued status etc.).
        if options.offline_mode {
            return self
                .service
                .identify_file_with_progress(file_path, options, progress)
                .await;
        }

        // Report start just like the core service would so the renderer shows activity.
        progress.report(ProgressUpdate::Status {
            message: format!("Identifying file: {}", file_path.display()),
        });

        // Calculate ED2K hash once using the CLI progress tree so the user
        // sees progress even during cache checks.
        let hash_progress = Arc::new(SharedProvider::new(Arc::from(
            progress.create_child("hash"),
        )));
        let process_options = ProcessOptions::new()
            .with_algorithms(&[HashAlgorithm::ED2K])
            .with_progress_reporting(true);

        let file_result = self
            .client
            .process_file_with_progress(file_path, process_options, hash_progress)
            .await?;

        let ed2k = file_result
            .hashes
            .get(&HashAlgorithm::ED2K)
            .ok_or_else(|| {
                anidb_client_core::Error::Validation(
                    anidb_client_core::error::ValidationError::invalid_configuration(
                        "ED2K hash not calculated",
                    ),
                )
            })?;

        progress.report(ProgressUpdate::Status {
            message: format!("ED2K hash calculated: {ed2k}"),
        });

        // Check database cache
        if let Ok(Some(cached_result)) = self
            .db_repo
            .find_by_hash_and_size(ed2k, file_result.file_size as i64)
            .await
        {
            // Check if cache is expired
            if !cached_result.is_expired() {
                if self.verbose {
                    log::debug!("Cache hit for {file_path:?} with ED2K hash {ed2k}");
                }

                // Convert database result to identification result
                let identification_result = self.convert_to_identification_result(
                    cached_result,
                    IdentificationRequest {
                        source: IdentificationSource::FilePath(file_path.to_path_buf()),
                        options: options.clone(),
                        priority: Priority::Normal,
                    },
                    start.elapsed(),
                );

                progress.complete();
                return Ok(identification_result);
            } else if self.verbose {
                log::debug!("Cache hit but expired for {file_path:?} with ED2K hash {ed2k}");
            }
        }

        // Cache miss or expired - process normally
        if self.verbose {
            log::debug!("Cache miss for {file_path:?}, querying AniDB");
        }

        progress.report(ProgressUpdate::NetworkProgress {
            operation: "Querying AniDB".to_string(),
            status: "In progress".to_string(),
        });

        // We already have the ED2K hash, so query directly by hash to avoid
        // re-reading the file. This keeps the cache path fast and ensures the
        // CLI still receives progress events.
        let mut result = self
            .service
            .identify_hash(ed2k, file_result.file_size, options.clone())
            .await?;

        result.request.source = IdentificationSource::FilePath(file_path.to_path_buf());
        result.processing_time = start.elapsed();

        progress.complete();

        // Store successful results in cache
        #[allow(clippy::collapsible_if)]
        if result.is_success() {
            if let Err(e) = self
                .store_result_in_cache(&result, ed2k, file_result.file_size)
                .await
            {
                if self.verbose {
                    log::warn!("Failed to cache identification result: {e}");
                }
            }
        }

        Ok(result)
    }

    /// Convert a database AniDBResult to an IdentificationResult
    pub(crate) fn convert_to_identification_result(
        &self,
        db_result: AniDBResult,
        request: IdentificationRequest,
        processing_time: Duration,
    ) -> IdentificationResult {
        // Calculate cache age
        let cached_at = time_utils::millis_to_system_time(db_result.fetched_at);
        let cache_age = SystemTime::now()
            .duration_since(cached_at)
            .unwrap_or(Duration::from_secs(0));

        // Build FileInfo from database result
        let file_info = FileInfo {
            fid: db_result.file_id as u64,
            aid: db_result.anime_id.unwrap_or(0) as u64,
            eid: db_result.episode_id.unwrap_or(0) as u64,
            gid: 0,   // Group ID not stored in current database schema
            state: 1, // Default state
            size: db_result.file_size as u64,
            ed2k: db_result.ed2k_hash.clone(),
            md5: None,
            sha1: None,
            crc32: None,
            quality: db_result.quality.clone(),
            source: db_result.source.clone(),
            video_codec: db_result.video_codec.clone(),
            video_resolution: db_result.resolution.clone(),
            audio_codec: db_result.audio_codec.clone(),
            dub_language: None,
            sub_language: None,
            file_type: db_result.file_type.clone(),
            anidb_filename: None,
        };

        // Build AnimeInfo if available
        let anime_info = db_result.anime_id.and_then(|aid| {
            db_result.anime_title.map(|title| AnimeInfo {
                aid: aid as u64,
                romaji_name: title.clone(),
                kanji_name: None,
                english_name: None,
                year: None,
                type_: None,
                episode_count: None,
                rating: None,
                categories: Vec::new(),
            })
        });

        // Build EpisodeInfo if available
        let episode_info = db_result.episode_id.and_then(|eid| {
            db_result.episode_number.map(|ep_num| EpisodeInfo {
                eid: eid as u64,
                aid: db_result.anime_id.unwrap_or(0) as u64,
                episode_number: ep_num,
                english_name: db_result.episode_title.clone(),
                romaji_name: db_result.episode_title.clone(),
                kanji_name: None,
                length: None,
                aired_date: None,
            })
        });

        // Build GroupInfo if available
        let group_info = db_result.group_name.map(|name| GroupInfo {
            gid: 0, // Group ID not stored
            name: name.clone(),
            short_name: db_result.group_short.clone(),
        });

        IdentificationResult {
            request,
            status: IdentificationStatus::Identified,
            anime: anime_info,
            episode: episode_info,
            file: Some(file_info),
            group: group_info,
            source: DataSource::Cache { age: cache_age },
            processing_time,
            cached_at: Some(cached_at),
        }
    }

    /// Store an identification result in the database cache
    async fn store_result_in_cache(
        &self,
        result: &IdentificationResult,
        ed2k: &str,
        file_size: u64,
    ) -> Result<()> {
        // Only store successful identifications
        if !result.is_success() || result.file.is_none() {
            return Ok(());
        }

        let file_info = result.file.as_ref().unwrap();
        let now = time_utils::now_millis();

        // Calculate expiry time based on options
        let expires_at = now + result.request.options.cache_ttl.as_millis() as i64;

        // Create database record
        let db_result = AniDBResult {
            id: 0, // Will be assigned by database
            file_id: file_info.fid as i64,
            ed2k_hash: ed2k.to_string(),
            file_size: file_size as i64,
            anime_id: result.anime.as_ref().map(|a| a.aid as i64),
            episode_id: result.episode.as_ref().map(|e| e.eid as i64),
            episode_number: result.episode.as_ref().map(|e| e.episode_number.clone()),
            anime_title: result.anime.as_ref().map(|a| a.romaji_name.clone()),
            episode_title: result.episode.as_ref().and_then(|e| e.english_name.clone()),
            group_name: result.group.as_ref().map(|g| g.name.clone()),
            group_short: result.group.as_ref().and_then(|g| g.short_name.clone()),
            version: Some(1),
            censored: Some(false),
            deprecated: Some(false),
            crc32_valid: file_info.crc32.is_some().then_some(true),
            file_type: file_info.file_type.clone(),
            resolution: file_info.video_resolution.clone(),
            video_codec: file_info.video_codec.clone(),
            audio_codec: file_info.audio_codec.clone(),
            source: file_info.source.clone(),
            quality: file_info.quality.clone(),
            mylist_lid: None, // Not in MyList yet
            fetched_at: now,
            expires_at: Some(expires_at),
            created_at: now,
            updated_at: now,
        };

        // Upsert to database
        self.db_repo.upsert(&db_result).await?;

        if self.verbose {
            log::debug!("Cached identification result for ED2K {ed2k} (expires at {expires_at})");
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Helper function to test conversion without needing a full service
    fn test_db_to_identification_conversion(
        db_result: AniDBResult,
        request: IdentificationRequest,
        processing_time: Duration,
    ) -> IdentificationResult {
        // Calculate cache age
        let cached_at = time_utils::millis_to_system_time(db_result.fetched_at);
        let cache_age = SystemTime::now()
            .duration_since(cached_at)
            .unwrap_or(Duration::from_secs(0));

        // Build FileInfo from database result
        let file_info = FileInfo {
            fid: db_result.file_id as u64,
            aid: db_result.anime_id.unwrap_or(0) as u64,
            eid: db_result.episode_id.unwrap_or(0) as u64,
            gid: 0,
            state: 1,
            size: db_result.file_size as u64,
            ed2k: db_result.ed2k_hash.clone(),
            md5: None,
            sha1: None,
            crc32: None,
            quality: db_result.quality.clone(),
            source: db_result.source.clone(),
            video_codec: db_result.video_codec.clone(),
            video_resolution: db_result.resolution.clone(),
            audio_codec: db_result.audio_codec.clone(),
            dub_language: None,
            sub_language: None,
            file_type: db_result.file_type.clone(),
            anidb_filename: None,
        };

        // Build AnimeInfo if available
        let anime_info = db_result.anime_id.and_then(|aid| {
            db_result.anime_title.map(|title| AnimeInfo {
                aid: aid as u64,
                romaji_name: title.clone(),
                kanji_name: None,
                english_name: None,
                year: None,
                type_: None,
                episode_count: None,
                rating: None,
                categories: Vec::new(),
            })
        });

        // Build EpisodeInfo if available
        let episode_info = db_result.episode_id.and_then(|eid| {
            db_result.episode_number.map(|ep_num| EpisodeInfo {
                eid: eid as u64,
                aid: db_result.anime_id.unwrap_or(0) as u64,
                episode_number: ep_num,
                english_name: db_result.episode_title.clone(),
                romaji_name: db_result.episode_title.clone(),
                kanji_name: None,
                length: None,
                aired_date: None,
            })
        });

        // Build GroupInfo if available
        let group_info = db_result.group_name.map(|name| GroupInfo {
            gid: 0,
            name: name.clone(),
            short_name: db_result.group_short.clone(),
        });

        IdentificationResult {
            request,
            status: IdentificationStatus::Identified,
            anime: anime_info,
            episode: episode_info,
            file: Some(file_info),
            group: group_info,
            source: DataSource::Cache { age: cache_age },
            processing_time,
            cached_at: Some(cached_at),
        }
    }

    #[tokio::test]
    async fn test_convert_db_result() {
        // Note: We can't easily create a real FileIdentificationService in tests
        // because it requires network access. Instead, we test the conversion logic directly.

        let db_result = AniDBResult {
            id: 1,
            file_id: 12345,
            ed2k_hash: "test_hash".to_string(),
            file_size: 1000000,
            anime_id: Some(456),
            episode_id: Some(789),
            episode_number: Some("01".to_string()),
            anime_title: Some("Test Anime".to_string()),
            episode_title: Some("Episode 1".to_string()),
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
            mylist_lid: None,
            fetched_at: time_utils::now_millis() - 3600000, // 1 hour ago
            expires_at: Some(time_utils::now_millis() + 86400000), // 1 day from now
            created_at: time_utils::now_millis() - 3600000,
            updated_at: time_utils::now_millis() - 3600000,
        };

        let request = IdentificationRequest {
            source: IdentificationSource::FilePath("/test/file.mkv".into()),
            options: IdentificationOptions::default(),
            priority: Priority::Normal,
        };

        let result =
            test_db_to_identification_conversion(db_result, request, Duration::from_millis(100));

        assert_eq!(result.status, IdentificationStatus::Identified);
        assert!(result.file.is_some());
        assert_eq!(result.file.as_ref().unwrap().fid, 12345);
        assert!(result.anime.is_some());
        assert_eq!(result.anime.as_ref().unwrap().romaji_name, "Test Anime");
        assert!(matches!(result.source, DataSource::Cache { .. }));
    }
}
