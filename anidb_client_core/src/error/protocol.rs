//! Protocol related error types

use thiserror::Error;

/// Protocol-related errors for AniDB communication
#[derive(Error, Debug)]
pub enum ProtocolError {
    /// Network is offline or AniDB service unavailable
    #[error("Network is offline or AniDB service unavailable")]
    NetworkOffline,

    /// AniDB API error with response code
    #[error("AniDB API error: {code} - {message}")]
    ServerError { code: u16, message: String },

    /// Generic protocol error
    #[error("Protocol error: {message}")]
    Other { message: String },
}

impl ProtocolError {
    /// Create a server error with code and message
    pub fn server_error(code: u16, message: &str) -> Self {
        Self::ServerError {
            code,
            message: message.to_string(),
        }
    }

    /// Create a generic protocol error
    pub fn other(message: impl Into<String>) -> Self {
        Self::Other {
            message: message.into(),
        }
    }

    /// Check if this error is transient and can be retried
    pub fn is_transient(&self) -> bool {
        match self {
            Self::NetworkOffline => true,
            Self::ServerError { code, .. } => matches!(code, 500..=504 | 600..=604),
            Self::Other { .. } => false,
        }
    }

    /// Check if this error indicates a permanent failure
    pub fn is_permanent(&self) -> bool {
        match self {
            Self::ServerError { code, .. } => matches!(code, 400..=499),
            _ => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_network_offline_error() {
        let error = ProtocolError::NetworkOffline;
        assert!(error.to_string().contains("Network is offline"));
        assert!(error.is_transient());
        assert!(!error.is_permanent());
    }

    #[test]
    fn test_server_error() {
        let error = ProtocolError::server_error(500, "Internal server error");
        assert!(error.to_string().contains("500"));
        assert!(error.to_string().contains("Internal server error"));
        assert!(error.is_transient());
        assert!(!error.is_permanent());
    }

    #[test]
    fn test_permanent_error() {
        let error = ProtocolError::server_error(404, "Not found");
        assert!(!error.is_transient());
        assert!(error.is_permanent());
    }

    #[test]
    fn test_other_error() {
        let error = ProtocolError::other("Custom protocol error");
        assert!(error.to_string().contains("Custom protocol error"));
        assert!(!error.is_transient());
        assert!(!error.is_permanent());
    }
}
