//! Validation related error types

use std::path::PathBuf;
use thiserror::Error;

/// Validation and configuration errors
#[derive(Error, Debug)]
pub enum ValidationError {
    /// Invalid configuration
    #[error("Invalid configuration: {message}")]
    InvalidConfiguration { message: String },

    /// Path too long for the platform
    #[error("Path too long: {path} exceeds maximum length of {max_length} characters")]
    PathTooLong { path: PathBuf, max_length: usize },

    /// Invalid input parameter
    #[error("Invalid parameter '{parameter}': {reason}")]
    InvalidParameter { parameter: String, reason: String },

    /// Missing required field
    #[error("Missing required field: {field}")]
    MissingField { field: String },
}

impl ValidationError {
    /// Create an invalid configuration error
    pub fn invalid_configuration(message: &str) -> Self {
        Self::InvalidConfiguration {
            message: message.to_string(),
        }
    }

    /// Create a path too long error
    pub fn path_too_long(path: &std::path::Path, max_length: usize) -> Self {
        Self::PathTooLong {
            path: path.to_path_buf(),
            max_length,
        }
    }

    /// Create an invalid parameter error
    pub fn invalid_parameter(parameter: &str, reason: &str) -> Self {
        Self::InvalidParameter {
            parameter: parameter.to_string(),
            reason: reason.to_string(),
        }
    }

    /// Create a missing field error
    pub fn missing_field(field: &str) -> Self {
        Self::MissingField {
            field: field.to_string(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn test_invalid_configuration_error() {
        let error = ValidationError::invalid_configuration("Bad config");
        assert!(error.to_string().contains("Invalid configuration"));
        assert!(error.to_string().contains("Bad config"));
    }

    #[test]
    fn test_path_too_long_error() {
        let path = Path::new("/very/long/path");
        let error = ValidationError::path_too_long(path, 100);
        assert!(error.to_string().contains("Path too long"));
        assert!(error.to_string().contains("/very/long/path"));
        assert!(error.to_string().contains("100"));
    }

    #[test]
    fn test_invalid_parameter_error() {
        let error = ValidationError::invalid_parameter("buffer_size", "must be positive");
        assert!(error.to_string().contains("Invalid parameter"));
        assert!(error.to_string().contains("buffer_size"));
        assert!(error.to_string().contains("must be positive"));
    }

    #[test]
    fn test_missing_field_error() {
        let error = ValidationError::missing_field("username");
        assert!(error.to_string().contains("Missing required field"));
        assert!(error.to_string().contains("username"));
    }
}
