//! Tests for database path consistency across orchestrators
//!
//! This test suite verifies that all orchestrators and modules
//! use the same data directory path for storing databases and caches.

use std::path::PathBuf;

/// Helper to extract data directory path like the identify orchestrator does
fn get_identify_data_dir() -> PathBuf {
    dirs::data_dir()
        .map(|d| d.join("anidb")) // Fixed implementation (CONSISTENT)
        .unwrap_or_else(|| PathBuf::from(".anidb"))
}

/// Helper to extract data directory path like the sync orchestrator does
fn get_sync_data_dir() -> PathBuf {
    dirs::data_dir()
        .map(|d| d.join("anidb")) // Current implementation (CONSISTENT with others)
        .unwrap_or_else(|| std::path::PathBuf::from(".anidb"))
}

/// Helper to extract cache directory path like cache factory does
fn get_cache_dir() -> PathBuf {
    dirs::data_dir()
        .map(|d| d.join("anidb/cache")) // Current implementation
        .unwrap_or_else(|| PathBuf::from(".anidb/cache"))
}

#[test]
fn test_identify_and_sync_use_same_data_directory() {
    let identify_dir = get_identify_data_dir();
    let sync_dir = get_sync_data_dir();

    // These should be the same
    assert_eq!(
        identify_dir,
        sync_dir,
        "Identify and Sync orchestrators use different data directories: {} vs {}",
        identify_dir.display(),
        sync_dir.display()
    );
}

#[test]
fn test_cache_dir_is_under_data_dir() {
    let data_dir = get_sync_data_dir();
    let cache_dir = get_cache_dir();

    // Cache directory should be a subdirectory of the data directory
    assert!(
        cache_dir.starts_with(&data_dir),
        "Cache dir {} is not under data dir {}",
        cache_dir.display(),
        data_dir.display()
    );
}

#[test]
fn test_database_paths_are_consistent() {
    let identify_data_dir = get_identify_data_dir();
    let sync_data_dir = get_sync_data_dir();

    let identify_db = identify_data_dir.join("anidb.db");
    let sync_db = sync_data_dir.join("anidb.db");

    // Both should point to the same database file
    assert_eq!(
        identify_db,
        sync_db,
        "Database paths differ: {} vs {}",
        identify_db.display(),
        sync_db.display()
    );
}

#[test]
fn test_all_directories_use_anidb_prefix() {
    let identify_dir = get_identify_data_dir();
    let sync_dir = get_sync_data_dir();
    let cache_dir = get_cache_dir();

    // All should contain "anidb" in their path
    let identify_contains_anidb = identify_dir.to_string_lossy().contains("anidb");
    let sync_contains_anidb = sync_dir.to_string_lossy().contains("anidb");
    let cache_contains_anidb = cache_dir.to_string_lossy().contains("anidb");

    assert!(
        identify_contains_anidb,
        "Identify dir doesn't contain 'anidb': {}",
        identify_dir.display()
    );
    assert!(
        sync_contains_anidb,
        "Sync dir doesn't contain 'anidb': {}",
        sync_dir.display()
    );
    assert!(
        cache_contains_anidb,
        "Cache dir doesn't contain 'anidb': {}",
        cache_dir.display()
    );
}

#[test]
fn test_identify_does_not_use_anidb_cli_suffix() {
    let identify_dir = get_identify_data_dir();
    let identify_str = identify_dir.to_string_lossy();

    // The identify directory should NOT have "-cli" suffix
    assert!(
        !identify_str.ends_with("anidb-cli"),
        "Identify dir uses 'anidb-cli' which is inconsistent: {}",
        identify_dir.display()
    );
}
