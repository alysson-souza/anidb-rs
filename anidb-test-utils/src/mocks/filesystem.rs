//! Mock file system implementation for testing

use anidb_client_core::{Error, Result, error::IoError};
use std::collections::HashMap;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

/// Mock file system for testing
pub struct MockFileSystem {
    files: HashMap<PathBuf, MockFile>,
    directories: HashMap<PathBuf, Vec<PathBuf>>,
}

#[derive(Debug, Clone)]
struct MockFile {
    content: Vec<u8>,
    metadata: MockFileMetadata,
}

#[derive(Debug, Clone)]
pub struct MockFileMetadata {
    pub size: u64,
    pub created_timestamp: u64,
    pub modified_timestamp: u64,
}

impl MockFileSystem {
    /// Create a new mock file system
    pub fn new() -> Self {
        Self {
            files: HashMap::new(),
            directories: HashMap::new(),
        }
    }

    /// Check if the mock file system is empty
    pub fn is_empty(&self) -> bool {
        self.files.is_empty() && self.directories.is_empty()
    }

    /// Add a file to the mock file system
    pub fn add_file(&mut self, path: &str, content: &[u8], metadata: Option<MockFileMetadata>) {
        let path_buf = PathBuf::from(path);
        let metadata = metadata.unwrap_or_else(|| {
            let now = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs();

            MockFileMetadata {
                size: content.len() as u64,
                created_timestamp: now,
                modified_timestamp: now,
            }
        });

        self.files.insert(
            path_buf,
            MockFile {
                content: content.to_vec(),
                metadata,
            },
        );
    }

    /// Add file with specific metadata
    pub fn add_file_with_metadata(
        &mut self,
        path: &str,
        content: &[u8],
        created: u64,
        modified: u64,
    ) {
        let metadata = MockFileMetadata {
            size: content.len() as u64,
            created_timestamp: created,
            modified_timestamp: modified,
        };
        self.add_file(path, content, Some(metadata));
    }

    /// Check if a file exists
    pub fn file_exists(&self, path: &str) -> bool {
        self.files.contains_key(&PathBuf::from(path))
    }

    /// Read a file from the mock file system
    pub fn read_file(&self, path: &str) -> Result<Vec<u8>> {
        let path_buf = PathBuf::from(path);
        self.files
            .get(&path_buf)
            .map(|file| file.content.clone())
            .ok_or_else(|| Error::Io(IoError::file_not_found(&path_buf)))
    }

    /// Get file metadata
    pub fn get_metadata(&self, path: &str) -> Result<MockFileMetadata> {
        let path_buf = PathBuf::from(path);
        self.files
            .get(&path_buf)
            .map(|file| file.metadata.clone())
            .ok_or_else(|| Error::Io(IoError::file_not_found(&path_buf)))
    }

    /// Create a directory
    pub fn create_directory(&mut self, path: &str) {
        let path_buf = PathBuf::from(path);
        self.directories.insert(path_buf, Vec::new());
    }

    /// List directory contents
    pub fn list_directory(&self, path: &str) -> Result<Vec<PathBuf>> {
        let path_buf = PathBuf::from(path);

        // Check if directory exists
        if !self.directories.contains_key(&path_buf) {
            // Also check if any files exist under this path
            let has_files = self.files.keys().any(|file_path| {
                if let Some(parent) = file_path.parent() {
                    parent == path_buf
                } else {
                    false
                }
            });

            if !has_files {
                return Err(Error::Io(IoError::file_not_found(&path_buf)));
            }
        }

        // Find all files and subdirectories under this path
        let mut entries = Vec::new();

        for file_path in self.files.keys() {
            if let Some(parent) = file_path.parent()
                && parent == path_buf
            {
                entries.push(file_path.clone());
            }
        }

        Ok(entries)
    }

    /// Reset the mock file system
    pub fn reset(&mut self) {
        self.files.clear();
        self.directories.clear();
    }
}

impl Default for MockFileSystem {
    fn default() -> Self {
        Self::new()
    }
}
