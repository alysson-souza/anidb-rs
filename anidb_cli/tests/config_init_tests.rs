//! Unit tests for interactive config initialization
//!
//! These tests verify the configuration setup wizard behavior,
//! including credential storage, client config validation, and
//! proper handling of existing configurations.

use anidb_cli::config::ConfigManager;
use std::fs;
use tempfile::TempDir;

// Test helper: Create a temporary config manager with isolated directory
fn create_test_config_manager(temp_dir: &TempDir) -> ConfigManager {
    let config_path = temp_dir.path().join("config.toml");
    ConfigManager::with_path(config_path)
}

#[test]
fn test_config_manager_set_client_name() {
    let temp_dir = TempDir::new().unwrap();
    let mut config_manager = create_test_config_manager(&temp_dir);

    let result = config_manager.set("client.client_name", "myawesomeclient");

    assert!(result.is_ok());

    // Verify the config was saved
    let config_path = temp_dir.path().join("config.toml");
    assert!(config_path.exists());

    // Verify the value can be read back
    let value = config_manager.get("client.client_name").unwrap();
    assert_eq!(value, "myawesomeclient");
}

#[test]
fn test_config_manager_set_client_version() {
    let temp_dir = TempDir::new().unwrap();
    let mut config_manager = create_test_config_manager(&temp_dir);

    let result = config_manager.set("client.client_version", "1");

    assert!(result.is_ok());

    let value = config_manager.get("client.client_version").unwrap();
    assert_eq!(value, "1");
}

#[test]
fn test_config_manager_set_both_client_fields() {
    let temp_dir = TempDir::new().unwrap();
    let mut config_manager = create_test_config_manager(&temp_dir);

    // Set client name
    config_manager
        .set("client.client_name", "testclient")
        .unwrap();

    // Set client version
    config_manager.set("client.client_version", "2").unwrap();

    // Verify both can be read
    let name = config_manager.get("client.client_name").unwrap();
    let version = config_manager.get("client.client_version").unwrap();

    assert_eq!(name, "testclient");
    assert_eq!(version, "2");
}

#[test]
fn test_config_manager_allows_any_string_for_client_version() {
    let temp_dir = TempDir::new().unwrap();
    let mut config_manager = create_test_config_manager(&temp_dir);

    // ConfigManager stores client_version as string (validation happens in interactive_init)
    let result = config_manager.set("client.client_version", "not_a_number");

    // Should succeed - validation happens in the interactive setup
    assert!(result.is_ok());
    let value = config_manager.get("client.client_version").unwrap();
    assert_eq!(value, "not_a_number");
}

#[test]
fn test_config_manager_accepts_valid_integer_versions() {
    let temp_dir = TempDir::new().unwrap();
    let mut config_manager = create_test_config_manager(&temp_dir);

    let versions = vec!["0", "1", "99", "65535"];

    for version in versions {
        let result = config_manager.set("client.client_version", version);
        assert!(result.is_ok(), "Failed to set version: {}", version);

        let stored = config_manager.get("client.client_version").unwrap();
        assert_eq!(stored, version);
    }
}

#[test]
fn test_config_manager_preserves_existing_config_on_new_set() {
    let temp_dir = TempDir::new().unwrap();
    let mut config_manager = create_test_config_manager(&temp_dir);

    // Set first value
    config_manager
        .set("client.client_name", "firstclient")
        .unwrap();

    // Set second value
    config_manager.set("client.client_version", "5").unwrap();

    // Verify first value is still there
    let name = config_manager.get("client.client_name").unwrap();
    assert_eq!(name, "firstclient");

    // Verify second value is set
    let version = config_manager.get("client.client_version").unwrap();
    assert_eq!(version, "5");
}

#[test]
fn test_config_manager_overwrites_existing_value() {
    let temp_dir = TempDir::new().unwrap();
    let mut config_manager = create_test_config_manager(&temp_dir);

    // Set initial value
    config_manager
        .set("client.client_name", "oldclient")
        .unwrap();
    assert_eq!(
        config_manager.get("client.client_name").unwrap(),
        "oldclient"
    );

    // Overwrite with new value
    config_manager
        .set("client.client_name", "newclient")
        .unwrap();
    assert_eq!(
        config_manager.get("client.client_name").unwrap(),
        "newclient"
    );
}

#[test]
fn test_config_manager_creates_config_directory() {
    let temp_dir = TempDir::new().unwrap();
    let config_path = temp_dir
        .path()
        .join("nested")
        .join("config")
        .join("config.toml");

    let mut config_manager = ConfigManager::with_path(config_path.clone());

    // Directory shouldn't exist yet
    assert!(!config_path.parent().unwrap().exists());

    // Setting a value should create the directory
    config_manager.set("client.client_name", "test").unwrap();

    assert!(config_path.parent().unwrap().exists());
    assert!(config_path.exists());
}

#[test]
fn test_config_manager_list_shows_all_values() {
    let temp_dir = TempDir::new().unwrap();
    let mut config_manager = create_test_config_manager(&temp_dir);

    config_manager
        .set("client.client_name", "testclient")
        .unwrap();
    config_manager.set("client.client_version", "1").unwrap();

    let items = config_manager.list().unwrap();

    assert!(!items.is_empty());

    // Verify our values are in the list
    let client_name_item = items.iter().find(|(key, _)| key == "client.client_name");
    let client_version_item = items.iter().find(|(key, _)| key == "client.client_version");

    assert!(client_name_item.is_some());
    assert_eq!(client_name_item.unwrap().1, "testclient");

    assert!(client_version_item.is_some());
    assert_eq!(client_version_item.unwrap().1, "1");
}

#[test]
fn test_config_manager_default_values_when_file_missing() {
    let temp_dir = TempDir::new().unwrap();
    let config_manager = create_test_config_manager(&temp_dir);

    // Load config when file doesn't exist
    let config = config_manager.load().unwrap();

    // Should have default values
    assert_eq!(config.client.max_concurrent_files, 4);
    assert_eq!(config.network.timeout_seconds, 30);
    assert_eq!(config.network.retry_count, 3);
    assert_eq!(config.output.default_format, "text");
    assert!(config.output.color_enabled);
    assert!(config.output.progress_enabled);
}

#[test]
fn test_config_manager_merges_file_with_defaults() {
    let temp_dir = TempDir::new().unwrap();
    let mut config_manager = create_test_config_manager(&temp_dir);

    // Set only client name
    config_manager.set("client.client_name", "custom").unwrap();

    // Load should merge with defaults
    let config = config_manager.load().unwrap();

    assert_eq!(config.client.client_name, Some("custom".to_string()));
    // Other defaults should still be present
    assert_eq!(config.network.timeout_seconds, 30);
}

#[test]
fn test_config_file_is_valid_toml() {
    let temp_dir = TempDir::new().unwrap();
    let mut config_manager = create_test_config_manager(&temp_dir);

    config_manager
        .set("client.client_name", "testclient")
        .unwrap();
    config_manager.set("client.client_version", "1").unwrap();

    // Read the raw file and verify it's valid TOML
    let config_path = temp_dir.path().join("config.toml");
    let content = fs::read_to_string(&config_path).unwrap();

    // Try to parse it as TOML
    let parsed: toml::Value = toml::from_str(&content).expect("Invalid TOML generated");

    // Verify structure
    assert!(parsed["client"].is_table());
    assert_eq!(
        parsed["client"]["client_name"].as_str().unwrap(),
        "testclient"
    );
    assert_eq!(parsed["client"]["client_version"].as_str().unwrap(), "1");
}

#[test]
fn test_config_manager_get_nonexistent_key_fails() {
    let temp_dir = TempDir::new().unwrap();
    let config_manager = create_test_config_manager(&temp_dir);

    let result = config_manager.get("nonexistent.key");

    assert!(result.is_err());
}

#[test]
fn test_config_manager_validates_chunk_size_minimum() {
    let temp_dir = TempDir::new().unwrap();
    let mut config_manager = create_test_config_manager(&temp_dir);

    // Try to set chunk size below minimum
    let result = config_manager.set("client.chunk_size", "512");

    assert!(result.is_err());
}

#[test]
fn test_config_manager_accepts_valid_chunk_size() {
    let temp_dir = TempDir::new().unwrap();
    let mut config_manager = create_test_config_manager(&temp_dir);

    // Set valid chunk size
    let result = config_manager.set("client.chunk_size", "65536");

    assert!(result.is_ok());
    let value = config_manager.get("client.chunk_size").unwrap();
    assert_eq!(value, "65536");
}

#[test]
fn test_config_manager_validates_timeout_minimum() {
    let temp_dir = TempDir::new().unwrap();
    let mut config_manager = create_test_config_manager(&temp_dir);

    // Try to set timeout to 0
    let result = config_manager.set("network.timeout_seconds", "0");

    assert!(result.is_err());
}

#[test]
fn test_config_manager_accepts_valid_timeout() {
    let temp_dir = TempDir::new().unwrap();
    let mut config_manager = create_test_config_manager(&temp_dir);

    let result = config_manager.set("network.timeout_seconds", "60");

    assert!(result.is_ok());
    let value = config_manager.get("network.timeout_seconds").unwrap();
    assert_eq!(value, "60");
}

#[test]
fn test_config_manager_validates_boolean_output_settings() {
    let temp_dir = TempDir::new().unwrap();
    let mut config_manager = create_test_config_manager(&temp_dir);

    // Valid boolean values
    assert!(config_manager.set("output.color_enabled", "true").is_ok());
    assert!(
        config_manager
            .set("output.progress_enabled", "false")
            .is_ok()
    );

    // Invalid boolean value
    let result = config_manager.set("output.color_enabled", "maybe");
    assert!(result.is_err());
}

#[test]
fn test_config_manager_handles_special_characters_in_client_name() {
    let temp_dir = TempDir::new().unwrap();
    let mut config_manager = create_test_config_manager(&temp_dir);

    // AniDB allows client names with various characters
    let names = vec!["my-client", "myclient2", "test_client"];

    for name in names {
        let result = config_manager.set("client.client_name", name);
        assert!(result.is_ok());

        let stored = config_manager.get("client.client_name").unwrap();
        assert_eq!(stored, name);
    }
}

#[test]
fn test_config_manager_path_isolation() {
    let temp_dir1 = TempDir::new().unwrap();
    let temp_dir2 = TempDir::new().unwrap();

    let mut config1 = create_test_config_manager(&temp_dir1);
    let config2 = create_test_config_manager(&temp_dir2);

    // Set value in first config
    config1.set("client.client_name", "config1").unwrap();

    // Second config should not have this value
    let result = config2.get("client.client_name");

    assert!(result.is_err());
}
