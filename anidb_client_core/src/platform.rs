//! Platform Abstraction Layer
//!
//! This module provides abstractions for platform-specific functionality,
//! ensuring clean separation between platform-specific optimizations and
//! core business logic. No platform-specific code should leak into core modules.

pub mod build_config;
pub mod io_optimization;
pub mod path_handling;

// Re-export main types for convenience
pub use build_config::{BuildConfig, PlatformFeatures, TargetPlatform};
pub use io_optimization::{
    IoOptimizer, IoStrategy, MemoryPreference, OptimizationHint, ReadPattern,
};
pub use path_handling::{PathInfo, PathValidation, PlatformPathHandler};

/// Platform-aware streaming trait that extends the base streaming functionality
pub trait PlatformAwareStreaming {
    fn enable_platform_optimization(&mut self, enabled: bool);
    fn get_optimal_chunk_size(&self, file_size: u64) -> usize;
    fn supports_memory_mapping(&self) -> bool;
}

/// Platform detection utilities
pub struct Platform;

impl Platform {
    /// Get the current platform
    pub fn current() -> TargetPlatform {
        BuildConfig::current().target_platform
    }

    /// Check if the current platform supports a specific feature
    pub fn supports_feature(feature: &str) -> bool {
        let features = PlatformFeatures::detect();
        match feature {
            "memory_mapping" => features.supports_memory_mapping,
            "async_io" => features.supports_async_io,
            "direct_io" => features.supports_direct_io,
            "overlapped_io" => features.supports_overlapped_io,
            "kqueue" => features.supports_kqueue,
            _ => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_current_platform_detection() {
        let platform = Platform::current();

        #[cfg(target_os = "linux")]
        assert_eq!(platform, TargetPlatform::LinuxGnu);

        #[cfg(target_os = "windows")]
        assert_eq!(platform, TargetPlatform::WindowsMsvc);

        #[cfg(target_os = "macos")]
        assert_eq!(platform, TargetPlatform::MacOS);
    }

    #[test]
    fn test_platform_features() {
        let features = PlatformFeatures::detect();

        // All platforms should support async I/O
        assert!(features.supports_async_io);

        #[cfg(target_os = "linux")]
        {
            assert!(features.supports_memory_mapping);
            assert!(features.supports_direct_io);
        }

        #[cfg(target_os = "windows")]
        {
            assert!(features.supports_overlapped_io);
        }

        #[cfg(target_os = "macos")]
        {
            assert!(features.supports_kqueue);
        }
    }

    #[test]
    fn test_supports_feature() {
        // Test known features
        assert!(Platform::supports_feature("async_io"));

        // Test unknown feature
        assert!(!Platform::supports_feature("unknown_feature"));
    }
}
