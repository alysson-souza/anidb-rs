//! Data types and DTOs for identification service
//!
//! This module defines the core types used throughout the identification system.

use crate::error::{Error, InternalError, IoError, ProtocolError, ValidationError};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::time::{Duration, SystemTime};

/// Source of identification request
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum IdentificationSource {
    /// Identify by file path (will calculate hash if needed)
    FilePath(PathBuf),
    /// Identify by ED2K hash and file size
    HashWithSize { ed2k: String, size: u64 },
    /// Identify by AniDB file ID
    FileId(u64),
}

/// Priority level for identification requests
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, Default)]
pub enum Priority {
    Low,
    #[default]
    Normal,
    High,
}

/// Identification request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IdentificationRequest {
    pub source: IdentificationSource,
    pub options: IdentificationOptions,
    pub priority: Priority,
}

/// Options for identification
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IdentificationOptions {
    /// Use cache for lookups
    pub use_cache: bool,
    /// Cache time-to-live
    pub cache_ttl: Duration,
    /// Network timeout
    pub timeout: Duration,
    /// Maximum retry attempts
    pub max_retries: u32,
    /// Work in offline mode (cache only)
    pub offline_mode: bool,
    /// Include metadata in results
    pub include_metadata: bool,
    /// Include MyList information
    pub include_mylist: bool,
    /// Field masks for API queries
    pub fmask: Option<String>,
    pub amask: Option<String>,
}

impl Default for IdentificationOptions {
    fn default() -> Self {
        Self {
            use_cache: true,
            cache_ttl: Duration::from_secs(86400 * 30), // 30 days
            timeout: Duration::from_secs(30),
            max_retries: 3,
            offline_mode: false,
            include_metadata: true,
            include_mylist: false,
            fmask: Some("78C8FEF8".to_string()), // Basic fields
            amask: Some("00E03000".to_string()), // All anime and episode names
        }
    }
}

/// Source of identification data
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum DataSource {
    /// Data from cache
    Cache { age: Duration },
    /// Data from network
    Network { response_time: Duration },
    /// Offline placeholder
    Offline,
}

/// Status of identification
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum IdentificationStatus {
    /// Successfully identified
    Identified,
    /// File not found in database
    NotFound,
    /// Network error occurred
    NetworkError,
    /// Queued for later processing
    Queued,
    /// Cache hit but expired
    Expired,
}

/// Anime information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnimeInfo {
    pub aid: u64,
    pub romaji_name: String,
    pub kanji_name: Option<String>,
    pub english_name: Option<String>,
    pub year: Option<u16>,
    pub type_: Option<String>,
    pub episode_count: Option<u32>,
    pub rating: Option<f32>,
    pub categories: Vec<String>,
}

/// Episode information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EpisodeInfo {
    pub eid: u64,
    pub aid: u64,
    pub episode_number: String,
    pub english_name: Option<String>,
    pub romaji_name: Option<String>,
    pub kanji_name: Option<String>,
    pub length: Option<u32>,
    pub aired_date: Option<SystemTime>,
}

/// File information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileInfo {
    pub fid: u64,
    pub aid: u64,
    pub eid: u64,
    pub gid: u64,
    pub state: u32,
    pub size: u64,
    pub ed2k: String,
    pub md5: Option<String>,
    pub sha1: Option<String>,
    pub crc32: Option<String>,
    pub quality: Option<String>,
    pub source: Option<String>,
    pub video_codec: Option<String>,
    pub video_resolution: Option<String>,
    pub audio_codec: Option<String>,
    pub dub_language: Option<String>,
    pub sub_language: Option<String>,
    pub file_type: Option<String>,
    pub anidb_filename: Option<String>,
}

/// Group information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GroupInfo {
    pub gid: u64,
    pub name: String,
    pub short_name: Option<String>,
}

/// Complete identification result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IdentificationResult {
    pub request: IdentificationRequest,
    pub status: IdentificationStatus,
    pub anime: Option<AnimeInfo>,
    pub episode: Option<EpisodeInfo>,
    pub file: Option<FileInfo>,
    pub group: Option<GroupInfo>,
    pub source: DataSource,
    pub processing_time: Duration,
    pub cached_at: Option<SystemTime>,
}

impl IdentificationResult {
    /// Create a successful identification result
    pub fn success(
        request: IdentificationRequest,
        file: FileInfo,
        source: DataSource,
        processing_time: Duration,
    ) -> Self {
        Self {
            request,
            status: IdentificationStatus::Identified,
            anime: None,
            episode: None,
            file: Some(file),
            group: None,
            source,
            processing_time,
            cached_at: None,
        }
    }

    /// Create a not found result
    pub fn not_found(request: IdentificationRequest, processing_time: Duration) -> Self {
        Self {
            request,
            status: IdentificationStatus::NotFound,
            anime: None,
            episode: None,
            file: None,
            group: None,
            source: DataSource::Network {
                response_time: processing_time,
            },
            processing_time,
            cached_at: None,
        }
    }

    /// Create an error result
    pub fn error(
        request: IdentificationRequest,
        status: IdentificationStatus,
        processing_time: Duration,
    ) -> Self {
        Self {
            request,
            status,
            anime: None,
            episode: None,
            file: None,
            group: None,
            source: DataSource::Offline,
            processing_time,
            cached_at: None,
        }
    }

    /// Check if identification was successful
    pub fn is_success(&self) -> bool {
        self.status == IdentificationStatus::Identified
    }

    /// Get the file ID if available
    pub fn file_id(&self) -> Option<u64> {
        self.file.as_ref().map(|f| f.fid)
    }

    /// Get the anime ID if available
    pub fn anime_id(&self) -> Option<u64> {
        self.file.as_ref().map(|f| f.aid)
    }
}

/// Batch identification result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatchIdentificationResult {
    pub results: Vec<IdentificationResult>,
    pub total_time: Duration,
    pub success_count: usize,
    pub failure_count: usize,
}

/// Anime identification type
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnimeIdentification {
    pub anime_id: Option<u64>,
    pub episode_id: Option<u64>,
    pub file_id: Option<u64>,
    pub group_id: Option<u64>,
    pub anime_title: Option<String>,
    pub episode_number: Option<String>,
    pub episode_title: Option<String>,
    pub group_name: Option<String>,
}

impl From<&IdentificationResult> for AnimeIdentification {
    fn from(result: &IdentificationResult) -> Self {
        Self {
            anime_id: result.anime.as_ref().map(|a| a.aid),
            episode_id: result.episode.as_ref().map(|e| e.eid),
            file_id: result.file.as_ref().map(|f| f.fid),
            group_id: result.file.as_ref().map(|f| f.gid),
            anime_title: result.anime.as_ref().map(|a| a.romaji_name.clone()),
            episode_number: result.episode.as_ref().map(|e| e.episode_number.clone()),
            episode_title: result.episode.as_ref().and_then(|e| e.romaji_name.clone()),
            group_name: result.group.as_ref().map(|g| g.name.clone()),
        }
    }
}

/// Identification error types
#[derive(Debug, Clone, thiserror::Error)]
pub enum IdentificationError {
    /// Transient errors (can retry)
    #[error("Network timeout after {duration:?}")]
    NetworkTimeout { duration: Duration },

    #[error("Rate limit exceeded, retry after {retry_after:?}")]
    RateLimitExceeded { retry_after: Duration },

    #[error("Server busy: {message}")]
    ServerBusy { message: String },

    /// Permanent errors
    #[error("File not found: {path}")]
    FileNotFound { path: PathBuf },

    #[error("Invalid hash: {hash}")]
    InvalidHash { hash: String },

    #[error("Unauthorized")]
    Unauthorized,

    /// Cache errors
    #[error("Cache corrupted")]
    CacheCorrupted,

    #[error("Cache unavailable")]
    CacheUnavailable,

    /// Other errors
    #[error("Protocol error: {0}")]
    Protocol(String),

    #[error("IO error: {0}")]
    Io(String),
}

impl From<IdentificationError> for Error {
    fn from(err: IdentificationError) -> Self {
        match err {
            IdentificationError::NetworkTimeout { .. }
            | IdentificationError::RateLimitExceeded { .. }
            | IdentificationError::ServerBusy { .. } => {
                Error::Protocol(ProtocolError::NetworkOffline)
            }
            IdentificationError::FileNotFound { ref path } => {
                Error::Io(IoError::file_not_found(path))
            }
            IdentificationError::InvalidHash { .. } => {
                Error::Validation(ValidationError::invalid_configuration(&err.to_string()))
            }
            IdentificationError::Unauthorized => {
                Error::Validation(ValidationError::invalid_configuration(&err.to_string()))
            }
            IdentificationError::CacheCorrupted | IdentificationError::CacheUnavailable => {
                Error::Internal(InternalError::assertion(err.to_string()))
            }
            IdentificationError::Protocol(ref msg) => {
                Error::Protocol(ProtocolError::server_error(500, msg))
            }
            IdentificationError::Io(ref msg) => {
                Error::Io(IoError::from_std(std::io::Error::other(msg.clone())))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_identification_source() {
        let file_source = IdentificationSource::FilePath(PathBuf::from("/test/file.mkv"));
        let hash_source = IdentificationSource::HashWithSize {
            ed2k: "abc123".to_string(),
            size: 1000000,
        };
        let id_source = IdentificationSource::FileId(12345);

        assert_ne!(file_source, hash_source);
        assert_ne!(hash_source, id_source);
    }

    #[test]
    fn test_identification_options_default() {
        let options = IdentificationOptions::default();
        assert!(options.use_cache);
        assert!(!options.offline_mode);
        assert_eq!(options.max_retries, 3);
    }

    #[test]
    fn test_identification_result_success() {
        let request = IdentificationRequest {
            source: IdentificationSource::FileId(123),
            options: IdentificationOptions::default(),
            priority: Priority::Normal,
        };

        let file_info = FileInfo {
            fid: 123,
            aid: 456,
            eid: 789,
            gid: 42,
            state: 1,
            size: 1000000,
            ed2k: "test_hash".to_string(),
            md5: None,
            sha1: None,
            crc32: None,
            quality: Some("HD".to_string()),
            source: Some("BluRay".to_string()),
            video_codec: Some("H.264".to_string()),
            video_resolution: Some("1920x1080".to_string()),
            audio_codec: Some("FLAC".to_string()),
            dub_language: Some("Japanese".to_string()),
            sub_language: Some("English".to_string()),
            file_type: Some("mkv".to_string()),
            anidb_filename: None,
        };

        let result = IdentificationResult::success(
            request,
            file_info,
            DataSource::Cache {
                age: Duration::from_secs(3600),
            },
            Duration::from_millis(50),
        );

        assert!(result.is_success());
        assert_eq!(result.file_id(), Some(123));
        assert_eq!(result.anime_id(), Some(456));
    }

    #[test]
    fn test_anime_identification_conversion() {
        let mut result = IdentificationResult::not_found(
            IdentificationRequest {
                source: IdentificationSource::FileId(123),
                options: IdentificationOptions::default(),
                priority: Priority::Normal,
            },
            Duration::from_millis(100),
        );

        result.anime = Some(AnimeInfo {
            aid: 456,
            romaji_name: "Test Anime".to_string(),
            kanji_name: Some("テストアニメ".to_string()),
            english_name: Some("Test Anime".to_string()),
            year: Some(2024),
            type_: Some("TV".to_string()),
            episode_count: Some(12),
            rating: Some(8.5),
            categories: vec!["Action".to_string(), "Sci-Fi".to_string()],
        });

        result.episode = Some(EpisodeInfo {
            eid: 789,
            aid: 456,
            episode_number: "01".to_string(),
            english_name: Some("Episode 1".to_string()),
            romaji_name: Some("Episode 1".to_string()),
            kanji_name: Some("エピソード1".to_string()),
            length: Some(1440),
            aired_date: None,
        });

        let anime_id: AnimeIdentification = (&result).into();
        assert_eq!(anime_id.anime_id, Some(456));
        assert_eq!(anime_id.anime_title, Some("Test Anime".to_string()));
        assert_eq!(anime_id.episode_number, Some("01".to_string()));
    }

    #[test]
    fn test_identification_error_conversion() {
        use crate::error::{ProtocolError, ValidationError};

        let timeout_err = IdentificationError::NetworkTimeout {
            duration: Duration::from_secs(30),
        };
        let error: Error = timeout_err.into();
        assert!(matches!(
            error,
            Error::Protocol(ProtocolError::NetworkOffline)
        ));

        let auth_err = IdentificationError::Unauthorized;
        let error: Error = auth_err.into();
        assert!(matches!(
            error,
            Error::Validation(ValidationError::InvalidConfiguration { .. })
        ));
    }
}
