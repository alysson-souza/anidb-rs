//! Authentication commands for AniDB CLI
//!
//! This module handles user authentication including login, logout,
//! and credential management using secure storage.

use anidb_client_core::security::{Credential, SecureString, create_credential_store};
use anyhow::{Context, Result};
use dialoguer::{Input, Password};

/// Service name for credential storage
const SERVICE_NAME: &str = "anidb";

/// Authenticate with AniDB and store credentials securely
pub async fn login() -> Result<()> {
    println!("AniDB Authentication");
    println!("===================");

    // Get username
    let username: String = Input::new()
        .with_prompt("Username")
        .interact_text()
        .context("Failed to read username")?;

    // Get password (masked input)
    let password = Password::new()
        .with_prompt("Password")
        .interact()
        .context("Failed to read password")?;

    // Convert password to SecureString immediately
    let secure_password = SecureString::new(password);

    // Store credentials
    let store = create_credential_store()
        .await
        .context("Failed to create credential store")?;

    let credential = Credential::new(SERVICE_NAME, &username, secure_password);

    store
        .store(&credential)
        .await
        .context("Failed to store credentials")?;

    println!("\n✓ Credentials stored securely");
    println!("You can now use AniDB commands that require authentication.");

    Ok(())
}

/// Remove stored credentials
pub async fn logout() -> Result<()> {
    let store = create_credential_store()
        .await
        .context("Failed to create credential store")?;

    // List all stored accounts
    let accounts = store
        .list_accounts(SERVICE_NAME)
        .await
        .context("Failed to list accounts")?;

    if accounts.is_empty() {
        println!("No stored credentials found.");
        return Ok(());
    }

    if accounts.len() == 1 {
        // Single account - remove it
        store
            .delete(SERVICE_NAME, &accounts[0])
            .await
            .context("Failed to delete credentials")?;
        println!("✓ Removed credentials for: {}", accounts[0]);
    } else {
        // Multiple accounts - let user choose
        println!("Multiple accounts found:");
        for (i, account) in accounts.iter().enumerate() {
            println!("{}. {}", i + 1, account);
        }

        let choice: String = Input::new()
            .with_prompt("Enter account name to remove (or 'all' to remove all)")
            .interact_text()
            .context("Failed to read choice")?;

        if choice.to_lowercase() == "all" {
            for account in &accounts {
                store
                    .delete(SERVICE_NAME, account)
                    .await
                    .context("Failed to delete credentials")?;
            }
            println!("✓ Removed all stored credentials");
        } else if accounts.contains(&choice) {
            store
                .delete(SERVICE_NAME, &choice)
                .await
                .context("Failed to delete credentials")?;
            println!("✓ Removed credentials for: {choice}");
        } else {
            println!("Account not found: {choice}");
        }
    }

    Ok(())
}

/// Show stored accounts (without exposing passwords)
pub async fn status() -> Result<()> {
    let store = create_credential_store()
        .await
        .context("Failed to create credential store")?;

    let accounts = store
        .list_accounts(SERVICE_NAME)
        .await
        .context("Failed to list accounts")?;

    if accounts.is_empty() {
        println!("No stored credentials found.");
        println!("Use 'anidb auth login' to add credentials.");
    } else {
        println!("Stored AniDB accounts:");
        for account in accounts {
            println!("  • {account}");
        }
    }

    Ok(())
}

/// Get stored credentials for the given username
#[allow(dead_code)] // Will be used when protocol integration is complete
pub async fn get_credentials(username: Option<&str>) -> Result<Option<(String, SecureString)>> {
    let store = create_credential_store()
        .await
        .context("Failed to create credential store")?;

    let accounts = store
        .list_accounts(SERVICE_NAME)
        .await
        .context("Failed to list accounts")?;

    if accounts.is_empty() {
        return Ok(None);
    }

    let account = if let Some(username) = username {
        // Use specified username
        if !accounts.contains(&username.to_string()) {
            return Ok(None);
        }
        username.to_string()
    } else if accounts.len() == 1 {
        // Single account - use it
        accounts[0].clone()
    } else {
        // Multiple accounts - prompt user
        println!("Multiple accounts available:");
        for (i, account) in accounts.iter().enumerate() {
            println!("{}. {}", i + 1, account);
        }

        let choice: usize = Input::new()
            .with_prompt("Select account (enter number)")
            .interact()
            .context("Failed to read choice")?;

        if choice == 0 || choice > accounts.len() {
            anyhow::bail!("Invalid choice");
        }

        accounts[choice - 1].clone()
    };

    let credential = store
        .retrieve(SERVICE_NAME, &account)
        .await
        .context("Failed to retrieve credentials")?;

    Ok(Some((credential.account, credential.secret)))
}

/// Main auth command handler
#[allow(dead_code)] // Will be used when main.rs is updated to support subcommands
pub async fn auth_command(subcommand: Option<&str>) -> Result<()> {
    match subcommand {
        Some("login") => login().await,
        Some("logout") => logout().await,
        Some("status") => status().await,
        None => {
            println!("AniDB Authentication Commands");
            println!("============================");
            println!();
            println!("Usage: anidb auth <subcommand>");
            println!();
            println!("Subcommands:");
            println!("  login   - Store AniDB credentials securely");
            println!("  logout  - Remove stored credentials");
            println!("  status  - Show stored accounts");
            Ok(())
        }
        Some(cmd) => {
            anyhow::bail!("Unknown auth subcommand: {}", cmd);
        }
    }
}
