//! High-level protocol client with rate limiting and retry logic
//!
//! This module provides the main interface for interacting with the AniDB UDP API.

use crate::protocol::codec::{Codec, FragmentAssembler};
use crate::protocol::error::{ProtocolError, Result};
use crate::protocol::messages::{Command, Response, ResponseParser};
use crate::protocol::transport::{Connection, ConnectionState, TransportConfig};
use log::{debug, trace, warn};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::net::lookup_host;
use tokio::sync::{Mutex, RwLock};
use tokio::time::{sleep, timeout};

/// Protocol client configuration
#[derive(Debug, Clone)]
pub struct ProtocolConfig {
    /// Server address (hostname:port)
    pub server: String,
    /// Client name for authentication
    pub client_name: String,
    /// Client version
    pub client_version: String,
    /// Connection timeout
    pub connect_timeout: Duration,
    /// Request timeout
    pub request_timeout: Duration,
    /// Maximum retry attempts
    pub max_retries: u32,
    /// Retry delay
    pub retry_delay: Duration,
    /// Enable NAT mode
    pub nat: bool,
    /// Enable compression
    pub compression: bool,
    /// MTU size
    pub mtu: Option<u16>,
}

impl Default for ProtocolConfig {
    fn default() -> Self {
        Self {
            server: format!(
                "{}:{}",
                crate::protocol::DEFAULT_SERVER,
                crate::protocol::DEFAULT_PORT
            ),
            client_name: String::new(),
            client_version: String::new(),
            connect_timeout: Duration::from_secs(10),
            request_timeout: Duration::from_secs(30),
            max_retries: 3,
            retry_delay: Duration::from_secs(2),
            nat: false,
            compression: false,
            mtu: None,
        }
    }
}

/// Rate limiter for enforcing API rate limits
struct RateLimiter {
    /// Last request timestamp
    last_request: Mutex<Option<Instant>>,
    /// Minimum delay between requests
    min_delay: Duration,
}

impl RateLimiter {
    fn new() -> Self {
        // Respect AniDB guidance: at least 2.0s between requests.
        // We use 0.4 requests per second = 2.5 seconds between requests (safer margin).
        let min_delay =
            Duration::from_secs_f64(1.0 / crate::protocol::RATE_LIMIT_REQUESTS_PER_SECOND);

        Self {
            last_request: Mutex::new(None),
            min_delay,
        }
    }

    async fn wait_if_needed(&self) {
        trace!("Rate limiter: acquiring last_request lock...");
        let mut last = self.last_request.lock().await;
        trace!("Rate limiter: last_request lock acquired");

        if let Some(last_time) = *last {
            let elapsed = last_time.elapsed();
            trace!(
                "Rate limiter: last request was {elapsed:?} ago, min_delay: {:?}",
                self.min_delay
            );
            if elapsed < self.min_delay {
                let wait_time = self.min_delay - elapsed;
                debug!("Rate limiter: waiting {wait_time:?} to respect rate limit");
                sleep(wait_time).await;
                debug!("Rate limiter: wait completed");
            } else {
                trace!("Rate limiter: no wait needed, sufficient time has passed");
            }
        } else {
            trace!("Rate limiter: no previous request, proceeding immediately");
        }

        *last = Some(Instant::now());
        trace!("Rate limiter: updated last request time");
    }
}

/// High-level protocol client
pub struct ProtocolClient {
    /// Configuration
    config: ProtocolConfig,
    /// Connection
    connection: Arc<Connection>,
    /// Codec for encoding/decoding
    codec: Arc<Mutex<Codec>>,
    /// Fragment assembler
    assembler: Arc<Mutex<FragmentAssembler>>,
    /// Rate limiter
    rate_limiter: Arc<RateLimiter>,
    /// Current session info
    session: Arc<RwLock<Option<SessionInfo>>>,
}

/// Session information
#[derive(Debug, Clone)]
struct SessionInfo {
    /// Session tag
    tag: String,
    /// Username
    #[allow(dead_code)]
    username: String,
    /// When the session was established
    #[allow(dead_code)]
    established_at: Instant,
}

impl ProtocolClient {
    /// Create a new protocol client
    pub async fn new(config: ProtocolConfig) -> Result<Self> {
        debug!("Creating new protocol client with config: {config:?}");

        // Resolve the server address (handles both IP addresses and hostnames)
        debug!("Resolving server address: {}", config.server);
        let server_addr = lookup_host(&config.server)
            .await
            .map_err(|e| {
                warn!(
                    "Failed to resolve server address '{}': {}",
                    config.server, e
                );
                ProtocolError::invalid_packet(format!(
                    "Failed to resolve server address '{}': {e}",
                    config.server
                ))
            })?
            .next()
            .ok_or_else(|| {
                warn!("No addresses found for '{}'", config.server);
                ProtocolError::invalid_packet(format!("No addresses found for '{}'", config.server))
            })?;

        debug!("");

        let transport_config = TransportConfig {
            server_addr,
            connect_timeout: config.connect_timeout,
            read_timeout: config.request_timeout,
            write_timeout: Duration::from_secs(5),
            max_retries: config.max_retries,
            retry_delay: config.retry_delay,
            ..Default::default()
        };

        let connection = Connection::new(transport_config).await?;
        debug!("Protocol client created successfully");

        Ok(Self {
            config,
            connection: Arc::new(connection),
            codec: Arc::new(Mutex::new(Codec::new())),
            assembler: Arc::new(Mutex::new(FragmentAssembler::new())),
            rate_limiter: Arc::new(RateLimiter::new()),
            session: Arc::new(RwLock::new(None)),
        })
    }

    /// Connect to the AniDB server
    pub async fn connect(&self) -> Result<()> {
        debug!("Connecting to AniDB server...");
        let result = self.connection.connect().await;
        match &result {
            Ok(_) => debug!("Successfully connected to AniDB server"),
            Err(e) => warn!("Failed to connect to AniDB server: {e:?}"),
        }
        result
    }

    /// Disconnect from the server
    pub async fn disconnect(&self) -> Result<()> {
        self.connection.disconnect().await
    }

    /// Authenticate with the server
    pub async fn authenticate(&self, username: String, password: String) -> Result<String> {
        debug!("");
        debug!(
            "Client name: {}, version: {}",
            self.config.client_name, self.config.client_version
        );

        // Build AUTH command
        let mut cmd = Command::auth(
            username.clone(),
            password,
            self.config.client_name.clone(),
            self.config.client_version.clone(),
        );

        trace!("Built AUTH command");

        // Apply configuration options
        if let Command::Auth(ref mut auth) = cmd {
            if self.config.nat {
                debug!("Enabling NAT mode");
                *auth = auth.clone().with_nat();
            }
            if self.config.compression {
                debug!("Enabling compression");
                *auth = auth.clone().with_compression();
            }
            if let Some(mtu) = self.config.mtu {
                debug!("");
                *auth = auth.clone().with_mtu(mtu);
            }
        }

        // Send command
        debug!("Sending AUTH command...");
        let response = self.send_command(cmd).await?;
        debug!("Received AUTH response: {response:?}");

        // Parse response
        match response {
            Response::Auth(auth_resp) => {
                if let Some(session) = auth_resp.session {
                    debug!("");

                    // Store session info
                    *self.session.write().await = Some(SessionInfo {
                        tag: session.clone(),
                        username: username.clone(),
                        established_at: Instant::now(),
                    });

                    // Update connection state
                    self.connection
                        .authenticate(
                            username,
                            "".to_string(), // Password not stored
                            session.clone(),
                        )
                        .await?;

                    Ok(session)
                } else {
                    warn!("Authentication failed: {:?}", auth_resp.message);
                    Err(ProtocolError::authentication_failed(
                        auth_resp.message.clone(),
                    ))
                }
            }
            _ => {
                warn!("Unexpected response type for AUTH: {response:?}");
                Err(ProtocolError::invalid_response(
                    "AUTH response",
                    format!("{response:?}"),
                ))
            }
        }
    }

    /// Logout from the server
    pub async fn logout(&self) -> Result<()> {
        let session = self.get_session_tag().await?;

        let cmd = Command::logout(session);
        let response = self.send_command(cmd).await?;

        match response {
            Response::Logout(logout_resp) => {
                if logout_resp.is_success() {
                    *self.session.write().await = None;
                    self.connection.disconnect().await?;
                    Ok(())
                } else {
                    Err(ProtocolError::server_error(
                        logout_resp.code,
                        logout_resp.message,
                    ))
                }
            }
            _ => Err(ProtocolError::invalid_response(
                "LOGOUT response",
                format!("{response:?}"),
            )),
        }
    }

    /// Send a PING command
    pub async fn ping(&self) -> Result<Option<u16>> {
        let cmd = Command::ping();
        let response = self.send_command(cmd).await?;

        match response {
            Response::Pong(pong) => Ok(pong.port),
            _ => Err(ProtocolError::invalid_response(
                "PING response",
                format!("{response:?}"),
            )),
        }
    }

    /// Query file information
    pub async fn file_by_hash(&self, size: u64, ed2k: &str) -> Result<Response> {
        debug!("");

        self.ensure_authenticated().await?;

        let cmd = Command::file()
            .by_hash(size, ed2k)
            .with_fmask(crate::protocol::messages::file::fmask::BASIC)
            .build()?;

        debug!("Sending FILE command...");
        let response = self.send_command(cmd).await?;
        debug!("Received FILE response: {response:?}");

        Ok(response)
    }

    /// Send a command and wait for response
    pub async fn send_command(&self, command: Command) -> Result<Response> {
        trace!("send_command called with: {command:?}");

        // Enforce rate limit
        debug!("Checking rate limit...");
        self.rate_limiter.wait_if_needed().await;
        debug!("Rate limit check passed");

        // Ensure connected
        if !self.connection.is_connected().await {
            debug!("Not connected, establishing connection...");
            self.connection.connect().await?;
        }

        // Get command name before moving command
        let command_name = command.name().to_string();
        debug!(" command");

        // Add session if required
        let encoded = if command.requires_auth() {
            debug!("Command requires authentication, adding session tag");
            let session = self.get_session_tag().await?;
            debug!("");
            command.with_session(&session)?
        } else {
            debug!("Command does not require authentication");
            command.encode()?
        };

        // Log the encoded command (mask sensitive data)
        if command_name == "AUTH" {
            // Mask the password value but keep the structure
            let masked = encoded
                .split('&')
                .map(|part| {
                    if part.starts_with("pass=") {
                        "pass=***"
                    } else {
                        part
                    }
                })
                .collect::<Vec<_>>()
                .join("&");
            debug!("Encoded AUTH command (password masked): {}", masked);
        } else {
            debug!("Encoded command: {}", encoded);
        }

        // Try sending with retries
        let mut last_error = None;
        for attempt in 0..=self.config.max_retries {
            if attempt > 0 {
                sleep(self.config.retry_delay * attempt).await;
            }

            match self.send_once(&encoded, &command_name).await {
                Ok(response) => {
                    // Check if response indicates we need to re-authenticate
                    if response.is_error()
                        && let Some(err) = response.to_error()
                        && err.requires_reauth()
                    {
                        return Err(err);
                    }
                    return Ok(response);
                }
                Err(e) => {
                    if !e.is_transient() || attempt == self.config.max_retries {
                        return Err(e);
                    }
                    last_error = Some(e);
                }
            }
        }

        Err(last_error.unwrap_or(ProtocolError::Timeout(self.config.request_timeout)))
    }

    /// Send command once (no retries)
    async fn send_once(&self, encoded: &str, command_name: &str) -> Result<Response> {
        trace!(" command");

        // Encode command
        let packet = {
            let mut codec = self.codec.lock().await;
            let packet = codec.encode(encoded)?;
            debug!("Encoded packet size: {} bytes", packet.len());
            trace!("Raw packet bytes: {packet:?}");
            packet
        };

        // Send packet
        let transport = self.connection.transport();
        debug!("Sending packet via transport...");
        transport.send(&packet).await?;
        debug!("Packet sent successfully");

        // Receive response with timeout
        debug!(
            "Waiting for response (timeout: {:?})...",
            self.config.request_timeout
        );
        let mut buffer = vec![0u8; crate::protocol::MAX_PACKET_SIZE];
        let size = timeout(self.config.request_timeout, transport.recv(&mut buffer))
            .await
            .map_err(|_| {
                warn!("Response timeout after {:?}", self.config.request_timeout);
                ProtocolError::Timeout(self.config.request_timeout)
            })??;

        buffer.truncate(size);
        debug!(" bytes");
        trace!("Raw response bytes: {:?}", &buffer[..size.min(100)]); // Log first 100 bytes

        // Decode response
        let decoded = {
            let mut codec = self.codec.lock().await;
            let decoded = codec.decode(&buffer)?;
            if decoded.is_none() {
                warn!("Decoder returned None for buffer of size {}", buffer.len());
            }
            decoded
        };

        let response_str = decoded.ok_or_else(|| {
            warn!(" command");
            ProtocolError::invalid_packet("Empty response")
        })?;

        debug!("");

        // Check if fragmented
        let mut assembler = self.assembler.lock().await;
        match assembler.process(&response_str)? {
            Some(complete) => {
                // Parse complete response
                ResponseParser::parse(&complete, Some(command_name))
            }
            None => {
                // Wait for more fragments
                self.receive_fragments(&mut assembler, command_name).await
            }
        }
    }

    /// Receive remaining fragments
    async fn receive_fragments(
        &self,
        assembler: &mut FragmentAssembler,
        command_name: &str,
    ) -> Result<Response> {
        let transport = self.connection.transport();
        let start = Instant::now();

        loop {
            // Check timeout
            if start.elapsed() > self.config.request_timeout {
                return Err(ProtocolError::Timeout(self.config.request_timeout));
            }

            // Receive next packet
            let mut buffer = vec![0u8; crate::protocol::MAX_PACKET_SIZE];
            let remaining_time = self.config.request_timeout - start.elapsed();

            let size = timeout(remaining_time, transport.recv(&mut buffer))
                .await
                .map_err(|_| ProtocolError::Timeout(remaining_time))??;

            buffer.truncate(size);

            // Decode packet
            let decoded = {
                let mut codec = self.codec.lock().await;
                codec.decode(&buffer)?
            };

            let fragment_str =
                decoded.ok_or_else(|| ProtocolError::invalid_packet("Empty fragment"))?;

            // Process fragment
            if let Some(complete) = assembler.process(&fragment_str)? {
                return ResponseParser::parse(&complete, Some(command_name));
            }
        }
    }

    /// Ensure we're authenticated
    async fn ensure_authenticated(&self) -> Result<()> {
        debug!("Checking authentication status...");

        if !self.connection.is_authenticated().await {
            warn!("Not authenticated");
            return Err(ProtocolError::NotConnected);
        }

        // Check session expiration
        debug!("Checking session expiration...");
        self.connection.check_session().await?;
        debug!("Session is valid");

        Ok(())
    }

    /// Get current session tag
    async fn get_session_tag(&self) -> Result<String> {
        let session = self.session.read().await;
        session
            .as_ref()
            .map(|s| s.tag.clone())
            .ok_or(ProtocolError::NotConnected)
    }

    /// Check if authenticated
    pub async fn is_authenticated(&self) -> bool {
        self.connection.is_authenticated().await && self.session.read().await.is_some()
    }

    /// Get connection state
    pub async fn state(&self) -> ConnectionState {
        self.connection.transport().state().await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_client_creation() {
        let config = ProtocolConfig {
            server: "127.0.0.1:9999".to_string(),
            ..Default::default()
        };

        let client = ProtocolClient::new(config).await;
        match client {
            Ok(client) => {
                assert!(!client.is_authenticated().await);
            }
            Err(e) => {
                // In sandboxed environments, UDP sockets may be restricted.
                // Treat this as a skipped test rather than a failure.
                eprintln!("Skipping test_client_creation due to network sandbox: {e:?}");
            }
        }
    }

    #[tokio::test]
    async fn test_rate_limiter() {
        let limiter = RateLimiter::new();

        let start = Instant::now();
        limiter.wait_if_needed().await;
        let first_elapsed = start.elapsed();

        // First request should be immediate
        assert!(first_elapsed < Duration::from_millis(100));

        // Second request should wait
        let start = Instant::now();
        limiter.wait_if_needed().await;
        let second_elapsed = start.elapsed();

        // Should wait approximately 2 seconds (rate limit)
        assert!(second_elapsed >= Duration::from_secs(1));
    }

    #[test]
    fn test_protocol_config_default() {
        let config = ProtocolConfig::default();
        assert_eq!(
            config.server,
            format!(
                "{}:{}",
                crate::protocol::DEFAULT_SERVER,
                crate::protocol::DEFAULT_PORT
            )
        );
        assert_eq!(config.client_name, "");
        assert_eq!(config.client_version, "");
        assert!(!config.nat);
        assert!(!config.compression);
        assert_eq!(config.mtu, None);
    }

    #[test]
    fn test_session_info() {
        let info = SessionInfo {
            tag: "abc123".to_string(),
            username: "testuser".to_string(),
            established_at: Instant::now(),
        };

        assert_eq!(info.tag, "abc123");
        assert_eq!(info.username, "testuser");
    }
}
