//! Directory walker module for file discovery
//!
//! Provides streaming file discovery using walkdir with pattern filtering
//! and memory-efficient iteration.

use std::path::Path;
use walkdir::{DirEntry, WalkDir};

use super::{
    DiscoveredFile, DiscoveryError, Result,
    extensions::{DEFAULT_MEDIA_EXTENSIONS, extensions_to_patterns},
    filter::FileFilter,
};

/// Options for file discovery
#[derive(Debug, Clone)]
pub struct FileDiscoveryOptions {
    /// Patterns to include (glob patterns)
    pub include_patterns: Vec<String>,
    /// Patterns to exclude (glob patterns, override includes)
    pub exclude_patterns: Vec<String>,
    /// Use default media extensions when no include patterns specified
    pub use_defaults: bool,
    /// Process directories recursively
    pub recursive: bool,
    /// Follow symbolic links
    pub follow_links: bool,
    /// Maximum depth for recursive search (None = unlimited)
    pub max_depth: Option<usize>,
}

impl Default for FileDiscoveryOptions {
    fn default() -> Self {
        Self {
            include_patterns: Vec::new(),
            exclude_patterns: Vec::new(),
            use_defaults: true,
            recursive: true,
            follow_links: false,
            max_depth: None,
        }
    }
}

#[allow(dead_code)]
impl FileDiscoveryOptions {
    /// Create new options with default settings
    pub fn new() -> Self {
        Self::default()
    }

    /// Add include patterns
    pub fn with_include_patterns(mut self, patterns: Vec<String>) -> Self {
        self.include_patterns = patterns;
        self
    }

    /// Add exclude patterns
    pub fn with_exclude_patterns(mut self, patterns: Vec<String>) -> Self {
        self.exclude_patterns = patterns;
        self
    }

    /// Set whether to use default media extensions
    pub fn with_use_defaults(mut self, use_defaults: bool) -> Self {
        self.use_defaults = use_defaults;
        self
    }

    /// Set recursive processing
    pub fn with_recursive(mut self, recursive: bool) -> Self {
        self.recursive = recursive;
        self
    }

    /// Set whether to follow symbolic links
    pub fn with_follow_links(mut self, follow: bool) -> Self {
        self.follow_links = follow;
        self
    }

    /// Set maximum depth for recursive search
    pub fn with_max_depth(mut self, depth: Option<usize>) -> Self {
        self.max_depth = depth;
        self
    }
}

/// File discovery iterator for streaming file enumeration
pub struct FileDiscovery {
    /// Walker for directory traversal
    walker: Box<dyn Iterator<Item = walkdir::Result<DirEntry>>>,
    /// File filter for pattern matching
    filter: FileFilter,
    /// Options used for discovery
    #[allow(dead_code)]
    options: FileDiscoveryOptions,
}

impl FileDiscovery {
    /// Create a new file discovery iterator
    pub fn new(path: &Path, options: FileDiscoveryOptions) -> Result<Self> {
        // Verify path exists
        if !path.exists() {
            return Err(DiscoveryError::PathNotFound(path.to_path_buf()));
        }

        // Prepare include patterns
        let mut include_patterns = options.include_patterns.clone();

        // Add default media patterns if requested and no includes specified
        if options.use_defaults && include_patterns.is_empty() {
            include_patterns = extensions_to_patterns(DEFAULT_MEDIA_EXTENSIONS);
        }

        // Create file filter
        let filter = FileFilter::new(include_patterns, options.exclude_patterns.clone())?;

        // Configure walker
        let mut walker = WalkDir::new(path).follow_links(options.follow_links);

        if !options.recursive {
            walker = walker.max_depth(1);
        } else if let Some(depth) = options.max_depth {
            walker = walker.max_depth(depth);
        }

        Ok(Self {
            walker: Box::new(walker.into_iter()),
            filter,
            options,
        })
    }

    /// Check if an entry is a file we should include
    fn should_include_entry(&self, entry: &DirEntry) -> bool {
        // Skip directories
        if !entry.file_type().is_file() {
            return false;
        }

        // Apply filter
        self.filter.should_include(entry.path())
    }
}

impl Iterator for FileDiscovery {
    type Item = Result<DiscoveredFile>;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            match self.walker.next()? {
                Ok(entry) => {
                    if self.should_include_entry(&entry) {
                        // Get file metadata
                        match entry.metadata() {
                            Ok(metadata) => {
                                return Some(Ok(DiscoveredFile {
                                    path: entry.path().to_path_buf(),
                                    size: metadata.len(),
                                }));
                            }
                            Err(e) => {
                                // Skip files we can't read metadata for
                                log::warn!("Failed to read metadata for {:?}: {}", entry.path(), e);
                                continue;
                            }
                        }
                    }
                }
                Err(e) => {
                    // Log walk errors but continue
                    log::warn!("Walk error: {e}");
                    continue;
                }
            }
        }
    }
}

/// Convenience function to discover files in a directory
#[allow(dead_code)]
pub fn discover_files(path: &Path, options: FileDiscoveryOptions) -> Result<Vec<DiscoveredFile>> {
    FileDiscovery::new(path, options)?.collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    fn create_test_directory() -> TempDir {
        let dir = TempDir::new().unwrap();
        let base = dir.path();

        // Create test files
        fs::write(base.join("video1.mkv"), b"test").unwrap();
        fs::write(base.join("video2.mp4"), b"test").unwrap();
        fs::write(base.join("document.txt"), b"test").unwrap();
        fs::write(base.join("backup.bak"), b"test").unwrap();

        // Create subdirectory with files
        let subdir = base.join("subdir");
        fs::create_dir(&subdir).unwrap();
        fs::write(subdir.join("nested.mkv"), b"test").unwrap();
        fs::write(subdir.join("nested.avi"), b"test").unwrap();

        dir
    }

    #[test]
    fn test_discovery_with_patterns() {
        let dir = create_test_directory();
        let options = FileDiscoveryOptions::new()
            .with_include_patterns(vec!["*.mkv".to_string(), "*.mp4".to_string()])
            .with_use_defaults(false);

        let files: Vec<_> = FileDiscovery::new(dir.path(), options)
            .unwrap()
            .collect::<Result<Vec<_>>>()
            .unwrap();

        assert_eq!(files.len(), 3); // video1.mkv, video2.mp4, subdir/nested.mkv

        let paths: Vec<_> = files.iter().map(|f| f.path.file_name().unwrap()).collect();
        assert!(paths.iter().any(|p| *p == "video1.mkv"));
        assert!(paths.iter().any(|p| *p == "video2.mp4"));
        assert!(paths.iter().any(|p| *p == "nested.mkv"));
    }

    #[test]
    fn test_discovery_with_excludes() {
        let dir = create_test_directory();
        let options = FileDiscoveryOptions::new()
            .with_include_patterns(vec!["*".to_string()])
            .with_exclude_patterns(vec!["*.bak".to_string(), "*.txt".to_string()])
            .with_use_defaults(false);

        let files: Vec<_> = FileDiscovery::new(dir.path(), options)
            .unwrap()
            .collect::<Result<Vec<_>>>()
            .unwrap();

        let paths: Vec<_> = files.iter().map(|f| f.path.file_name().unwrap()).collect();
        assert!(!paths.iter().any(|p| *p == "document.txt"));
        assert!(!paths.iter().any(|p| *p == "backup.bak"));
    }

    #[test]
    fn test_discovery_non_recursive() {
        let dir = create_test_directory();
        let options = FileDiscoveryOptions::new()
            .with_include_patterns(vec!["*.mkv".to_string()])
            .with_recursive(false)
            .with_use_defaults(false);

        let files: Vec<_> = FileDiscovery::new(dir.path(), options)
            .unwrap()
            .collect::<Result<Vec<_>>>()
            .unwrap();

        assert_eq!(files.len(), 1); // Only video1.mkv, not nested.mkv
        assert_eq!(files[0].path.file_name().unwrap(), "video1.mkv");
    }

    #[test]
    fn test_discovery_with_defaults() {
        let dir = create_test_directory();
        let options = FileDiscoveryOptions::new().with_use_defaults(true);

        let files: Vec<_> = FileDiscovery::new(dir.path(), options)
            .unwrap()
            .collect::<Result<Vec<_>>>()
            .unwrap();

        // Should find mkv, mp4, and avi files (all are in default media extensions)
        // video1.mkv, video2.mp4, nested.mkv, nested.avi
        assert_eq!(files.len(), 4);
    }

    #[test]
    fn test_discovery_exclude_overrides_include() {
        let dir = create_test_directory();

        let options = FileDiscoveryOptions::new()
            .with_include_patterns(vec!["*.mkv".to_string()])
            .with_exclude_patterns(vec!["**/subdir/*".to_string()])
            .with_use_defaults(false);

        let files: Vec<_> = FileDiscovery::new(dir.path(), options)
            .unwrap()
            .collect::<Result<Vec<_>>>()
            .unwrap();

        assert_eq!(files.len(), 1); // Only video1.mkv, not nested.mkv
        assert_eq!(files[0].path.file_name().unwrap(), "video1.mkv");
    }
}
