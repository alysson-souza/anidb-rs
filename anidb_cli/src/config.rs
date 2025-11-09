use anidb_client_core::ClientConfig;
use anidb_client_core::security::{Credential, SecureString, create_credential_store};
use anyhow::{Context, Result};
use colored::Colorize;
use dialoguer::{Confirm, Input, Password};
use figment::{
    Figment,
    providers::{Env, Format, Serialized, Toml},
};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

#[derive(Deserialize, Serialize, Debug, Default, Clone)]
pub struct AppConfig {
    #[serde(default)]
    pub client: ClientConfig,

    #[serde(default)]
    pub network: NetworkConfig,

    #[serde(default)]
    pub output: OutputConfig,
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct NetworkConfig {
    pub timeout_seconds: u64,
    pub retry_count: u32,
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct OutputConfig {
    pub default_format: String,
    pub color_enabled: bool,
    pub progress_enabled: bool,
}

impl Default for NetworkConfig {
    fn default() -> Self {
        Self {
            timeout_seconds: 30,
            retry_count: 3,
        }
    }
}

impl Default for OutputConfig {
    fn default() -> Self {
        Self {
            default_format: "text".to_string(),
            color_enabled: true,
            progress_enabled: true,
        }
    }
}

impl AppConfig {
    /// Apply CLI argument overrides to the configuration
    #[allow(dead_code)]
    pub fn apply_cli_overrides(&mut self, chunk_size: Option<usize>) {
        if let Some(size) = chunk_size {
            self.client.chunk_size = size;
        }
    }
}

/// Configuration manager that handles XDG-compliant paths and layered configuration
pub struct ConfigManager {
    config_path: PathBuf,
}

impl Default for ConfigManager {
    fn default() -> Self {
        Self::new()
    }
}

impl ConfigManager {
    /// Create a new ConfigManager with default XDG-compliant paths
    pub fn new() -> Self {
        Self {
            config_path: Self::default_config_path(),
        }
    }

    /// Create a ConfigManager with a specific path (for testing)
    #[allow(dead_code)]
    pub fn with_path(path: PathBuf) -> Self {
        Self { config_path: path }
    }

    /// Get the configuration file path
    pub fn get_config_path(&self) -> PathBuf {
        self.config_path.clone()
    }

    /// Get the default XDG-compliant configuration path
    fn default_config_path() -> PathBuf {
        // Check for XDG_CONFIG_HOME override first (Linux/macOS)
        #[cfg(not(target_os = "windows"))]
        if let Ok(xdg_config) = std::env::var("XDG_CONFIG_HOME") {
            return PathBuf::from(xdg_config).join("anidb/config.toml");
        }

        // Use platform-specific defaults
        #[cfg(target_os = "linux")]
        {
            dirs::home_dir()
                .unwrap_or_else(|| PathBuf::from("."))
                .join(".config/anidb/config.toml")
        }

        #[cfg(target_os = "macos")]
        {
            dirs::home_dir()
                .unwrap_or_else(|| PathBuf::from("."))
                .join("Library/Application Support/anidb/config.toml")
        }

        #[cfg(target_os = "windows")]
        {
            dirs::data_dir()
                .unwrap_or_else(|| PathBuf::from("."))
                .join("anidb\\config.toml")
        }
    }

    /// Load configuration with layered priority: CLI > ENV > File > Defaults
    pub fn load(&self) -> Result<AppConfig> {
        let mut figment = Figment::new();

        // Layer 1: Defaults
        figment = figment.merge(Serialized::defaults(AppConfig::default()));

        // Layer 2: Config file (if exists)
        if self.config_path.exists() {
            figment = figment.merge(Toml::file(&self.config_path));
        }

        // Layer 3: Environment variables
        figment = figment.merge(Env::prefixed("ANIDB_").split("__"));

        figment.extract().context("Failed to load configuration")
    }

    /// Get a configuration value by key (dot notation)
    pub fn get(&self, key: &str) -> Result<String> {
        let config = self.load()?;
        let toml_string = toml::to_string(&config)?;
        let value: toml::Value = toml::from_str(&toml_string)?;

        let parts: Vec<&str> = key.split('.').collect();
        let mut current = &value;

        for part in parts {
            match current {
                toml::Value::Table(table) => {
                    current = table
                        .get(part)
                        .ok_or_else(|| anyhow::anyhow!("Key '{}' not found", key))?;
                }
                _ => anyhow::bail!("Invalid key path: {}", key),
            }
        }

        match current {
            toml::Value::String(s) => Ok(s.clone()),
            toml::Value::Integer(i) => Ok(i.to_string()),
            toml::Value::Float(f) => Ok(f.to_string()),
            toml::Value::Boolean(b) => Ok(b.to_string()),
            _ => anyhow::bail!("Value at '{}' is not a simple type", key),
        }
    }

    /// Set a configuration value by key (dot notation)
    pub fn set(&mut self, key: &str, value: &str) -> Result<()> {
        // Validate the value based on the key
        self.validate_config_value(key, value)?;

        // Load existing config or create new
        let mut config = if self.config_path.exists() {
            let content = fs::read_to_string(&self.config_path)?;
            toml::from_str(&content)?
        } else {
            toml::Value::Table(toml::map::Map::new())
        };

        // Parse the key path
        let parts: Vec<&str> = key.split('.').collect();
        if parts.is_empty() {
            anyhow::bail!("Empty key");
        }

        // Navigate to the correct position and set the value
        let mut current = &mut config;
        for (i, part) in parts.iter().enumerate() {
            if i == parts.len() - 1 {
                // Last part - set the value
                if let toml::Value::Table(table) = current {
                    // Parse the value to the appropriate type
                    let parsed_value = self.parse_config_value(key, value)?;
                    table.insert(part.to_string(), parsed_value);
                } else {
                    anyhow::bail!("Cannot set value on non-table");
                }
            } else {
                // Intermediate part - ensure table exists
                if let toml::Value::Table(table) = current {
                    if !table.contains_key(*part) {
                        table.insert(part.to_string(), toml::Value::Table(toml::map::Map::new()));
                    }
                    current = table.get_mut(*part).unwrap();
                } else {
                    anyhow::bail!("Invalid key path: expected table at '{}'", part);
                }
            }
        }

        // Ensure directory exists
        if let Some(parent) = self.config_path.parent() {
            fs::create_dir_all(parent)?;
        }

        // Write the updated config
        let toml_string = toml::to_string_pretty(&config)?;
        fs::write(&self.config_path, toml_string)?;

        Ok(())
    }

    /// List all configuration values
    pub fn list(&self) -> Result<Vec<(String, String)>> {
        let config = self.load()?;
        let toml_string = toml::to_string(&config)?;
        let value: toml::Value = toml::from_str(&toml_string)?;

        let mut items = Vec::new();
        Self::collect_values(&value, String::new(), &mut items);
        items.sort_by(|a, b| a.0.cmp(&b.0));

        Ok(items)
    }

    /// Recursively collect all key-value pairs from TOML
    fn collect_values(value: &toml::Value, prefix: String, items: &mut Vec<(String, String)>) {
        match value {
            toml::Value::Table(table) => {
                for (key, val) in table {
                    let new_prefix = if prefix.is_empty() {
                        key.clone()
                    } else {
                        format!("{prefix}.{key}")
                    };
                    Self::collect_values(val, new_prefix, items);
                }
            }
            toml::Value::String(s) => items.push((prefix, s.clone())),
            toml::Value::Integer(i) => items.push((prefix, i.to_string())),
            toml::Value::Float(f) => items.push((prefix, f.to_string())),
            toml::Value::Boolean(b) => items.push((prefix, b.to_string())),
            _ => {} // Skip arrays and other complex types
        }
    }

    /// Validate a configuration value
    fn validate_config_value(&self, key: &str, value: &str) -> Result<()> {
        match key {
            "client.chunk_size" => {
                let size: usize = value
                    .parse()
                    .context("chunk_size must be a positive integer")?;
                if size < 1024 {
                    anyhow::bail!("chunk_size must be at least 1024 bytes");
                }
            }
            "client.buffer_size" => {
                let size: usize = value
                    .parse()
                    .context("buffer_size must be a positive integer")?;
                if size == 0 {
                    anyhow::bail!("buffer_size must be greater than 0");
                }
            }
            "network.timeout_seconds" => {
                let timeout: u64 = value
                    .parse()
                    .context("timeout_seconds must be a positive integer")?;
                if timeout == 0 {
                    anyhow::bail!("timeout_seconds must be greater than 0");
                }
            }
            "network.retry_count" => {
                let _: u32 = value
                    .parse()
                    .context("retry_count must be a non-negative integer")?;
            }
            "output.color_enabled" | "output.progress_enabled" => {
                let _: bool = value.parse().context("Value must be 'true' or 'false'")?;
            }
            _ => {} // No validation for unknown keys
        }
        Ok(())
    }

    /// Parse a value to the appropriate TOML type
    fn parse_config_value(&self, key: &str, value: &str) -> Result<toml::Value> {
        // Try to infer type from the key or parse as best fit
        match key {
            k if k.ends_with("_size")
                || k.ends_with("_count")
                || k.ends_with("_seconds")
                || k.ends_with("_files") =>
            {
                let num: i64 = value.parse().context("Expected integer value")?;
                Ok(toml::Value::Integer(num))
            }
            k if k.ends_with("_enabled") => {
                let bool_val: bool = value
                    .parse()
                    .context("Expected boolean value (true/false)")?;
                Ok(toml::Value::Boolean(bool_val))
            }
            // Force string types for these fields
            k if k == "client.client_name" || k == "client.client_version" => {
                Ok(toml::Value::String(value.to_string()))
            }
            _ => {
                // Try parsing as different types
                if let Ok(b) = value.parse::<bool>() {
                    Ok(toml::Value::Boolean(b))
                } else if let Ok(i) = value.parse::<i64>() {
                    Ok(toml::Value::Integer(i))
                } else if let Ok(f) = value.parse::<f64>() {
                    Ok(toml::Value::Float(f))
                } else {
                    Ok(toml::Value::String(value.to_string()))
                }
            }
        }
    }
}

/// Get the default configuration
pub fn get_config() -> Result<AppConfig, Box<figment::Error>> {
    ConfigManager::new()
        .load()
        .map_err(|e| Box::new(figment::Error::from(e.to_string())))
}

/// Get configuration path for CLI display
#[allow(dead_code)]
pub fn get_config_path() -> PathBuf {
    ConfigManager::new().get_config_path()
}

/// Interactive setup wizard for required AniDB credentials and client info
pub async fn interactive_init(force: bool) -> Result<()> {
    println!("{}", "AniDB CLI Setup".bold());
    println!("{}", "===============".bold());
    println!();

    // Check if already configured
    if !force && is_configured().await? {
        let reconfigure = Confirm::new()
            .with_prompt("Configuration already exists. Reconfigure?")
            .default(false)
            .interact()
            .context("Failed to read input")?;

        if !reconfigure {
            println!("Setup cancelled.");
            return Ok(());
        }
    }

    println!("This tool requires:");
    println!("  • An AniDB account (create at https://anidb.net)");
    println!("  • A registered API client (register at https://anidb.net/software/add)");
    println!();

    // Step 1: Credentials
    println!("{}", "AniDB Credentials".bold());

    // Check if credentials already exist
    let store = create_credential_store()
        .await
        .context("Failed to create credential store")?;
    let existing_accounts = store
        .list_accounts("anidb")
        .await
        .context("Failed to list existing accounts")?;

    let default_username = existing_accounts.first().cloned();

    let username: String = if let Some(ref existing) = default_username {
        Input::new()
            .with_prompt("Username")
            .default(existing.clone())
            .interact_text()
            .context("Failed to read username")?
    } else {
        Input::new()
            .with_prompt("Username")
            .interact_text()
            .context("Failed to read username")?
    };

    let password = Password::new()
        .with_prompt("Password")
        .interact()
        .context("Failed to read password")?;

    // Store credentials securely
    let credential = Credential::new("anidb", &username, SecureString::new(password));
    store
        .store(&credential)
        .await
        .context("Failed to store credentials")?;

    println!();

    // Step 2: Client info
    println!("{}", "Client Registration".bold());
    println!("Register your client at: https://anidb.net/software/add");
    println!("Note: Client name is case sensitive");
    println!();

    // Load existing config for defaults
    let mut config_mgr = ConfigManager::new();
    let current = config_mgr.load().ok();

    let default_name = current
        .as_ref()
        .and_then(|c| c.client.client_name.clone())
        .unwrap_or_else(|| "anidbrs".to_string());

    let default_version = current
        .as_ref()
        .and_then(|c| c.client.client_version.clone())
        .unwrap_or_else(|| "1".to_string());

    let client_name: String = Input::new()
        .with_prompt("Client name")
        .default(default_name)
        .interact_text()
        .context("Failed to read client name")?;

    let client_version: String = Input::new()
        .with_prompt("Client version")
        .default(default_version)
        .validate_with(|input: &String| -> Result<(), &str> {
            input
                .parse::<u32>()
                .map(|_| ())
                .map_err(|_| "Must be a positive integer")
        })
        .interact_text()
        .context("Failed to read client version")?;

    // Save configuration
    config_mgr.set("client.client_name", &client_name)?;
    config_mgr.set("client.client_version", &client_version)?;

    println!();
    println!("{}", "✓ Configuration saved".green());
    println!();
    println!("You can now use:");
    println!("  anidb identify <file>  - Identify anime files");
    println!("  anidb sync             - Sync with MyList");

    Ok(())
}

/// Check if configuration is already set up
async fn is_configured() -> Result<bool> {
    // Check credentials
    let store = create_credential_store()
        .await
        .context("Failed to create credential store")?;
    let accounts = store
        .list_accounts("anidb")
        .await
        .context("Failed to list accounts")?;

    // Check client config
    let config = ConfigManager::new().load().ok();
    let has_client = config
        .as_ref()
        .and_then(|c| c.client.client_name.as_ref())
        .is_some();

    Ok(!accounts.is_empty() && has_client)
}
