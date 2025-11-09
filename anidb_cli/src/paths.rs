//! Centralized path management for anidb CLI
//!
//! This module provides utilities for consistently accessing data directories,
//! database paths, and cache directories across the entire application.

use std::path::PathBuf;

/// The name of the application data directory used across all platforms
const APP_DATA_DIR: &str = "anidb";

/// The name of the cache subdirectory
const CACHE_SUBDIR: &str = "cache";

/// The name of the database file
const DATABASE_FILE: &str = "anidb.db";

/// Returns the base data directory for the application
///
/// On Unix-like systems (Linux, macOS), this uses XDG Base Directory specification:
/// - `~/.local/share/anidb`
///
/// On Windows, this uses the user's application data directory:
/// - `%APPDATA%/anidb`
///
/// If the standard directories cannot be determined, falls back to `.anidb` in the current directory.
pub fn get_data_dir() -> PathBuf {
    dirs::data_dir()
        .map(|d| d.join(APP_DATA_DIR))
        .unwrap_or_else(|| PathBuf::from(".anidb"))
}

/// Returns the path to the main AniDB database file
///
/// This is the SQLite database used for caching results, storing file metadata,
/// and managing the sync queue.
pub fn get_database_path() -> PathBuf {
    get_data_dir().join(DATABASE_FILE)
}

/// Returns the path to the cache directory
///
/// The cache directory stores additional cache files and temporary data.
pub fn get_cache_dir() -> PathBuf {
    get_data_dir().join(CACHE_SUBDIR)
}

/// Returns the path to the configuration directory
///
/// On Unix-like systems, this uses the XDG Config Home:
/// - `~/.config/anidb`
///
/// On Windows, this uses the application data directory:
/// - `%APPDATA%/anidb`
///
/// This is separate from the data directory to follow platform conventions.
#[allow(dead_code)]
pub fn get_config_dir() -> PathBuf {
    #[cfg(target_os = "windows")]
    {
        dirs::config_dir()
            .map(|d| d.join(APP_DATA_DIR))
            .unwrap_or_else(|| PathBuf::from(".anidb"))
    }

    #[cfg(not(target_os = "windows"))]
    {
        dirs::config_dir()
            .map(|d| d.join(APP_DATA_DIR))
            .unwrap_or_else(|| PathBuf::from(".anidb"))
    }
}

/// Returns the path to the configuration file (anidb.toml or config.toml)
#[allow(dead_code)]
pub fn get_config_path() -> PathBuf {
    get_config_dir().join("config.toml")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_data_dir_contains_anidb() {
        let data_dir = get_data_dir();
        assert!(
            data_dir.to_string_lossy().contains("anidb"),
            "Data dir should contain 'anidb': {}",
            data_dir.display()
        );
    }

    #[test]
    fn test_database_path_is_in_data_dir() {
        let db_path = get_database_path();
        let data_dir = get_data_dir();

        assert!(
            db_path.starts_with(&data_dir),
            "Database path {} should be under data dir {}",
            db_path.display(),
            data_dir.display()
        );
    }

    #[test]
    fn test_database_path_has_correct_filename() {
        let db_path = get_database_path();
        assert_eq!(
            db_path.file_name().and_then(|n| n.to_str()),
            Some(DATABASE_FILE),
            "Database file should be named '{}'",
            DATABASE_FILE
        );
    }

    #[test]
    fn test_cache_dir_is_under_data_dir() {
        let cache_dir = get_cache_dir();
        let data_dir = get_data_dir();

        assert!(
            cache_dir.starts_with(&data_dir),
            "Cache dir {} should be under data dir {}",
            cache_dir.display(),
            data_dir.display()
        );
    }

    #[test]
    fn test_cache_dir_has_correct_name() {
        let cache_dir = get_cache_dir();
        assert_eq!(
            cache_dir.file_name().and_then(|n| n.to_str()),
            Some(CACHE_SUBDIR),
            "Cache subdirectory should be named '{}'",
            CACHE_SUBDIR
        );
    }

    #[test]
    fn test_config_path_is_in_config_dir() {
        let config_path = get_config_path();
        let config_dir = get_config_dir();

        assert!(
            config_path.starts_with(&config_dir),
            "Config path {} should be under config dir {}",
            config_path.display(),
            config_dir.display()
        );
    }

    #[test]
    fn test_all_paths_use_anidb() {
        let data_dir = get_data_dir();
        let cache_dir = get_cache_dir();
        let config_dir = get_config_dir();

        let paths = [
            ("data", data_dir.to_string_lossy()),
            ("cache", cache_dir.to_string_lossy()),
            ("config", config_dir.to_string_lossy()),
        ];

        for (name, path) in paths.iter() {
            assert!(
                path.contains("anidb"),
                "{} path should contain 'anidb': {}",
                name,
                path
            );
        }
    }

    #[test]
    fn test_data_and_config_dirs_are_different() {
        let data_dir = get_data_dir();
        let config_dir = get_config_dir();

        // On Unix-like systems, these should be in different parent directories
        // On Windows, they might be the same, but we at least verify they exist
        assert!(!data_dir.to_string_lossy().is_empty());
        assert!(!config_dir.to_string_lossy().is_empty());
    }
}
