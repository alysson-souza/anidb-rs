//! Message encoder for AniDB protocol
//!
//! This module handles encoding of commands into bytes for transmission.

use crate::protocol::error::{ProtocolError, Result};
use bytes::{BufMut, Bytes, BytesMut};
use log::{debug, trace};

/// Encoder for AniDB protocol messages
pub struct Encoder {
    /// Buffer for encoding
    buffer: BytesMut,
}

impl Encoder {
    /// Create a new encoder
    pub fn new() -> Self {
        Self {
            buffer: BytesMut::with_capacity(crate::protocol::MAX_PACKET_SIZE),
        }
    }

    /// Encode a command string into bytes
    pub fn encode(&mut self, command: &str) -> Result<Bytes> {
        trace!("Encoding command: {command}");
        self.buffer.clear();

        // Validate command is not empty
        if command.is_empty() {
            debug!("Attempted to encode empty command");
            return Err(ProtocolError::encoding("Empty command"));
        }

        // Check command won't exceed packet size
        if command.len() > crate::protocol::MAX_PACKET_SIZE {
            debug!(
                "Command too large: {} bytes (max: {})",
                command.len(),
                crate::protocol::MAX_PACKET_SIZE
            );
            return Err(ProtocolError::packet_too_large(
                command.len(),
                crate::protocol::MAX_PACKET_SIZE,
            ));
        }

        // Encode as UTF-8
        self.buffer.put(command.as_bytes());

        // AniDB protocol doesn't require newline at the end of commands
        // The server will parse based on packet boundaries

        let result = self.buffer.split().freeze();
        debug!("Encoded {} bytes", result.len());
        trace!("Encoded bytes: {:?}", &result[..result.len().min(100)]);
        Ok(result)
    }

    /// Encode with a session tag appended
    pub fn encode_with_session(&mut self, command: &str, session: &str) -> Result<Bytes> {
        debug!("Encoding command with session: {session}");
        self.buffer.clear();

        // Build command with session
        let with_session = if command.contains(" s=") {
            // Session already included
            debug!("Command already contains session tag");
            command.to_string()
        } else {
            let cmd = format!("{command} s={session}");
            debug!("Appended session tag to command");
            cmd
        };

        self.encode(&with_session)
    }

    /// Encode multiple values with a separator
    pub fn encode_fields(&mut self, fields: &[&str], separator: char) -> Result<Bytes> {
        self.buffer.clear();

        for (i, field) in fields.iter().enumerate() {
            if i > 0 {
                self.buffer.put_u8(separator as u8);
            }
            self.buffer.put(field.as_bytes());
        }

        Ok(self.buffer.split().freeze())
    }

    /// Get the current buffer capacity
    pub fn capacity(&self) -> usize {
        self.buffer.capacity()
    }

    /// Reserve additional capacity in the buffer
    pub fn reserve(&mut self, additional: usize) {
        self.buffer.reserve(additional);
    }
}

impl Default for Encoder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_encode_simple_command() {
        let mut encoder = Encoder::new();
        let encoded = encoder.encode("PING").unwrap();
        assert_eq!(&encoded[..], b"PING");
    }

    #[test]
    fn test_encode_command_with_params() {
        let mut encoder = Encoder::new();
        let command = "AUTH user=test&pass=secret&protover=3";
        let encoded = encoder.encode(command).unwrap();
        assert_eq!(&encoded[..], command.as_bytes());
    }

    #[test]
    fn test_encode_empty_command() {
        let mut encoder = Encoder::new();
        let result = encoder.encode("");
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            ProtocolError::Encoding { .. }
        ));
    }

    #[test]
    fn test_encode_oversized_command() {
        let mut encoder = Encoder::new();
        let large_command = "A".repeat(crate::protocol::MAX_PACKET_SIZE + 1);
        let result = encoder.encode(&large_command);
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            ProtocolError::PacketTooLarge { .. }
        ));
    }

    #[test]
    fn test_encode_with_session() {
        let mut encoder = Encoder::new();

        // Command without session
        let encoded = encoder
            .encode_with_session("FILE fid=12345", "abc123")
            .unwrap();
        let decoded = String::from_utf8(encoded.to_vec()).unwrap();
        assert_eq!(decoded, "FILE fid=12345 s=abc123");

        // Command already has session
        let encoded = encoder
            .encode_with_session("FILE fid=12345 s=old", "new")
            .unwrap();
        let decoded = String::from_utf8(encoded.to_vec()).unwrap();
        assert_eq!(decoded, "FILE fid=12345 s=old");
    }

    #[test]
    fn test_encode_fields() {
        let mut encoder = Encoder::new();

        let fields = vec!["field1", "field2", "field3"];
        let encoded = encoder.encode_fields(&fields, '|').unwrap();
        let decoded = String::from_utf8(encoded.to_vec()).unwrap();
        assert_eq!(decoded, "field1|field2|field3");

        // Single field
        let fields = vec!["single"];
        let encoded = encoder.encode_fields(&fields, '|').unwrap();
        let decoded = String::from_utf8(encoded.to_vec()).unwrap();
        assert_eq!(decoded, "single");

        // Empty fields
        let fields: Vec<&str> = vec![];
        let encoded = encoder.encode_fields(&fields, '|').unwrap();
        assert!(encoded.is_empty());
    }

    #[test]
    fn test_buffer_reuse() {
        let mut encoder = Encoder::new();

        // First encode
        let encoded1 = encoder.encode("PING").unwrap();
        assert_eq!(&encoded1[..], b"PING");

        // Second encode should reuse buffer
        let encoded2 = encoder.encode("AUTH user=test").unwrap();
        assert_eq!(&encoded2[..], b"AUTH user=test");

        // Verify buffer was cleared between encodes
        assert_ne!(encoded1, encoded2);
    }

    #[test]
    fn test_capacity_management() {
        let mut encoder = Encoder::new();
        let initial_capacity = encoder.capacity();
        assert!(initial_capacity >= crate::protocol::MAX_PACKET_SIZE);

        // Reserve more capacity
        let before_reserve = encoder.capacity();
        encoder.reserve(1000);
        // The buffer should have at least 1000 bytes available
        assert!(encoder.capacity() >= 1000);
        // And it should be at least as large as before
        assert!(encoder.capacity() >= before_reserve);
    }

    #[test]
    fn test_encode_special_characters() {
        let mut encoder = Encoder::new();

        // Test encoding with special characters
        // Note: The encoder itself doesn't do URL encoding, that's handled by encode_value
        let command = "FILE ed2k=abc123 file=test%26file.mkv";
        let encoded = encoder.encode(command).unwrap();
        let decoded = String::from_utf8(encoded.to_vec()).unwrap();
        assert_eq!(decoded, command);
    }
}
