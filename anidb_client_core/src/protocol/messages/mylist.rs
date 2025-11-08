//! MyList-related messages
//!
//! This module contains MYLISTADD, MYLISTDEL, and related command implementations.

use crate::protocol::error::Result;
use crate::protocol::messages::{AniDBCommand, AniDBResponse};
use std::collections::HashMap;

/// MYLISTADD command for adding files to MyList
#[derive(Debug, Clone)]
pub struct MyListAddCommand {
    /// Command parameters
    pub params: HashMap<String, String>,
}

impl MyListAddCommand {
    /// Create a MYLISTADD command by file ID
    pub fn by_id(fid: u64) -> Self {
        let mut params = HashMap::new();
        params.insert("fid".to_string(), fid.to_string());
        Self { params }
    }

    /// Create a MYLISTADD command by size and ED2K hash
    pub fn by_hash(size: u64, ed2k: &str) -> Self {
        let mut params = HashMap::new();
        params.insert("size".to_string(), size.to_string());
        params.insert("ed2k".to_string(), ed2k.to_string());
        Self { params }
    }

    /// Create a MYLISTADD command by anime and episode IDs
    pub fn by_anime_episode(aid: u64, epno: &str) -> Self {
        let mut params = HashMap::new();
        params.insert("aid".to_string(), aid.to_string());
        params.insert("epno".to_string(), epno.to_string());
        Self { params }
    }

    /// Set the state (watching status)
    /// 0 = unknown, 1 = on HDD, 2 = on CD, 3 = deleted
    pub fn with_state(mut self, state: u8) -> Self {
        self.params.insert("state".to_string(), state.to_string());
        self
    }

    /// Set whether the file has been viewed
    pub fn with_viewed(mut self, viewed: bool) -> Self {
        self.params.insert(
            "viewed".to_string(),
            if viewed { "1" } else { "0" }.to_string(),
        );
        self
    }

    /// Set the view date (Unix timestamp)
    pub fn with_viewdate(mut self, timestamp: u64) -> Self {
        self.params
            .insert("viewdate".to_string(), timestamp.to_string());
        self
    }

    /// Set the source (where you got the file from)
    pub fn with_source(mut self, source: &str) -> Self {
        self.params.insert("source".to_string(), source.to_string());
        self
    }

    /// Set the storage location
    pub fn with_storage(mut self, storage: &str) -> Self {
        self.params
            .insert("storage".to_string(), storage.to_string());
        self
    }

    /// Set other information
    pub fn with_other(mut self, other: &str) -> Self {
        self.params.insert("other".to_string(), other.to_string());
        self
    }

    /// Enable edit mode (update existing entry)
    pub fn with_edit(mut self, edit: bool) -> Self {
        self.params
            .insert("edit".to_string(), if edit { "1" } else { "0" }.to_string());
        self
    }
}

impl AniDBCommand for MyListAddCommand {
    fn name(&self) -> &str {
        "MYLISTADD"
    }

    fn parameters(&self) -> HashMap<String, String> {
        self.params.clone()
    }
}

/// Response to MYLISTADD command
#[derive(Debug, Clone, Default)]
pub struct MyListAddResponse {
    /// Response code
    pub code: u16,
    /// Response message
    pub message: String,
    /// MyList ID (for new entries)
    pub lid: Option<u64>,
    /// File ID
    pub fid: Option<u64>,
    /// Number of entries added
    pub entries_added: u32,
    /// Raw fields for unmapped data
    pub raw_fields: Vec<String>,
}

impl MyListAddResponse {
    /// Parse a MYLISTADD response from raw data
    pub fn parse(code: u16, message: String, fields: Vec<String>) -> Result<Self> {
        let mut response = Self {
            code,
            message: message.clone(),
            lid: None,
            fid: None,
            entries_added: 0,
            raw_fields: fields.clone(),
        };

        match code {
            210 => {
                // MYLIST ENTRY ADDED
                // Format: {lid}
                if !fields.is_empty() {
                    response.lid = fields[0].parse().ok();
                    response.entries_added = 1;
                }
            }
            310 => {
                // FILE ALREADY IN MYLIST
                // Format: {lid}|{fid}
                if !fields.is_empty() {
                    let parts: Vec<&str> = fields[0].split('|').collect();
                    if !parts.is_empty() {
                        response.lid = parts[0].parse().ok();
                    }
                    if parts.len() > 1 {
                        response.fid = parts[1].parse().ok();
                    }
                }
            }
            311 => {
                // MYLIST ENTRY EDITED
                // Format: {entries_edited}
                if !fields.is_empty() {
                    response.entries_added = fields[0].parse().unwrap_or(0);
                }
            }
            320 => {
                // NO SUCH FILE
                // No additional fields
            }
            330 => {
                // NO SUCH ANIME
                // No additional fields
            }
            340 => {
                // NO SUCH EPISODE
                // No additional fields
            }
            350 => {
                // NO SUCH MYLIST ENTRY
                // No additional fields
            }
            411 => {
                // NO SUCH MYLIST FILE
                // No additional fields
            }
            _ => {
                // Other error codes
            }
        }

        Ok(response)
    }

    /// Check if the add was successful
    pub fn success(&self) -> bool {
        self.code == 210 || self.code == 311
    }

    /// Check if the file was already in MyList
    pub fn already_in_list(&self) -> bool {
        self.code == 310
    }

    /// Check if the file was not found
    pub fn file_not_found(&self) -> bool {
        self.code == 320
    }

    /// Get a user-friendly status message
    pub fn status_message(&self) -> String {
        match self.code {
            210 => format!(
                "Successfully added to MyList (ID: {})",
                self.lid.unwrap_or(0)
            ),
            310 => format!("File already in MyList (ID: {})", self.lid.unwrap_or(0)),
            311 => format!("MyList entry updated ({} entries)", self.entries_added),
            320 => "File not found in AniDB".to_string(),
            330 => "Anime not found in AniDB".to_string(),
            340 => "Episode not found in AniDB".to_string(),
            350 => "MyList entry not found".to_string(),
            411 => "MyList file not found".to_string(),
            _ => self.message.clone(),
        }
    }
}

impl AniDBResponse for MyListAddResponse {
    fn code(&self) -> u16 {
        self.code
    }

    fn message(&self) -> &str {
        &self.message
    }

    fn fields(&self) -> &[String] {
        &self.raw_fields
    }
}

/// MYLISTDEL command for removing files from MyList
#[derive(Debug, Clone)]
pub struct MyListDelCommand {
    /// Command parameters
    pub params: HashMap<String, String>,
}

impl MyListDelCommand {
    /// Create a MYLISTDEL command by MyList ID
    pub fn by_lid(lid: u64) -> Self {
        let mut params = HashMap::new();
        params.insert("lid".to_string(), lid.to_string());
        Self { params }
    }

    /// Create a MYLISTDEL command by file ID
    pub fn by_fid(fid: u64) -> Self {
        let mut params = HashMap::new();
        params.insert("fid".to_string(), fid.to_string());
        Self { params }
    }

    /// Create a MYLISTDEL command by anime and episode
    pub fn by_anime_episode(aid: u64, epno: &str) -> Self {
        let mut params = HashMap::new();
        params.insert("aid".to_string(), aid.to_string());
        params.insert("epno".to_string(), epno.to_string());
        Self { params }
    }
}

impl AniDBCommand for MyListDelCommand {
    fn name(&self) -> &str {
        "MYLISTDEL"
    }

    fn parameters(&self) -> HashMap<String, String> {
        self.params.clone()
    }
}

/// Response to MYLISTDEL command
#[derive(Debug, Clone, Default)]
pub struct MyListDelResponse {
    /// Response code
    pub code: u16,
    /// Response message
    pub message: String,
    /// Number of entries deleted
    pub entries_deleted: u32,
    /// Raw fields
    pub raw_fields: Vec<String>,
}

impl MyListDelResponse {
    /// Parse a MYLISTDEL response from raw data
    pub fn parse(code: u16, message: String, fields: Vec<String>) -> Result<Self> {
        let mut response = Self {
            code,
            message,
            entries_deleted: 0,
            raw_fields: fields.clone(),
        };

        match code {
            211 => {
                // MYLIST ENTRY DELETED
                // Format: {entries_deleted}
                if !fields.is_empty() {
                    response.entries_deleted = fields[0].parse().unwrap_or(1);
                }
            }
            411 => {
                // NO SUCH MYLIST ENTRY
                // No additional fields
            }
            _ => {
                // Other error codes
            }
        }

        Ok(response)
    }

    /// Check if the delete was successful
    pub fn success(&self) -> bool {
        self.code == 211
    }
}

impl AniDBResponse for MyListDelResponse {
    fn code(&self) -> u16 {
        self.code
    }

    fn message(&self) -> &str {
        &self.message
    }

    fn fields(&self) -> &[String] {
        &self.raw_fields
    }
}

/// Builder for MYLISTADD commands
pub struct MyListAddCommandBuilder {
    fid: Option<u64>,
    size: Option<u64>,
    ed2k: Option<String>,
    aid: Option<u64>,
    epno: Option<String>,
    state: Option<u8>,
    viewed: Option<bool>,
    viewdate: Option<u64>,
    source: Option<String>,
    storage: Option<String>,
    other: Option<String>,
    edit: bool,
}

impl MyListAddCommandBuilder {
    /// Create a new MyList add command builder
    pub fn new() -> Self {
        Self {
            fid: None,
            size: None,
            ed2k: None,
            aid: None,
            epno: None,
            state: None,
            viewed: None,
            viewdate: None,
            source: None,
            storage: None,
            other: None,
            edit: false,
        }
    }

    /// Set file ID for the command
    pub fn by_id(mut self, fid: u64) -> Self {
        self.fid = Some(fid);
        self
    }

    /// Set size and ED2K hash for the command
    pub fn by_hash(mut self, size: u64, ed2k: &str) -> Self {
        self.size = Some(size);
        self.ed2k = Some(ed2k.to_string());
        self
    }

    /// Set anime and episode for the command
    pub fn by_anime_episode(mut self, aid: u64, epno: &str) -> Self {
        self.aid = Some(aid);
        self.epno = Some(epno.to_string());
        self
    }

    /// Set the state
    pub fn with_state(mut self, state: u8) -> Self {
        self.state = Some(state);
        self
    }

    /// Set viewed status
    pub fn with_viewed(mut self, viewed: bool) -> Self {
        self.viewed = Some(viewed);
        self
    }

    /// Set view date
    pub fn with_viewdate(mut self, timestamp: u64) -> Self {
        self.viewdate = Some(timestamp);
        self
    }

    /// Set source
    pub fn with_source(mut self, source: &str) -> Self {
        self.source = Some(source.to_string());
        self
    }

    /// Set storage
    pub fn with_storage(mut self, storage: &str) -> Self {
        self.storage = Some(storage.to_string());
        self
    }

    /// Set other info
    pub fn with_other(mut self, other: &str) -> Self {
        self.other = Some(other.to_string());
        self
    }

    /// Enable edit mode
    pub fn with_edit(mut self, edit: bool) -> Self {
        self.edit = edit;
        self
    }

    /// Build the MYLISTADD command
    pub fn build(self) -> Result<crate::protocol::messages::Command> {
        let mut cmd = if let Some(fid) = self.fid {
            MyListAddCommand::by_id(fid)
        } else if let (Some(size), Some(ed2k)) = (self.size, self.ed2k.clone()) {
            MyListAddCommand::by_hash(size, &ed2k)
        } else if let (Some(aid), Some(epno)) = (self.aid, self.epno.clone()) {
            MyListAddCommand::by_anime_episode(aid, &epno)
        } else {
            return Err(crate::protocol::error::ProtocolError::invalid_packet(
                "MYLISTADD requires either file ID, (size + ED2K hash), or (anime ID + episode)",
            ));
        };

        if let Some(state) = self.state {
            cmd = cmd.with_state(state);
        }

        if let Some(viewed) = self.viewed {
            cmd = cmd.with_viewed(viewed);
        }

        if let Some(viewdate) = self.viewdate {
            cmd = cmd.with_viewdate(viewdate);
        }

        if let Some(source) = self.source {
            cmd = cmd.with_source(&source);
        }

        if let Some(storage) = self.storage {
            cmd = cmd.with_storage(&storage);
        }

        if let Some(other) = self.other {
            cmd = cmd.with_other(&other);
        }

        cmd = cmd.with_edit(self.edit);

        Ok(crate::protocol::messages::Command::MyListAdd(cmd))
    }
}

impl Default for MyListAddCommandBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mylistadd_command_by_id() {
        let cmd = MyListAddCommand::by_id(12345);
        assert_eq!(cmd.name(), "MYLISTADD");
        assert!(cmd.requires_auth());

        let params = cmd.parameters();
        assert_eq!(params.get("fid").unwrap(), "12345");
    }

    #[test]
    fn test_mylistadd_command_by_hash() {
        let cmd = MyListAddCommand::by_hash(1234567890, "abc123def456")
            .with_state(1)
            .with_viewed(true);

        let params = cmd.parameters();
        assert_eq!(params.get("size").unwrap(), "1234567890");
        assert_eq!(params.get("ed2k").unwrap(), "abc123def456");
        assert_eq!(params.get("state").unwrap(), "1");
        assert_eq!(params.get("viewed").unwrap(), "1");
    }

    #[test]
    fn test_mylistadd_response_added() {
        let fields = vec!["54321".to_string()];
        let response =
            MyListAddResponse::parse(210, "MYLIST ENTRY ADDED".to_string(), fields).unwrap();

        assert!(response.success());
        assert_eq!(response.lid, Some(54321));
        assert_eq!(response.entries_added, 1);
    }

    #[test]
    fn test_mylistadd_response_already_in_list() {
        let fields = vec!["54321|12345".to_string()];
        let response =
            MyListAddResponse::parse(310, "FILE ALREADY IN MYLIST".to_string(), fields).unwrap();

        assert!(response.already_in_list());
        assert!(!response.success());
        assert_eq!(response.lid, Some(54321));
        assert_eq!(response.fid, Some(12345));
    }

    #[test]
    fn test_mylistadd_response_file_not_found() {
        let response = MyListAddResponse::parse(320, "NO SUCH FILE".to_string(), vec![]).unwrap();

        assert!(response.file_not_found());
        assert!(!response.success());
        assert!(!response.already_in_list());
    }

    #[test]
    fn test_mylistdel_command() {
        let cmd = MyListDelCommand::by_lid(12345);
        assert_eq!(cmd.name(), "MYLISTDEL");
        assert!(cmd.requires_auth());

        let params = cmd.parameters();
        assert_eq!(params.get("lid").unwrap(), "12345");
    }

    #[test]
    fn test_mylistdel_response() {
        let fields = vec!["3".to_string()];
        let response =
            MyListDelResponse::parse(211, "MYLIST ENTRY DELETED".to_string(), fields).unwrap();

        assert!(response.success());
        assert_eq!(response.entries_deleted, 3);
    }

    #[test]
    fn test_mylistadd_builder() {
        let builder = MyListAddCommandBuilder::new()
            .by_hash(1000000, "hash123")
            .with_state(1)
            .with_viewed(false)
            .with_source("torrent")
            .with_storage("HDD")
            .with_edit(true);

        let cmd = builder.build().unwrap();
        match cmd {
            crate::protocol::messages::Command::MyListAdd(mylist_cmd) => {
                let params = mylist_cmd.parameters();
                assert_eq!(params.get("size").unwrap(), "1000000");
                assert_eq!(params.get("ed2k").unwrap(), "hash123");
                assert_eq!(params.get("state").unwrap(), "1");
                assert_eq!(params.get("viewed").unwrap(), "0");
                assert_eq!(params.get("source").unwrap(), "torrent");
                assert_eq!(params.get("storage").unwrap(), "HDD");
                assert_eq!(params.get("edit").unwrap(), "1");
            }
            _ => panic!("Expected MyListAdd command"),
        }
    }
}
