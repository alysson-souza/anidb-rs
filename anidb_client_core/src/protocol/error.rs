//! Protocol-specific error types
//!
//! This module defines error types for the AniDB UDP protocol implementation.

use std::fmt;
use std::time::Duration;
use thiserror::Error;

/// Result type alias for protocol operations
pub type Result<T> = std::result::Result<T, ProtocolError>;

/// Protocol-specific error types
#[derive(Error, Debug)]
pub enum ProtocolError {
    /// Network I/O error
    #[error("Network I/O error: {0}")]
    Io(#[from] std::io::Error),

    /// Connection timeout
    #[error("Connection timeout after {0:?}")]
    Timeout(Duration),

    /// Invalid packet format
    #[error("Invalid packet format: {message}")]
    InvalidPacket { message: String },

    /// Encoding error
    #[error("Encoding error: {message}")]
    Encoding { message: String },

    /// Decoding error
    #[error("Decoding error: {message}")]
    Decoding { message: String },

    /// Packet too large
    #[error("Packet size {size} exceeds maximum {max_size}")]
    PacketTooLarge { size: usize, max_size: usize },

    /// Session expired
    #[error("Session expired after {duration:?}")]
    SessionExpired { duration: Duration },

    /// Not connected
    #[error("Not connected to AniDB server")]
    NotConnected,

    /// Already connected
    #[error("Already connected to AniDB server")]
    AlreadyConnected,

    /// Rate limit exceeded
    #[error("Rate limit exceeded: must wait {wait_time:?} before next request")]
    RateLimitExceeded { wait_time: Duration },

    /// AniDB server error
    #[error("AniDB server error: {code} - {message}")]
    ServerError { code: u16, message: String },

    /// Authentication failed
    #[error("Authentication failed: {reason}")]
    AuthenticationFailed { reason: String },

    /// Invalid session
    #[error("Invalid session tag: {session}")]
    InvalidSession { session: String },

    /// Fragmentation error
    #[error("Fragmentation error: {message}")]
    Fragmentation { message: String },

    /// Missing required field
    #[error("Missing required field: {field}")]
    MissingField { field: String },

    /// Invalid response format
    #[error("Invalid response format: expected {expected}, got {actual}")]
    InvalidResponse { expected: String, actual: String },

    /// Unsupported command
    #[error("Unsupported command: {command}")]
    UnsupportedCommand { command: String },

    /// Buffer overflow
    #[error("Buffer overflow: {message}")]
    BufferOverflow { message: String },
}

impl ProtocolError {
    /// Create an invalid packet error
    pub fn invalid_packet(message: impl Into<String>) -> Self {
        Self::InvalidPacket {
            message: message.into(),
        }
    }

    /// Create an encoding error
    pub fn encoding(message: impl Into<String>) -> Self {
        Self::Encoding {
            message: message.into(),
        }
    }

    /// Create a decoding error
    pub fn decoding(message: impl Into<String>) -> Self {
        Self::Decoding {
            message: message.into(),
        }
    }

    /// Create a packet too large error
    pub fn packet_too_large(size: usize, max_size: usize) -> Self {
        Self::PacketTooLarge { size, max_size }
    }

    /// Create a session expired error
    pub fn session_expired(duration: Duration) -> Self {
        Self::SessionExpired { duration }
    }

    /// Create a rate limit exceeded error
    pub fn rate_limit_exceeded(wait_time: Duration) -> Self {
        Self::RateLimitExceeded { wait_time }
    }

    /// Create a server error
    pub fn server_error(code: u16, message: impl Into<String>) -> Self {
        Self::ServerError {
            code,
            message: message.into(),
        }
    }

    /// Create an authentication failed error
    pub fn authentication_failed(reason: impl Into<String>) -> Self {
        Self::AuthenticationFailed {
            reason: reason.into(),
        }
    }

    /// Create an invalid session error
    pub fn invalid_session(session: impl Into<String>) -> Self {
        Self::InvalidSession {
            session: session.into(),
        }
    }

    /// Create a fragmentation error
    pub fn fragmentation(message: impl Into<String>) -> Self {
        Self::Fragmentation {
            message: message.into(),
        }
    }

    /// Create a missing field error
    pub fn missing_field(field: impl Into<String>) -> Self {
        Self::MissingField {
            field: field.into(),
        }
    }

    /// Create an invalid response error
    pub fn invalid_response(expected: impl Into<String>, actual: impl Into<String>) -> Self {
        Self::InvalidResponse {
            expected: expected.into(),
            actual: actual.into(),
        }
    }

    /// Create an unsupported command error
    pub fn unsupported_command(command: impl Into<String>) -> Self {
        Self::UnsupportedCommand {
            command: command.into(),
        }
    }

    /// Create a buffer overflow error
    pub fn buffer_overflow(message: impl Into<String>) -> Self {
        Self::BufferOverflow {
            message: message.into(),
        }
    }

    /// Check if this error is transient and can be retried
    pub fn is_transient(&self) -> bool {
        matches!(
            self,
            Self::Io(_)
                | Self::Timeout(_)
                | Self::RateLimitExceeded { .. }
                | Self::ServerError {
                    code: 600..=604,
                    ..
                }
        )
    }

    /// Check if this error indicates a need to re-authenticate
    pub fn requires_reauth(&self) -> bool {
        matches!(
            self,
            Self::SessionExpired { .. }
                | Self::InvalidSession { .. }
                | Self::ServerError {
                    code: 501 | 506,
                    ..
                }
        )
    }
}

/// Response code returned by AniDB server
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ResponseCode(pub u16);

impl ResponseCode {
    /// Check if the response code indicates success
    pub fn is_success(&self) -> bool {
        matches!(self.0, 200..=299)
    }

    /// Check if the response code indicates an error
    pub fn is_error(&self) -> bool {
        self.0 >= 500
    }

    /// Check if the response code indicates a client error
    pub fn is_client_error(&self) -> bool {
        matches!(self.0, 400..=499)
    }

    /// Check if the response code indicates a server error
    pub fn is_server_error(&self) -> bool {
        matches!(self.0, 500..=599)
    }

    /// Get a human-readable description of the response code
    pub fn description(&self) -> &'static str {
        match self.0 {
            // Success codes
            200 => "LOGIN ACCEPTED",
            201 => "LOGIN ACCEPTED - NEW VERSION AVAILABLE",
            209 => "ENCRYPTION ENABLED",

            // File/Data codes
            220 => "FILE",
            230 => "ANIME",
            240 => "MYLIST",
            250 => "MYLIST STATS",

            // Client errors
            401 => "AUTHENTICATION FAILED",
            403 => "NOT LOGGED IN",
            410 => "INVALID PARAMETERS",
            411 => "ILLEGAL INPUT OR ACCESS DENIED",

            // Server errors
            500 => "LOGIN FAILED",
            501 => "LOGIN FIRST",
            502 => "ACCESS DENIED",
            503 => "CLIENT VERSION OUTDATED",
            504 => "CLIENT BANNED",
            505 => "ILLEGAL INPUT OR ACCESS DENIED",
            506 => "INVALID SESSION",
            555 => "BANNED",
            598 => "UNKNOWN COMMAND",
            600 => "INTERNAL SERVER ERROR",
            601 => "ANIDB OUT OF SERVICE",
            602 => "SERVER BUSY",
            604 => "TIMEOUT - DELAY AND RESUBMIT",

            _ => "UNKNOWN RESPONSE CODE",
        }
    }
}

impl fmt::Display for ResponseCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} {}", self.0, self.description())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_creation() {
        let err = ProtocolError::invalid_packet("bad format");
        assert!(matches!(err, ProtocolError::InvalidPacket { .. }));
        assert!(err.to_string().contains("bad format"));
    }

    #[test]
    fn test_transient_errors() {
        let errors = vec![
            ProtocolError::Io(std::io::Error::new(std::io::ErrorKind::TimedOut, "timeout")),
            ProtocolError::Timeout(Duration::from_secs(30)),
            ProtocolError::RateLimitExceeded {
                wait_time: Duration::from_secs(2),
            },
            ProtocolError::ServerError {
                code: 602,
                message: "busy".to_string(),
            },
        ];

        for err in errors {
            assert!(err.is_transient(), "{err:?} should be transient");
        }
    }

    #[test]
    fn test_non_transient_errors() {
        let errors = vec![
            ProtocolError::InvalidPacket {
                message: "bad".to_string(),
            },
            ProtocolError::AuthenticationFailed {
                reason: "wrong password".to_string(),
            },
            ProtocolError::ServerError {
                code: 404,
                message: "not found".to_string(),
            },
        ];

        for err in errors {
            assert!(!err.is_transient(), "{err:?} should not be transient");
        }
    }

    #[test]
    fn test_requires_reauth() {
        let errors = vec![
            ProtocolError::SessionExpired {
                duration: Duration::from_secs(1800),
            },
            ProtocolError::InvalidSession {
                session: "abc123".to_string(),
            },
            ProtocolError::ServerError {
                code: 501,
                message: "login first".to_string(),
            },
            ProtocolError::ServerError {
                code: 506,
                message: "invalid session".to_string(),
            },
        ];

        for err in errors {
            assert!(err.requires_reauth(), "{err:?} should require reauth");
        }
    }

    #[test]
    fn test_response_code() {
        // Success codes
        assert!(ResponseCode(200).is_success());
        assert!(ResponseCode(220).is_success());
        assert!(!ResponseCode(200).is_error());

        // Client errors
        assert!(ResponseCode(401).is_client_error());
        assert!(!ResponseCode(401).is_server_error());

        // Server errors
        assert!(ResponseCode(500).is_error());
        assert!(ResponseCode(500).is_server_error());
        assert!(!ResponseCode(500).is_client_error());

        // Descriptions
        assert_eq!(ResponseCode(200).description(), "LOGIN ACCEPTED");
        assert_eq!(ResponseCode(555).description(), "BANNED");
        assert_eq!(ResponseCode(999).description(), "UNKNOWN RESPONSE CODE");
    }

    #[test]
    fn test_response_code_display() {
        let code = ResponseCode(200);
        assert_eq!(code.to_string(), "200 LOGIN ACCEPTED");
    }

    #[test]
    fn test_error_conversions() {
        let io_err = std::io::Error::other("network error");
        let proto_err: ProtocolError = io_err.into();
        assert!(matches!(proto_err, ProtocolError::Io(_)));
    }
}
