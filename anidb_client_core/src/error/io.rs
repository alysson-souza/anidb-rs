//! I/O related error types

use std::path::PathBuf;
use thiserror::Error;

/// I/O error with additional context
#[derive(Error, Debug)]
#[error("{}", format_io_error(self))]
pub struct IoError {
    /// The kind of I/O error
    pub kind: IoErrorKind,
    /// Path associated with the error (if any)
    pub path: Option<PathBuf>,
    /// Underlying I/O error (if any)
    #[source]
    pub source: Option<std::io::Error>,
}

/// Kind of I/O error
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IoErrorKind {
    /// File not found
    FileNotFound,
    /// Permission denied
    PermissionDenied,
    /// Generic I/O error
    Other,
}

impl IoError {
    /// Create a file not found error
    pub fn file_not_found(path: &std::path::Path) -> Self {
        Self {
            kind: IoErrorKind::FileNotFound,
            path: Some(path.to_path_buf()),
            source: None,
        }
    }

    /// Create a permission denied error
    pub fn permission_denied(path: &std::path::Path, source: std::io::Error) -> Self {
        Self {
            kind: IoErrorKind::PermissionDenied,
            path: Some(path.to_path_buf()),
            source: Some(source),
        }
    }

    /// Create an I/O error from a standard I/O error
    pub fn from_std(source: std::io::Error) -> Self {
        let kind = match source.kind() {
            std::io::ErrorKind::NotFound => IoErrorKind::FileNotFound,
            std::io::ErrorKind::PermissionDenied => IoErrorKind::PermissionDenied,
            _ => IoErrorKind::Other,
        };

        Self {
            kind,
            path: None,
            source: Some(source),
        }
    }

    /// Create an I/O error with a path
    pub fn with_path(mut self, path: &std::path::Path) -> Self {
        self.path = Some(path.to_path_buf());
        self
    }
}

fn format_io_error(error: &IoError) -> String {
    match (&error.kind, &error.path) {
        (IoErrorKind::FileNotFound, Some(path)) => {
            format!("File not found: {}", path.display())
        }
        (IoErrorKind::FileNotFound, None) => "File not found".to_string(),
        (IoErrorKind::PermissionDenied, Some(path)) => {
            format!("Permission denied for file: {}", path.display())
        }
        (IoErrorKind::PermissionDenied, None) => "Permission denied".to_string(),
        (IoErrorKind::Other, _) => {
            if let Some(source) = &error.source {
                format!("I/O error: {source}")
            } else {
                "I/O error".to_string()
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io;

    #[test]
    fn test_file_not_found_error() {
        let path = std::path::Path::new("/test/file.mkv");
        let error = IoError::file_not_found(path);

        assert_eq!(error.kind, IoErrorKind::FileNotFound);
        assert_eq!(error.path, Some(path.to_path_buf()));
        assert!(error.source.is_none());
        assert!(error.to_string().contains("File not found"));
        assert!(error.to_string().contains("/test/file.mkv"));
    }

    #[test]
    fn test_permission_denied_error() {
        let path = std::path::Path::new("/root/protected.mkv");
        let io_error = io::Error::new(io::ErrorKind::PermissionDenied, "Access denied");
        let error = IoError::permission_denied(path, io_error);

        assert_eq!(error.kind, IoErrorKind::PermissionDenied);
        assert_eq!(error.path, Some(path.to_path_buf()));
        assert!(error.source.is_some());
        assert!(error.to_string().contains("Permission denied"));
        assert!(error.to_string().contains("/root/protected.mkv"));
    }

    #[test]
    fn test_from_std_io_error() {
        let io_error = io::Error::new(io::ErrorKind::NotFound, "Not found");
        let error = IoError::from_std(io_error);

        assert_eq!(error.kind, IoErrorKind::FileNotFound);
        assert!(error.path.is_none());
        assert!(error.source.is_some());
    }

    #[test]
    fn test_with_path() {
        let io_error = io::Error::other("Generic error");
        let path = std::path::Path::new("/test.mkv");
        let error = IoError::from_std(io_error).with_path(path);

        assert_eq!(error.kind, IoErrorKind::Other);
        assert_eq!(error.path, Some(path.to_path_buf()));
    }
}
