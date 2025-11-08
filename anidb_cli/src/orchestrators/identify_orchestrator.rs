//! Identify command orchestrator
//!
//! This module handles the business logic for file identification,
//! coordinating between the CLI and core identification service.

use crate::cache::IdentificationCacheService;
use crate::progress::{create_progress_infrastructure, render_progress};
use crate::terminal;
use anidb_client_core::api::AniDBClient;
use anidb_client_core::database::repositories::anidb_result::AniDBResultRepository;
use anidb_client_core::identification::{
    FileIdentificationService, IdentificationOptions, IdentificationResult, ServiceConfig,
};
use anidb_client_core::{ClientConfig, Database};
use anyhow::{Context, Result};
use colored::*;
use log::{debug, trace};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Instant;

/// Output format for identification results
#[derive(Debug, Clone, Copy)]
#[allow(dead_code)] // Used in main.rs when identify command is fully integrated
pub enum OutputFormat {
    Human,
    Json,
    Csv,
}

/// Options for directory identification
#[derive(Debug, Clone)]
pub struct DirectoryIdentifyOptions {
    pub recursive: bool,
    pub format: OutputFormat,
    pub include_patterns: Vec<String>,
    pub exclude_patterns: Vec<String>,
    pub use_defaults: bool,
    pub no_cache: bool,
}

/// Orchestrator for the identify command
#[allow(dead_code)] // Used in main.rs when identify command is fully integrated
pub struct IdentifyOrchestrator {
    cache_service: IdentificationCacheService,
    verbose: bool,
}

#[allow(dead_code)] // Used in main.rs when identify command is fully integrated
impl IdentifyOrchestrator {
    /// Create a new identify orchestrator
    pub async fn new(client_config: ClientConfig, verbose: bool) -> Result<Self> {
        debug!("Creating identify orchestrator with verbose: {verbose}");

        let service_config = ServiceConfig {
            verbose,
            ..Default::default()
        };

        debug!("Creating file identification service...");
        let service = Arc::new(
            FileIdentificationService::new(client_config.clone(), service_config)
                .await
                .context("Failed to create identification service")?,
        );

        // Create AniDB client for hash calculations
        let client = Arc::new(
            AniDBClient::new(client_config.clone())
                .await
                .context("Failed to create AniDB client")?,
        );

        // Determine database path
        let data_dir = dirs::data_dir()
            .map(|d| d.join("anidb-cli"))
            .unwrap_or_else(|| PathBuf::from(".anidb"));
        std::fs::create_dir_all(&data_dir).ok();
        let db_path = data_dir.join("anidb.db");

        debug!("Creating database at: {}", db_path.display());
        let db = Database::new(&db_path)
            .await
            .context("Failed to create database")?;

        // Create repositories
        let db_repo = Arc::new(AniDBResultRepository::new(db.pool().clone()));

        // Create cache service
        let cache_service =
            IdentificationCacheService::new(service, client, db_repo).with_verbose(verbose);

        debug!("Identify orchestrator created successfully with cache service");
        Ok(Self {
            cache_service,
            verbose,
        })
    }

    /// Identify a single file
    pub async fn identify_file(
        &self,
        path: &Path,
        format: OutputFormat,
        no_cache: bool,
    ) -> Result<IdentificationResult> {
        debug!("Identifying file: {path:?} with format: {format:?}, no_cache: {no_cache}");

        if !path.exists() {
            anyhow::bail!("File not found: {}", path.display());
        }

        if !path.is_file() {
            anyhow::bail!("Path is not a file: {}", path.display());
        }

        let start = Instant::now();

        // Use default options and handle no_cache flag
        let mut options = IdentificationOptions::default();
        if no_cache {
            options.use_cache = false;
        }
        trace!("Using identification options: {options:?}");

        // Create progress infrastructure
        let (provider, rx) = create_progress_infrastructure();

        // Spawn progress renderer in background
        let progress_handle = tokio::spawn(render_progress(rx));

        debug!("Calling identification cache service with progress...");
        let result = self
            .cache_service
            .identify_file_with_cache_and_progress(path, options, provider.as_ref())
            .await
            .context("Failed to identify file")?;

        // Signal completion
        provider.complete();

        // Wait for renderer to finish (with timeout)
        let _ = tokio::time::timeout(std::time::Duration::from_millis(100), progress_handle).await;

        let elapsed = start.elapsed();
        debug!(
            "Identification completed in {:?}, status: {:?}",
            elapsed, result.status
        );

        // Output the result based on format
        debug!("Outputting result in {format:?} format");
        self.output_result(&result, format, elapsed)?;

        Ok(result)
    }

    /// Identify files in a directory
    pub async fn identify_directory(
        &self,
        path: &Path,
        options: DirectoryIdentifyOptions,
    ) -> Result<Vec<IdentificationResult>> {
        use crate::file_discovery::{FileDiscovery, FileDiscoveryOptions};

        if !path.exists() {
            anyhow::bail!("Directory not found: {}", path.display());
        }

        if !path.is_dir() {
            anyhow::bail!("Path is not a directory: {}", path.display());
        }

        eprintln!("{}", "Discovering files...".bold().cyan());

        // Configure file discovery
        let discovery_options = FileDiscoveryOptions::new()
            .with_include_patterns(options.include_patterns.clone())
            .with_exclude_patterns(options.exclude_patterns.clone())
            .with_use_defaults(options.use_defaults)
            .with_recursive(options.recursive);

        // Discover files
        let discovery = FileDiscovery::new(path, discovery_options)?;
        let files: Vec<_> = discovery
            .map(|entry| entry.map(|e| e.path))
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| anyhow::anyhow!("File discovery error: {}", e))?;

        if files.is_empty() {
            eprintln!("{}", "No matching files found.".yellow());
            return Ok(vec![]);
        }

        eprintln!("Found {} file(s) to identify", files.len());
        eprintln!();

        let mut results = Vec::new();
        let total_start = Instant::now();

        // Process each file
        for (idx, file) in files.iter().enumerate() {
            eprintln!(
                "[{}/{}] Processing: {}",
                idx + 1,
                files.len(),
                file.display()
            );

            match self
                .identify_file(file, options.format, options.no_cache)
                .await
            {
                Ok(result) => {
                    results.push(result);
                }
                Err(e) => {
                    eprintln!("  {} Error: {}", "✗".red(), e);
                }
            }
        }

        let total_elapsed = total_start.elapsed();

        if files.len() > 1 {
            eprintln!();
            eprintln!("{}", "Summary:".bold().green());
            eprintln!("Files processed: {}", files.len());
            eprintln!("Successfully identified: {}", results.len());
            eprintln!("Total time: {:.2}s", total_elapsed.as_secs_f64());
        }

        Ok(results)
    }

    /// Output a single result
    fn output_result(
        &self,
        result: &IdentificationResult,
        format: OutputFormat,
        elapsed: std::time::Duration,
    ) -> Result<()> {
        match format {
            OutputFormat::Human => self.output_human(result, elapsed),
            OutputFormat::Json => self.output_json(result),
            OutputFormat::Csv => self.output_csv(result),
        }
    }

    /// Human-readable output
    fn output_human(
        &self,
        result: &IdentificationResult,
        elapsed: std::time::Duration,
    ) -> Result<()> {
        use anidb_client_core::identification::IdentificationStatus;

        match result.status {
            IdentificationStatus::Identified => {
                eprintln!("{}", "✓ File Identified".bold().green());
                eprintln!();

                // Anime section
                if let Some(ref anime) = result.anime {
                    eprintln!("{}", "═══ Anime ═══════════════════════════════════════════════════════════════════".dimmed());

                    // Display title based on availability
                    if let Some(ref english) = anime.english_name {
                        eprintln!("  {}: {}", "Title".bold().bright_white(), english.cyan());
                        if english != &anime.romaji_name {
                            eprintln!(
                                "  {}: {}",
                                "Romaji".bold().bright_white(),
                                anime.romaji_name
                            );
                        }
                    } else {
                        eprintln!(
                            "  {}: {}",
                            "Title".bold().bright_white(),
                            anime.romaji_name.cyan()
                        );
                    }

                    if let Some(ref kanji) = anime.kanji_name {
                        eprintln!("  {}: {}", "Japanese".bold().bright_white(), kanji);
                    }

                    eprintln!();

                    // Metadata line
                    let anime_url = format!("https://anidb.net/anime/{}", anime.aid);
                    let anime_link =
                        terminal::hyperlink_with_fallback(&anime_url, &anime.aid.to_string());

                    let mut meta_parts = vec![format!("ID: {}", anime_link.blue())];
                    if let Some(year) = anime.year {
                        meta_parts.push(format!("Year: {}", year.to_string().bright_white()));
                    }
                    if let Some(ref type_) = anime.type_ {
                        meta_parts.push(format!("Type: {}", type_.bright_white()));
                    }
                    if let Some(count) = anime.episode_count {
                        meta_parts.push(format!("Episodes: {}", count.to_string().bright_white()));
                    }

                    eprintln!("  {}", meta_parts.join(" • ").dimmed());
                    eprintln!();
                }

                // Episode section
                if let Some(ref episode) = result.episode {
                    eprintln!("{}", "═══ Episode ═════════════════════════════════════════════════════════════════".dimmed());

                    // Display episode number and title in the same format as anime
                    let episode_display = format!("{}: ", episode.episode_number);

                    // Display title based on availability (English preferred, then romaji)
                    if let Some(ref english) = episode.english_name {
                        eprintln!(
                            "  {}{}",
                            episode_display.bold().bright_white(),
                            english.cyan()
                        );
                        if let Some(ref romaji) = episode.romaji_name
                            && english != romaji
                        {
                            eprintln!("  {}: {}", "Romaji".bold().bright_white(), romaji);
                        }
                    } else if let Some(ref romaji) = episode.romaji_name {
                        eprintln!(
                            "  {}{}",
                            episode_display.bold().bright_white(),
                            romaji.cyan()
                        );
                    } else {
                        eprintln!(
                            "  {}: {}",
                            "Number".bold().bright_white(),
                            episode.episode_number.cyan()
                        );
                    }

                    if let Some(ref kanji) = episode.kanji_name {
                        eprintln!("  {}: {}", "Japanese".bold().bright_white(), kanji);
                    }

                    eprintln!();

                    // Metadata line
                    let episode_url = format!("https://anidb.net/episode/{}", episode.eid);
                    let episode_link =
                        terminal::hyperlink_with_fallback(&episode_url, &episode.eid.to_string());

                    let mut meta_parts = vec![format!("ID: {}", episode_link.blue())];
                    if let Some(length) = episode.length {
                        meta_parts.push(format!(
                            "Duration: {} min",
                            length.to_string().bright_white()
                        ));
                    }

                    eprintln!("  {}", meta_parts.join(" • ").dimmed());
                    eprintln!();
                }

                // File section
                if let Some(ref file) = result.file {
                    eprintln!("{}", "═══ File ════════════════════════════════════════════════════════════════════".dimmed());

                    let file_url = format!("https://anidb.net/file/{}", file.fid);
                    let file_link =
                        terminal::hyperlink_with_fallback(&file_url, &file.fid.to_string());

                    eprintln!(
                        "  {}: {}",
                        "File ID".bold().bright_white(),
                        file_link.blue()
                    );

                    // Format file size nicely
                    let size_mib = file.size as f64 / 1_048_576.0;
                    let size_gib = size_mib / 1024.0;
                    let size_display = if size_gib >= 1.0 {
                        format!("{size_gib:.2} GiB")
                    } else {
                        format!("{size_mib:.2} MiB")
                    };
                    eprintln!(
                        "  {}: {} ({} bytes)",
                        "Size".bold().bright_white(),
                        size_display.cyan(),
                        file.size.to_string().dimmed()
                    );

                    eprintln!(
                        "  {}: {}",
                        "ED2K Hash".bold().bright_white(),
                        file.ed2k.yellow()
                    );

                    eprintln!();

                    // Quality line
                    let mut quality_parts = Vec::new();
                    if let Some(ref quality) = file.quality {
                        let quality_display = match quality.as_str() {
                            "very high" => "Very High".green(),
                            "high" => "High".bright_green(),
                            "med" => "Medium".yellow(),
                            "low" => "Low".red(),
                            "very low" => "Very Low".bright_red(),
                            _ => quality.normal(),
                        };
                        quality_parts.push(format!("Quality: {quality_display}"));
                    }

                    if let Some(ref source) = file.source {
                        quality_parts.push(format!("Source: {}", source.bright_white()));
                    }

                    if !quality_parts.is_empty() {
                        eprintln!("  {}", quality_parts.join(" • "));
                    }

                    // Video line
                    let mut video_parts = Vec::new();
                    if let Some(ref codec) = file.video_codec {
                        video_parts.push(codec.bright_white().to_string());
                    }
                    if let Some(ref res) = file.video_resolution {
                        video_parts.push(res.bright_white().to_string());
                    }

                    if !video_parts.is_empty() {
                        eprintln!(
                            "  {}: {}",
                            "Video".bold().bright_white(),
                            video_parts.join(" • ")
                        );
                    }

                    eprintln!();
                }

                // Release Group section
                if let Some(ref group) = result.group {
                    eprintln!("{}", "═══ Release Group ═══════════════════════════════════════════════════════════".dimmed());
                    let group_url = format!("https://anidb.net/group/{}", group.gid);
                    let group_link =
                        terminal::hyperlink_with_fallback(&group_url, &group.gid.to_string());
                    eprintln!("  {} (ID: {})", group.name.cyan(), group_link.blue());
                    eprintln!();
                }

                // Footer with source and timing
                eprintln!(
                    "{}",
                    "═════════════════════════════════════════════════════════════════════════════"
                        .dimmed()
                );

                let source_str = match &result.source {
                    anidb_client_core::identification::DataSource::Cache { age } => {
                        format!("Cache ({:.1}h old)", age.as_secs_f64() / 3600.0)
                            .green()
                            .to_string()
                    }
                    anidb_client_core::identification::DataSource::Network { response_time } => {
                        format!("Network ({:.1}s)", response_time.as_secs_f64())
                            .blue()
                            .to_string()
                    }
                    anidb_client_core::identification::DataSource::Offline => {
                        "Offline".yellow().to_string()
                    }
                };

                eprintln!(
                    "  Source: {} • Processing Time: {:.2}s",
                    source_str,
                    elapsed.as_secs_f64()
                );
            }
            IdentificationStatus::NotFound => {
                eprintln!("{}", "✗ File Not Found in AniDB".bold().red());
                eprintln!();
                eprintln!("This file is not recognized by AniDB.");
                eprintln!("It may be a new release or an unofficial version.");
            }
            IdentificationStatus::NetworkError => {
                eprintln!("{}", "✗ Network Error".bold().red());
                eprintln!();
                eprintln!("Failed to connect to AniDB. Please check your internet connection.");
            }
            IdentificationStatus::Queued => {
                eprintln!("{}", "⏳ Identification Queued".bold().yellow());
                eprintln!();
                eprintln!("The file has been queued for identification when network is available.");
            }
            IdentificationStatus::Expired => {
                eprintln!("{}", "⚠ Cache Expired".bold().yellow());
                eprintln!();
                eprintln!("Cached data has expired. Unable to refresh due to network issues.");
            }
        }

        eprintln!();
        eprintln!(
            "{}: {:.2}s",
            "Processing Time".dimmed(),
            elapsed.as_secs_f64()
        );

        if self.verbose {
            eprintln!();
            eprintln!("{}", "Debug Information:".dimmed());
            eprintln!("  Request: {:?}", result.request.source);
            eprintln!("  Options: {:?}", result.request.options);
        }

        Ok(())
    }

    /// JSON output
    fn output_json(&self, result: &IdentificationResult) -> Result<()> {
        let json = serde_json::to_string_pretty(result)?;
        println!("{json}");
        Ok(())
    }

    /// CSV output
    fn output_csv(&self, result: &IdentificationResult) -> Result<()> {
        // CSV header
        println!(
            "status,anime_id,anime_title,episode_id,episode_number,file_id,ed2k,size,group_id,group_name,source"
        );

        // CSV row
        let status = format!("{:?}", result.status);
        let anime_id = result
            .anime
            .as_ref()
            .map(|a| a.aid.to_string())
            .unwrap_or_default();
        let anime_title = result
            .anime
            .as_ref()
            .map(|a| {
                // Use English name if available, otherwise romaji
                if let Some(ref english) = a.english_name {
                    if english != &a.romaji_name {
                        format!("{} ({})", english, a.romaji_name)
                    } else {
                        english.clone()
                    }
                } else {
                    a.romaji_name.clone()
                }
            })
            .unwrap_or_default();
        let episode_id = result
            .episode
            .as_ref()
            .map(|e| e.eid.to_string())
            .unwrap_or_default();
        let episode_number = result
            .episode
            .as_ref()
            .map(|e| e.episode_number.clone())
            .unwrap_or_default();
        let file_id = result
            .file
            .as_ref()
            .map(|f| f.fid.to_string())
            .unwrap_or_default();
        let ed2k = result
            .file
            .as_ref()
            .map(|f| f.ed2k.clone())
            .unwrap_or_default();
        let size = result
            .file
            .as_ref()
            .map(|f| f.size.to_string())
            .unwrap_or_default();
        let group_id = result
            .group
            .as_ref()
            .map(|g| g.gid.to_string())
            .unwrap_or_default();
        let group_name = result
            .group
            .as_ref()
            .map(|g| g.name.clone())
            .unwrap_or_default();
        let source = match &result.source {
            anidb_client_core::identification::DataSource::Cache { .. } => "cache",
            anidb_client_core::identification::DataSource::Network { .. } => "network",
            anidb_client_core::identification::DataSource::Offline => "offline",
        };

        println!(
            "{},{},{},{},{},{},{},{},{},{},{}",
            status,
            anime_id,
            self.escape_csv(&anime_title),
            episode_id,
            episode_number,
            file_id,
            ed2k,
            size,
            group_id,
            self.escape_csv(&group_name),
            source
        );

        Ok(())
    }

    /// Escape CSV fields that contain commas or quotes
    fn escape_csv(&self, field: &str) -> String {
        if field.contains(',') || field.contains('"') || field.contains('\n') {
            format!("\"{}\"", field.replace('"', "\"\""))
        } else {
            field.to_string()
        }
    }
}
