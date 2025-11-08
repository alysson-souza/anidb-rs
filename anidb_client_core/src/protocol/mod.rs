//! AniDB UDP Protocol Implementation
//!
//! This module implements the AniDB UDP API protocol with a modular architecture:
//! - `transport`: Low-level UDP socket management and connection state
//! - `codec`: Message encoding/decoding with packet fragmentation support
//! - `messages`: Type-safe message definitions and builders
//! - `client`: High-level protocol client with rate limiting and retry logic

pub mod client;
pub mod codec;
pub mod error;
pub mod messages;
pub mod transport;

// Re-export main types
pub use client::{ProtocolClient, ProtocolConfig};
pub use error::{ProtocolError, Result};
pub use messages::{Command, Response};
pub use transport::{ConnectionState, Transport};

/// Protocol version supported by this implementation
pub const PROTOCOL_VERSION: &str = "3";

/// Maximum UDP packet size (considering PPPoE)
pub const MAX_PACKET_SIZE: usize = 1400;

/// Default AniDB server address
pub const DEFAULT_SERVER: &str = "api.anidb.net";

/// Default AniDB UDP port
pub const DEFAULT_PORT: u16 = 9000;

/// Session timeout in seconds (30 minutes)
pub const SESSION_TIMEOUT_SECS: u64 = 1800;

/// Rate limit: maximum requests per second (0.5 req/sec = 1 req per 2 seconds)
pub const RATE_LIMIT_REQUESTS_PER_SECOND: f64 = 0.5;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_protocol_constants() {
        assert_eq!(PROTOCOL_VERSION, "3");
        assert_eq!(MAX_PACKET_SIZE, 1400);
        assert_eq!(DEFAULT_SERVER, "api.anidb.net");
        assert_eq!(DEFAULT_PORT, 9000);
        assert_eq!(SESSION_TIMEOUT_SECS, 1800);
        assert_eq!(RATE_LIMIT_REQUESTS_PER_SECOND, 0.5);
    }
}
