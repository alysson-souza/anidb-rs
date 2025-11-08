//! File-related messages
//!
//! This module contains FILE command and response implementations.

use crate::protocol::error::Result;
use crate::protocol::messages::{AniDBCommand, AniDBResponse};
use std::collections::HashMap;

/// FILE command for querying file information
#[derive(Debug, Clone)]
pub struct FileCommand {
    /// Command parameters
    pub params: HashMap<String, String>,
}

impl FileCommand {
    /// Create a FILE command by file ID
    pub fn by_id(fid: u64) -> Self {
        let mut params = HashMap::new();
        params.insert("fid".to_string(), fid.to_string());
        Self { params }
    }

    /// Create a FILE command by size and ED2K hash
    pub fn by_hash(size: u64, ed2k: &str) -> Self {
        let mut params = HashMap::new();
        params.insert("size".to_string(), size.to_string());
        params.insert("ed2k".to_string(), ed2k.to_string());
        Self { params }
    }

    /// Add field mask
    pub fn with_fmask(mut self, fmask: &str) -> Self {
        self.params.insert("fmask".to_string(), fmask.to_string());
        self
    }

    /// Add anime field mask
    pub fn with_amask(mut self, amask: &str) -> Self {
        self.params.insert("amask".to_string(), amask.to_string());
        self
    }
}

impl AniDBCommand for FileCommand {
    fn name(&self) -> &str {
        "FILE"
    }

    fn parameters(&self) -> HashMap<String, String> {
        self.params.clone()
    }
}

/// Response to FILE command
#[derive(Debug, Clone, Default)]
pub struct FileResponse {
    /// Response code
    pub code: u16,
    /// Response message
    pub message: String,
    /// File ID
    pub fid: Option<u64>,
    /// Anime ID
    pub aid: Option<u64>,
    /// Episode ID
    pub eid: Option<u64>,
    /// Group ID
    pub gid: Option<u64>,
    /// MyList ID
    pub lid: Option<u64>,
    /// File state
    pub state: Option<u32>,
    /// File size in bytes
    pub size: Option<u64>,
    /// ED2K hash
    pub ed2k: Option<String>,
    /// MD5 hash
    pub md5: Option<String>,
    /// SHA1 hash
    pub sha1: Option<String>,
    /// CRC32 checksum
    pub crc32: Option<String>,
    /// Video color depth
    pub color_depth: Option<String>,
    /// Quality rating
    pub quality: Option<String>,
    /// Source media
    pub source: Option<String>,
    /// Audio codec list
    pub audio_codec: Option<String>,
    /// Audio bitrate list
    pub audio_bitrate: Option<String>,
    /// Video codec
    pub video_codec: Option<String>,
    /// Video bitrate
    pub video_bitrate: Option<String>,
    /// Video resolution
    pub video_resolution: Option<String>,
    /// File type/extension
    pub file_type: Option<String>,
    /// Dub language
    pub dub_language: Option<String>,
    /// Sub language
    pub sub_language: Option<String>,
    /// Length in seconds
    pub length: Option<u32>,
    /// Description
    pub description: Option<String>,
    /// Release date
    pub aired_date: Option<u64>,
    /// AniDB filename
    pub anidb_filename: Option<String>,
    /// Raw fields for unmapped data
    pub raw_fields: Vec<String>,
}

impl FileResponse {
    /// Parse a FILE response from raw data
    pub fn parse(code: u16, message: String, fields: Vec<String>) -> Result<Self> {
        let mut response = Self {
            code,
            message,
            fid: None,
            aid: None,
            eid: None,
            gid: None,
            lid: None,
            state: None,
            size: None,
            ed2k: None,
            md5: None,
            sha1: None,
            crc32: None,
            color_depth: None,
            quality: None,
            source: None,
            audio_codec: None,
            audio_bitrate: None,
            video_codec: None,
            video_bitrate: None,
            video_resolution: None,
            file_type: None,
            dub_language: None,
            sub_language: None,
            length: None,
            description: None,
            aired_date: None,
            anidb_filename: None,
            raw_fields: fields.clone(),
        };

        match code {
            220 => {
                // FILE response - parse based on fmask/amask
                // Default fmask 78C8FEF8 field order:
                // fid, aid, eid, gid, state, size, ed2k, crc32,
                // quality, source, audio_codec, audio_bitrate,
                // video_codec, video_bitrate, video_resolution,
                // dub_language, sub_language, length, description, aired_date
                let mut idx = 0;

                // Always present: fid
                if idx < fields.len() {
                    response.fid = fields[idx].parse().ok();
                    idx += 1;
                }

                // aid
                if idx < fields.len() {
                    response.aid = fields[idx].parse().ok();
                    idx += 1;
                }

                // eid
                if idx < fields.len() {
                    response.eid = fields[idx].parse().ok();
                    idx += 1;
                }

                // gid
                if idx < fields.len() {
                    response.gid = fields[idx].parse().ok();
                    idx += 1;
                }

                // state (not lid - lid is not in fmask 78C8FEF8)
                if idx < fields.len() {
                    response.state = fields[idx].parse().ok();
                    idx += 1;
                }

                // size
                if idx < fields.len() {
                    response.size = fields[idx].parse().ok();
                    idx += 1;
                }

                // ed2k
                if idx < fields.len() && !fields[idx].is_empty() {
                    response.ed2k = Some(fields[idx].clone());
                    idx += 1;
                }

                // crc32
                if idx < fields.len() && !fields[idx].is_empty() {
                    response.crc32 = Some(fields[idx].clone());
                    idx += 1;
                }

                // quality
                if idx < fields.len() && !fields[idx].is_empty() {
                    response.quality = Some(fields[idx].clone());
                    idx += 1;
                }

                // source
                if idx < fields.len() && !fields[idx].is_empty() {
                    response.source = Some(fields[idx].clone());
                    idx += 1;
                }

                // audio_codec
                if idx < fields.len() && !fields[idx].is_empty() {
                    response.audio_codec = Some(fields[idx].clone());
                    idx += 1;
                }

                // audio_bitrate
                if idx < fields.len() && !fields[idx].is_empty() {
                    response.audio_bitrate = Some(fields[idx].clone());
                    idx += 1;
                }

                // video_codec
                if idx < fields.len() && !fields[idx].is_empty() {
                    response.video_codec = Some(fields[idx].clone());
                    idx += 1;
                }

                // video_bitrate
                if idx < fields.len() && !fields[idx].is_empty() {
                    response.video_bitrate = Some(fields[idx].clone());
                    idx += 1;
                }

                // video_resolution
                if idx < fields.len() && !fields[idx].is_empty() {
                    response.video_resolution = Some(fields[idx].clone());
                    idx += 1;
                }

                // dub_language
                if idx < fields.len() && !fields[idx].is_empty() {
                    response.dub_language = Some(fields[idx].clone());
                    idx += 1;
                }

                // sub_language
                if idx < fields.len() && !fields[idx].is_empty() {
                    response.sub_language = Some(fields[idx].clone());
                    idx += 1;
                }

                // length
                if idx < fields.len() {
                    response.length = fields[idx].parse().ok();
                    idx += 1;
                }

                // description
                if idx < fields.len() && !fields[idx].is_empty() {
                    response.description = Some(fields[idx].clone());
                    idx += 1;
                }

                // aired_date
                if idx < fields.len() {
                    response.aired_date = fields[idx].parse().ok();
                }
            }
            320 => {
                // NO SUCH FILE
                // No additional fields
            }
            _ => {
                // Other error codes
            }
        }

        Ok(response)
    }

    /// Check if the file was found
    pub fn found(&self) -> bool {
        self.code == 220
    }

    /// Get a formatted file info string
    pub fn format_info(&self) -> String {
        if !self.found() {
            return self.message.clone();
        }

        let mut info = Vec::new();

        if let Some(fid) = self.fid {
            info.push(format!("File ID: {fid}"));
        }
        if let Some(aid) = self.aid {
            info.push(format!("Anime ID: {aid}"));
        }
        if let Some(size) = self.size {
            info.push(format!("Size: {size} bytes"));
        }
        if let Some(ed2k) = &self.ed2k {
            info.push(format!("ED2K: {ed2k}"));
        }

        info.join(", ")
    }
}

impl AniDBResponse for FileResponse {
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

/// Builder for FILE commands
pub struct FileCommandBuilder {
    fid: Option<u64>,
    size: Option<u64>,
    ed2k: Option<String>,
    fmask: Option<String>,
    amask: Option<String>,
}

impl FileCommandBuilder {
    /// Create a new file command builder
    pub fn new() -> Self {
        Self {
            fid: None,
            size: None,
            ed2k: None,
            fmask: None,
            amask: None,
        }
    }

    /// Set file ID for query
    pub fn by_id(mut self, fid: u64) -> Self {
        self.fid = Some(fid);
        self
    }

    /// Set size and hash for query
    pub fn by_hash(mut self, size: u64, ed2k: &str) -> Self {
        self.size = Some(size);
        self.ed2k = Some(ed2k.to_string());
        self
    }

    /// Add field mask
    pub fn with_fmask(mut self, fmask: &str) -> Self {
        self.fmask = Some(fmask.to_string());
        self
    }

    /// Add anime mask
    pub fn with_amask(mut self, amask: &str) -> Self {
        self.amask = Some(amask.to_string());
        self
    }

    /// Build the FILE command
    pub fn build(self) -> Result<crate::protocol::messages::Command> {
        let mut cmd = if let Some(fid) = self.fid {
            FileCommand::by_id(fid)
        } else if let (Some(size), Some(ed2k)) = (self.size, self.ed2k) {
            FileCommand::by_hash(size, &ed2k)
        } else {
            return Err(crate::protocol::error::ProtocolError::invalid_packet(
                "FILE command requires either file ID or (size + ED2K hash)",
            ));
        };

        if let Some(fmask) = self.fmask {
            cmd = cmd.with_fmask(&fmask);
        }

        if let Some(amask) = self.amask {
            cmd = cmd.with_amask(&amask);
        }

        Ok(crate::protocol::messages::Command::File(cmd))
    }
}

impl Default for FileCommandBuilder {
    fn default() -> Self {
        Self::new()
    }
}

/// Field masks for FILE command responses
pub mod fmask {
    /// Anime info fields
    pub const ANIME_TOTAL_EPISODES: &str = "80000000";
    pub const ANIME_HIGHEST_EPISODE: &str = "40000000";
    pub const ANIME_YEAR: &str = "20000000";
    pub const ANIME_TYPE: &str = "10000000";
    pub const ANIME_RELATED_AID_LIST: &str = "08000000";
    pub const ANIME_RELATED_AID_TYPE: &str = "04000000";
    pub const ANIME_CATEGORY_LIST: &str = "02000000";

    /// File info fields  
    pub const AID: &str = "00800000";
    pub const EID: &str = "00400000";
    pub const GID: &str = "00200000";
    pub const LID: &str = "00100000";
    pub const STATUS: &str = "00010000";
    pub const SIZE: &str = "00008000";
    pub const ED2K: &str = "00004000";
    pub const MD5: &str = "00002000";
    pub const SHA1: &str = "00001000";
    pub const CRC32: &str = "00000800";

    /// Common combinations
    pub const BASIC: &str = "78C8FEF8"; // Most common fields
    pub const HASHES: &str = "00007800"; // All hash fields
    pub const IDS: &str = "00F00000"; // All ID fields
}

/// Anime field masks for FILE command responses (amask)
pub mod amask {
    /// Name fields (byte 2)
    pub const ROMAJI_NAME: &str = "00800000";
    pub const KANJI_NAME: &str = "00400000";
    pub const ENGLISH_NAME: &str = "00200000";

    /// Episode name fields (byte 3)
    pub const EPISODE_ROMAJI_NAME: &str = "00002000";
    pub const EPISODE_KANJI_NAME: &str = "00001000";

    /// Common combinations
    pub const ALL_NAMES: &str = "00E03000"; // All anime and episode names
    pub const DEFAULT: &str = "00E03000"; // Same as ALL_NAMES
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_file_command_by_id() {
        let cmd = FileCommand::by_id(312498);
        assert_eq!(cmd.name(), "FILE");
        assert!(cmd.requires_auth());

        let params = cmd.parameters();
        assert_eq!(params.get("fid").unwrap(), "312498");
    }

    #[test]
    fn test_file_command_by_hash() {
        let cmd = FileCommand::by_hash(1234567890, "abc123def456")
            .with_fmask("78C8FEF8")
            .with_amask("00000000");

        let params = cmd.parameters();
        assert_eq!(params.get("size").unwrap(), "1234567890");
        assert_eq!(params.get("ed2k").unwrap(), "abc123def456");
        assert_eq!(params.get("fmask").unwrap(), "78C8FEF8");
        assert_eq!(params.get("amask").unwrap(), "00000000");
    }

    #[test]
    fn test_file_response_found() {
        let fields = vec![
            "312498".to_string(),    // fid
            "4896".to_string(),      // aid
            "69260".to_string(),     // eid
            "41".to_string(),        // gid
            "1".to_string(),         // state (not lid - lid is not in fmask 78C8FEF8)
            "233647104".to_string(), // size
            "abc123".to_string(),    // ed2k
            "12345678".to_string(),  // crc32
        ];

        let response = FileResponse::parse(220, "FILE".to_string(), fields).unwrap();

        assert!(response.found());
        assert_eq!(response.fid, Some(312498));
        assert_eq!(response.aid, Some(4896));
        assert_eq!(response.eid, Some(69260));
        assert_eq!(response.gid, Some(41));
        assert_eq!(response.state, Some(1));
        assert_eq!(response.size, Some(233647104));
        assert_eq!(response.ed2k, Some("abc123".to_string()));
        assert_eq!(response.crc32, Some("12345678".to_string()));
    }

    #[test]
    fn test_file_response_not_found() {
        let response = FileResponse::parse(320, "NO SUCH FILE".to_string(), vec![]).unwrap();

        assert!(!response.found());
        assert_eq!(response.code(), 320);
        assert_eq!(response.message(), "NO SUCH FILE");
    }

    #[test]
    fn test_file_response_format_info() {
        let mut response = FileResponse {
            code: 220,
            message: "FILE".to_string(),
            fid: Some(12345),
            aid: Some(678),
            size: Some(1000000),
            ed2k: Some("hash123".to_string()),
            ..Default::default()
        };

        let info = response.format_info();
        assert!(info.contains("File ID: 12345"));
        assert!(info.contains("Anime ID: 678"));
        assert!(info.contains("Size: 1000000"));
        assert!(info.contains("ED2K: hash123"));

        response.code = 320;
        response.message = "NO SUCH FILE".to_string();
        assert_eq!(response.format_info(), "NO SUCH FILE");
    }

    #[test]
    fn test_fmask_constants() {
        assert_eq!(fmask::ED2K, "00004000");
        assert_eq!(fmask::BASIC, "78C8FEF8");
        assert_eq!(fmask::HASHES, "00007800");
    }
}
