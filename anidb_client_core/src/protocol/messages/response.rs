//! Response parsing and type management
//!
//! This module handles parsing of raw AniDB responses into typed structures.

use crate::protocol::error::{ProtocolError, Result};
use crate::protocol::messages::{
    AniDBResponse, AnimeResponse, AuthResponse, EpisodeResponse, FileResponse, GroupResponse,
    parse_response_fields, parse_response_header,
};

/// Enumeration of all possible AniDB responses
#[derive(Debug, Clone)]
pub enum Response {
    /// Authentication response
    Auth(AuthResponse),
    /// File information response
    File(Box<FileResponse>),
    /// Anime information response
    Anime(Box<AnimeResponse>),
    /// Episode information response
    Episode(Box<EpisodeResponse>),
    /// Group information response
    Group(Box<GroupResponse>),
    /// Generic response
    Generic(GenericResponse),
    /// Pong response
    Pong(PongResponse),
    /// Logout response
    Logout(LogoutResponse),
}

impl Response {
    /// Get the response code
    pub fn code(&self) -> u16 {
        match self {
            Response::Auth(r) => r.code(),
            Response::File(r) => r.code(),
            Response::Anime(r) => r.code(),
            Response::Episode(r) => r.code(),
            Response::Group(r) => r.code(),
            Response::Generic(r) => r.code(),
            Response::Pong(r) => r.code(),
            Response::Logout(r) => r.code(),
        }
    }

    /// Get the response message
    pub fn message(&self) -> &str {
        match self {
            Response::Auth(r) => r.message(),
            Response::File(r) => r.message(),
            Response::Anime(r) => r.message(),
            Response::Episode(r) => r.message(),
            Response::Group(r) => r.message(),
            Response::Generic(r) => r.message(),
            Response::Pong(r) => r.message(),
            Response::Logout(r) => r.message(),
        }
    }

    /// Check if the response indicates success
    pub fn is_success(&self) -> bool {
        match self {
            Response::Auth(r) => r.is_success(),
            Response::File(r) => r.is_success(),
            Response::Anime(r) => r.is_success(),
            Response::Episode(r) => r.is_success(),
            Response::Group(r) => r.is_success(),
            Response::Generic(r) => r.is_success(),
            Response::Pong(r) => r.is_success(),
            Response::Logout(r) => r.is_success(),
        }
    }

    /// Check if the response indicates an error
    pub fn is_error(&self) -> bool {
        self.code() >= 500
    }

    /// Convert to a protocol error if this is an error response
    pub fn to_error(&self) -> Option<ProtocolError> {
        if self.is_error() {
            Some(ProtocolError::server_error(self.code(), self.message()))
        } else {
            None
        }
    }
}

/// Generic response for untyped responses
#[derive(Debug, Clone)]
pub struct GenericResponse {
    pub code: u16,
    pub message: String,
    pub fields: Vec<String>,
}

impl AniDBResponse for GenericResponse {
    fn code(&self) -> u16 {
        self.code
    }

    fn message(&self) -> &str {
        &self.message
    }

    fn fields(&self) -> &[String] {
        &self.fields
    }
}

/// PONG response
#[derive(Debug, Clone)]
pub struct PongResponse {
    pub code: u16,
    pub message: String,
    pub port: Option<u16>,
}

impl AniDBResponse for PongResponse {
    fn code(&self) -> u16 {
        self.code
    }

    fn message(&self) -> &str {
        &self.message
    }

    fn fields(&self) -> &[String] {
        &[]
    }
}

/// LOGOUT response
#[derive(Debug, Clone)]
pub struct LogoutResponse {
    pub code: u16,
    pub message: String,
}

impl LogoutResponse {
    /// Check if the logout was successful
    pub fn is_success(&self) -> bool {
        self.code == 203
    }
}

impl AniDBResponse for LogoutResponse {
    fn code(&self) -> u16 {
        self.code
    }

    fn message(&self) -> &str {
        &self.message
    }

    fn fields(&self) -> &[String] {
        &[]
    }
}

/// Response parser for converting raw data to typed responses
pub struct ResponseParser;

impl ResponseParser {
    /// Parse a raw response into a typed Response
    pub fn parse(raw_response: &str, expected_command: Option<&str>) -> Result<Response> {
        let lines: Vec<&str> = raw_response.lines().collect();
        if lines.is_empty() {
            return Err(ProtocolError::invalid_packet("Empty response"));
        }

        // Parse the response header (code and message)
        let (code, message) = parse_response_header(lines[0])?;

        // Parse additional data fields if present
        let fields = if lines.len() > 1 {
            parse_response_fields(lines[1])
        } else {
            Vec::new()
        };

        // Route to specific parser based on response code and expected command
        match (code, expected_command) {
            // AUTH responses - including all error codes
            (200..=201, Some("AUTH")) | (500, Some("AUTH")) | (503..=505, Some("AUTH")) => {
                Ok(Response::Auth(AuthResponse::parse(code, message, fields)?))
            }

            // FILE responses
            (220, Some("FILE")) | (320, Some("FILE")) => Ok(Response::File(Box::new(
                FileResponse::parse(code, message, fields)?,
            ))),

            // ANIME responses
            (230, Some("ANIME")) | (330, Some("ANIME")) => Ok(Response::Anime(Box::new(
                AnimeResponse::new(code, message, fields)?,
            ))),

            // EPISODE responses
            (240, Some("EPISODE")) | (340, Some("EPISODE")) => Ok(Response::Episode(Box::new(
                EpisodeResponse::new(code, message, fields)?,
            ))),

            // GROUP responses
            (250, Some("GROUP")) | (350, Some("GROUP")) => Ok(Response::Group(Box::new(
                GroupResponse::new(code, message, fields)?,
            ))),

            // PING response
            (300, Some("PING")) => {
                let port = if !fields.is_empty() {
                    fields[0].parse().ok()
                } else {
                    None
                };
                Ok(Response::Pong(PongResponse {
                    code,
                    message,
                    port,
                }))
            }

            // LOGOUT response
            (203, Some("LOGOUT")) | (403, Some("LOGOUT")) => {
                Ok(Response::Logout(LogoutResponse { code, message }))
            }

            // Generic error responses that apply to all commands
            (501, _) | (502, _) | (505, _) | (506, _) | (555, _) | (598, _) | (600..=604, _) => {
                Ok(Response::Generic(GenericResponse {
                    code,
                    message,
                    fields,
                }))
            }

            // Default to generic response
            _ => Ok(Response::Generic(GenericResponse {
                code,
                message,
                fields,
            })),
        }
    }

    /// Parse a multi-packet fragmented response
    pub fn parse_fragmented(
        packets: &[String],
        expected_command: Option<&str>,
    ) -> Result<Response> {
        if packets.is_empty() {
            return Err(ProtocolError::fragmentation("No packets to parse"));
        }

        // Combine all packets
        let combined = packets.join("\n");
        Self::parse(&combined, expected_command)
    }

    /// Check if a response code indicates the response might be fragmented
    pub fn might_be_fragmented(code: u16) -> bool {
        // Responses that commonly exceed packet size
        matches!(
            code,
            220 | 230 | 231 | 250 | 290 | 291 | 310 | 311 | 321 | 322
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_auth_success() {
        let raw = "200 LOGIN ACCEPTED\nabc123def|1";
        let response = ResponseParser::parse(raw, Some("AUTH")).unwrap();

        match response {
            Response::Auth(auth) => {
                assert_eq!(auth.code(), 200);
                assert_eq!(auth.message(), "LOGIN ACCEPTED");
                assert_eq!(auth.session, Some("abc123def".to_string()));
                assert!(auth.imgserver);
            }
            _ => panic!("Expected Auth response"),
        }
    }

    #[test]
    fn test_parse_auth_new_version() {
        let raw = "201 LOGIN ACCEPTED - NEW VERSION AVAILABLE\nsession123|2.0|0";
        let response = ResponseParser::parse(raw, Some("AUTH")).unwrap();

        match response {
            Response::Auth(auth) => {
                assert_eq!(auth.code(), 201);
                assert!(auth.message().contains("NEW VERSION"));
                assert_eq!(auth.session, Some("session123".to_string()));
                assert_eq!(auth.new_version, Some("2.0".to_string()));
                assert!(!auth.imgserver);
            }
            _ => panic!("Expected Auth response"),
        }
    }

    #[test]
    fn test_parse_file_found() {
        let raw = "220 FILE\n312498|4896|69260|41|1|233647104|abc123";
        let response = ResponseParser::parse(raw, Some("FILE")).unwrap();

        match response {
            Response::File(file) => {
                assert_eq!(file.code(), 220);
                assert!(file.found());
                assert_eq!(file.fid, Some(312498));
                assert_eq!(file.aid, Some(4896));
                assert_eq!(file.size, Some(233647104));
            }
            _ => panic!("Expected File response"),
        }
    }

    #[test]
    fn test_parse_file_not_found() {
        let raw = "320 NO SUCH FILE";
        let response = ResponseParser::parse(raw, Some("FILE")).unwrap();

        match response {
            Response::File(file) => {
                assert_eq!(file.code(), 320);
                assert!(!file.found());
            }
            _ => panic!("Expected File response"),
        }
    }

    #[test]
    fn test_parse_pong() {
        let raw = "300 PONG\n12345";
        let response = ResponseParser::parse(raw, Some("PING")).unwrap();

        match response {
            Response::Pong(pong) => {
                assert_eq!(pong.code(), 300);
                assert_eq!(pong.message(), "PONG");
                assert_eq!(pong.port, Some(12345));
            }
            _ => panic!("Expected Pong response"),
        }
    }

    #[test]
    fn test_parse_logout() {
        let raw = "203 LOGGED OUT";
        let response = ResponseParser::parse(raw, Some("LOGOUT")).unwrap();

        match response {
            Response::Logout(logout) => {
                assert_eq!(logout.code(), 203);
                assert_eq!(logout.message(), "LOGGED OUT");
            }
            _ => panic!("Expected Logout response"),
        }
    }

    #[test]
    fn test_parse_generic_error() {
        let raw = "501 LOGIN FIRST";
        let response = ResponseParser::parse(raw, None).unwrap();

        match response {
            Response::Generic(generic) => {
                assert_eq!(generic.code(), 501);
                assert_eq!(generic.message(), "LOGIN FIRST");
                assert!(generic.fields.is_empty());
            }
            _ => panic!("Expected Generic response"),
        }
    }

    #[test]
    fn test_response_to_error() {
        let raw = "500 LOGIN FAILED";
        let response = ResponseParser::parse(raw, Some("AUTH")).unwrap();

        assert!(response.is_error());
        assert!(!response.is_success());

        let error = response.to_error();
        assert!(error.is_some());

        match error.unwrap() {
            ProtocolError::ServerError { code, message } => {
                assert_eq!(code, 500);
                assert_eq!(message, "LOGIN FAILED");
            }
            _ => panic!("Expected ServerError"),
        }
    }

    #[test]
    fn test_might_be_fragmented() {
        assert!(ResponseParser::might_be_fragmented(220)); // FILE
        assert!(ResponseParser::might_be_fragmented(230)); // ANIME
        assert!(ResponseParser::might_be_fragmented(250)); // MYLIST STATS
        assert!(!ResponseParser::might_be_fragmented(200)); // LOGIN ACCEPTED
        assert!(!ResponseParser::might_be_fragmented(300)); // PONG
    }

    #[test]
    fn test_parse_empty_response() {
        let result = ResponseParser::parse("", None);
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            ProtocolError::InvalidPacket { .. }
        ));
    }

    #[test]
    fn test_parse_malformed_response() {
        let result = ResponseParser::parse("INVALID RESPONSE FORMAT", None);
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_auth_error_505() {
        let raw = "505 ILLEGAL INPUT OR ACCESS DENIED";
        let response = ResponseParser::parse(raw, Some("AUTH")).unwrap();

        match response {
            Response::Auth(auth) => {
                assert_eq!(auth.code(), 505);
                assert_eq!(auth.message(), "ILLEGAL INPUT OR ACCESS DENIED");
                assert!(auth.is_error());
                assert_eq!(auth.session, None);
            }
            _ => panic!("Expected Auth response for 505 error"),
        }
    }

    #[test]
    fn test_parse_auth_error_504() {
        let raw = "504 CLIENT BANNED - Using outdated client";
        let response = ResponseParser::parse(raw, Some("AUTH")).unwrap();

        match response {
            Response::Auth(auth) => {
                assert_eq!(auth.code(), 504);
                assert_eq!(auth.message(), "CLIENT BANNED - Using outdated client");
                assert!(auth.is_error());
                assert_eq!(auth.session, None);
            }
            _ => panic!("Expected Auth response for 504 error"),
        }
    }
}
