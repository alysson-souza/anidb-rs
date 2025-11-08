//! Message encoding and decoding with packet fragmentation support
//!
//! This module handles the conversion between typed messages and raw bytes,
//! including support for fragmented responses and zero-copy operations.

mod decoder;
mod encoder;
mod fragmentation;

pub use decoder::{Decoder, DecoderState};
pub use encoder::Encoder;
pub use fragmentation::{FragmentAssembler, FragmentHeader};

use crate::protocol::error::{ProtocolError, Result};
use bytes::Bytes;
use log::{debug, trace};

/// Codec for encoding and decoding AniDB protocol messages
pub struct Codec {
    encoder: Encoder,
    decoder: Decoder,
}

impl Codec {
    /// Create a new codec instance
    pub fn new() -> Self {
        Self {
            encoder: Encoder::new(),
            decoder: Decoder::new(),
        }
    }

    /// Encode a command string into bytes
    pub fn encode(&mut self, command: &str) -> Result<Bytes> {
        debug!("Codec encoding command");
        self.encoder.encode(command)
    }

    /// Decode bytes into a response string
    pub fn decode(&mut self, data: &[u8]) -> Result<Option<String>> {
        debug!("Codec decoding {} bytes", data.len());
        let result = self.decoder.decode(data)?;
        if let Some(ref decoded) = result {
            debug!("Codec successfully decoded {} characters", decoded.len());
            trace!("Decoded content: {decoded}");
        } else {
            debug!("Codec needs more data for complete decode");
        }
        Ok(result)
    }

    /// Reset the codec state
    pub fn reset(&mut self) {
        self.decoder.reset();
    }

    /// Check if the decoder is waiting for more data
    pub fn is_waiting_for_data(&self) -> bool {
        self.decoder.is_waiting_for_data()
    }
}

impl Default for Codec {
    fn default() -> Self {
        Self::new()
    }
}

/// Helper functions for encoding/decoding primitives
pub mod primitives {
    use super::*;

    /// Encode a string as UTF-8 bytes
    pub fn encode_string(s: &str) -> Bytes {
        Bytes::from(s.to_string())
    }

    /// Decode UTF-8 bytes to string
    pub fn decode_string(data: &[u8]) -> Result<String> {
        String::from_utf8(data.to_vec())
            .map_err(|e| ProtocolError::decoding(format!("Invalid UTF-8: {e}")))
    }

    /// Encode a u16 as string bytes
    pub fn encode_u16(value: u16) -> Bytes {
        Bytes::from(value.to_string())
    }

    /// Decode string bytes to u16
    pub fn decode_u16(data: &[u8]) -> Result<u16> {
        let s = decode_string(data)?;
        s.parse()
            .map_err(|e| ProtocolError::decoding(format!("Invalid u16: {e}")))
    }

    /// Encode a u64 as string bytes
    pub fn encode_u64(value: u64) -> Bytes {
        Bytes::from(value.to_string())
    }

    /// Decode string bytes to u64
    pub fn decode_u64(data: &[u8]) -> Result<u64> {
        let s = decode_string(data)?;
        s.parse()
            .map_err(|e| ProtocolError::decoding(format!("Invalid u64: {e}")))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_codec_encode_decode() {
        let mut codec = Codec::new();

        // Test encoding a command
        let command = "AUTH user=test&pass=test123&protover=3&client=test&clientver=1.0";
        let encoded = codec.encode(command).unwrap();
        assert!(!encoded.is_empty());
        assert_eq!(&encoded[..], command.as_bytes());

        // Test decoding a response (not a command)
        let response = "200 LOGIN ACCEPTED\nabc123|1";
        let decoded = codec.decode(response.as_bytes()).unwrap();
        assert_eq!(decoded, Some(response.to_string()));
    }

    #[test]
    fn test_codec_reset() {
        let mut codec = Codec::new();

        // Simulate partial data
        let partial = b"200 LOGIN";
        let result = codec.decode(partial).unwrap();
        assert_eq!(result, None); // Waiting for more data
        assert!(codec.is_waiting_for_data());

        // Reset should clear state
        codec.reset();
        assert!(!codec.is_waiting_for_data());
    }

    #[test]
    fn test_primitives_string() {
        let original = "test string";
        let encoded = primitives::encode_string(original);
        let decoded = primitives::decode_string(&encoded).unwrap();
        assert_eq!(decoded, original);
    }

    #[test]
    fn test_primitives_u16() {
        let value = 12345u16;
        let encoded = primitives::encode_u16(value);
        let decoded = primitives::decode_u16(&encoded).unwrap();
        assert_eq!(decoded, value);
    }

    #[test]
    fn test_primitives_u64() {
        let value = 1234567890123u64;
        let encoded = primitives::encode_u64(value);
        let decoded = primitives::decode_u64(&encoded).unwrap();
        assert_eq!(decoded, value);
    }

    #[test]
    fn test_primitives_invalid_utf8() {
        let invalid = vec![0xFF, 0xFE];
        let result = primitives::decode_string(&invalid);
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            ProtocolError::Decoding { .. }
        ));
    }

    #[test]
    fn test_primitives_invalid_number() {
        let invalid = b"not_a_number";
        let result = primitives::decode_u16(invalid);
        assert!(result.is_err());

        let result = primitives::decode_u64(invalid);
        assert!(result.is_err());
    }
}
