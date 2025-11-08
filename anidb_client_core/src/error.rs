//! Error types for the AniDB Client Core Library
//!
//! This module contains all error types used throughout the library, organized
//! into logical categories for better maintainability and clarity.

use thiserror::Error;

pub mod internal;
pub mod io;
pub mod protocol;
pub mod validation;

pub use self::io::{IoError, IoErrorKind};
pub use self::protocol::ProtocolError;
pub use self::validation::ValidationError;
pub use internal::InternalError;

/// Result type alias for the library
pub type Result<T> = std::result::Result<T, Error>;

/// Main error type for the AniDB Client Core Library
///
/// Errors are categorized into four main types:
/// - I/O errors: File system and network I/O operations
/// - Protocol errors: AniDB protocol-specific errors
/// - Validation errors: Input validation and configuration errors
/// - Internal errors: Library internal errors (memory, cache, etc.)
#[derive(Error, Debug)]
pub enum Error {
    /// I/O related errors
    #[error(transparent)]
    Io(#[from] IoError),

    /// Protocol related errors
    #[error(transparent)]
    Protocol(#[from] ProtocolError),

    /// Validation related errors
    #[error(transparent)]
    Validation(#[from] ValidationError),

    /// Internal library errors
    #[error(transparent)]
    Internal(#[from] InternalError),
}

// Conversions from external error types

impl From<std::io::Error> for Error {
    fn from(source: std::io::Error) -> Self {
        Self::Io(IoError::from_std(source))
    }
}

#[cfg(feature = "database")]
impl From<sqlx::Error> for Error {
    fn from(err: sqlx::Error) -> Self {
        Self::Internal(InternalError::assertion(format!("Database error: {err}")))
    }
}

impl From<crate::protocol::error::ProtocolError> for Error {
    fn from(err: crate::protocol::error::ProtocolError) -> Self {
        use crate::protocol::error::ProtocolError as ProtoErr;

        match err {
            ProtoErr::Io(io_err) => Self::Io(IoError::from_std(io_err)),
            ProtoErr::Timeout(_) => Self::Protocol(ProtocolError::NetworkOffline),
            ProtoErr::AuthenticationFailed { reason } => Self::Validation(
                ValidationError::invalid_configuration(&format!("Authentication failed: {reason}")),
            ),
            ProtoErr::SessionExpired { .. } => Self::Validation(
                ValidationError::invalid_configuration("AniDB session expired"),
            ),
            ProtoErr::ServerError { code, message } => {
                Self::Protocol(ProtocolError::server_error(code, &message))
            }
            _ => Self::Protocol(ProtocolError::other(format!("Protocol error: {err}"))),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::error::Error as StdError;
    use std::io;
    use std::path::Path;

    #[test]
    fn test_file_not_found_error_creation() {
        let path = Path::new("/non/existent/file.mkv");
        let error = Error::Io(IoError::file_not_found(path));

        match error {
            Error::Io(io_err) => {
                assert_eq!(io_err.kind, IoErrorKind::FileNotFound);
                assert_eq!(io_err.path, Some(path.to_path_buf()));
            }
            _ => panic!("Expected Io error"),
        }
    }

    #[test]
    fn test_permission_denied_error_creation() {
        let path = Path::new("/root/protected.mkv");
        let io_error = io::Error::new(io::ErrorKind::PermissionDenied, "Access denied");
        let error = Error::Io(IoError::permission_denied(path, io_error));

        match error {
            Error::Io(io_err) => {
                assert_eq!(io_err.kind, IoErrorKind::PermissionDenied);
                assert_eq!(io_err.path, Some(path.to_path_buf()));
            }
            _ => panic!("Expected Io error"),
        }
    }

    #[test]
    fn test_hash_calculation_error_creation() {
        let algorithm = "ED2K";
        let message = "Failed to calculate hash";
        let error = Error::Internal(InternalError::hash_calculation(algorithm, message));

        match error {
            Error::Internal(InternalError::HashCalculation {
                algorithm: error_algorithm,
                message: error_message,
            }) => {
                assert_eq!(error_algorithm, algorithm);
                assert_eq!(error_message, message);
            }
            _ => panic!("Expected Internal::HashCalculation error"),
        }
    }

    #[test]
    fn test_ffi_error_creation() {
        let function = "calculate_hash";
        let message = "Invalid parameters";
        let error = Error::Internal(InternalError::ffi(function, message));

        match error {
            Error::Internal(InternalError::Ffi {
                function: error_function,
                message: error_message,
            }) => {
                assert_eq!(error_function, function);
                assert_eq!(error_message, message);
            }
            _ => panic!("Expected Internal::Ffi error"),
        }
    }

    #[test]
    fn test_memory_limit_exceeded_error() {
        let limit = 500_000_000; // 500MB
        let current = 600_000_000; // 600MB
        let error = Error::Internal(InternalError::memory_limit_exceeded(limit, current));

        assert!(matches!(
            error,
            Error::Internal(InternalError::MemoryLimitExceeded { .. })
        ));
        assert!(error.to_string().contains("Memory limit exceeded"));
        assert!(error.to_string().contains("500000000"));
        assert!(error.to_string().contains("600000000"));
    }

    #[test]
    fn test_network_offline_error() {
        let error = Error::Protocol(ProtocolError::NetworkOffline);

        assert!(matches!(
            error,
            Error::Protocol(ProtocolError::NetworkOffline)
        ));
        assert!(error.to_string().contains("Network is offline"));
    }

    #[test]
    fn test_invalid_configuration_error() {
        let message = "At least one hash algorithm must be specified";
        let error = Error::Validation(ValidationError::invalid_configuration(message));

        assert!(matches!(
            error,
            Error::Validation(ValidationError::InvalidConfiguration { .. })
        ));
        assert!(error.to_string().contains("Invalid configuration"));
        assert!(error.to_string().contains("hash algorithm"));
    }

    #[test]
    fn test_anidb_api_error() {
        let code = 500;
        let message = "Server temporarily overloaded";
        let error = Error::Protocol(ProtocolError::server_error(code, message));

        assert!(matches!(
            error,
            Error::Protocol(ProtocolError::ServerError { .. })
        ));
        assert!(error.to_string().contains("AniDB API error"));
        assert!(error.to_string().contains("500"));
        assert!(error.to_string().contains("overloaded"));
    }

    #[test]
    fn test_error_display() {
        let path = Path::new("/test/file.mkv");
        let error = Error::Io(IoError::file_not_found(path));
        let display_string = format!("{error}");

        assert!(display_string.contains("File not found"));
        assert!(display_string.contains("/test/file.mkv"));
    }

    #[test]
    fn test_error_debug() {
        let error = Error::Internal(InternalError::hash_calculation("ED2K", "Test error"));
        let debug_string = format!("{error:?}");

        assert!(debug_string.contains("Internal"));
        assert!(debug_string.contains("HashCalculation"));
    }

    #[test]
    fn test_error_trait_implementation() {
        let error = Error::Internal(InternalError::hash_calculation("ED2K", "Test error"));

        // Should compile if Error implements std::error::Error
        let _: &dyn StdError = &error;
    }

    #[test]
    fn test_from_io_error() {
        let io_error = io::Error::new(io::ErrorKind::NotFound, "File not found");
        let error: Error = io_error.into();

        match error {
            Error::Io(io_err) => {
                assert_eq!(io_err.kind, IoErrorKind::FileNotFound);
            }
            _ => panic!("Expected Io error"),
        }
    }

    #[test]
    fn test_result_type_alias() {
        fn returns_error() -> Result<()> {
            Err(Error::Internal(InternalError::hash_calculation(
                "ED2K", "Test",
            )))
        }

        let result = returns_error();
        assert!(result.is_err());
    }

    #[test]
    fn test_error_source_chain() {
        let io_error = io::Error::new(io::ErrorKind::PermissionDenied, "Access denied");
        let path = Path::new("/test/file.mkv");
        let error = Error::Io(IoError::permission_denied(path, io_error));

        // Should have a source error
        assert!(error.source().is_some());
    }

    #[test]
    fn test_error_is_send_and_sync() {
        fn assert_send<T: Send>() {}
        fn assert_sync<T: Sync>() {}

        assert_send::<Error>();
        assert_sync::<Error>();
    }

    #[test]
    fn test_error_display_formatting() {
        let errors = vec![
            Error::Io(IoError::file_not_found(&std::path::PathBuf::from(
                "test.mkv",
            ))),
            Error::Protocol(ProtocolError::NetworkOffline),
            Error::Validation(ValidationError::invalid_configuration("Invalid setting")),
            Error::Protocol(ProtocolError::server_error(404, "Not found")),
            Error::Internal(InternalError::hash_calculation("ED2K", "File corrupted")),
            Error::Internal(InternalError::ffi(
                "test_function",
                "Parameter validation failed",
            )),
            Error::Internal(InternalError::memory_limit_exceeded(1000, 1500)),
        ];

        for error in errors {
            let display_string = error.to_string();
            assert!(!display_string.is_empty());
        }
    }

    #[test]
    fn test_file_errors_include_path_context() {
        let path = std::path::PathBuf::from(
            "/very/long/path/to/anime/[SubsPlease] One Piece - 1000 [1080p].mkv",
        );

        let error1 = Error::Io(IoError::file_not_found(&path));
        assert!(error1.to_string().contains("[SubsPlease] One Piece - 1000"));

        let io_error = io::Error::new(io::ErrorKind::PermissionDenied, "Access denied");
        let error2 = Error::Io(IoError::permission_denied(&path, io_error));
        assert!(error2.to_string().contains("[SubsPlease] One Piece - 1000"));
    }

    #[test]
    fn test_hash_errors_include_algorithm_context() {
        let algorithms = ["ED2K", "CRC32", "MD5", "SHA1"];

        for algorithm in algorithms {
            let error = Error::Internal(InternalError::hash_calculation(algorithm, "Test error"));
            assert!(error.to_string().contains(algorithm));
        }
    }

    #[test]
    fn test_anidb_errors_include_response_codes() {
        let common_codes = [200, 201, 500, 501, 502, 503, 504, 555, 598, 600, 601, 602];

        for code in common_codes {
            let error = Error::Protocol(ProtocolError::server_error(code, "Test response"));
            assert!(error.to_string().contains(&code.to_string()));
        }
    }

    #[test]
    fn test_memory_errors_include_size_context() {
        let error = Error::Internal(InternalError::memory_limit_exceeded(
            100_000_000,
            150_000_000,
        ));
        let error_string = error.to_string();

        assert!(error_string.contains("100000000"));
        assert!(error_string.contains("150000000"));
        assert!(error_string.contains("bytes"));
    }

    #[test]
    fn test_path_too_long_error() {
        let path = std::path::PathBuf::from("/very/long/path");
        let error = Error::Validation(ValidationError::path_too_long(&path, 100));

        assert!(matches!(
            error,
            Error::Validation(ValidationError::PathTooLong { .. })
        ));
        assert!(error.to_string().contains("Path too long"));
        assert!(error.to_string().contains("/very/long/path"));
        assert!(error.to_string().contains("100"));
    }

    #[test]
    fn test_unsupported_io_strategy_error() {
        let error = Error::Internal(InternalError::unsupported_io_strategy(
            "mmap",
            "not available",
        ));

        assert!(matches!(
            error,
            Error::Internal(InternalError::UnsupportedIoStrategy { .. })
        ));
        assert!(error.to_string().contains("Unsupported I/O strategy"));
        assert!(error.to_string().contains("mmap"));
        assert!(error.to_string().contains("not available"));
    }
}
