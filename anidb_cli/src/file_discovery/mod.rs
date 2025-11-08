//! File discovery module for finding files based on glob patterns
//!
//! This module provides functionality to discover files in directories
//! based on include and exclude glob patterns, with support for default
//! media file extensions.

mod extensions;
mod filter;
mod walker;

#[allow(unused_imports)]
pub use walker::{FileDiscovery, FileDiscoveryOptions};

use std::path::PathBuf;

/// Result of file discovery
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct DiscoveredFile {
    /// Path to the discovered file
    pub path: PathBuf,
    /// Size of the file in bytes
    pub size: u64,
}

/// Error type for file discovery operations
#[derive(Debug, thiserror::Error)]
pub enum DiscoveryError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Invalid glob pattern: {0}")]
    InvalidPattern(String),

    #[error("Path not found: {0}")]
    PathNotFound(PathBuf),
}

/// Result type for file discovery operations
pub type Result<T> = std::result::Result<T, DiscoveryError>;
