//! Message decoder for AniDB protocol
//!
//! This module handles decoding of response bytes into strings.

use crate::protocol::error::{ProtocolError, Result};
use bytes::BytesMut;
use log::{debug, trace, warn};

/// Decoder state for handling partial packets
#[derive(Debug, Clone, PartialEq)]
pub enum DecoderState {
    /// Waiting for data
    Idle,
    /// Accumulating partial data
    Buffering { expected_size: Option<usize> },
    /// Complete message ready
    Complete,
}

/// Decoder for AniDB protocol messages
pub struct Decoder {
    /// Buffer for accumulating data
    buffer: BytesMut,
    /// Current decoder state
    state: DecoderState,
    /// Maximum buffer size to prevent memory exhaustion
    max_buffer_size: usize,
}

impl Decoder {
    /// Create a new decoder
    pub fn new() -> Self {
        Self {
            buffer: BytesMut::with_capacity(crate::protocol::MAX_PACKET_SIZE * 2),
            state: DecoderState::Idle,
            max_buffer_size: crate::protocol::MAX_PACKET_SIZE * 10, // Allow for fragmented responses
        }
    }

    /// Decode bytes into a response string
    /// Returns None if more data is needed for incomplete UTF-8 sequences
    pub fn decode(&mut self, data: &[u8]) -> Result<Option<String>> {
        trace!("Decoder::decode called with {} bytes", data.len());
        trace!(
            "Current buffer size: {}, state: {:?}",
            self.buffer.len(),
            self.state
        );

        // Check for empty input
        if data.is_empty() && self.buffer.is_empty() {
            debug!("No data to decode (empty input and buffer)");
            return Ok(None);
        }

        // Add new data to buffer if we're already buffering
        if !data.is_empty() {
            if self.buffer.len() + data.len() > self.max_buffer_size {
                warn!(
                    "Buffer overflow: {} + {} > {}",
                    self.buffer.len(),
                    data.len(),
                    self.max_buffer_size
                );
                return Err(ProtocolError::buffer_overflow(format!(
                    "Buffer size {} would exceed maximum {}",
                    self.buffer.len() + data.len(),
                    self.max_buffer_size
                )));
            }

            // If we have buffered data, add to it, otherwise process directly
            if !self.buffer.is_empty() {
                self.buffer.extend_from_slice(data);
            } else {
                // Try to decode the new data directly first
                match String::from_utf8(data.to_vec()) {
                    Ok(response) => {
                        debug!("Decoded {} characters from raw data", response.len());
                        trace!("Decoded response: {response}");

                        if self.is_complete_response(&response) {
                            debug!("Response is complete");
                            self.state = DecoderState::Complete;
                            return Ok(Some(response));
                        } else {
                            debug!("Response incomplete, buffering for more data");
                            // Buffer it for more data
                            self.buffer.extend_from_slice(data);
                            self.state = DecoderState::Buffering {
                                expected_size: None,
                            };
                            return Ok(None);
                        }
                    }
                    Err(e) => {
                        // Check if this is just incomplete UTF-8
                        let error_len = e.utf8_error().error_len();
                        if error_len.is_none() {
                            debug!("Incomplete UTF-8 sequence detected, buffering data");
                            // Incomplete UTF-8 sequence at the end, buffer it
                            self.buffer.extend_from_slice(data);
                            self.state = DecoderState::Buffering {
                                expected_size: None,
                            };
                            return Ok(None);
                        } else {
                            warn!("Invalid UTF-8 sequence: {e}");
                            // Invalid UTF-8
                            return Err(ProtocolError::decoding(format!("Invalid UTF-8: {e}")));
                        }
                    }
                }
            }
        }

        // Process buffered data
        match String::from_utf8(self.buffer.to_vec()) {
            Ok(response) => {
                // Check if we have a complete response
                if self.is_complete_response(&response) {
                    self.buffer.clear();
                    self.state = DecoderState::Complete;
                    Ok(Some(response))
                } else {
                    // Still need more data
                    self.state = DecoderState::Buffering {
                        expected_size: None,
                    };
                    Ok(None)
                }
            }
            Err(e) => {
                // Check if this is just incomplete UTF-8
                let error_len = e.utf8_error().error_len();

                // If error_len is None, it means we have an incomplete sequence at the end
                if error_len.is_none() {
                    // Incomplete UTF-8 sequence at the end, wait for more data
                    self.state = DecoderState::Buffering {
                        expected_size: None,
                    };
                    Ok(None)
                } else {
                    // Invalid UTF-8 in the middle
                    Err(ProtocolError::decoding(format!("Invalid UTF-8: {e}")))
                }
            }
        }
    }

    /// Check if we have a complete response
    fn is_complete_response(&self, response: &str) -> bool {
        // AniDB responses are complete UDP packets
        // A complete response has at least a response code and message
        if response.is_empty() {
            return false;
        }

        // If the response ends with a newline, it's complete
        // This handles single-line error responses like "505 ILLEGAL INPUT OR ACCESS DENIED\n"
        if response.ends_with('\n') {
            return true;
        }

        // Check for valid response format: "{code} {message}"
        let parts: Vec<&str> = response.splitn(2, ' ').collect();
        if parts.is_empty() {
            return false;
        }

        // First part should be a valid response code
        if parts[0].parse::<u16>().is_err() {
            return false;
        }

        // UDP packets are complete by nature - if we have a valid response code
        // and the response is well-formed, it's complete
        // Special case: "200 LOGIN" by itself is incomplete (missing ACCEPTED/FAILED)
        if response == "200 LOGIN" {
            return false;
        }

        true
    }

    /// Reset the decoder state
    pub fn reset(&mut self) {
        self.buffer.clear();
        self.state = DecoderState::Idle;
    }

    /// Check if the decoder is waiting for more data
    pub fn is_waiting_for_data(&self) -> bool {
        matches!(self.state, DecoderState::Buffering { .. })
    }

    /// Get the current buffer size
    pub fn buffer_size(&self) -> usize {
        self.buffer.len()
    }

    /// Get the current decoder state
    pub fn state(&self) -> &DecoderState {
        &self.state
    }

    /// Set maximum buffer size
    pub fn set_max_buffer_size(&mut self, size: usize) {
        self.max_buffer_size = size;
    }
}

impl Default for Decoder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_decode_complete_response() {
        let mut decoder = Decoder::new();

        let response = b"200 LOGIN ACCEPTED\nabc123|1";
        let result = decoder.decode(response).unwrap();

        assert_eq!(result, Some("200 LOGIN ACCEPTED\nabc123|1".to_string()));
        assert_eq!(decoder.state(), &DecoderState::Complete);
        assert_eq!(decoder.buffer_size(), 0); // Buffer should be cleared
    }

    #[test]
    fn test_decode_partial_response() {
        let mut decoder = Decoder::new();

        // First part
        let part1 = b"200 LOGIN";
        let result = decoder.decode(part1).unwrap();
        assert_eq!(result, None);
        assert!(decoder.is_waiting_for_data());

        // Second part
        let part2 = b" ACCEPTED\nabc123";
        let result = decoder.decode(part2).unwrap();
        assert_eq!(result, Some("200 LOGIN ACCEPTED\nabc123".to_string()));
        assert_eq!(decoder.state(), &DecoderState::Complete);
    }

    #[test]
    fn test_decode_empty_input() {
        let mut decoder = Decoder::new();

        let result = decoder.decode(&[]).unwrap();
        assert_eq!(result, None);
        assert_eq!(decoder.state(), &DecoderState::Idle);
    }

    #[test]
    fn test_decode_invalid_utf8() {
        let mut decoder = Decoder::new();

        // Invalid UTF-8 sequence
        let invalid = vec![0xFF, 0xFE, 0xFD];
        let result = decoder.decode(&invalid);
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            ProtocolError::Decoding { .. }
        ));
    }

    #[test]
    fn test_decode_incomplete_utf8() {
        let mut decoder = Decoder::new();

        // Incomplete UTF-8 sequence (first byte of a 2-byte sequence)
        let incomplete = vec![0xC3];
        let result = decoder.decode(&incomplete).unwrap();
        assert_eq!(result, None);
        assert!(decoder.is_waiting_for_data());

        // Complete the sequence
        let completion = vec![0xA9]; // Forms 'é'
        let result = decoder.decode(&completion).unwrap();
        assert_eq!(result, None); // Still not a complete response (no valid code)
    }

    #[test]
    fn test_decode_buffer_overflow() {
        let mut decoder = Decoder::new();
        decoder.set_max_buffer_size(100);

        let large_data = vec![b'A'; 101];
        let result = decoder.decode(&large_data);
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            ProtocolError::BufferOverflow { .. }
        ));
    }

    #[test]
    fn test_decode_reset() {
        let mut decoder = Decoder::new();

        // Add some data
        decoder.decode(b"200 LOGIN").unwrap();
        assert!(decoder.buffer_size() > 0);
        assert!(decoder.is_waiting_for_data());

        // Reset
        decoder.reset();
        assert_eq!(decoder.buffer_size(), 0);
        assert_eq!(decoder.state(), &DecoderState::Idle);
        assert!(!decoder.is_waiting_for_data());
    }

    #[test]
    fn test_is_complete_response() {
        let decoder = Decoder::new();

        assert!(decoder.is_complete_response("200 OK"));
        assert!(decoder.is_complete_response("500"));
        assert!(decoder.is_complete_response("220 FILE\ndata"));
        assert!(decoder.is_complete_response("505 ILLEGAL INPUT OR ACCESS DENIED\n"));
        assert!(decoder.is_complete_response("200 LOGIN ACCEPTED\n"));

        assert!(!decoder.is_complete_response(""));
        assert!(!decoder.is_complete_response("INVALID"));
        assert!(!decoder.is_complete_response("ABC NOT A CODE"));
        assert!(!decoder.is_complete_response("200 LOGIN"));
    }

    #[test]
    fn test_decode_multiline_response() {
        let mut decoder = Decoder::new();

        let response = b"220 FILE\n12345|67890|data1|data2\nmore|fields|here";
        let result = decoder.decode(response).unwrap();

        assert!(result.is_some());
        let decoded = result.unwrap();
        assert!(decoded.contains("220 FILE"));
        assert!(decoded.contains("12345|67890"));
        assert!(decoded.contains("more|fields|here"));
    }

    #[test]
    fn test_decode_response_with_special_chars() {
        let mut decoder = Decoder::new();

        let response = b"230 ANIME\nTitle with spaces|Another field";
        let result = decoder.decode(response).unwrap();

        assert_eq!(
            result,
            Some("230 ANIME\nTitle with spaces|Another field".to_string())
        );
    }

    #[test]
    fn test_decode_single_line_with_newline() {
        let mut decoder = Decoder::new();

        // Test 505 error response
        let response = b"505 ILLEGAL INPUT OR ACCESS DENIED\n";
        let result = decoder.decode(response).unwrap();
        assert_eq!(
            result,
            Some("505 ILLEGAL INPUT OR ACCESS DENIED\n".to_string())
        );
        assert_eq!(decoder.state(), &DecoderState::Complete);

        // Reset and test another single-line response
        decoder.reset();
        let response = b"200 LOGIN ACCEPTED\n";
        let result = decoder.decode(response).unwrap();
        assert_eq!(result, Some("200 LOGIN ACCEPTED\n".to_string()));
        assert_eq!(decoder.state(), &DecoderState::Complete);
    }
}
