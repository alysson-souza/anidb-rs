//! EPISODE command for AniDB protocol
//!
//! This module implements the EPISODE command to fetch episode information.

use crate::protocol::error::{ProtocolError, Result};
use crate::protocol::messages::{AniDBCommand, AniDBResponse};
use std::collections::HashMap;
use std::time::{SystemTime, UNIX_EPOCH};

/// EPISODE command for querying episode information
#[derive(Debug, Clone)]
pub struct EpisodeCommand {
    /// Episode ID to query
    eid: u64,
}

impl EpisodeCommand {
    /// Create a new EPISODE command
    pub fn new(eid: u64) -> Self {
        Self { eid }
    }
}

impl AniDBCommand for EpisodeCommand {
    fn name(&self) -> &str {
        "EPISODE"
    }

    fn parameters(&self) -> HashMap<String, String> {
        let mut params = HashMap::new();
        params.insert("eid".to_string(), self.eid.to_string());
        params
    }
}

/// Response to EPISODE command
#[derive(Debug, Clone)]
pub struct EpisodeResponse {
    code: u16,
    message: String,
    fields: Vec<String>,
    // Parsed fields
    pub eid: Option<u64>,
    pub aid: Option<u64>,
    pub episode_number: Option<String>,
    pub english_name: Option<String>,
    pub romaji_name: Option<String>,
    pub kanji_name: Option<String>,
    pub length: Option<u32>,
    pub aired_date: Option<SystemTime>,
    pub episode_type: Option<u32>,
}

impl EpisodeResponse {
    /// Create a new episode response
    pub fn new(code: u16, message: String, fields: Vec<String>) -> Result<Self> {
        let mut response = Self {
            code,
            message,
            fields: fields.clone(),
            eid: None,
            aid: None,
            episode_number: None,
            english_name: None,
            romaji_name: None,
            kanji_name: None,
            length: None,
            aired_date: None,
            episode_type: None,
        };

        // Parse fields if we have data
        if response.is_success() && !fields.is_empty() {
            response.parse_fields(&fields)?;
        }

        Ok(response)
    }

    /// Parse response fields
    fn parse_fields(&mut self, fields: &[String]) -> Result<()> {
        // Expected fields: eid|aid|length|rating|votes|episode_number|english_name|romaji_name|kanji_name|aired_date|episode_type
        if fields.len() >= 6 {
            // Parse EID
            if let Ok(eid) = fields[0].parse::<u64>() {
                self.eid = Some(eid);
            }

            // Parse AID
            if let Ok(aid) = fields[1].parse::<u64>() {
                self.aid = Some(aid);
            }

            // Parse length (in minutes)
            if !fields[2].is_empty()
                && let Ok(length) = fields[2].parse::<u32>()
            {
                self.length = Some(length);
            }

            // Skip rating (field[3]) and votes (field[4]) for now

            // Parse episode number
            if !fields[5].is_empty() {
                self.episode_number = Some(fields[5].clone());
            }

            // Parse episode names
            if fields.len() > 6 && !fields[6].is_empty() {
                // English name
                self.english_name = Some(fields[6].clone());
            }

            if fields.len() > 7 && !fields[7].is_empty() {
                // Romaji name
                self.romaji_name = Some(fields[7].clone());
            }

            if fields.len() > 8 && !fields[8].is_empty() {
                // Kanji name
                self.kanji_name = Some(fields[8].clone());
            }

            // Parse aired date if available
            if fields.len() > 9
                && !fields[9].is_empty()
                && let Ok(timestamp) = fields[9].parse::<u64>()
            {
                self.aired_date = Some(UNIX_EPOCH + std::time::Duration::from_secs(timestamp));
            }

            // Parse episode type
            if fields.len() > 10
                && !fields[10].is_empty()
                && let Ok(ep_type) = fields[10].parse::<u32>()
            {
                self.episode_type = Some(ep_type);
            }
        }

        Ok(())
    }

    /// Check if episode was found
    pub fn found(&self) -> bool {
        self.is_success() && self.eid.is_some()
    }
}

impl AniDBResponse for EpisodeResponse {
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

/// Builder for EPISODE command
pub struct EpisodeCommandBuilder {
    eid: Option<u64>,
}

impl EpisodeCommandBuilder {
    /// Create a new episode command builder
    pub fn new() -> Self {
        Self { eid: None }
    }

    /// Set the episode ID
    pub fn eid(mut self, eid: u64) -> Self {
        self.eid = Some(eid);
        self
    }

    /// Build the command
    pub fn build(self) -> Result<EpisodeCommand> {
        let eid = self.eid.ok_or_else(|| {
            ProtocolError::invalid_packet("Episode ID is required for EPISODE command")
        })?;

        Ok(EpisodeCommand::new(eid))
    }
}

impl Default for EpisodeCommandBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_episode_command_creation() {
        let cmd = EpisodeCommand::new(93300);
        assert_eq!(cmd.name(), "EPISODE");

        let params = cmd.parameters();
        assert_eq!(params.get("eid"), Some(&"93300".to_string()));
    }

    #[test]
    fn test_episode_command_builder() {
        let cmd = EpisodeCommandBuilder::new().eid(93300).build().unwrap();

        assert_eq!(cmd.eid, 93300);
    }

    #[test]
    fn test_episode_response_parsing() {
        let fields = vec![
            "93300".to_string(),        // eid
            "5975".to_string(),         // aid
            "24".to_string(),           // length
            "0".to_string(),            // rating
            "0".to_string(),            // votes
            "01".to_string(),           // episode_number
            "Academy City".to_string(), // english_name
            "".to_string(),             // romaji_name
            "".to_string(),             // kanji_name
            "1234567890".to_string(),   // aired_date
            "1".to_string(),            // episode_type
        ];

        let response = EpisodeResponse::new(240, "EPISODE".to_string(), fields).unwrap();

        assert!(response.found());
        assert_eq!(response.eid, Some(93300));
        assert_eq!(response.aid, Some(5975));
        assert_eq!(response.episode_number, Some("01".to_string()));
        assert_eq!(response.english_name, Some("Academy City".to_string()));
        assert_eq!(response.length, Some(24));
        assert_eq!(response.episode_type, Some(1));
    }
}
