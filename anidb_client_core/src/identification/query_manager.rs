//! AniDB query orchestration
//!
//! This module manages interactions with the AniDB protocol for identification queries.

use crate::error::Result;
use crate::identification::types::{
    AnimeInfo, DataSource, FileInfo, IdentificationError, IdentificationResult,
    IdentificationSource, IdentificationStatus,
};
use crate::protocol::client::ProtocolClient;
use crate::protocol::messages::{Command, Response};
use log::{debug, trace, warn};
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::Mutex;

/// Manager for AniDB protocol queries
pub struct AniDBQueryManager {
    client: Arc<Mutex<ProtocolClient>>,
    authenticated: Arc<Mutex<bool>>,
}

impl AniDBQueryManager {
    /// Create a new query manager
    pub fn new(client: Arc<Mutex<ProtocolClient>>) -> Self {
        Self {
            client,
            authenticated: Arc::new(Mutex::new(false)),
        }
    }

    /// Ensure we're authenticated with AniDB
    pub async fn ensure_authenticated(&self, username: &str, password: &str) -> Result<()> {
        debug!("Ensuring authentication for user: {username}");

        let mut auth = self.authenticated.lock().await;
        if *auth {
            debug!("Already authenticated");
            return Ok(());
        }

        debug!("Not authenticated, starting authentication process");
        let client = self.client.lock().await;

        // Connect if needed
        let state = client.state().await;
        debug!("Connection state: {state:?}");

        if state == crate::protocol::ConnectionState::Disconnected {
            debug!("Client disconnected, connecting...");
            client.connect().await?;
        }

        // Authenticate
        debug!("Sending AUTH command...");
        let _session = client
            .authenticate(username.to_string(), password.to_string())
            .await
            .map_err(|e| {
                warn!("Authentication failed: {e}");
                IdentificationError::Protocol(e.to_string())
            })?;

        debug!("Authentication successful with session: {_session}");

        *auth = true;
        Ok(())
    }

    /// Query file information from AniDB
    pub async fn query_file(
        &self,
        source: &IdentificationSource,
        fmask_str: Option<&str>,
        amask_str: Option<&str>,
    ) -> Result<IdentificationResult> {
        debug!("Querying file with source: {source:?}");
        debug!("fmask: {fmask_str:?}, amask: {amask_str:?}");

        let start = Instant::now();

        // Build FILE command based on source
        let mut builder = Command::file();

        builder = match source {
            IdentificationSource::FileId(fid) => {
                debug!("Building FILE command for file ID: {fid}");
                builder.by_id(*fid)
            }
            IdentificationSource::HashWithSize { ed2k, size } => {
                debug!("Building FILE command for hash: {ed2k} (size: {size})");
                builder.by_hash(*size, ed2k)
            }
            IdentificationSource::FilePath(_) => {
                warn!("File path queries not supported directly");
                return Err(IdentificationError::Protocol(
                    "File path queries require hash calculation first".to_string(),
                )
                .into());
            }
        };

        // Add masks if provided
        if let Some(fmask) = fmask_str {
            debug!("Adding fmask: {fmask}");
            builder = builder.with_fmask(fmask);
        }
        if let Some(amask) = amask_str {
            debug!("Adding amask: {amask}");
            builder = builder.with_amask(amask);
        }

        let command = builder.build().map_err(|e| {
            warn!("Failed to build FILE command: {e}");
            IdentificationError::Protocol(e.to_string())
        })?;

        trace!("Built FILE command: {command:?}");

        // Send query
        debug!("Sending FILE query to AniDB...");
        let response = {
            let client = self.client.lock().await;
            client.send_command(command).await.map_err(|e| {
                warn!("Failed to send FILE command: {e}");
                IdentificationError::Protocol(e.to_string())
            })?
        }; // Release the lock immediately after the FILE command

        let processing_time = start.elapsed();
        debug!("FILE query completed in {processing_time:?}");
        debug!("Response type: {:?}", std::mem::discriminant(&response));

        // Parse response
        match response {
            Response::File(file_resp) => {
                debug!("Received FILE response, found: {}", file_resp.found());

                if file_resp.found() {
                    // Convert protocol response to our types
                    debug!("Parsing file response...");
                    let file_info = self.parse_file_response(&file_resp)?;
                    debug!(
                        "File info parsed: FID={}, AID={}",
                        file_info.fid, file_info.aid
                    );

                    // Create a basic result with file info
                    let mut result = IdentificationResult {
                        request: crate::identification::types::IdentificationRequest {
                            source: source.clone(),
                            options: Default::default(),
                            priority: Default::default(),
                        },
                        status: IdentificationStatus::Identified,
                        anime: None,
                        episode: None,
                        file: Some(file_info.clone()),
                        group: None,
                        source: DataSource::Network {
                            response_time: processing_time,
                        },
                        processing_time,
                        cached_at: None,
                    };

                    // Make additional API calls to fetch names
                    debug!("Making additional API calls to fetch anime, episode, and group names");

                    // Query anime information
                    if file_info.aid > 0 {
                        match self.query_anime(file_info.aid).await {
                            Ok(Some(anime_info)) => {
                                debug!(
                                    "Successfully fetched anime info: {}",
                                    anime_info.romaji_name
                                );
                                result.anime = Some(anime_info);
                            }
                            Ok(None) => {
                                debug!("Anime not found, using basic info");
                                result.anime = Some(AnimeInfo {
                                    aid: file_info.aid,
                                    romaji_name: "Unknown".to_string(),
                                    kanji_name: None,
                                    english_name: None,
                                    year: None,
                                    type_: None,
                                    episode_count: None,
                                    rating: None,
                                    categories: vec![],
                                });
                            }
                            Err(e) => {
                                warn!("Failed to fetch anime info: {e}");
                                result.anime = Some(AnimeInfo {
                                    aid: file_info.aid,
                                    romaji_name: "Unknown".to_string(),
                                    kanji_name: None,
                                    english_name: None,
                                    year: None,
                                    type_: None,
                                    episode_count: None,
                                    rating: None,
                                    categories: vec![],
                                });
                            }
                        }
                    }

                    // Query episode information
                    if file_info.eid > 0 {
                        match self.query_episode(file_info.eid).await {
                            Ok(Some(episode_info)) => {
                                debug!(
                                    "Successfully fetched episode info: {}",
                                    episode_info.episode_number
                                );
                                result.episode = Some(episode_info);
                            }
                            Ok(None) => {
                                debug!("Episode not found, using basic info");
                                result.episode = Some(crate::identification::types::EpisodeInfo {
                                    eid: file_info.eid,
                                    aid: file_info.aid,
                                    episode_number: "Unknown".to_string(),
                                    english_name: None,
                                    romaji_name: None,
                                    kanji_name: None,
                                    length: None,
                                    aired_date: None,
                                });
                            }
                            Err(e) => {
                                warn!("Failed to fetch episode info: {e}");
                                result.episode = Some(crate::identification::types::EpisodeInfo {
                                    eid: file_info.eid,
                                    aid: file_info.aid,
                                    episode_number: "Unknown".to_string(),
                                    english_name: None,
                                    romaji_name: None,
                                    kanji_name: None,
                                    length: None,
                                    aired_date: None,
                                });
                            }
                        }
                    }

                    // Query group information
                    if file_info.gid > 0 {
                        match self.query_group(file_info.gid).await {
                            Ok(Some(group_info)) => {
                                debug!("Successfully fetched group info: {}", group_info.name);
                                result.group = Some(group_info);
                            }
                            Ok(None) => {
                                debug!("Group not found, using basic info");
                                result.group = Some(crate::identification::types::GroupInfo {
                                    gid: file_info.gid,
                                    name: "Unknown".to_string(),
                                    short_name: None,
                                });
                            }
                            Err(e) => {
                                warn!("Failed to fetch group info: {e}");
                                result.group = Some(crate::identification::types::GroupInfo {
                                    gid: file_info.gid,
                                    name: "Unknown".to_string(),
                                    short_name: None,
                                });
                            }
                        }
                    }

                    debug!("Completed all API calls for file identification");
                    Ok(result)
                } else {
                    // File not found
                    debug!("File not found in AniDB");
                    Ok(IdentificationResult {
                        request: crate::identification::types::IdentificationRequest {
                            source: source.clone(),
                            options: Default::default(),
                            priority: Default::default(),
                        },
                        status: IdentificationStatus::NotFound,
                        anime: None,
                        episode: None,
                        file: None,
                        group: None,
                        source: DataSource::Network {
                            response_time: processing_time,
                        },
                        processing_time,
                        cached_at: None,
                    })
                }
            }
            _ => {
                warn!("Unexpected response type for FILE query: {response:?}");
                Err(IdentificationError::Protocol(format!(
                    "Unexpected response type: {response:?}"
                ))
                .into())
            }
        }
    }

    /// Parse file response into FileInfo
    fn parse_file_response(
        &self,
        resp: &crate::protocol::messages::file::FileResponse,
    ) -> Result<FileInfo> {
        Ok(FileInfo {
            fid: resp
                .fid
                .ok_or_else(|| IdentificationError::Protocol("Missing file ID".to_string()))?,
            aid: resp
                .aid
                .ok_or_else(|| IdentificationError::Protocol("Missing anime ID".to_string()))?,
            eid: resp
                .eid
                .ok_or_else(|| IdentificationError::Protocol("Missing episode ID".to_string()))?,
            gid: resp
                .gid
                .ok_or_else(|| IdentificationError::Protocol("Missing group ID".to_string()))?,
            state: resp.state.unwrap_or(0),
            size: resp
                .size
                .ok_or_else(|| IdentificationError::Protocol("Missing file size".to_string()))?,
            ed2k: resp
                .ed2k
                .clone()
                .ok_or_else(|| IdentificationError::Protocol("Missing ED2K hash".to_string()))?,
            md5: resp.md5.clone(),
            sha1: resp.sha1.clone(),
            crc32: resp.crc32.clone(),
            quality: resp.quality.clone(),
            source: resp.source.clone(),
            video_codec: resp.video_codec.clone(),
            video_resolution: resp.video_resolution.clone(),
            audio_codec: resp.audio_codec.clone(),
            dub_language: resp.dub_language.clone(),
            sub_language: resp.sub_language.clone(),
            file_type: resp.file_type.clone(),
            anidb_filename: resp.anidb_filename.clone(),
        })
    }

    /// Logout from AniDB
    pub async fn logout(&self) -> Result<()> {
        let client = self.client.lock().await;
        client
            .logout()
            .await
            .map_err(|e| IdentificationError::Protocol(e.to_string()))?;

        let mut auth = self.authenticated.lock().await;
        *auth = false;

        Ok(())
    }

    /// Check if we're currently authenticated
    pub async fn is_authenticated(&self) -> bool {
        *self.authenticated.lock().await
    }

    /// Query anime information from AniDB
    pub async fn query_anime(&self, aid: u64) -> Result<Option<AnimeInfo>> {
        debug!("Querying anime with AID: {aid}");

        let start = Instant::now();

        // Build ANIME command with basic amask
        let command = Command::anime(aid);
        trace!("Built ANIME command: {command:?}");

        // Send query
        debug!("Sending ANIME query to AniDB...");
        let response = {
            let client = self.client.lock().await;
            client.send_command(command).await.map_err(|e| {
                warn!("Failed to send ANIME command: {e}");
                IdentificationError::Protocol(e.to_string())
            })?
        };

        let processing_time = start.elapsed();
        debug!("ANIME query completed in {processing_time:?}");
        debug!("Response type: {:?}", std::mem::discriminant(&response));

        // Parse response
        match response {
            Response::Anime(anime_resp) => {
                debug!("Received ANIME response, found: {}", anime_resp.found());

                if anime_resp.found() {
                    debug!("Parsing anime response...");
                    let anime_info = AnimeInfo {
                        aid: anime_resp.aid.unwrap_or(aid),
                        romaji_name: anime_resp
                            .romaji_name
                            .unwrap_or_else(|| "Unknown".to_string()),
                        kanji_name: anime_resp.kanji_name,
                        english_name: anime_resp.english_name,
                        year: anime_resp.year,
                        type_: anime_resp.type_,
                        episode_count: anime_resp.episode_count,
                        rating: anime_resp.rating,
                        categories: anime_resp.categories,
                    };
                    debug!(
                        "Anime info parsed: AID={}, romaji_name={}",
                        anime_info.aid, anime_info.romaji_name
                    );
                    Ok(Some(anime_info))
                } else {
                    debug!("Anime not found in AniDB");
                    Ok(None)
                }
            }
            _ => {
                warn!("Unexpected response type for ANIME query: {response:?}");
                Err(IdentificationError::Protocol(format!(
                    "Unexpected response type: {response:?}"
                ))
                .into())
            }
        }
    }

    /// Query episode information from AniDB
    pub async fn query_episode(
        &self,
        eid: u64,
    ) -> Result<Option<crate::identification::types::EpisodeInfo>> {
        debug!("Querying episode with EID: {eid}");

        let start = Instant::now();

        // Build EPISODE command
        let command = Command::episode(eid);
        trace!("Built EPISODE command: {command:?}");

        // Send query
        debug!("Sending EPISODE query to AniDB...");
        let response = {
            let client = self.client.lock().await;
            client.send_command(command).await.map_err(|e| {
                warn!("Failed to send EPISODE command: {e}");
                IdentificationError::Protocol(e.to_string())
            })?
        };

        let processing_time = start.elapsed();
        debug!("EPISODE query completed in {processing_time:?}");
        debug!("Response type: {:?}", std::mem::discriminant(&response));

        // Parse response
        match response {
            Response::Episode(episode_resp) => {
                debug!("Received EPISODE response, found: {}", episode_resp.found());

                if episode_resp.found() {
                    debug!("Parsing episode response...");
                    let episode_info = crate::identification::types::EpisodeInfo {
                        eid: episode_resp.eid.unwrap_or(eid),
                        aid: episode_resp.aid.unwrap_or(0),
                        episode_number: episode_resp
                            .episode_number
                            .unwrap_or_else(|| "Unknown".to_string()),
                        english_name: episode_resp.english_name,
                        romaji_name: episode_resp.romaji_name,
                        kanji_name: episode_resp.kanji_name,
                        length: episode_resp.length,
                        aired_date: episode_resp.aired_date,
                    };
                    debug!(
                        "Episode info parsed: EID={}, number={}",
                        episode_info.eid, episode_info.episode_number
                    );
                    Ok(Some(episode_info))
                } else {
                    debug!("Episode not found in AniDB");
                    Ok(None)
                }
            }
            _ => {
                warn!("Unexpected response type for EPISODE query: {response:?}");
                Err(IdentificationError::Protocol(format!(
                    "Unexpected response type: {response:?}"
                ))
                .into())
            }
        }
    }

    /// Query group information from AniDB
    pub async fn query_group(
        &self,
        gid: u64,
    ) -> Result<Option<crate::identification::types::GroupInfo>> {
        debug!("Querying group with GID: {gid}");

        let start = Instant::now();

        // Build GROUP command
        let command = Command::group(gid);
        trace!("Built GROUP command: {command:?}");

        // Send query
        debug!("Sending GROUP query to AniDB...");
        let response = {
            let client = self.client.lock().await;
            client.send_command(command).await.map_err(|e| {
                warn!("Failed to send GROUP command: {e}");
                IdentificationError::Protocol(e.to_string())
            })?
        };

        let processing_time = start.elapsed();
        debug!("GROUP query completed in {processing_time:?}");
        debug!("Response type: {:?}", std::mem::discriminant(&response));

        // Parse response
        match response {
            Response::Group(group_resp) => {
                debug!("Received GROUP response, found: {}", group_resp.found());

                if group_resp.found() {
                    debug!("Parsing group response...");
                    let group_info = crate::identification::types::GroupInfo {
                        gid: group_resp.gid.unwrap_or(gid),
                        name: group_resp.name.unwrap_or_else(|| "Unknown".to_string()),
                        short_name: group_resp.short_name,
                    };
                    debug!(
                        "Group info parsed: GID={}, name={}",
                        group_info.gid, group_info.name
                    );
                    Ok(Some(group_info))
                } else {
                    debug!("Group not found in AniDB");
                    Ok(None)
                }
            }
            _ => {
                warn!("Unexpected response type for GROUP query: {response:?}");
                Err(IdentificationError::Protocol(format!(
                    "Unexpected response type: {response:?}"
                ))
                .into())
            }
        }
    }
}
