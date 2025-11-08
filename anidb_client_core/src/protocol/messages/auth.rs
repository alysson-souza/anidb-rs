//! Authentication-related messages
//!
//! This module contains AUTH and LOGOUT command implementations.

use crate::protocol::error::{ProtocolError, Result};
use crate::protocol::messages::{AniDBCommand, AniDBResponse};
use crate::security::SecureString;
use std::collections::HashMap;

/// AUTH command for authenticating with the AniDB server
#[derive(Clone)]
pub struct AuthCommand {
    /// Username
    pub user: String,
    /// Password (will be hashed)
    pub pass: SecureString,
    /// Protocol version
    pub protover: String,
    /// Client name
    pub client: String,
    /// Client version
    pub clientver: String,
    /// NAT mode (1 if behind NAT)
    pub nat: Option<u8>,
    /// Compression (1 to enable)
    pub comp: Option<u8>,
    /// Encoding (utf8)
    pub enc: Option<String>,
    /// MTU size
    pub mtu: Option<u16>,
    /// Image server (1 to enable)
    pub imgserver: Option<u8>,
}

impl std::fmt::Debug for AuthCommand {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AuthCommand")
            .field("user", &self.user)
            .field("pass", &"***") // Never log passwords
            .field("protover", &self.protover)
            .field("client", &self.client)
            .field("clientver", &self.clientver)
            .field("nat", &self.nat)
            .field("comp", &self.comp)
            .field("enc", &self.enc)
            .field("mtu", &self.mtu)
            .field("imgserver", &self.imgserver)
            .finish()
    }
}

impl AuthCommand {
    /// Create a new AUTH command with required fields
    pub fn new(
        user: String,
        pass: impl Into<SecureString>,
        client: String,
        clientver: String,
    ) -> Self {
        Self {
            user,
            pass: pass.into(),
            protover: crate::protocol::PROTOCOL_VERSION.to_string(),
            client,
            clientver,
            nat: None,
            comp: None,
            enc: Some("utf8".to_string()),
            mtu: None,
            imgserver: None,
        }
    }

    /// Enable NAT mode
    pub fn with_nat(mut self) -> Self {
        self.nat = Some(1);
        self
    }

    /// Enable compression
    pub fn with_compression(mut self) -> Self {
        self.comp = Some(1);
        self
    }

    /// Set MTU size
    pub fn with_mtu(mut self, mtu: u16) -> Self {
        self.mtu = Some(mtu);
        self
    }

    /// Enable image server
    pub fn with_imgserver(mut self) -> Self {
        self.imgserver = Some(1);
        self
    }
}

impl AniDBCommand for AuthCommand {
    fn name(&self) -> &str {
        "AUTH"
    }

    fn parameters(&self) -> HashMap<String, String> {
        let mut params = HashMap::new();
        params.insert("user".to_string(), self.user.clone());
        params.insert("pass".to_string(), self.pass.expose_secret());
        params.insert("protover".to_string(), self.protover.clone());
        params.insert("client".to_string(), self.client.clone());
        params.insert("clientver".to_string(), self.clientver.clone());

        if let Some(nat) = self.nat {
            params.insert("nat".to_string(), nat.to_string());
        }
        if let Some(comp) = self.comp {
            params.insert("comp".to_string(), comp.to_string());
        }
        if let Some(enc) = &self.enc {
            params.insert("enc".to_string(), enc.clone());
        }
        if let Some(mtu) = self.mtu {
            params.insert("mtu".to_string(), mtu.to_string());
        }
        if let Some(imgserver) = self.imgserver {
            params.insert("imgserver".to_string(), imgserver.to_string());
        }

        params
    }

    fn encode(&self) -> Result<String> {
        // Override the default encode to ensure correct parameter order
        // According to API docs: AUTH user={str username}&pass={str password}&protover={int4 apiversion}&client={str clientname}&clientver={int4 clientversion}
        let mut parts = vec!["AUTH".to_string()];

        // Required parameters in the correct order
        parts.push(format!(
            "user={}",
            crate::protocol::messages::encode_value(&self.user)
        ));
        parts.push(format!(
            "pass={}",
            crate::protocol::messages::encode_value(&self.pass.expose_secret())
        ));
        parts.push(format!(
            "protover={}",
            crate::protocol::messages::encode_value(&self.protover)
        ));
        parts.push(format!(
            "client={}",
            crate::protocol::messages::encode_value(&self.client)
        ));
        parts.push(format!(
            "clientver={}",
            crate::protocol::messages::encode_value(&self.clientver)
        ));

        // Optional parameters
        if let Some(nat) = self.nat {
            parts.push(format!("nat={nat}"));
        }
        if let Some(comp) = self.comp {
            parts.push(format!("comp={comp}"));
        }
        if let Some(enc) = &self.enc {
            parts.push(format!(
                "enc={}",
                crate::protocol::messages::encode_value(enc)
            ));
        }
        if let Some(mtu) = self.mtu {
            parts.push(format!("mtu={mtu}"));
        }
        if let Some(imgserver) = self.imgserver {
            parts.push(format!("imgserver={imgserver}"));
        }

        // Join with spaces between command and first param, then & between params
        if parts.len() <= 1 {
            Ok(parts.join(""))
        } else {
            Ok(format!("{} {}", parts[0], parts[1..].join("&")))
        }
    }

    fn requires_auth(&self) -> bool {
        false // AUTH itself doesn't require authentication
    }
}

/// Response to AUTH command
#[derive(Debug, Clone)]
pub struct AuthResponse {
    /// Response code
    pub code: u16,
    /// Response message
    pub message: String,
    /// Session tag (if successful)
    pub session: Option<String>,
    /// New client version available
    pub new_version: Option<String>,
    /// Image server enabled
    pub imgserver: bool,
}

impl AuthResponse {
    /// Parse an AUTH response from raw data
    pub fn parse(code: u16, message: String, fields: Vec<String>) -> Result<Self> {
        let mut response = Self {
            code,
            message: message.clone(),
            session: None,
            new_version: None,
            imgserver: false,
        };

        match code {
            200 => {
                // LOGIN ACCEPTED
                // The session key can be either:
                // 1. In the message: "iQUO2 LOGIN ACCEPTED" (when no tag is used)
                // 2. In the fields: ["session_key", "1"] (when tag is used)

                // Check if session key is in the message
                let msg_parts: Vec<&str> = message.split_whitespace().collect();
                if msg_parts.len() >= 3
                    && msg_parts.contains(&"LOGIN")
                    && msg_parts.contains(&"ACCEPTED")
                {
                    // Session key is the first part of the message
                    response.session = Some(msg_parts[0].to_string());
                    // Check for NAT info (ip:port)
                    if msg_parts.len() >= 4 && msg_parts[1].contains(':') {
                        // NAT format: "{session_key} {ip:port} LOGIN ACCEPTED"
                        response.message = msg_parts[2..].join(" ");
                    } else {
                        // Standard format: "{session_key} LOGIN ACCEPTED"
                        response.message = msg_parts[1..].join(" ");
                    }
                } else if !fields.is_empty() {
                    // Session key is in the fields
                    response.session = Some(fields[0].clone());
                    if fields.len() > 1 && !fields[1].is_empty() {
                        response.imgserver = fields[1] == "1";
                    }
                } else {
                    return Err(ProtocolError::missing_field("session"));
                }
            }
            201 => {
                // LOGIN ACCEPTED - NEW VERSION AVAILABLE
                // Similar to 200, check if session key is in the message first
                let msg_parts: Vec<&str> = message.split_whitespace().collect();
                if msg_parts.len() >= 6 && msg_parts[1] == "LOGIN" && msg_parts[2] == "ACCEPTED" {
                    // Session key is the first part of the message
                    response.session = Some(msg_parts[0].to_string());
                    response.message = msg_parts[1..].join(" ");
                    // Extract version from message if present
                    if msg_parts.len() >= 6
                        && msg_parts[4] == "VERSION"
                        && msg_parts[5] == "AVAILABLE"
                    {
                        // Version might be in fields
                        if !fields.is_empty() {
                            response.new_version = Some(fields[0].clone());
                        }
                    }
                } else if fields.len() >= 2 {
                    // Session key and version are in the fields
                    response.session = Some(fields[0].clone());
                    response.new_version = Some(fields[1].clone());
                    if fields.len() > 2 && !fields[2].is_empty() {
                        response.imgserver = fields[2] == "1";
                    }
                } else {
                    return Err(ProtocolError::missing_field("session or version"));
                }
            }
            500 => {
                // LOGIN FAILED
                response.session = None;
            }
            503 => {
                // CLIENT VERSION OUTDATED
                if !fields.is_empty() {
                    response.new_version = Some(fields[0].clone());
                }
            }
            504 => {
                // CLIENT BANNED - {str reason}
                // The reason is in the message, not in fields
                response.session = None;
            }
            505 => {
                // ILLEGAL INPUT OR ACCESS DENIED
                response.session = None;
            }
            _ => {
                // Other error codes
            }
        }

        Ok(response)
    }
}

impl AniDBResponse for AuthResponse {
    fn code(&self) -> u16 {
        self.code
    }

    fn message(&self) -> &str {
        &self.message
    }

    fn fields(&self) -> &[String] {
        &[] // Fields are parsed into struct members
    }
}

/// LOGOUT command
#[derive(Debug, Clone)]
pub struct LogoutCommand {
    /// Session tag
    pub session: String,
}

impl LogoutCommand {
    /// Create a new LOGOUT command
    pub fn new(session: String) -> Self {
        Self { session }
    }
}

impl AniDBCommand for LogoutCommand {
    fn name(&self) -> &str {
        "LOGOUT"
    }

    fn parameters(&self) -> HashMap<String, String> {
        let mut params = HashMap::new();
        params.insert("s".to_string(), self.session.clone());
        params
    }
}

/// PING command (for testing connectivity)
#[derive(Debug, Clone, Default)]
pub struct PingCommand {
    /// Optional NAT parameter
    pub nat: Option<u8>,
}

impl PingCommand {
    /// Create a new PING command
    pub fn new() -> Self {
        Self::default()
    }

    /// Enable NAT mode
    pub fn with_nat(mut self) -> Self {
        self.nat = Some(1);
        self
    }
}

impl AniDBCommand for PingCommand {
    fn name(&self) -> &str {
        "PING"
    }

    fn parameters(&self) -> HashMap<String, String> {
        let mut params = HashMap::new();
        if let Some(nat) = self.nat {
            params.insert("nat".to_string(), nat.to_string());
        }
        params
    }

    fn requires_auth(&self) -> bool {
        false // PING doesn't require authentication
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_auth_command_basic() {
        let cmd = AuthCommand::new(
            "testuser".to_string(),
            "testpass".to_string(),
            "testclient".to_string(),
            "1".to_string(),
        );

        assert_eq!(cmd.name(), "AUTH");
        assert!(!cmd.requires_auth());

        let params = cmd.parameters();
        assert_eq!(params.get("user").unwrap(), "testuser");
        assert_eq!(params.get("pass").unwrap(), "testpass");
        assert_eq!(
            params.get("protover").unwrap(),
            crate::protocol::PROTOCOL_VERSION
        );
        assert_eq!(params.get("client").unwrap(), "testclient");
        assert_eq!(params.get("clientver").unwrap(), "1");
        assert_eq!(params.get("enc").unwrap(), "utf8");
    }

    #[test]
    fn test_auth_command_with_options() {
        let cmd = AuthCommand::new(
            "user".to_string(),
            "pass".to_string(),
            "client".to_string(),
            "1".to_string(),
        )
        .with_nat()
        .with_compression()
        .with_mtu(1400)
        .with_imgserver();

        let params = cmd.parameters();
        assert_eq!(params.get("nat").unwrap(), "1");
        assert_eq!(params.get("comp").unwrap(), "1");
        assert_eq!(params.get("mtu").unwrap(), "1400");
        assert_eq!(params.get("imgserver").unwrap(), "1");
    }

    #[test]
    fn test_auth_command_encode() {
        let cmd = AuthCommand::new(
            "user".to_string(),
            "pass".to_string(),
            "client".to_string(),
            "1".to_string(),
        );

        let encoded = cmd.encode().unwrap();
        assert_eq!(
            encoded,
            "AUTH user=user&pass=pass&protover=3&client=client&clientver=1&enc=utf8"
        );
    }

    #[test]
    fn test_auth_command_encode_special_characters() {
        let cmd = AuthCommand::new(
            "user@example.com".to_string(),
            "P@ssw0rd!#2024".to_string(),
            "client name".to_string(),
            "1.0".to_string(),
        );

        let encoded = cmd.encode().unwrap();
        assert!(encoded.starts_with("AUTH "));
        // Special characters are NOT URL encoded (except &)
        assert!(encoded.contains("user=user@example.com"));
        assert!(encoded.contains("pass=P@ssw0rd!#2024"));
        assert!(encoded.contains("client=client name"));
        // Verify order
        let user_pos = encoded.find("user=").unwrap();
        let pass_pos = encoded.find("pass=").unwrap();
        let protover_pos = encoded.find("protover=").unwrap();
        let client_pos = encoded.find("client=").unwrap();
        let clientver_pos = encoded.find("clientver=").unwrap();
        assert!(user_pos < pass_pos);
        assert!(pass_pos < protover_pos);
        assert!(protover_pos < client_pos);
        assert!(client_pos < clientver_pos);
    }

    #[test]
    fn test_auth_command_encode_with_ampersand() {
        let cmd = AuthCommand::new(
            "user&name".to_string(),
            "pass&word".to_string(),
            "client&app".to_string(),
            "1".to_string(),
        );

        let encoded = cmd.encode().unwrap();
        // Ampersands should be encoded as &amp;
        assert!(encoded.contains("user=user&amp;name"));
        assert!(encoded.contains("pass=pass&amp;word"));
        assert!(encoded.contains("client=client&amp;app"));
    }

    #[test]
    fn test_auth_response_success() {
        let response = AuthResponse::parse(
            200,
            "LOGIN ACCEPTED".to_string(),
            vec!["abc123def456".to_string(), "1".to_string()],
        )
        .unwrap();

        assert_eq!(response.code(), 200);
        assert_eq!(response.message(), "LOGIN ACCEPTED");
        assert!(response.is_success());
        assert_eq!(response.session, Some("abc123def456".to_string()));
        assert!(response.imgserver);
        assert_eq!(response.new_version, None);
    }

    #[test]
    fn test_auth_response_success_no_tag() {
        // Test format when no tag is used in request
        let response =
            AuthResponse::parse(200, "iQUO2 LOGIN ACCEPTED".to_string(), vec![]).unwrap();

        assert_eq!(response.code(), 200);
        assert_eq!(response.message(), "LOGIN ACCEPTED");
        assert!(response.is_success());
        assert_eq!(response.session, Some("iQUO2".to_string()));
        assert!(!response.imgserver);
        assert_eq!(response.new_version, None);
    }

    #[test]
    fn test_auth_response_success_with_nat() {
        // Test format with NAT info
        let response = AuthResponse::parse(
            200,
            "abc123 192.168.1.1:9000 LOGIN ACCEPTED".to_string(),
            vec![],
        )
        .unwrap();

        assert_eq!(response.code(), 200);
        assert_eq!(response.message(), "LOGIN ACCEPTED");
        assert!(response.is_success());
        assert_eq!(response.session, Some("abc123".to_string()));
        assert!(!response.imgserver);
    }

    #[test]
    fn test_auth_response_new_version() {
        let response = AuthResponse::parse(
            201,
            "LOGIN ACCEPTED - NEW VERSION AVAILABLE".to_string(),
            vec!["session123".to_string(), "2.0".to_string(), "0".to_string()],
        )
        .unwrap();

        assert_eq!(response.code(), 201);
        assert!(response.is_success());
        assert_eq!(response.session, Some("session123".to_string()));
        assert_eq!(response.new_version, Some("2.0".to_string()));
        assert!(!response.imgserver);
    }

    #[test]
    fn test_auth_response_failed() {
        let response = AuthResponse::parse(500, "LOGIN FAILED".to_string(), vec![]).unwrap();

        assert_eq!(response.code(), 500);
        assert!(!response.is_success());
        assert!(response.is_error());
        assert_eq!(response.session, None);
    }

    #[test]
    fn test_auth_response_client_banned() {
        let response =
            AuthResponse::parse(504, "CLIENT BANNED - Reason for ban".to_string(), vec![]).unwrap();

        assert_eq!(response.code(), 504);
        assert!(!response.is_success());
        assert!(response.is_error());
        assert_eq!(response.session, None);
        assert_eq!(response.message(), "CLIENT BANNED - Reason for ban");
    }

    #[test]
    fn test_auth_response_illegal_input() {
        let response =
            AuthResponse::parse(505, "ILLEGAL INPUT OR ACCESS DENIED".to_string(), vec![]).unwrap();

        assert_eq!(response.code(), 505);
        assert!(!response.is_success());
        assert!(response.is_error());
        assert_eq!(response.session, None);
        assert_eq!(response.message(), "ILLEGAL INPUT OR ACCESS DENIED");
    }

    #[test]
    fn test_logout_command() {
        let cmd = LogoutCommand::new("session123".to_string());

        assert_eq!(cmd.name(), "LOGOUT");
        assert!(cmd.requires_auth());

        let params = cmd.parameters();
        assert_eq!(params.get("s").unwrap(), "session123");
    }

    #[test]
    fn test_ping_command() {
        let cmd = PingCommand::new();
        assert_eq!(cmd.name(), "PING");
        assert!(!cmd.requires_auth());
        assert!(cmd.parameters().is_empty());

        let cmd_nat = PingCommand::new().with_nat();
        let params = cmd_nat.parameters();
        assert_eq!(params.get("nat").unwrap(), "1");
    }
}
