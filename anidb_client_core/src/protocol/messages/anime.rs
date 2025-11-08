//! ANIME command for AniDB protocol
//!
//! This module implements the ANIME command to fetch anime information.

use crate::protocol::error::{ProtocolError, Result};
use crate::protocol::messages::{AniDBCommand, AniDBResponse};
use std::collections::HashMap;

/// ANIME command for querying anime information
#[derive(Debug, Clone)]
pub struct AnimeCommand {
    /// Anime ID to query
    aid: u64,
    /// Anime mask (amask) for specifying which fields to return
    amask: Option<String>,
}

impl AnimeCommand {
    /// Create a new ANIME command
    pub fn new(aid: u64) -> Self {
        Self { aid, amask: None }
    }

    /// Set the anime mask (amask)
    pub fn with_amask(mut self, amask: &str) -> Self {
        self.amask = Some(amask.to_string());
        self
    }
}

impl AniDBCommand for AnimeCommand {
    fn name(&self) -> &str {
        "ANIME"
    }

    fn parameters(&self) -> HashMap<String, String> {
        let mut params = HashMap::new();
        params.insert("aid".to_string(), self.aid.to_string());

        if let Some(ref amask) = self.amask {
            params.insert("amask".to_string(), amask.clone());
        }

        params
    }
}

/// Response to ANIME command
#[derive(Debug, Clone)]
pub struct AnimeResponse {
    code: u16,
    message: String,
    fields: Vec<String>,
    // Parsed fields
    pub aid: Option<u64>,
    pub romaji_name: Option<String>,
    pub kanji_name: Option<String>,
    pub english_name: Option<String>,
    pub year: Option<u16>,
    pub type_: Option<String>,
    pub episode_count: Option<u32>,
    pub rating: Option<f32>,
    pub vote_count: Option<u32>,
    pub temp_rating: Option<f32>,
    pub temp_vote_count: Option<u32>,
    pub avg_review_rating: Option<f32>,
    pub review_count: Option<u32>,
    pub categories: Vec<String>,
}

impl AnimeResponse {
    /// Create a new anime response
    pub fn new(code: u16, message: String, fields: Vec<String>) -> Result<Self> {
        let mut response = Self {
            code,
            message,
            fields: fields.clone(),
            aid: None,
            romaji_name: None,
            kanji_name: None,
            english_name: None,
            year: None,
            type_: None,
            episode_count: None,
            rating: None,
            vote_count: None,
            temp_rating: None,
            temp_vote_count: None,
            avg_review_rating: None,
            review_count: None,
            categories: Vec::new(),
        };

        // Parse fields if we have data
        if response.is_success() && !fields.is_empty() {
            response.parse_fields(&fields)?;
        }

        Ok(response)
    }

    /// Parse response fields
    fn parse_fields(&mut self, fields: &[String]) -> Result<()> {
        // Parse fields based on amask b2f0e0fc000000 order:
        // aid|year|type|categories|romaji_name|kanji_name|english_name|...
        if fields.len() >= 5 {
            // Parse AID
            if let Ok(aid) = fields[0].parse::<u64>() {
                self.aid = Some(aid);
            }

            // Parse year - handle ranges like "1999-1999"
            if !fields[1].is_empty() {
                // Extract first year from range (e.g., "1999-1999" -> 1999)
                let year_str = fields[1].split('-').next().unwrap_or(&fields[1]);
                if let Ok(year) = year_str.parse::<u16>() {
                    self.year = Some(year);
                }
            }

            // Parse type
            if !fields[2].is_empty() {
                self.type_ = Some(fields[2].clone());
            }

            // Categories are at index 3, but we skip them for now

            // Parse romaji name - at index 4
            if fields.len() > 4 && !fields[4].is_empty() {
                self.romaji_name = Some(fields[4].clone());
            }

            // Parse kanji name - at index 5
            if fields.len() > 5 && !fields[5].is_empty() {
                self.kanji_name = Some(fields[5].clone());
            }

            // Parse english name - at index 6
            if fields.len() > 6 && !fields[6].is_empty() {
                self.english_name = Some(fields[6].clone());
            }

            // Parse episode count - based on example, likely at index 8
            if fields.len() > 8
                && !fields[8].is_empty()
                && let Ok(count) = fields[8].parse::<u32>()
            {
                self.episode_count = Some(count);
            }

            // Parse categories from index 3
            if fields.len() > 3 && !fields[3].is_empty() {
                self.categories = fields[3]
                    .split(',')
                    .map(|s| s.trim().to_string())
                    .filter(|s| !s.is_empty())
                    .collect();
            }

            // Parse rating and vote counts - these would be at later indices
            // Based on example pattern, ratings and votes appear near the end
            // For now, we'll skip detailed parsing of these fields as our focus
            // is on title, year, and type which are working correctly now
        }

        Ok(())
    }

    /// Check if anime was found
    pub fn found(&self) -> bool {
        self.is_success() && self.aid.is_some()
    }
}

impl AniDBResponse for AnimeResponse {
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

/// Builder for ANIME command
pub struct AnimeCommandBuilder {
    aid: Option<u64>,
    amask: Option<String>,
}

impl AnimeCommandBuilder {
    /// Create a new anime command builder
    pub fn new() -> Self {
        Self {
            aid: None,
            amask: None,
        }
    }

    /// Set the anime ID
    pub fn aid(mut self, aid: u64) -> Self {
        self.aid = Some(aid);
        self
    }

    /// Set the anime mask
    pub fn with_amask(mut self, amask: &str) -> Self {
        self.amask = Some(amask.to_string());
        self
    }

    /// Build the command
    pub fn build(self) -> Result<AnimeCommand> {
        let aid = self.aid.ok_or_else(|| {
            ProtocolError::invalid_packet("Anime ID is required for ANIME command")
        })?;

        let mut cmd = AnimeCommand::new(aid);
        if let Some(amask) = self.amask {
            cmd = cmd.with_amask(&amask);
        }

        Ok(cmd)
    }
}

impl Default for AnimeCommandBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_anime_command_creation() {
        let cmd = AnimeCommand::new(5975);
        assert_eq!(cmd.name(), "ANIME");

        let params = cmd.parameters();
        assert_eq!(params.get("aid"), Some(&"5975".to_string()));
        assert!(!params.contains_key("amask"));
    }

    #[test]
    fn test_anime_command_with_amask() {
        let cmd = AnimeCommand::new(5975).with_amask("80000000");
        let params = cmd.parameters();
        assert_eq!(params.get("aid"), Some(&"5975".to_string()));
        assert_eq!(params.get("amask"), Some(&"80000000".to_string()));
    }

    #[test]
    fn test_anime_command_builder() {
        let cmd = AnimeCommandBuilder::new()
            .aid(5975)
            .with_amask("80000000")
            .build()
            .unwrap();

        assert_eq!(cmd.aid, 5975);
        assert_eq!(cmd.amask, Some("80000000".to_string()));
    }

    #[test]
    fn test_anime_response_parsing() {
        let fields = vec![
            "5975".to_string(),                                    // aid
            "2008-2008".to_string(),                               // year (range format)
            "TV Series".to_string(),                               // type
            "Action,SciFi,Super Power,School,Shounen".to_string(), // categories
            "To Aru Majutsu no Index".to_string(),                 // romaji_name (title)
            "とある魔術の禁書目録".to_string(),                    // kanji_name
            "A Certain Magical Index".to_string(),                 // english_name
            "".to_string(),                                        // empty field
            "24".to_string(),                                      // episode_count
            "24".to_string(),                                      // another count field
            "3".to_string(),                                       // some other field
        ];

        let response = AnimeResponse::new(230, "ANIME".to_string(), fields).unwrap();

        assert!(response.found());
        assert_eq!(response.aid, Some(5975));
        assert_eq!(response.year, Some(2008)); // Parsed from "2008-2008"
        assert_eq!(response.type_, Some("TV Series".to_string()));
        assert_eq!(
            response.romaji_name,
            Some("To Aru Majutsu no Index".to_string())
        );
        assert_eq!(response.episode_count, Some(24));
        assert_eq!(
            response.categories,
            vec!["Action", "SciFi", "Super Power", "School", "Shounen"]
        );
    }
}
