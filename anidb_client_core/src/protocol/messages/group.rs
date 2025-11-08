//! GROUP command for AniDB protocol
//!
//! This module implements the GROUP command to fetch group information.

use crate::protocol::error::{ProtocolError, Result};
use crate::protocol::messages::{AniDBCommand, AniDBResponse};
use std::collections::HashMap;

/// GROUP command for querying group information
#[derive(Debug, Clone)]
pub struct GroupCommand {
    /// Group ID to query
    gid: u64,
}

impl GroupCommand {
    /// Create a new GROUP command
    pub fn new(gid: u64) -> Self {
        Self { gid }
    }
}

impl AniDBCommand for GroupCommand {
    fn name(&self) -> &str {
        "GROUP"
    }

    fn parameters(&self) -> HashMap<String, String> {
        let mut params = HashMap::new();
        params.insert("gid".to_string(), self.gid.to_string());
        params
    }
}

/// Response to GROUP command
#[derive(Debug, Clone)]
pub struct GroupResponse {
    code: u16,
    message: String,
    fields: Vec<String>,
    // Parsed fields
    pub gid: Option<u64>,
    pub rating: Option<f32>,
    pub votes: Option<u32>,
    pub anime_count: Option<u32>,
    pub file_count: Option<u32>,
    pub name: Option<String>,
    pub short_name: Option<String>,
    pub irc_channel: Option<String>,
    pub irc_server: Option<String>,
    pub url: Option<String>,
    pub picture: Option<String>,
}

impl GroupResponse {
    /// Create a new group response
    pub fn new(code: u16, message: String, fields: Vec<String>) -> Result<Self> {
        let mut response = Self {
            code,
            message,
            fields: fields.clone(),
            gid: None,
            rating: None,
            votes: None,
            anime_count: None,
            file_count: None,
            name: None,
            short_name: None,
            irc_channel: None,
            irc_server: None,
            url: None,
            picture: None,
        };

        // Parse fields if we have data
        if response.is_success() && !fields.is_empty() {
            response.parse_fields(&fields)?;
        }

        Ok(response)
    }

    /// Parse response fields
    fn parse_fields(&mut self, fields: &[String]) -> Result<()> {
        // Expected fields: gid|rating|votes|anime_count|file_count|name|short_name|irc_channel|irc_server|url|picture
        if fields.len() >= 6 {
            // Parse GID
            if let Ok(gid) = fields[0].parse::<u64>() {
                self.gid = Some(gid);
            }

            // Parse rating
            if !fields[1].is_empty()
                && let Ok(rating) = fields[1].parse::<f32>()
            {
                self.rating = Some(rating / 100.0); // AniDB ratings are scaled by 100
            }

            // Parse votes
            if !fields[2].is_empty()
                && let Ok(votes) = fields[2].parse::<u32>()
            {
                self.votes = Some(votes);
            }

            // Parse anime count
            if !fields[3].is_empty()
                && let Ok(count) = fields[3].parse::<u32>()
            {
                self.anime_count = Some(count);
            }

            // Parse file count
            if !fields[4].is_empty()
                && let Ok(count) = fields[4].parse::<u32>()
            {
                self.file_count = Some(count);
            }

            // Parse name
            if !fields[5].is_empty() {
                self.name = Some(fields[5].clone());
            }

            // Parse short name
            if fields.len() > 6 && !fields[6].is_empty() {
                self.short_name = Some(fields[6].clone());
            }

            // Parse IRC channel
            if fields.len() > 7 && !fields[7].is_empty() {
                self.irc_channel = Some(fields[7].clone());
            }

            // Parse IRC server
            if fields.len() > 8 && !fields[8].is_empty() {
                self.irc_server = Some(fields[8].clone());
            }

            // Parse URL
            if fields.len() > 9 && !fields[9].is_empty() {
                self.url = Some(fields[9].clone());
            }

            // Parse picture
            if fields.len() > 10 && !fields[10].is_empty() {
                self.picture = Some(fields[10].clone());
            }
        }

        Ok(())
    }

    /// Check if group was found
    pub fn found(&self) -> bool {
        self.is_success() && self.gid.is_some()
    }
}

impl AniDBResponse for GroupResponse {
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

/// Builder for GROUP command
pub struct GroupCommandBuilder {
    gid: Option<u64>,
}

impl GroupCommandBuilder {
    /// Create a new group command builder
    pub fn new() -> Self {
        Self { gid: None }
    }

    /// Set the group ID
    pub fn gid(mut self, gid: u64) -> Self {
        self.gid = Some(gid);
        self
    }

    /// Build the command
    pub fn build(self) -> Result<GroupCommand> {
        let gid = self.gid.ok_or_else(|| {
            ProtocolError::invalid_packet("Group ID is required for GROUP command")
        })?;

        Ok(GroupCommand::new(gid))
    }
}

impl Default for GroupCommandBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_group_command_creation() {
        let cmd = GroupCommand::new(16539);
        assert_eq!(cmd.name(), "GROUP");

        let params = cmd.parameters();
        assert_eq!(params.get("gid"), Some(&"16539".to_string()));
    }

    #[test]
    fn test_group_command_builder() {
        let cmd = GroupCommandBuilder::new().gid(16539).build().unwrap();

        assert_eq!(cmd.gid, 16539);
    }

    #[test]
    fn test_group_response_parsing() {
        let fields = vec![
            "16539".to_string(),              // gid
            "750".to_string(),                // rating (7.50 * 100)
            "42".to_string(),                 // votes
            "150".to_string(),                // anime_count
            "5000".to_string(),               // file_count
            "Test Group".to_string(),         // name
            "TG".to_string(),                 // short_name
            "#testgroup".to_string(),         // irc_channel
            "irc.rizon.net".to_string(),      // irc_server
            "http://example.com".to_string(), // url
            "group.jpg".to_string(),          // picture
        ];

        let response = GroupResponse::new(250, "GROUP".to_string(), fields).unwrap();

        assert!(response.found());
        assert_eq!(response.gid, Some(16539));
        assert_eq!(response.rating, Some(7.50));
        assert_eq!(response.votes, Some(42));
        assert_eq!(response.anime_count, Some(150));
        assert_eq!(response.file_count, Some(5000));
        assert_eq!(response.name, Some("Test Group".to_string()));
        assert_eq!(response.short_name, Some("TG".to_string()));
        assert_eq!(response.irc_channel, Some("#testgroup".to_string()));
        assert_eq!(response.irc_server, Some("irc.rizon.net".to_string()));
        assert_eq!(response.url, Some("http://example.com".to_string()));
        assert_eq!(response.picture, Some("group.jpg".to_string()));
    }
}
