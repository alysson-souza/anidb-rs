//! Command type enumeration and builder
//!
//! This module provides a unified interface for all AniDB commands.

use crate::protocol::error::Result;
use crate::protocol::messages::{
    AniDBCommand,
    anime::AnimeCommand,
    auth::{AuthCommand, LogoutCommand, PingCommand},
    episode::EpisodeCommand,
    file::{FileCommand, FileCommandBuilder},
    group::GroupCommand,
    mylist::{MyListAddCommand, MyListDelCommand},
};
use std::collections::HashMap;

/// Enumeration of all supported AniDB commands
#[derive(Debug, Clone)]
pub enum Command {
    /// Authentication command
    Auth(AuthCommand),
    /// Logout command
    Logout(LogoutCommand),
    /// Ping command
    Ping(PingCommand),
    /// File query command
    File(FileCommand),
    /// Anime query command
    Anime(AnimeCommand),
    /// Episode query command
    Episode(EpisodeCommand),
    /// Group query command
    Group(GroupCommand),
    /// MyList add command
    MyListAdd(MyListAddCommand),
    /// MyList delete command
    MyListDel(MyListDelCommand),
    /// Generic command (for future extensions)
    Generic {
        name: String,
        params: HashMap<String, String>,
        requires_auth: bool,
    },
}

impl Command {
    /// Create an AUTH command
    pub fn auth(user: String, pass: String, client: String, clientver: String) -> Self {
        Command::Auth(AuthCommand::new(user, pass, client, clientver))
    }

    /// Create a LOGOUT command
    pub fn logout(session: String) -> Self {
        Command::Logout(LogoutCommand::new(session))
    }

    /// Create a PING command
    pub fn ping() -> Self {
        Command::Ping(PingCommand::new())
    }

    /// Create a FILE command builder
    pub fn file() -> FileCommandBuilder {
        FileCommandBuilder::new()
    }

    /// Create an ANIME command with appropriate amask for title, year, and type
    pub fn anime(aid: u64) -> Command {
        // Use the same amask as in the AniDB documentation example
        // This amask (b2f0e0fc000000) returns fields in this order:
        // aid, year, type, categories, romaji_name, kanji_name, english_name, ...
        // Our parsing expects aid(0), year(1), type(2), title(3) which matches
        Command::Anime(AnimeCommand::new(aid).with_amask("b2f0e0fc000000"))
    }

    /// Create an EPISODE command
    pub fn episode(eid: u64) -> Command {
        Command::Episode(EpisodeCommand::new(eid))
    }

    /// Create a GROUP command
    pub fn group(gid: u64) -> Command {
        Command::Group(GroupCommand::new(gid))
    }

    /// Get the command name
    pub fn name(&self) -> &str {
        match self {
            Command::Auth(cmd) => cmd.name(),
            Command::Logout(cmd) => cmd.name(),
            Command::Ping(cmd) => cmd.name(),
            Command::File(cmd) => cmd.name(),
            Command::Anime(cmd) => cmd.name(),
            Command::Episode(cmd) => cmd.name(),
            Command::Group(cmd) => cmd.name(),
            Command::MyListAdd(cmd) => cmd.name(),
            Command::MyListDel(cmd) => cmd.name(),
            Command::Generic { name, .. } => name,
        }
    }

    /// Check if the command requires authentication
    pub fn requires_auth(&self) -> bool {
        match self {
            Command::Auth(cmd) => cmd.requires_auth(),
            Command::Logout(cmd) => cmd.requires_auth(),
            Command::Ping(cmd) => cmd.requires_auth(),
            Command::File(cmd) => cmd.requires_auth(),
            Command::Anime(cmd) => cmd.requires_auth(),
            Command::Episode(cmd) => cmd.requires_auth(),
            Command::Group(cmd) => cmd.requires_auth(),
            Command::MyListAdd(cmd) => cmd.requires_auth(),
            Command::MyListDel(cmd) => cmd.requires_auth(),
            Command::Generic { requires_auth, .. } => *requires_auth,
        }
    }

    /// Encode the command for transmission
    pub fn encode(&self) -> Result<String> {
        match self {
            Command::Auth(cmd) => cmd.encode(),
            Command::Logout(cmd) => cmd.encode(),
            Command::Ping(cmd) => cmd.encode(),
            Command::File(cmd) => cmd.encode(),
            Command::Anime(cmd) => cmd.encode(),
            Command::Episode(cmd) => cmd.encode(),
            Command::Group(cmd) => cmd.encode(),
            Command::MyListAdd(cmd) => cmd.encode(),
            Command::MyListDel(cmd) => cmd.encode(),
            Command::Generic { name, params, .. } => {
                let mut parts = vec![name.clone()];
                for (key, value) in params {
                    parts.push(format!(
                        "{key}={}",
                        crate::protocol::messages::encode_value(value)
                    ));
                }
                // Join with space between command and first param, then & between params
                if parts.len() <= 1 {
                    Ok(parts.join(""))
                } else {
                    Ok(format!("{} {}", parts[0], parts[1..].join("&")))
                }
            }
        }
    }

    /// Add session tag to the command if it requires authentication
    pub fn with_session(self, session: &str) -> Result<String> {
        let mut encoded = self.encode()?;

        if self.requires_auth() && !matches!(self, Command::Logout(_)) {
            // Logout already includes session in its parameters
            // The session key should be appended as a parameter with & separator
            encoded.push_str(&format!("&s={session}"));
        }

        Ok(encoded)
    }
}

/// Generic command builder for extensibility
pub struct CommandBuilder {
    name: String,
    params: HashMap<String, String>,
    requires_auth: bool,
}

impl CommandBuilder {
    /// Create a new command builder
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            params: HashMap::new(),
            requires_auth: true,
        }
    }

    /// Set whether the command requires authentication
    pub fn requires_auth(mut self, requires: bool) -> Self {
        self.requires_auth = requires;
        self
    }

    /// Add a parameter
    pub fn param(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.params.insert(key.into(), value.into());
        self
    }

    /// Build the command
    pub fn build(self) -> Command {
        Command::Generic {
            name: self.name,
            params: self.params,
            requires_auth: self.requires_auth,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_command_auth() {
        let cmd = Command::auth(
            "user".to_string(),
            "pass".to_string(),
            "client".to_string(),
            "1".to_string(),
        );

        assert_eq!(cmd.name(), "AUTH");
        assert!(!cmd.requires_auth());
    }

    #[test]
    fn test_command_logout() {
        let cmd = Command::logout("session123".to_string());
        assert_eq!(cmd.name(), "LOGOUT");
        assert!(cmd.requires_auth());
    }

    #[test]
    fn test_command_ping() {
        let cmd = Command::ping();
        assert_eq!(cmd.name(), "PING");
        assert!(!cmd.requires_auth());
    }

    #[test]
    fn test_file_command_builder_by_id() {
        let cmd = Command::file()
            .by_id(12345)
            .with_fmask("7FF8FEF8")
            .build()
            .unwrap();

        assert_eq!(cmd.name(), "FILE");
        assert!(cmd.requires_auth());
    }

    #[test]
    fn test_file_command_builder_by_hash() {
        let cmd = Command::file()
            .by_hash(1234567890, "abcdef0123456789")
            .with_amask("C0F0F0C0")
            .build()
            .unwrap();

        assert_eq!(cmd.name(), "FILE");
        assert!(cmd.requires_auth());
    }

    #[test]
    fn test_file_command_builder_missing_params() {
        let result = Command::file().build();
        assert!(result.is_err());
    }

    #[test]
    fn test_command_with_session() {
        let cmd = Command::file().by_id(12345).build().unwrap();

        let encoded = cmd.with_session("abc123").unwrap();
        assert!(encoded.contains("s=abc123"));
    }

    #[test]
    fn test_generic_command_builder() {
        let cmd = CommandBuilder::new("MYLIST")
            .param("lid", "123")
            .param("edit", "1")
            .requires_auth(true)
            .build();

        assert_eq!(cmd.name(), "MYLIST");
        assert!(cmd.requires_auth());

        let encoded = cmd.encode().unwrap();
        assert!(encoded.contains("MYLIST"));
        assert!(encoded.contains("lid=123"));
        assert!(encoded.contains("edit=1"));
    }

    #[test]
    fn test_command_encoding() {
        let cmd = Command::auth(
            "test user".to_string(),
            "test&pass".to_string(),
            "client".to_string(),
            "1".to_string(),
        );

        let encoded = cmd.encode().unwrap();
        assert!(encoded.contains("user=test user")); // Space is not encoded
        assert!(encoded.contains("pass=test&amp;pass"));
    }
}
