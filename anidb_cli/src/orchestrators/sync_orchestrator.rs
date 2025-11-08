//! Sync command orchestrator
//!
//! This module handles the business logic for MyList synchronization,
//! coordinating between the CLI and core sync service.

use crate::progress::{create_progress_infrastructure, render_progress};
use crate::sync::{AniDBSyncService, ProcessResult, SyncService, SyncServiceConfig};
use anidb_client_core::database::repositories::{
    FileRepository, HashRepository, sync_queue::SyncQueueRepository,
};
use anidb_client_core::protocol::{ProtocolClient, ProtocolConfig};
use anidb_client_core::security::fallback::EncryptedFileStore;
use anidb_client_core::{ClientConfig, Database};
use anyhow::{Context, Result};
use colored::*;
use log::debug;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::Mutex;

/// Output format for sync results
#[derive(Debug, Clone, Copy)]
pub enum OutputFormat {
    Human,
    Json,
    Minimal,
}

/// Sync command options
#[derive(Debug, Clone)]
pub struct SyncOptions {
    /// Maximum number of items to process
    pub limit: usize,
    /// Enable dry-run mode (no actual syncing)
    pub dry_run: bool,
    /// Output format
    pub format: OutputFormat,
    /// Verbose output
    pub verbose: bool,
}

impl Default for SyncOptions {
    fn default() -> Self {
        Self {
            limit: 100,
            dry_run: false,
            format: OutputFormat::Human,
            verbose: false,
        }
    }
}

/// Orchestrator for the sync command
pub struct SyncOrchestrator {
    service: Arc<dyn SyncService>,
    sync_repo: Arc<SyncQueueRepository>,
    options: SyncOptions,
}

impl SyncOrchestrator {
    /// Create a new sync orchestrator
    pub async fn new(_client_config: ClientConfig, options: SyncOptions) -> Result<Self> {
        debug!("Creating sync orchestrator with options: {options:?}");

        // Get data directory (XDG compliant)
        let data_dir = dirs::data_dir()
            .map(|d| d.join("anidb"))
            .unwrap_or_else(|| std::path::PathBuf::from(".anidb"));

        // Ensure data directory exists
        std::fs::create_dir_all(&data_dir).context("Failed to create data directory")?;

        // Open database
        let db_path = data_dir.join("anidb.db");
        let db = Database::new(&db_path)
            .await
            .context("Failed to open database")?;

        // Create repositories
        let sync_repo = Arc::new(SyncQueueRepository::new(db.pool().clone()));
        let file_repo = Arc::new(FileRepository::new(db.pool().clone()));
        let hash_repo = Arc::new(HashRepository::new(db.pool().clone()));

        // Create protocol client
        let protocol_config = ProtocolConfig::default();
        let protocol_client = Arc::new(Mutex::new(
            ProtocolClient::new(protocol_config)
                .await
                .context("Failed to create protocol client")?,
        ));

        // Create credential store
        let credential_store = Arc::new(
            EncryptedFileStore::new()
                .await
                .context("Failed to create credential store")?,
        );

        // Create sync service
        let service_config = SyncServiceConfig {
            verbose: options.verbose,
            ..Default::default()
        };

        let service = Arc::new(AniDBSyncService::new(
            sync_repo.clone(),
            file_repo,
            hash_repo,
            protocol_client,
            credential_store,
            service_config,
        ));

        debug!("Sync orchestrator created successfully");
        Ok(Self {
            service,
            sync_repo,
            options,
        })
    }

    /// Process all pending sync items
    pub async fn sync_all(&self) -> Result<()> {
        println!("{}", "Starting MyList synchronization...".cyan().bold());

        if self.options.dry_run {
            println!(
                "{}",
                "DRY RUN MODE - No actual changes will be made".yellow()
            );
        }

        let start = Instant::now();

        // Get queue statistics first
        let stats = self.service.get_stats().await?;

        if stats.pending_count == 0 {
            println!("{}", "No pending items in sync queue".green());
            return Ok(());
        }

        println!(
            "Found {} pending items to sync",
            stats.pending_count.to_string().cyan()
        );

        if self.options.dry_run {
            self.display_pending_items().await?;
            return Ok(());
        }

        // Create progress infrastructure
        let (provider, rx) = create_progress_infrastructure();

        // Spawn progress renderer in background
        let progress_handle = tokio::spawn(render_progress(rx));

        // Process the queue
        let result = self
            .service
            .process_queue(self.options.limit)
            .await
            .context("Failed to process sync queue")?;

        // Signal completion
        provider.complete();

        // Wait for renderer to finish
        let _ = tokio::time::timeout(Duration::from_millis(100), progress_handle).await;

        // Display results
        self.display_results(&result, start.elapsed());

        Ok(())
    }

    /// Show pending sync items
    pub async fn show_pending(&self) -> Result<()> {
        let stats = self.service.get_stats().await?;

        match self.options.format {
            OutputFormat::Json => {
                let json = serde_json::to_string_pretty(&stats)?;
                println!("{json}");
            }
            OutputFormat::Minimal => {
                println!("{}", stats.pending_count);
            }
            OutputFormat::Human => {
                println!("{}", "MyList Sync Queue Status".cyan().bold());
                println!("{}", "=========================".cyan());
                println!();

                println!("  {} Pending:      {}", "●".yellow(), stats.pending_count);
                println!("  {} In Progress:  {}", "●".blue(), stats.in_progress_count);
                println!("  {} Completed:    {}", "●".green(), stats.completed_count);
                println!("  {} Failed:       {}", "●".red(), stats.failed_count);

                if stats.retriable_count > 0 {
                    println!();
                    println!("  {} items can be retried", stats.retriable_count);
                }

                if stats.pending_count > 0 {
                    println!();
                    println!("Run {} to process pending items", "anidb sync all".cyan());
                }
            }
        }

        Ok(())
    }

    /// Show sync statistics
    pub async fn show_status(&self) -> Result<()> {
        let stats = self.service.get_stats().await?;
        let total = stats.pending_count
            + stats.in_progress_count
            + stats.completed_count
            + stats.failed_count;

        match self.options.format {
            OutputFormat::Json => {
                let json = serde_json::to_string_pretty(&stats)?;
                println!("{json}");
            }
            OutputFormat::Minimal => {
                println!("{}/{}", stats.completed_count, total);
            }
            OutputFormat::Human => {
                println!("{}", "MyList Sync Statistics".cyan().bold());
                println!("{}", "======================".cyan());
                println!();

                if total == 0 {
                    println!("No sync operations recorded");
                    return Ok(());
                }

                // Calculate percentages
                let completed_pct = (stats.completed_count as f64 / total as f64) * 100.0;
                let failed_pct = (stats.failed_count as f64 / total as f64) * 100.0;

                // Display progress bar
                let bar_width: usize = 40;
                let completed_width = ((completed_pct / 100.0) * bar_width as f64) as usize;
                let failed_width = ((failed_pct / 100.0) * bar_width as f64) as usize;
                let pending_width = bar_width.saturating_sub(completed_width + failed_width);

                print!("Progress: [");
                print!("{}", "█".repeat(completed_width).green());
                print!("{}", "█".repeat(failed_width).red());
                print!("{}", "░".repeat(pending_width).dimmed());
                println!("] {completed_pct:.1}%");

                println!();
                println!("Total Operations: {}", total.to_string().bold());
                println!();
                println!(
                    "  {} Completed:    {} ({:.1}%)",
                    "✓".green(),
                    stats.completed_count,
                    completed_pct
                );
                println!(
                    "  {} Failed:       {} ({:.1}%)",
                    "✗".red(),
                    stats.failed_count,
                    failed_pct
                );
                println!("  {} Pending:      {}", "○".yellow(), stats.pending_count);
                println!("  {} In Progress:  {}", "●".blue(), stats.in_progress_count);

                if stats.retriable_count > 0 {
                    println!();
                    println!(
                        "{} {} items can be retried",
                        "ℹ".blue(),
                        stats.retriable_count
                    );
                }
            }
        }

        Ok(())
    }

    /// Clear completed sync items
    pub async fn clear_completed(&self, days: u32) -> Result<()> {
        let max_age = Duration::from_secs(days as u64 * 24 * 60 * 60);

        let count = self
            .service
            .clear_completed(max_age)
            .await
            .context("Failed to clear completed items")?;

        match self.options.format {
            OutputFormat::Json => {
                println!(r#"{{"cleared": {count}}}"#);
            }
            OutputFormat::Minimal => {
                println!("{count}");
            }
            OutputFormat::Human => {
                if count > 0 {
                    println!(
                        "✓ Cleared {} completed sync items older than {} days",
                        count.to_string().green(),
                        days
                    );
                } else {
                    println!("No completed items older than {days} days to clear");
                }
            }
        }

        Ok(())
    }

    /// Display pending items (for dry-run mode)
    async fn display_pending_items(&self) -> Result<()> {
        let items = self.sync_repo.find_ready(self.options.limit as i64).await?;

        if items.is_empty() {
            println!("No items ready for processing");
            return Ok(());
        }

        println!();
        println!("Items that would be processed:");
        println!("{}", "─".repeat(60));

        for (i, item) in items.iter().enumerate() {
            println!(
                "{}. File ID: {} | Operation: {} | Priority: {} | Retries: {}/{}",
                i + 1,
                item.file_id.to_string().cyan(),
                item.operation.yellow(),
                item.priority,
                item.retry_count,
                item.max_retries
            );

            if let Some(error) = &item.error_message {
                println!("   Last error: {}", error.red());
            }
        }

        Ok(())
    }

    /// Display sync results
    fn display_results(&self, result: &ProcessResult, elapsed: Duration) {
        println!();
        println!("{}", "Sync Complete".green().bold());
        println!("{}", "=============".green());
        println!();

        println!(
            "Processed:       {} items",
            result.processed.to_string().bold()
        );
        println!("  {} Succeeded:    {}", "✓".green(), result.succeeded);
        println!(
            "  {} Already in list: {}",
            "○".yellow(),
            result.already_in_list
        );
        println!("  {} Failed:       {}", "✗".red(), result.failed);
        println!();
        println!("Time elapsed:    {:.2}s", elapsed.as_secs_f64());

        if result.succeeded > 0 {
            let rate = result.succeeded as f64 / elapsed.as_secs_f64();
            println!("Sync rate:       {rate:.1} items/sec");
        }
    }
}

/// Retry failed sync items
pub async fn retry_failed(_client_config: ClientConfig, options: SyncOptions) -> Result<()> {
    debug!("Retrying failed sync items");

    // Get data directory (XDG compliant)
    let data_dir = dirs::data_dir()
        .map(|d| d.join("anidb"))
        .unwrap_or_else(|| std::path::PathBuf::from(".anidb"));

    // Ensure data directory exists
    std::fs::create_dir_all(&data_dir).context("Failed to create data directory")?;

    // Open database
    let db_path = data_dir.join("anidb.db");
    let db = Database::new(&db_path)
        .await
        .context("Failed to open database")?;

    let sync_repo = SyncQueueRepository::new(db.pool().clone());

    // Find retriable items
    let items = sync_repo.find_retriable(options.limit as i64).await?;

    if items.is_empty() {
        println!("No failed items to retry");
        return Ok(());
    }

    println!(
        "Found {} failed items to retry",
        items.len().to_string().cyan()
    );

    if options.dry_run {
        println!(
            "{}",
            "DRY RUN MODE - No actual changes will be made".yellow()
        );
        return Ok(());
    }

    // Reset retry delay for all items
    let ids: Vec<i64> = items.iter().map(|i| i.id).collect();
    let affected = sync_repo.batch_retry(&ids, 0).await?;

    println!(
        "✓ Scheduled {} items for retry",
        affected.to_string().green()
    );
    println!();
    println!("Run {} to process them", "anidb sync all".cyan());

    Ok(())
}
