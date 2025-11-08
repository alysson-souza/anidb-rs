//! Platform-specific build configuration and feature detection
//!
//! Provides compile-time and runtime detection of platform capabilities
//! to enable appropriate optimizations and feature selection.

/// Target platform enumeration
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TargetPlatform {
    Windows,
    MacOS,
    Linux,
    Other(u32), // For future platform support
}

/// Platform-specific features and capabilities
#[derive(Debug, Clone)]
pub struct PlatformFeatures {
    pub supports_memory_mapping: bool,
    pub supports_async_io: bool,
    pub supports_direct_io: bool,
    pub supports_overlapped_io: bool,
    pub supports_kqueue: bool,
    pub max_path_length: usize,
    pub optimal_chunk_size: usize,
    pub max_mmap_size: usize,
}

impl PlatformFeatures {
    /// Detect platform features at runtime
    pub fn detect() -> Self {
        #[cfg(target_os = "windows")]
        {
            Self::windows_features()
        }

        #[cfg(target_os = "macos")]
        {
            Self::macos_features()
        }

        #[cfg(target_os = "linux")]
        {
            Self::linux_features()
        }

        #[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
        {
            Self::default_features()
        }
    }

    /// Check if memory mapping can be used for a given size
    pub fn can_use_mmap_for_size(&self, size: usize) -> bool {
        self.supports_memory_mapping && size <= self.max_mmap_size
    }

    #[cfg(target_os = "windows")]
    fn windows_features() -> Self {
        Self {
            supports_memory_mapping: true, // Windows supports memory mapping
            supports_async_io: true,
            supports_direct_io: false,    // Not easily available
            supports_overlapped_io: true, // Windows-specific
            supports_kqueue: false,
            max_path_length: 260,                // Traditional MAX_PATH
            optimal_chunk_size: 4 * 1024 * 1024, // 4MB
            max_mmap_size: 1024 * 1024 * 1024,   // 1GB
        }
    }

    #[cfg(target_os = "macos")]
    fn macos_features() -> Self {
        Self {
            supports_memory_mapping: true,
            supports_async_io: true,
            supports_direct_io: true,
            supports_overlapped_io: false,
            supports_kqueue: true, // macOS-specific
            max_path_length: 1024,
            optimal_chunk_size: 8 * 1024 * 1024,   // 8MB
            max_mmap_size: 2 * 1024 * 1024 * 1024, // 2GB
        }
    }

    #[cfg(target_os = "linux")]
    fn linux_features() -> Self {
        Self {
            supports_memory_mapping: true,
            supports_async_io: true,
            supports_direct_io: true, // O_DIRECT support
            supports_overlapped_io: false,
            supports_kqueue: false,
            max_path_length: 4096,
            optimal_chunk_size: 8 * 1024 * 1024,   // 8MB
            max_mmap_size: 2 * 1024 * 1024 * 1024, // 2GB
        }
    }

    #[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
    fn default_features() -> Self {
        Self {
            supports_memory_mapping: false,
            supports_async_io: true, // Tokio should work everywhere
            supports_direct_io: false,
            supports_overlapped_io: false,
            supports_kqueue: false,
            max_path_length: 1024,
            optimal_chunk_size: 4 * 1024 * 1024, // 4MB conservative
            max_mmap_size: 512 * 1024 * 1024,    // 512MB conservative
        }
    }
}

/// Build configuration for the current platform
#[derive(Debug, Clone)]
pub struct BuildConfig {
    pub target_platform: TargetPlatform,
    pub features: PlatformFeatures,
    pub debug_build: bool,
    pub optimization_level: u8,
}

impl BuildConfig {
    /// Get build configuration for the current platform
    pub fn current() -> Self {
        let target_platform = Self::detect_platform();
        let features = PlatformFeatures::detect();

        Self {
            target_platform,
            features,
            debug_build: cfg!(debug_assertions),
            optimization_level: if cfg!(debug_assertions) { 0 } else { 3 },
        }
    }

    /// Detect the current platform at compile time
    fn detect_platform() -> TargetPlatform {
        #[cfg(target_os = "windows")]
        return TargetPlatform::Windows;

        #[cfg(target_os = "macos")]
        return TargetPlatform::MacOS;

        #[cfg(target_os = "linux")]
        return TargetPlatform::Linux;

        #[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
        return TargetPlatform::Other(0);
    }

    /// Check if a specific feature is available
    pub fn has_feature(&self, feature: &str) -> bool {
        match feature {
            "memory_mapping" => self.features.supports_memory_mapping,
            "async_io" => self.features.supports_async_io,
            "direct_io" => self.features.supports_direct_io,
            "overlapped_io" => self.features.supports_overlapped_io,
            "kqueue" => self.features.supports_kqueue,
            _ => false,
        }
    }

    /// Get the optimal chunk size for this platform
    pub fn optimal_chunk_size(&self) -> usize {
        self.features.optimal_chunk_size
    }
}
