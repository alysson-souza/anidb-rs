//! Sync commands for MyList management
//!
//! This module provides commands for synchronizing local files
//! with the user's MyList on AniDB.

pub mod service;

pub use service::{AniDBSyncService, ProcessResult, SyncService, SyncServiceConfig};

use crate::config::get_config;
use crate::orchestrators::sync_orchestrator::{
    OutputFormat, SyncOptions, SyncOrchestrator, retry_failed,
};
use anyhow::{Context, Result};
use clap::{Args, Subcommand};

/// Sync commands for MyList management
#[derive(Debug, Args)]
pub struct SyncCommand {
    /// Sync subcommand
    #[command(subcommand)]
    pub command: SyncSubcommand,

    /// Enable verbose output
    #[arg(short, long, global = true)]
    pub verbose: bool,

    /// Output format
    #[arg(short = 'f', long, value_enum, default_value = "human", global = true)]
    pub format: OutputFormatArg,
}

/// Output format argument
#[derive(Debug, Clone, Copy, clap::ValueEnum)]
pub enum OutputFormatArg {
    /// Human-readable output
    Human,
    /// JSON output
    Json,
    /// Minimal output (for scripting)
    Minimal,
}

impl From<OutputFormatArg> for OutputFormat {
    fn from(arg: OutputFormatArg) -> Self {
        match arg {
            OutputFormatArg::Human => OutputFormat::Human,
            OutputFormatArg::Json => OutputFormat::Json,
            OutputFormatArg::Minimal => OutputFormat::Minimal,
        }
    }
}

/// Sync subcommands
#[derive(Debug, Subcommand)]
pub enum SyncSubcommand {
    /// Process all pending sync items
    All {
        /// Maximum number of items to process
        #[arg(short, long, default_value = "100")]
        limit: usize,

        /// Dry-run mode (show what would be done without making changes)
        #[arg(short = 'n', long)]
        dry_run: bool,
    },

    /// Show pending sync items
    Pending {
        /// Maximum number of items to show
        #[arg(short, long, default_value = "50")]
        limit: usize,
    },

    /// Show sync statistics
    Status,

    /// Retry failed sync items
    Retry {
        /// Maximum number of items to retry
        #[arg(short, long, default_value = "50")]
        limit: usize,

        /// Dry-run mode
        #[arg(short = 'n', long)]
        dry_run: bool,
    },

    /// Clear all sync items unconditionally
    Clear,
}

/// Execute sync command
pub async fn execute(cmd: SyncCommand) -> Result<()> {
    let app_config = get_config()?;

    // Use the client config directly from app config
    let client_config = app_config.client.clone();

    match cmd.command {
        SyncSubcommand::All { limit, dry_run } => {
            let options = SyncOptions {
                limit,
                dry_run,
                format: cmd.format.into(),
                verbose: cmd.verbose,
            };

            let orchestrator = SyncOrchestrator::new(client_config.clone(), options)
                .await
                .context("Failed to initialize sync orchestrator")?;

            orchestrator
                .sync_all()
                .await
                .context("Failed to sync MyList")?;
        }

        SyncSubcommand::Pending { limit } => {
            let options = SyncOptions {
                limit,
                dry_run: false,
                format: cmd.format.into(),
                verbose: cmd.verbose,
            };

            let orchestrator = SyncOrchestrator::new(client_config.clone(), options)
                .await
                .context("Failed to initialize sync orchestrator")?;

            orchestrator
                .show_pending()
                .await
                .context("Failed to show pending items")?;
        }

        SyncSubcommand::Status => {
            let options = SyncOptions {
                format: cmd.format.into(),
                verbose: cmd.verbose,
                ..Default::default()
            };

            let orchestrator = SyncOrchestrator::new(client_config.clone(), options)
                .await
                .context("Failed to initialize sync orchestrator")?;

            orchestrator
                .show_status()
                .await
                .context("Failed to show sync status")?;
        }

        SyncSubcommand::Retry { limit, dry_run } => {
            let options = SyncOptions {
                limit,
                dry_run,
                format: cmd.format.into(),
                verbose: cmd.verbose,
            };

            retry_failed(client_config.clone(), options)
                .await
                .context("Failed to retry failed items")?;
        }

        SyncSubcommand::Clear => {
            let options = SyncOptions {
                format: cmd.format.into(),
                verbose: cmd.verbose,
                ..Default::default()
            };

            let orchestrator = SyncOrchestrator::new(client_config.clone(), options)
                .await
                .context("Failed to initialize sync orchestrator")?;

            orchestrator
                .clear_all()
                .await
                .context("Failed to clear sync queue")?;
        }
    }

    Ok(())
}
