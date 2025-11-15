//! Shared test fixtures for parallel test execution
//!
//! This module provides lazy-initialized test files that are created once
//! and shared across all tests, reducing disk I/O and test setup time.

use once_cell::sync::Lazy;
use std::io::Write;
use std::path::PathBuf;
use std::sync::Arc;
use tempfile::TempDir;

/// Shared test fixture containing pre-generated test files
pub struct SharedTestFixtures {
    /// Temporary directory that persists for the test process lifetime
    #[allow(dead_code)]
    temp_dir: Arc<TempDir>,

    /// Path to a 1KB test file (all 0xAB bytes)
    pub file_1kb: PathBuf,

    /// Path to a 64KB test file (all 0xAB bytes)
    pub file_64kb: PathBuf,

    /// Path to a 1MB test file (all 0xAB bytes)
    pub file_1mb: PathBuf,

    /// Path to a 10MB test file (all 0xAB bytes)
    pub file_10mb: PathBuf,

    /// Path to a 10MB test file with 0xCD bytes (different pattern)
    pub file_10mb_pattern_cd: PathBuf,

    /// Path to an exactly 9.728MB test file (one ED2K chunk, 0x42 bytes)
    pub file_ed2k_single_chunk: PathBuf,

    /// Path to a 50MB test file (multiple ED2K chunks, 0xEF bytes)
    pub file_50mb: PathBuf,
}

impl SharedTestFixtures {
    fn new() -> Self {
        let temp_dir = TempDir::new().expect("Failed to create temp directory for test fixtures");
        let temp_path = temp_dir.path();

        // Create 1KB file
        let file_1kb = temp_path.join("shared_1kb.bin");
        std::fs::write(&file_1kb, vec![0xABu8; 1024]).expect("Failed to create 1KB test file");

        // Create 64KB file
        let file_64kb = temp_path.join("shared_64kb.bin");
        std::fs::write(&file_64kb, vec![0xABu8; 64 * 1024])
            .expect("Failed to create 64KB test file");

        // Create 1MB file
        let file_1mb = temp_path.join("shared_1mb.bin");
        std::fs::write(&file_1mb, vec![0xABu8; 1024 * 1024])
            .expect("Failed to create 1MB test file");

        // Create 10MB file using streaming writes to avoid large memory allocation
        let file_10mb = temp_path.join("shared_10mb.bin");
        {
            let mut f = std::fs::File::create(&file_10mb).expect("Failed to create 10MB test file");
            let chunk = vec![0xABu8; 1024 * 1024]; // 1MB buffer
            for _ in 0..10 {
                f.write_all(&chunk).expect("Failed to write 10MB test file");
            }
        }

        // Create 10MB file with different pattern using streaming writes
        let file_10mb_pattern_cd = temp_path.join("shared_10mb_cd.bin");
        {
            let mut f = std::fs::File::create(&file_10mb_pattern_cd)
                .expect("Failed to create 10MB pattern test file");
            let chunk = vec![0xCDu8; 1024 * 1024]; // 1MB buffer
            for _ in 0..10 {
                f.write_all(&chunk)
                    .expect("Failed to write 10MB pattern test file");
            }
        }

        // Create exactly one ED2K chunk (9728000 bytes) using streaming writes
        let file_ed2k_single_chunk = temp_path.join("shared_ed2k_chunk.bin");
        {
            let mut f = std::fs::File::create(&file_ed2k_single_chunk)
                .expect("Failed to create ED2K single chunk test file");
            let chunk = vec![0x42u8; 1024 * 1024]; // 1MB buffer
            let remaining = vec![0x42u8; 9_728_000 % (1024 * 1024)]; // 0.728MB
            for _ in 0..9 {
                f.write_all(&chunk)
                    .expect("Failed to write ED2K single chunk test file");
            }
            f.write_all(&remaining)
                .expect("Failed to write ED2K single chunk test file");
        }

        // Create 50MB file using streaming writes to avoid large memory allocation
        let file_50mb = temp_path.join("shared_50mb.bin");
        {
            let mut f = std::fs::File::create(&file_50mb).expect("Failed to create 50MB test file");
            let chunk = vec![0xEFu8; 1024 * 1024]; // 1MB buffer
            for _ in 0..50 {
                f.write_all(&chunk).expect("Failed to write 50MB test file");
            }
        }

        Self {
            temp_dir: Arc::new(temp_dir),
            file_1kb,
            file_64kb,
            file_1mb,
            file_10mb,
            file_10mb_pattern_cd,
            file_ed2k_single_chunk,
            file_50mb,
        }
    }
}

/// Global shared test fixtures, initialized once per test process
pub static SHARED_FIXTURES: Lazy<SharedTestFixtures> = Lazy::new(SharedTestFixtures::new);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fixtures_exist() {
        let fixtures = &*SHARED_FIXTURES;

        assert!(fixtures.file_1kb.exists());
        assert!(fixtures.file_64kb.exists());
        assert!(fixtures.file_1mb.exists());
        assert!(fixtures.file_10mb.exists());
        assert!(fixtures.file_10mb_pattern_cd.exists());
        assert!(fixtures.file_ed2k_single_chunk.exists());
        assert!(fixtures.file_50mb.exists());
    }

    #[test]
    fn test_fixture_sizes() {
        let fixtures = &*SHARED_FIXTURES;

        assert_eq!(std::fs::metadata(&fixtures.file_1kb).unwrap().len(), 1024);
        assert_eq!(
            std::fs::metadata(&fixtures.file_64kb).unwrap().len(),
            64 * 1024
        );
        assert_eq!(
            std::fs::metadata(&fixtures.file_1mb).unwrap().len(),
            1024 * 1024
        );
        assert_eq!(
            std::fs::metadata(&fixtures.file_10mb).unwrap().len(),
            10 * 1024 * 1024
        );
        assert_eq!(
            std::fs::metadata(&fixtures.file_10mb_pattern_cd)
                .unwrap()
                .len(),
            10 * 1024 * 1024
        );
        assert_eq!(
            std::fs::metadata(&fixtures.file_ed2k_single_chunk)
                .unwrap()
                .len(),
            9_728_000
        );
        assert_eq!(
            std::fs::metadata(&fixtures.file_50mb).unwrap().len(),
            50 * 1024 * 1024
        );
    }
}
