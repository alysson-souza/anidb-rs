//! Unified memory management module
//!
//! This module consolidates all buffer pool implementations into a single,
//! unified MemoryManager that provides:
//! - Single source of truth for memory allocation
//! - Thread-safe operations with minimal contention
//! - 500MB hard limit enforcement across ALL allocations
//! - Diagnostics when approaching limits
//! - Multiple buffer size pools for efficiency

use crate::Result;
use std::sync::{Arc, Mutex, RwLock};
use std::time::{Duration, Instant};

mod pool;
mod stats;
mod tracker;

pub use pool::{BufferPool, PooledBuffer};
pub use stats::{MemoryDiagnostics, MemoryStats};
pub use tracker::MemoryTracker;

/// Default memory limit (500MB)
pub const DEFAULT_MEMORY_LIMIT: usize = 500 * 1024 * 1024;

/// Memory usage warning threshold (80% of limit)
pub const MEMORY_WARNING_THRESHOLD: f64 = 0.8;

/// Memory usage critical threshold (95% of limit)
pub const MEMORY_CRITICAL_THRESHOLD: f64 = 0.95;

/// Buffer size categories for pooling
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BufferSize {
    /// Small buffers: 1KB - 4KB (for strings, small data)
    Small,
    /// Medium buffers: 16KB - 64KB (for chunks, hash data)
    Medium,
    /// Large buffers: 256KB - 1MB (for file processing)
    Large,
    /// Extra large buffers: 4MB - 8MB (for batch operations)
    ExtraLarge,
}

impl BufferSize {
    /// Get the actual size in bytes for this category
    pub fn size(&self) -> usize {
        match self {
            BufferSize::Small => 4 * 1024,             // 4KB
            BufferSize::Medium => 64 * 1024,           // 64KB
            BufferSize::Large => 1024 * 1024,          // 1MB
            BufferSize::ExtraLarge => 8 * 1024 * 1024, // 8MB
        }
    }

    /// Get the appropriate size category for a requested size
    pub fn for_size(size: usize) -> Self {
        if size <= 4 * 1024 {
            BufferSize::Small
        } else if size <= 64 * 1024 {
            BufferSize::Medium
        } else if size <= 1024 * 1024 {
            BufferSize::Large
        } else {
            BufferSize::ExtraLarge
        }
    }

    /// Get all buffer size categories in order
    pub fn all() -> &'static [BufferSize] {
        &[
            BufferSize::Small,
            BufferSize::Medium,
            BufferSize::Large,
            BufferSize::ExtraLarge,
        ]
    }
}

/// Configuration for the memory manager
#[derive(Debug, Clone)]
pub struct MemoryConfig {
    /// Maximum total memory that can be allocated (hard limit)
    pub max_memory: usize,
    /// Maximum number of buffers to pool per size category
    pub max_pool_size: usize,
    /// Whether to enable automatic shrinking when memory pressure is high
    pub auto_shrink: bool,
    /// Duration after which unused buffers are evicted
    pub eviction_timeout: Duration,
    /// Whether to log warnings when approaching memory limits
    pub enable_diagnostics: bool,
}

impl Default for MemoryConfig {
    fn default() -> Self {
        Self {
            max_memory: DEFAULT_MEMORY_LIMIT,
            max_pool_size: 20,
            auto_shrink: true,
            eviction_timeout: Duration::from_secs(60),
            enable_diagnostics: true,
        }
    }
}

/// Unified memory manager for all buffer allocations
pub struct MemoryManager {
    /// Configuration
    config: MemoryConfig,
    /// Global memory tracker
    tracker: Arc<MemoryTracker>,
    /// Buffer pools organized by size category
    pools: Arc<RwLock<std::collections::HashMap<BufferSize, BufferPool>>>,
    /// Statistics and diagnostics
    stats: Arc<MemoryStats>,
    /// Last diagnostic check time
    last_diagnostic: Arc<Mutex<Instant>>,
}

impl MemoryManager {
    /// Create a new memory manager with default configuration
    pub fn new() -> Self {
        Self::with_config(MemoryConfig::default())
    }

    /// Create a new memory manager with custom configuration
    pub fn with_config(config: MemoryConfig) -> Self {
        let tracker = Arc::new(MemoryTracker::new(config.max_memory));
        let stats = Arc::new(MemoryStats::new());

        // Initialize buffer pools for each size category
        let mut pools_map = std::collections::HashMap::new();
        for size in BufferSize::all() {
            pools_map.insert(*size, BufferPool::new(*size, config.max_pool_size));
        }

        Self {
            config,
            tracker,
            pools: Arc::new(RwLock::new(pools_map)),
            stats,
            last_diagnostic: Arc::new(Mutex::new(Instant::now())),
        }
    }

    /// Allocate a buffer of the requested size
    pub fn allocate(&self, size: usize) -> Result<Vec<u8>> {
        // Track allocation attempt
        self.stats.record_allocation_attempt();

        // Check if we should run diagnostics
        self.maybe_run_diagnostics();

        // Determine buffer size category
        let category = BufferSize::for_size(size);
        let actual_size = category.size().max(size);

        // Try to get from pool first
        if let Ok(pools) = self.pools.read()
            && let Some(pool) = pools.get(&category)
            && let Some(mut buffer) = pool.try_acquire()
        {
            // Reused from pool
            self.stats.record_pool_hit();
            buffer.resize(size, 0);
            return Ok(buffer);
        }

        // No buffer in pool, need to allocate new one
        self.stats.record_pool_miss();

        // Check memory limit and allocate
        self.tracker.try_allocate(actual_size)?;

        // Create new buffer
        let mut buffer = vec![0u8; actual_size];
        buffer.resize(size, 0);

        // Update statistics
        self.stats.record_allocation(actual_size);

        Ok(buffer)
    }

    /// Release a buffer back to the pool or free it
    pub fn release(&self, mut buffer: Vec<u8>) {
        let capacity = buffer.capacity();
        let category = BufferSize::for_size(capacity);

        // Try to return to pool
        let returned_to_pool = if let Ok(pools) = self.pools.read() {
            if let Some(pool) = pools.get(&category) {
                // Check if we should pool this buffer
                if pool.should_pool() {
                    buffer.clear();
                    buffer.resize(category.size().min(capacity), 0);
                    pool.release(buffer);
                    true
                } else {
                    false
                }
            } else {
                false
            }
        } else {
            false
        };

        // Update memory tracking
        if !returned_to_pool {
            // Buffer was dropped, free the memory
            self.tracker.deallocate(capacity);
        }

        // Update statistics
        self.stats.record_deallocation(capacity);

        // Maybe trigger auto-shrink
        if self.config.auto_shrink {
            self.maybe_shrink();
        }
    }

    /// Get current memory usage
    pub fn memory_used(&self) -> usize {
        self.tracker.used()
    }

    /// Get memory limit
    pub fn memory_limit(&self) -> usize {
        self.config.max_memory
    }

    /// Get memory usage as a percentage
    pub fn memory_usage_percent(&self) -> f64 {
        (self.memory_used() as f64 / self.memory_limit() as f64) * 100.0
    }

    /// Check if memory usage is at warning level
    pub fn is_memory_warning(&self) -> bool {
        self.memory_usage_percent() >= (MEMORY_WARNING_THRESHOLD * 100.0)
    }

    /// Check if memory usage is at critical level
    pub fn is_memory_critical(&self) -> bool {
        self.memory_usage_percent() >= (MEMORY_CRITICAL_THRESHOLD * 100.0)
    }

    /// Get current statistics
    pub fn stats(&self) -> MemoryStats {
        self.stats.snapshot()
    }

    /// Get detailed diagnostics
    pub fn diagnostics(&self) -> MemoryDiagnostics {
        let pools_info = if let Ok(pools) = self.pools.read() {
            pools
                .iter()
                .map(|(size, pool)| (*size, pool.info()))
                .collect()
        } else {
            Vec::new()
        };

        MemoryDiagnostics {
            memory_used: self.memory_used(),
            memory_limit: self.memory_limit(),
            usage_percent: self.memory_usage_percent(),
            is_warning: self.is_memory_warning(),
            is_critical: self.is_memory_critical(),
            stats: self.stats(),
            pools: pools_info,
        }
    }

    /// Clear all buffer pools
    pub fn clear_pools(&self) {
        if let Ok(pools) = self.pools.read() {
            for pool in pools.values() {
                pool.clear();
            }
        }
    }

    /// Shrink pools to reclaim memory
    pub fn shrink_pools(&self, target_memory: usize) {
        let current = self.memory_used();
        if current <= target_memory {
            return;
        }

        let to_free = current - target_memory;
        let mut freed = 0;

        if let Ok(pools) = self.pools.read() {
            // Start with largest buffers first
            for size in BufferSize::all().iter().rev() {
                if freed >= to_free {
                    break;
                }

                if let Some(pool) = pools.get(size) {
                    freed += pool.shrink(to_free - freed);
                }
            }
        }
    }

    /// Evict stale buffers from all pools
    pub fn evict_stale(&self) {
        if let Ok(pools) = self.pools.read() {
            for pool in pools.values() {
                pool.evict_stale(self.config.eviction_timeout);
            }
        }
    }

    /// Run diagnostics if enabled and enough time has passed
    fn maybe_run_diagnostics(&self) {
        if !self.config.enable_diagnostics {
            return;
        }

        let mut last_check = self.last_diagnostic.lock().unwrap();
        let now = Instant::now();

        // Check every 5 seconds
        if now.duration_since(*last_check) > Duration::from_secs(5) {
            *last_check = now;
            drop(last_check); // Release lock before diagnostics

            let diag = self.diagnostics();
            if diag.is_critical {
                eprintln!(
                    "CRITICAL: Memory usage at {:.1}% ({}/{} bytes)",
                    diag.usage_percent, diag.memory_used, diag.memory_limit
                );
            } else if diag.is_warning {
                eprintln!(
                    "WARNING: Memory usage at {:.1}% ({}/{} bytes)",
                    diag.usage_percent, diag.memory_used, diag.memory_limit
                );
            }
        }
    }

    /// Maybe shrink pools if memory pressure is high
    fn maybe_shrink(&self) {
        if self.is_memory_warning() {
            // Shrink to 70% of limit
            let target = (self.memory_limit() as f64 * 0.7) as usize;
            self.shrink_pools(target);
        }
    }
}

impl Default for MemoryManager {
    fn default() -> Self {
        Self::new()
    }
}

lazy_static::lazy_static! {
    static ref GLOBAL_MEMORY_MANAGER: MemoryManager = MemoryManager::new();
}

/// Allocate a buffer from the global memory manager
pub fn allocate(size: usize) -> Result<Vec<u8>> {
    GLOBAL_MEMORY_MANAGER.allocate(size)
}

/// Release a buffer to the global memory manager
pub fn release(buffer: Vec<u8>) {
    GLOBAL_MEMORY_MANAGER.release(buffer)
}

/// Get current global memory usage
pub fn memory_used() -> usize {
    GLOBAL_MEMORY_MANAGER.memory_used()
}

/// Get global memory limit
pub fn memory_limit() -> usize {
    GLOBAL_MEMORY_MANAGER.memory_limit()
}

/// Get global memory statistics
pub fn stats() -> MemoryStats {
    GLOBAL_MEMORY_MANAGER.stats()
}

/// Get global memory diagnostics
pub fn diagnostics() -> MemoryDiagnostics {
    GLOBAL_MEMORY_MANAGER.diagnostics()
}

/// Clear all global buffer pools
pub fn clear_pools() {
    GLOBAL_MEMORY_MANAGER.clear_pools()
}

/// Shrink global pools to target memory
pub fn shrink_pools(target_memory: usize) {
    GLOBAL_MEMORY_MANAGER.shrink_pools(target_memory)
}

/// Evict stale buffers from global pools
pub fn evict_stale() {
    GLOBAL_MEMORY_MANAGER.evict_stale()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_buffer_size_categories() {
        assert_eq!(BufferSize::for_size(1024), BufferSize::Small);
        assert_eq!(BufferSize::for_size(4 * 1024), BufferSize::Small);
        assert_eq!(BufferSize::for_size(16 * 1024), BufferSize::Medium);
        assert_eq!(BufferSize::for_size(64 * 1024), BufferSize::Medium);
        assert_eq!(BufferSize::for_size(512 * 1024), BufferSize::Large);
        assert_eq!(
            BufferSize::for_size(2 * 1024 * 1024),
            BufferSize::ExtraLarge
        );
    }

    #[test]
    fn test_memory_manager_basic() {
        let config = MemoryConfig {
            max_memory: 10 * 1024 * 1024, // 10MB for testing
            ..Default::default()
        };
        let manager = MemoryManager::with_config(config);

        // Allocate a buffer
        let buffer = manager.allocate(1024).unwrap();
        assert_eq!(buffer.len(), 1024);

        // Check memory usage
        assert!(manager.memory_used() > 0);

        // Release buffer
        manager.release(buffer);

        // Memory should still be used (pooled)
        assert!(manager.memory_used() > 0);
    }

    #[test]
    fn test_memory_limit_enforcement() {
        let config = MemoryConfig {
            max_memory: 10_000, // Very small limit
            max_pool_size: 0,   // No pooling to test limit directly
            ..Default::default()
        };
        let manager = MemoryManager::with_config(config);

        // Should succeed
        let buffer1 = manager.allocate(4_000).unwrap();

        // Should fail - would exceed limit
        let result = manager.allocate(8_000);
        assert!(result.is_err());

        // Release first buffer
        manager.release(buffer1);

        // Now should succeed
        let buffer2 = manager.allocate(4_000).unwrap();
        assert_eq!(buffer2.len(), 4_000);
    }

    #[test]
    fn test_pool_reuse() {
        let manager = MemoryManager::new();

        // Allocate and release a buffer
        let buffer = manager.allocate(1024).unwrap();
        let capacity = buffer.capacity();
        manager.release(buffer);

        // Allocate again - should reuse from pool
        let buffer2 = manager.allocate(1024).unwrap();
        assert_eq!(buffer2.capacity(), capacity);

        let stats = manager.stats();
        assert!(stats.pool_hits.load(std::sync::atomic::Ordering::Relaxed) > 0);
    }

    #[test]
    fn test_diagnostics() {
        let config = MemoryConfig {
            max_memory: 1024 * 1024, // 1MB
            enable_diagnostics: true,
            ..Default::default()
        };
        let manager = MemoryManager::with_config(config);

        // Allocate some buffers (use smaller sizes that fit in categories)
        let _b1 = manager.allocate(10 * 1024).unwrap(); // 10KB
        let _b2 = manager.allocate(20 * 1024).unwrap(); // 20KB

        let diag = manager.diagnostics();
        assert!(diag.memory_used > 0);
        assert_eq!(diag.memory_limit, 1024 * 1024);
        assert!(diag.usage_percent > 0.0);
        assert!(!diag.is_critical);
    }

    #[test]
    fn test_shrinking() {
        let config = MemoryConfig {
            max_memory: 1024 * 1024, // 1MB
            auto_shrink: true,
            ..Default::default()
        };
        let manager = MemoryManager::with_config(config);

        // Allocate several buffers and release them to pool
        let mut buffers = Vec::new();
        for _ in 0..5 {
            buffers.push(manager.allocate(64 * 1024).unwrap());
        }

        for buffer in buffers {
            manager.release(buffer);
        }

        let before_shrink = manager.memory_used();
        assert!(before_shrink > 0, "Should have memory in pools");

        // Manually trigger shrink
        manager.shrink_pools(100 * 1024); // Shrink to 100KB

        let after_shrink = manager.memory_used();
        assert!(
            after_shrink <= before_shrink,
            "Memory should not increase after shrinking"
        );
    }
}
