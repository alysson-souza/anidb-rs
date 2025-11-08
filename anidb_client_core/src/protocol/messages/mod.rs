//! Type-safe message definitions for the AniDB protocol
//!
//! This module provides strongly-typed representations of all AniDB commands
//! and responses, with builder patterns for easy construction.

pub mod anime;
pub mod auth;
pub mod command;
pub mod episode;
pub mod file;
pub mod group;
pub mod mylist;
pub mod response;

pub use anime::{AnimeCommand, AnimeResponse};
pub use auth::{AuthCommand, AuthResponse, LogoutCommand, PingCommand};
pub use command::{Command, CommandBuilder};
pub use episode::{EpisodeCommand, EpisodeResponse};
pub use file::{FileCommand, FileCommandBuilder, FileResponse};
pub use group::{GroupCommand, GroupResponse};
pub use mylist::{
    MyListAddCommand, MyListAddCommandBuilder, MyListAddResponse, MyListDelCommand,
    MyListDelResponse,
};
pub use response::{Response, ResponseParser};

use crate::protocol::error::{ProtocolError, Result};
use std::collections::HashMap;
use std::fmt;

/// Parameter separator used in AniDB protocol
pub const PARAM_SEPARATOR: char = '|';

/// Field separator within parameters
pub const FIELD_SEPARATOR: char = ',';

/// Newline encoding for multiline values
pub const ENCODED_NEWLINE: &str = "<br />";

/// Quote encoding
pub const ENCODED_QUOTE: &str = "`";

/// Pipe encoding
pub const ENCODED_PIPE: &str = "/";

/// Base trait for all AniDB commands
pub trait AniDBCommand: fmt::Debug + Send + Sync {
    /// Get the command name
    fn name(&self) -> &str;

    /// Get command parameters
    fn parameters(&self) -> HashMap<String, String>;

    /// Encode the command for transmission
    fn encode(&self) -> Result<String> {
        let mut parts = vec![self.name().to_string()];

        for (key, value) in self.parameters() {
            let encoded_value = encode_value(&value);
            parts.push(format!("{key}={encoded_value}"));
        }

        // Join with space between command and first param, then & between params
        if parts.len() <= 1 {
            Ok(parts.join(""))
        } else {
            Ok(format!("{} {}", parts[0], parts[1..].join("&")))
        }
    }

    /// Check if this command requires authentication
    fn requires_auth(&self) -> bool {
        !matches!(
            self.name(),
            "PING" | "ENCRYPT" | "ENCODING" | "AUTH" | "VERSION"
        )
    }
}

/// Base trait for all AniDB responses
pub trait AniDBResponse: fmt::Debug + Send + Sync {
    /// Get the response code
    fn code(&self) -> u16;

    /// Get the response message
    fn message(&self) -> &str;

    /// Get response data fields
    fn fields(&self) -> &[String];

    /// Check if the response indicates success
    fn is_success(&self) -> bool {
        (200..300).contains(&self.code())
    }

    /// Check if the response indicates an error
    fn is_error(&self) -> bool {
        self.code() >= 500
    }
}

/// Encode a value for AniDB protocol transmission
///
/// According to AniDB protocol documentation:
/// "Escape scheme for option values (to server): html form encoding + newline
/// This means you have to encode at least & in your option values in html form
/// encoding style (&amp;) before sending them to the api server."
///
/// Based on working implementations and AniDB's actual behavior:
/// - Only & needs to be encoded as &amp;
/// - Newlines are encoded as <br />
/// - Other special characters are sent as-is (UTF-8 encoded at packet level)
pub fn encode_value(value: &str) -> String {
    let mut result = String::with_capacity(value.len() + 10);

    for ch in value.chars() {
        match ch {
            // HTML entity encoding for ampersand (required by AniDB)
            '&' => result.push_str("&amp;"),
            // Newline encoding
            '\n' => result.push_str(ENCODED_NEWLINE),
            '\r' => continue, // Skip carriage returns
            // All other characters pass through unchanged
            _ => result.push(ch),
        }
    }

    result
}

/// Decode a value from AniDB protocol format
///
/// Reverses the encoding applied by encode_value:
/// 1. HTML entity decoding for &amp;
/// 2. Newline decoding from <br />
/// 3. Special AniDB encodings for quotes and pipes in responses
pub fn decode_value(value: &str) -> String {
    let mut result = String::with_capacity(value.len());
    let mut chars = value.chars();

    while let Some(ch) = chars.next() {
        if ch == '&' {
            // Check for &amp;
            let remaining: String = chars.clone().take(4).collect();
            if remaining == "amp;" {
                result.push('&');
                // Skip the "amp;" part
                for _ in 0..4 {
                    chars.next();
                }
                continue;
            }
        } else if ch == '<' {
            // Check for <br />
            let remaining: String = chars.clone().take(5).collect();
            if remaining == "br />" {
                result.push('\n');
                // Skip the "br />" part
                for _ in 0..5 {
                    chars.next();
                }
                continue;
            }
        } else if ch == '`' {
            // AniDB uses backtick for quotes in responses
            result.push('\'');
            continue;
        } else if ch == '/' && value.len() == 1 {
            // Single "/" represents pipe in responses
            result.push('|');
            continue;
        }

        // All other characters pass through unchanged
        result.push(ch);
    }

    result
}

/// Parse a raw response line into code and message
///
/// AniDB responses can have different formats:
/// - Without tag: `{code} {message}` or `{code} {session_key} {message}` for AUTH
/// - With tag: `{tag} {code} {message}` or `{tag} {code} {session_key} {message}` for AUTH
pub fn parse_response_header(line: &str) -> Result<(u16, String)> {
    let parts: Vec<&str> = line.splitn(2, ' ').collect();

    if parts.is_empty() {
        return Err(ProtocolError::invalid_packet("Empty response"));
    }

    let code = parts[0].parse::<u16>().map_err(|_| {
        ProtocolError::invalid_packet(format!("Invalid response code: {}", parts[0]))
    })?;

    let message = parts.get(1).unwrap_or(&"").to_string();

    Ok((code, message))
}

/// Parse response fields from a data line
pub fn parse_response_fields(line: &str) -> Vec<String> {
    line.split(PARAM_SEPARATOR).map(decode_value).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_encode_value() {
        assert_eq!(encode_value("simple"), "simple");
        assert_eq!(encode_value("with&ampersand"), "with&amp;ampersand");
        assert_eq!(
            encode_value("line1\nline2"),
            format!("line1{ENCODED_NEWLINE}line2")
        );
        assert_eq!(
            encode_value("both&and\nnewline"),
            format!("both&amp;and{ENCODED_NEWLINE}newline")
        );
        // Special characters are NOT URL encoded anymore
        assert_eq!(encode_value("user@example.com"), "user@example.com");
        assert_eq!(encode_value("pass!word"), "pass!word");
        assert_eq!(encode_value("space test"), "space test");
        assert_eq!(encode_value("P@ssw0rd!#2024"), "P@ssw0rd!#2024");
        assert_eq!(encode_value("test&user"), "test&amp;user");
    }

    #[test]
    fn test_decode_value() {
        assert_eq!(decode_value("simple"), "simple");
        assert_eq!(
            decode_value(format!("line1{ENCODED_NEWLINE}line2").as_str()),
            "line1\nline2"
        );
        assert_eq!(
            decode_value(format!("quote{ENCODED_QUOTE}here").as_str()),
            "quote'here"
        );
        assert_eq!(decode_value("`"), "'"); // Single backtick
        assert_eq!(decode_value("/"), "|"); // Single slash
        assert_eq!(decode_value("/path/to/file"), "/path/to/file"); // Multiple slashes stay as-is
        assert_eq!(decode_value("&amp;"), "&");
        assert_eq!(decode_value("test&amp;user"), "test&user");
        // Special characters pass through unchanged (no URL decoding)
        assert_eq!(decode_value("user@example.com"), "user@example.com");
        assert_eq!(decode_value("pass!word"), "pass!word");
        assert_eq!(decode_value("space test"), "space test");
    }

    #[test]
    fn test_parse_response_header() {
        let (code, msg) = parse_response_header("200 LOGIN ACCEPTED").unwrap();
        assert_eq!(code, 200);
        assert_eq!(msg, "LOGIN ACCEPTED");

        let (code, msg) = parse_response_header("500").unwrap();
        assert_eq!(code, 500);
        assert_eq!(msg, "");

        assert!(parse_response_header("").is_err());
        assert!(parse_response_header("ABC INVALID").is_err());
    }

    #[test]
    fn test_parse_response_fields() {
        let fields = parse_response_fields("field1|field2|field3");
        assert_eq!(fields, vec!["field1", "field2", "field3"]);

        let fields = parse_response_fields("single");
        assert_eq!(fields, vec!["single"]);

        let fields = parse_response_fields("");
        assert_eq!(fields, vec![""]);

        // Test with encoded values
        // Note: "/" in field values is kept as "/" since it's not a single "/"
        let encoded = format!("field1{ENCODED_PIPE}with{ENCODED_PIPE}pipe|normal");
        let fields = parse_response_fields(&encoded);
        assert_eq!(fields, vec!["field1/with/pipe", "normal"]);
    }

    #[test]
    fn test_encode_decode_roundtrip() {
        let original = "Test with & and\nnewline";
        let encoded = encode_value(original);
        let decoded = decode_value(&encoded);
        assert_eq!(original, decoded);

        // Test with special characters
        let special = "user@example.com & pass!word";
        let encoded = encode_value(special);
        let decoded = decode_value(&encoded);
        assert_eq!(special, decoded);

        // Test with all special chars
        let all_special = "!@#$%^&*()[]{}|\\:;\"'<>?,./";
        let encoded = encode_value(all_special);
        let decoded = decode_value(&encoded);
        assert_eq!(all_special, decoded);
    }
}
