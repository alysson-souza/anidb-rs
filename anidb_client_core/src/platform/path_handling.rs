//! Platform-specific path handling and validation
//!
//! Provides abstractions for handling file paths across different platforms,
//! normalizing path separators, validating permissions, and handling
//! platform-specific limitations like path length on Windows.

use crate::{
    Error, Result,
    error::{IoError, ValidationError},
};
use std::fs;
use std::path::{Path, PathBuf};

/// Information about a validated path
#[derive(Debug, Clone)]
pub struct PathInfo {
    pub exists: bool,
    pub is_file: bool,
    pub is_readable: bool,
    pub is_writable: bool,
    pub size: u64,
    pub unicode_safe: bool,
    pub normalized_path: PathBuf,
}

/// Result of path validation
pub type PathValidation = Result<PathInfo>;

/// Platform-aware path handler
#[derive(Debug, Clone)]
pub struct PlatformPathHandler {
    // Configuration can be added here in the future
}

impl PlatformPathHandler {
    /// Create a new path handler
    pub fn new() -> Self {
        Self {}
    }

    /// Normalize path separators for the current platform
    pub fn normalize_path<P: AsRef<Path>>(&self, path: P) -> PathBuf {
        let path = path.as_ref();

        // Convert to string to handle separator normalization
        let path_str = path.to_string_lossy();

        #[cfg(windows)]
        {
            // On Windows, normalize forward slashes to backslashes
            let normalized = path_str.replace('/', "\\");
            PathBuf::from(normalized)
        }

        #[cfg(not(windows))]
        {
            // On Unix systems, normalize backslashes to forward slashes
            let normalized = path_str.replace('\\', "/");
            PathBuf::from(normalized)
        }
    }

    /// Validate a path and return detailed information
    pub fn validate_path<P: AsRef<Path>>(&self, path: P) -> PathValidation {
        let path = path.as_ref();
        let normalized_path = self.normalize_path(path);

        // Check if path exists
        let exists = normalized_path.exists();

        if !exists {
            return Ok(PathInfo {
                exists: false,
                is_file: false,
                is_readable: false,
                is_writable: false,
                size: 0,
                unicode_safe: self.is_unicode_safe(&normalized_path),
                normalized_path,
            });
        }

        // Get metadata
        let metadata = match fs::metadata(&normalized_path) {
            Ok(m) => m,
            Err(e) => {
                return match e.kind() {
                    std::io::ErrorKind::PermissionDenied => {
                        Err(Error::Io(IoError::permission_denied(&normalized_path, e)))
                    }
                    _ => Err(Error::from(e)),
                };
            }
        };

        let is_file = metadata.is_file();
        let size = if is_file { metadata.len() } else { 0 };

        // Test readability
        let is_readable = self.test_readable(&normalized_path);

        // Test writability (check parent directory if file doesn't exist)
        let is_writable = if exists {
            self.test_writable(&normalized_path)
        } else if let Some(parent) = normalized_path.parent() {
            self.test_writable(parent)
        } else {
            false
        };

        Ok(PathInfo {
            exists,
            is_file,
            is_readable,
            is_writable,
            size,
            unicode_safe: self.is_unicode_safe(&normalized_path),
            normalized_path,
        })
    }

    /// Check if the platform can handle long paths
    pub fn can_handle_long_paths(&self) -> bool {
        #[cfg(windows)]
        {
            // Windows 10 version 1607 and later can handle long paths if enabled
            // For simplicity, we'll assume it's not enabled by default
            false
        }

        #[cfg(not(windows))]
        {
            // Unix systems typically handle long paths well
            true
        }
    }

    /// Validate path length for the current platform
    pub fn validate_path_length<P: AsRef<Path>>(&self, path: P) -> Result<()> {
        let path = path.as_ref();
        let path_str = path.to_string_lossy();

        #[cfg(windows)]
        {
            const MAX_PATH_WINDOWS: usize = 260;
            if path_str.len() > MAX_PATH_WINDOWS && !self.can_handle_long_paths() {
                return Err(Error::path_too_long(path, MAX_PATH_WINDOWS));
            }
        }

        #[cfg(not(windows))]
        {
            const MAX_PATH_UNIX: usize = 4096; // Typical Linux/macOS limit
            if path_str.len() > MAX_PATH_UNIX {
                return Err(Error::Validation(ValidationError::path_too_long(
                    path,
                    MAX_PATH_UNIX,
                )));
            }
        }

        Ok(())
    }

    /// Test if a path is readable
    fn test_readable<P: AsRef<Path>>(&self, path: P) -> bool {
        fs::File::open(path.as_ref()).is_ok()
    }

    /// Test if a path is writable
    fn test_writable<P: AsRef<Path>>(&self, path: P) -> bool {
        let path = path.as_ref();

        if path.is_file() {
            // Test by opening in append mode
            fs::OpenOptions::new().append(true).open(path).is_ok()
        } else if path.is_dir() {
            // Test by creating a temporary file
            let temp_path = path.join(".write_test_temp");
            match fs::File::create(&temp_path) {
                Ok(_) => {
                    let _ = fs::remove_file(temp_path);
                    true
                }
                Err(_) => false,
            }
        } else {
            false
        }
    }

    /// Check if a path contains only Unicode-safe characters
    fn is_unicode_safe<P: AsRef<Path>>(&self, path: P) -> bool {
        // If we can convert to string and back without loss, it's Unicode-safe
        let path = path.as_ref();
        let path_str = path.to_string_lossy();
        let reconstructed = PathBuf::from(path_str.as_ref());
        reconstructed == path
    }
}

impl Default for PlatformPathHandler {
    fn default() -> Self {
        Self::new()
    }
}
