//! File filtering module using glob patterns
//!
//! Provides efficient pattern matching using GlobSet for include/exclude
//! filtering of file paths.

use globset::{Glob, GlobSet, GlobSetBuilder};
use std::path::Path;

use super::{DiscoveryError, Result};

/// Pattern matcher using GlobSet for efficient matching
#[derive(Debug, Clone)]
pub struct PatternMatcher {
    /// Compiled glob set for matching
    globset: GlobSet,
    /// Original patterns for debugging
    #[allow(dead_code)]
    patterns: Vec<String>,
}

impl PatternMatcher {
    /// Create a new pattern matcher from glob patterns
    pub fn new(patterns: &[String]) -> Result<Self> {
        let mut builder = GlobSetBuilder::new();

        for pattern in patterns {
            let glob = Glob::new(pattern)
                .map_err(|e| DiscoveryError::InvalidPattern(format!("{pattern}: {e}")))?;
            builder.add(glob);
        }

        let globset = builder
            .build()
            .map_err(|e| DiscoveryError::InvalidPattern(e.to_string()))?;

        Ok(Self {
            globset,
            patterns: patterns.to_vec(),
        })
    }

    /// Check if a path matches any of the patterns
    pub fn matches(&self, path: &Path) -> bool {
        self.globset.is_match(path)
    }

    /// Check if the matcher has any patterns
    #[allow(dead_code)]
    pub fn is_empty(&self) -> bool {
        self.patterns.is_empty()
    }

    /// Get the original patterns
    #[allow(dead_code)]
    pub fn patterns(&self) -> &[String] {
        &self.patterns
    }
}

/// File filter managing include and exclude patterns
#[derive(Debug)]
pub struct FileFilter {
    /// Include patterns matcher
    include_matcher: Option<PatternMatcher>,
    /// Exclude patterns matcher (overrides includes)
    exclude_matcher: Option<PatternMatcher>,
}

impl FileFilter {
    /// Create a new file filter
    pub fn new(include_patterns: Vec<String>, exclude_patterns: Vec<String>) -> Result<Self> {
        let include_matcher = if !include_patterns.is_empty() {
            Some(PatternMatcher::new(&include_patterns)?)
        } else {
            None
        };

        let exclude_matcher = if !exclude_patterns.is_empty() {
            Some(PatternMatcher::new(&exclude_patterns)?)
        } else {
            None
        };

        Ok(Self {
            include_matcher,
            exclude_matcher,
        })
    }

    /// Check if a file should be included based on patterns
    ///
    /// Rules:
    /// 1. If path matches exclude patterns -> false (exclude overrides)
    /// 2. If no include patterns -> true (include all by default)
    /// 3. If path matches include patterns -> true
    /// 4. Otherwise -> false
    pub fn should_include(&self, path: &Path) -> bool {
        // Check excludes first (they override includes)
        if let Some(ref exclude) = self.exclude_matcher
            && exclude.matches(path)
        {
            return false;
        }

        // If no include patterns, include everything (that wasn't excluded)
        if let Some(ref include) = self.include_matcher {
            include.matches(path)
        } else {
            true
        }
    }

    /// Check if the filter has any patterns configured
    #[allow(dead_code)]
    pub fn has_patterns(&self) -> bool {
        self.include_matcher.is_some() || self.exclude_matcher.is_some()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pattern_matcher_basic() {
        let patterns = vec!["*.mkv".to_string(), "*.mp4".to_string()];
        let matcher = PatternMatcher::new(&patterns).unwrap();

        assert!(matcher.matches(Path::new("test.mkv")));
        assert!(matcher.matches(Path::new("test.mp4")));
        assert!(!matcher.matches(Path::new("test.avi")));
        assert!(matcher.matches(Path::new("/path/to/file.mkv")));
    }

    #[test]
    fn test_pattern_matcher_complex() {
        let patterns = vec![
            "**/*.mkv".to_string(),
            "test_*.mp4".to_string(),
            "!backup/*".to_string(),
        ];
        let matcher = PatternMatcher::new(&patterns).unwrap();

        assert!(matcher.matches(Path::new("dir/subdir/file.mkv")));
        assert!(matcher.matches(Path::new("test_video.mp4")));
        assert!(!matcher.matches(Path::new("video.mp4")));
    }

    #[test]
    fn test_file_filter_include_only() {
        let filter =
            FileFilter::new(vec!["*.mkv".to_string(), "*.mp4".to_string()], vec![]).unwrap();

        assert!(filter.should_include(Path::new("test.mkv")));
        assert!(filter.should_include(Path::new("test.mp4")));
        assert!(!filter.should_include(Path::new("test.avi")));
    }

    #[test]
    fn test_file_filter_exclude_overrides() {
        let filter =
            FileFilter::new(vec!["*.mkv".to_string()], vec!["backup/*.mkv".to_string()]).unwrap();

        assert!(filter.should_include(Path::new("test.mkv")));
        assert!(filter.should_include(Path::new("videos/test.mkv")));
        assert!(!filter.should_include(Path::new("backup/test.mkv")));
    }

    #[test]
    fn test_file_filter_no_patterns() {
        let filter = FileFilter::new(vec![], vec![]).unwrap();

        // With no patterns, everything should be included
        assert!(filter.should_include(Path::new("test.mkv")));
        assert!(filter.should_include(Path::new("test.txt")));
        assert!(filter.should_include(Path::new("any/path/file.xyz")));
    }

    #[test]
    fn test_file_filter_exclude_only() {
        let filter =
            FileFilter::new(vec![], vec!["*.tmp".to_string(), "*.bak".to_string()]).unwrap();

        // Exclude patterns without includes means include everything except excludes
        assert!(filter.should_include(Path::new("test.mkv")));
        assert!(filter.should_include(Path::new("test.mp4")));
        assert!(!filter.should_include(Path::new("test.tmp")));
        assert!(!filter.should_include(Path::new("backup.bak")));
    }
}
