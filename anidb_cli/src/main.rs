use anyhow::{Context, Result};
use clap::{CommandFactory, Parser, Subcommand};
use clap_complete::{Shell, generate};
use colored::*;
use std::path::PathBuf;
use std::time::Instant;
use tokio::sync::mpsc;

mod auth;
mod cache;
mod config;
mod file_discovery;
mod orchestrators;
mod progress;
mod sync;
mod terminal;

use crate::cache::{factory::CacheFactory, service::HashCacheService};
use crate::config::{AppConfig, ConfigManager, get_config};
use anidb_client_core::progress::NullProvider;
use anidb_client_core::{AniDBClient, HashAlgorithm, ProcessOptions};
use std::sync::Arc;

#[derive(Parser)]
#[command(name = "anidb")]
#[command(author, version, about = "AniDB Client - File processing and anime database management", long_about = None)]
struct Cli {
    /// Enable debug logging
    #[arg(short, long, global = true)]
    debug: bool,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Calculate hash(es) for file(s)
    Hash {
        /// File or directory to hash
        path: PathBuf,

        /// Hash algorithm to use
        #[arg(short, long, value_enum, default_value = "ed2k")]
        algorithm: HashAlgorithmArg,

        /// Output format
        #[arg(short, long, value_enum, default_value = "text")]
        format: OutputFormat,

        /// Include patterns (glob patterns, can be specified multiple times)
        #[arg(short = 'i', long = "include", value_name = "PATTERN")]
        include_patterns: Vec<String>,

        /// Exclude patterns (glob patterns, can be specified multiple times, overrides includes)
        #[arg(short = 'e', long = "exclude", value_name = "PATTERN")]
        exclude_patterns: Vec<String>,

        /// Don't use default media extensions when no include patterns are specified
        #[arg(long)]
        no_defaults: bool,

        /// Process recursively (for directories)
        #[arg(short, long)]
        recursive: bool,

        /// Disable progress bar display
        #[arg(long)]
        no_progress: bool,

        /// Bypass cache and recalculate hashes
        #[arg(long)]
        no_cache: bool,
    },

    /// Identify file(s) via AniDB
    Identify {
        /// File or directory to identify
        path: PathBuf,

        /// Include patterns (glob patterns, can be specified multiple times)
        #[arg(short = 'i', long = "include", value_name = "PATTERN")]
        include_patterns: Vec<String>,

        /// Exclude patterns (glob patterns, can be specified multiple times, overrides includes)
        #[arg(short = 'e', long = "exclude", value_name = "PATTERN")]
        exclude_patterns: Vec<String>,

        /// Don't use default media extensions when no include patterns are specified
        #[arg(long)]
        no_defaults: bool,

        /// Process recursively
        #[arg(short, long)]
        recursive: bool,

        /// Enable verbose output
        #[arg(short, long)]
        verbose: bool,

        /// Bypass cache and force network query
        #[arg(long)]
        no_cache: bool,
    },

    /// Manage configuration
    Config {
        #[command(subcommand)]
        command: ConfigCommand,
    },

    /// Authenticate with AniDB
    Auth {
        #[command(subcommand)]
        command: AuthCommand,
    },

    /// Sync files with AniDB MyList
    Sync(sync::SyncCommand),

    /// Generate shell completions
    Completions {
        /// Shell to generate completions for
        #[arg(value_enum)]
        shell: clap_complete::Shell,
    },
}

#[derive(Subcommand)]
enum ConfigCommand {
    /// Interactive setup for required AniDB credentials and client info
    Init {
        /// Reconfigure even if already set up
        #[arg(short, long)]
        force: bool,
    },

    /// Get a configuration value
    Get {
        /// Configuration key (e.g., client.memory_limit_mb)
        key: String,
    },

    /// Set a configuration value
    Set {
        /// Configuration key (e.g., client.memory_limit_mb)
        key: String,

        /// Value to set
        value: String,
    },

    /// List all configuration values
    List,
}

#[derive(Subcommand)]
enum AuthCommand {
    /// Login to AniDB and store credentials securely
    Login,

    /// Logout from AniDB and remove stored credentials
    Logout,

    /// Show authentication status
    Status,
}

#[derive(clap::ValueEnum, Clone, Debug)]
enum HashAlgorithmArg {
    Ed2k,
    Crc32,
    Md5,
    Sha1,
    Tth,
    All,
}

#[derive(clap::ValueEnum, Clone, Debug)]
enum OutputFormat {
    Text,
    Json,
    Csv,
}

impl From<HashAlgorithmArg> for Vec<HashAlgorithm> {
    fn from(arg: HashAlgorithmArg) -> Self {
        match arg {
            HashAlgorithmArg::Ed2k => vec![HashAlgorithm::ED2K],
            HashAlgorithmArg::Crc32 => vec![HashAlgorithm::CRC32],
            HashAlgorithmArg::Md5 => vec![HashAlgorithm::MD5],
            HashAlgorithmArg::Sha1 => vec![HashAlgorithm::SHA1],
            HashAlgorithmArg::Tth => vec![HashAlgorithm::TTH],
            HashAlgorithmArg::All => vec![
                HashAlgorithm::ED2K,
                HashAlgorithm::CRC32,
                HashAlgorithm::MD5,
                HashAlgorithm::SHA1,
                HashAlgorithm::TTH,
            ],
        }
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    // Initialize logging based on debug flag
    if cli.debug {
        env_logger::Builder::from_env(env_logger::Env::default())
            .filter_level(log::LevelFilter::Debug)
            .filter_module("anidb_client_core", log::LevelFilter::Debug)
            .filter_module("anidb_cli", log::LevelFilter::Debug)
            .format_timestamp_millis()
            .init();
        eprintln!("Debug logging enabled");
    } else {
        env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("warn")).init();
    }

    let config = get_config().context("Failed to load configuration")?;

    match cli.command {
        Commands::Hash {
            path,
            algorithm,
            format,
            include_patterns,
            exclude_patterns,
            no_defaults,
            recursive,
            no_progress,
            no_cache,
        } => {
            hash_command(
                config,
                path,
                algorithm,
                format,
                include_patterns,
                exclude_patterns,
                !no_defaults,
                recursive,
                no_progress,
                no_cache,
            )
            .await?;
        }
        Commands::Identify {
            path,
            include_patterns,
            exclude_patterns,
            no_defaults,
            recursive,
            verbose,
            no_cache,
        } => {
            use anidb_cli::orchestrators::identify_orchestrator::{
                DirectoryIdentifyOptions, IdentifyOrchestrator, OutputFormat,
            };

            log::debug!("Starting identify command for path: {path:?}");
            log::debug!("Include patterns: {include_patterns:?}");
            log::debug!("Exclude patterns: {exclude_patterns:?}");
            log::debug!("Use defaults: {}", !no_defaults);
            log::debug!("Recursive: {recursive}");
            log::debug!("Verbose: {verbose}");
            log::debug!("No cache: {no_cache}");

            // Determine output format based on terminal
            let format = if terminal::is_interactive() {
                OutputFormat::Human
            } else {
                OutputFormat::Json
            };

            // Create orchestrator
            log::debug!("Creating identify orchestrator...");
            let orchestrator = IdentifyOrchestrator::new(config.client, verbose)
                .await
                .context("Failed to create identify orchestrator")?;
            log::debug!("Orchestrator created successfully");

            // Process based on path type
            if path.is_file() {
                log::debug!("Path is a file, identifying single file");
                orchestrator.identify_file(&path, format, no_cache).await?;
            } else if path.is_dir() {
                log::debug!("Path is a directory, identifying multiple files");
                let options = DirectoryIdentifyOptions {
                    recursive,
                    format,
                    include_patterns,
                    exclude_patterns,
                    use_defaults: !no_defaults,
                    no_cache,
                };
                orchestrator.identify_directory(&path, options).await?;
            } else {
                log::warn!("Path does not exist: {}", path.display());
                anyhow::bail!("Path does not exist: {}", path.display());
            }
        }
        Commands::Config { command } => {
            config_command(command).await?;
        }
        Commands::Auth { command } => {
            auth_command(command).await?;
        }
        Commands::Sync(command) => {
            sync::execute(command).await?;
        }
        Commands::Completions { shell } => {
            generate_completions(shell);
        }
    }

    Ok(())
}

#[allow(clippy::too_many_arguments)]
async fn hash_command(
    config: AppConfig,
    path: PathBuf,
    algorithm: HashAlgorithmArg,
    format: OutputFormat,
    include_patterns: Vec<String>,
    exclude_patterns: Vec<String>,
    use_defaults: bool,
    recursive: bool,
    no_progress: bool,
    no_cache: bool,
) -> Result<()> {
    use anidb_cli::file_discovery::{FileDiscovery, FileDiscoveryOptions};

    if !path.exists() {
        anyhow::bail!("Path not found: {}", path.display());
    }

    // Collect files to process
    let files_to_process = if path.is_file() {
        // Single file processing
        vec![path.clone()]
    } else if path.is_dir() {
        // Directory processing with patterns
        eprintln!("{}", "Discovering files...".bold().cyan());

        let options = FileDiscoveryOptions::new()
            .with_include_patterns(include_patterns.clone())
            .with_exclude_patterns(exclude_patterns.clone())
            .with_use_defaults(use_defaults)
            .with_recursive(recursive);

        let discovery = FileDiscovery::new(&path, options)?;
        let files: Vec<_> = discovery
            .map(|entry| entry.map(|e| e.path))
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| anyhow::anyhow!("File discovery error: {}", e))?;

        if files.is_empty() {
            eprintln!("{}", "No matching files found.".yellow());
            return Ok(());
        }

        eprintln!("Found {} file(s) to hash", files.len());
        files
    } else {
        anyhow::bail!("Path is neither a file nor a directory: {}", path.display());
    };

    // Create the AniDB client
    let client = Arc::new(
        AniDBClient::new(config.client.clone())
            .await
            .context("Failed to create AniDB client")?,
    );

    // Create the cache based on configuration
    let cache = if no_cache {
        // Use no-op cache when caching is disabled
        CacheFactory::noop()?
    } else {
        // Get data directory for cache (XDG compliant)
        let cache_dir = dirs::data_dir()
            .map(|d| d.join("anidb/cache"))
            .unwrap_or_else(|| std::path::PathBuf::from(".anidb/cache"));
        // Use file cache with the configured cache directory
        CacheFactory::file(cache_dir)?
    };

    // Create the cache service
    let cache_service = HashCacheService::new(client, cache);

    let algorithms: Vec<HashAlgorithm> = algorithm.clone().into();

    // Determine if we should show progress
    let show_progress = !no_progress && terminal::should_show_progress_by_default();

    // Create progress infrastructure
    let (progress_provider, progress_rx) = if show_progress {
        let (tx, rx) = mpsc::channel(100);
        let provider = Arc::new(progress::provider::ChannelProvider::new(tx))
            as Arc<dyn anidb_client_core::progress::ProgressProvider>;
        (provider, Some(rx))
    } else {
        (
            Arc::new(NullProvider) as Arc<dyn anidb_client_core::progress::ProgressProvider>,
            None,
        )
    };

    let options = ProcessOptions::new()
        .with_algorithms(&algorithms)
        .with_progress_reporting(show_progress);

    // Create progress renderer (new ProgressUpdate-based)
    let progress_handle =
        progress_rx.map(|rx| tokio::spawn(progress::renderer::render_progress(rx)));

    // Process all files
    let mut all_results = Vec::new();
    let total_start = Instant::now();

    for file in &files_to_process {
        if files_to_process.len() > 1 {
            eprintln!("\nProcessing: {}", file.display());
        }

        let start = Instant::now();
        // Use the cache service instead of direct client call
        let result = cache_service
            .process_file_with_cache_and_progress(
                file,
                options.clone(),
                !no_cache, // use_cache is the inverse of no_cache
                progress_provider.clone(),
            )
            .await
            .context(format!("Failed to process file: {}", file.display()))?;
        let duration = start.elapsed();

        all_results.push((file.clone(), result, duration));
    }

    let total_duration = total_start.elapsed();

    // Wait for progress renderer to finish (channel closes on completion)
    // Signal completion so the renderer can exit its loop
    if show_progress {
        progress_provider.complete();
    }
    if let Some(handle) = progress_handle {
        let _ = handle.await;
    }

    // Output results
    match format {
        OutputFormat::Text => {
            if terminal::is_interactive() {
                // Rich output for interactive terminals
                eprintln!("\n{}", "Hash Results:".bold().green());

                for (file, result, duration) in &all_results {
                    if files_to_process.len() > 1 {
                        eprintln!("\nFile: {}", file.display());
                    } else {
                        eprintln!("File: {}", file.display());
                    }
                    eprintln!(
                        "Size: {} ({})",
                        progress::format_bytes(result.file_size),
                        result.file_size
                    );

                    for (algo, hash) in &result.hashes {
                        println!("{}: {}", format!("{algo:?}").yellow(), hash.cyan());
                    }

                    if files_to_process.len() == 1 {
                        eprintln!("\nTime: {:.2}s", duration.as_secs_f64());
                        if show_progress {
                            let throughput =
                                (result.file_size as f64 / 1_048_576.0) / duration.as_secs_f64();
                            eprintln!("Throughput: {}", progress::format_throughput(throughput));
                        }
                    }
                }

                if files_to_process.len() > 1 {
                    eprintln!("\n{}", "Summary:".bold().green());
                    eprintln!("Files processed: {}", files_to_process.len());
                    eprintln!("Total time: {:.2}s", total_duration.as_secs_f64());

                    let total_size: u64 = all_results.iter().map(|(_, r, _)| r.file_size).sum();
                    eprintln!("Total size: {}", progress::format_bytes(total_size));

                    if show_progress {
                        let throughput =
                            (total_size as f64 / 1_048_576.0) / total_duration.as_secs_f64();
                        eprintln!(
                            "Average throughput: {}",
                            progress::format_throughput(throughput)
                        );
                    }
                }
            } else {
                // Simple output for piping
                for (file, result, _) in &all_results {
                    for (algo, hash) in &result.hashes {
                        if files_to_process.len() > 1 {
                            println!("{}: {algo:?}: {hash}", file.display());
                        } else {
                            println!("{algo:?}: {hash}");
                        }
                    }
                }
            }
        }
        OutputFormat::Json => {
            if files_to_process.len() == 1 {
                let (_, result, _) = &all_results[0];
                let json = serde_json::to_string_pretty(&result)?;
                println!("{json}");
            } else {
                // For multiple files, create a JSON array
                let json_results: Vec<_> = all_results
                    .iter()
                    .map(|(file, result, _)| {
                        serde_json::json!({
                            "file": file.display().to_string(),
                            "size": result.file_size,
                            "hashes": result.hashes,
                            "processing_time_ms": result.processing_time.as_millis()
                        })
                    })
                    .collect();
                let json = serde_json::to_string_pretty(&json_results)?;
                println!("{json}");
            }
        }
        OutputFormat::Csv => {
            println!("file,algorithm,hash,size,time_ms");
            for (file, result, _) in &all_results {
                for (algo, hash) in &result.hashes {
                    println!(
                        "{},{:?},{},{},{}",
                        file.display(),
                        algo,
                        hash,
                        result.file_size,
                        result.processing_time.as_millis()
                    );
                }
            }
        }
    }

    Ok(())
}

async fn config_command(command: ConfigCommand) -> Result<()> {
    let mut manager = ConfigManager::new();

    match command {
        ConfigCommand::Init { force } => {
            config::interactive_init(force).await?;
        }
        ConfigCommand::Get { key } => match manager.get(&key) {
            Ok(value) => {
                println!("{value}");
            }
            Err(e) => {
                eprintln!("{}", format!("Error: {e}").red());
                std::process::exit(1);
            }
        },
        ConfigCommand::Set { key, value } => match manager.set(&key, &value) {
            Ok(()) => {
                eprintln!("{}", format!("Set {key} = {value}").green());
                eprintln!(
                    "Configuration saved to: {}",
                    manager.get_config_path().display()
                );
            }
            Err(e) => {
                eprintln!("{}", format!("Error: {e}").red());
                std::process::exit(1);
            }
        },
        ConfigCommand::List => {
            match manager.list() {
                Ok(items) => {
                    if items.is_empty() {
                        eprintln!("No configuration values set. Using defaults.");
                        eprintln!("Config file: {}", manager.get_config_path().display());
                    } else {
                        eprintln!("{}", "Configuration:".bold().blue());
                        eprintln!("Config file: {}", manager.get_config_path().display());
                        eprintln!();

                        // Group items by section
                        let mut sections: std::collections::HashMap<String, Vec<(String, String)>> =
                            std::collections::HashMap::new();

                        for (key, value) in items {
                            let section = key.split('.').next().unwrap_or("general");
                            sections
                                .entry(section.to_string())
                                .or_default()
                                .push((key, value));
                        }

                        // Display by section
                        for (section, mut items) in sections {
                            eprintln!("[{}]", section.yellow());
                            items.sort_by(|a, b| a.0.cmp(&b.0));

                            for (key, value) in items {
                                let key_parts: Vec<&str> = key.split('.').collect();
                                let display_key = key_parts[1..].join(".");
                                eprintln!("  {} = {}", display_key.cyan(), value);
                            }
                            eprintln!();
                        }
                    }
                }
                Err(e) => {
                    eprintln!("{}", format!("Error: {e}").red());
                    std::process::exit(1);
                }
            }
        }
    }

    Ok(())
}

async fn auth_command(command: AuthCommand) -> Result<()> {
    match command {
        AuthCommand::Login => auth::login().await,
        AuthCommand::Logout => auth::logout().await,
        AuthCommand::Status => auth::status().await,
    }
}

fn generate_completions(shell: Shell) {
    let mut cmd = Cli::command();
    let name = cmd.get_name().to_string();

    generate(shell, &mut cmd, name, &mut std::io::stdout());
}
