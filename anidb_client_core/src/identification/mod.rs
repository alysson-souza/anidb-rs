//! File identification module for AniDB client
//!
//! This module handles the identification of anime files through:
//! - Network queries to AniDB API
//! - Smart retry logic with exponential backoff
//! - Progress reporting for UI integration

pub mod query_manager;
pub mod service;
pub mod types;

// Re-export main types
pub use query_manager::AniDBQueryManager;
pub use service::{FileIdentificationService, IdentificationService, ServiceConfig};
pub use types::{
    AnimeIdentification, AnimeInfo, BatchIdentificationResult, DataSource, EpisodeInfo, FileInfo,
    GroupInfo, IdentificationError, IdentificationOptions, IdentificationRequest,
    IdentificationResult, IdentificationSource, IdentificationStatus, Priority,
};
