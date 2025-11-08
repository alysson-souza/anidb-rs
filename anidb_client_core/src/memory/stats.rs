//! Memory statistics and diagnostics

use super::BufferSize;
use super::pool::PoolInfo;
use std::sync::atomic::{AtomicUsize, Ordering};

/// Memory statistics for monitoring and optimization
#[derive(Debug)]
pub struct MemoryStats {
    /// Total allocation attempts
    pub allocation_attempts: AtomicUsize,
    /// Successful allocations
    pub allocations: AtomicUsize,
    /// Failed allocations
    pub allocation_failures: AtomicUsize,
    /// Pool hits (reused buffers)
    pub pool_hits: AtomicUsize,
    /// Pool misses (new allocations)
    pub pool_misses: AtomicUsize,
    /// Total bytes allocated
    pub bytes_allocated: AtomicUsize,
    /// Total bytes deallocated
    pub bytes_deallocated: AtomicUsize,
    /// Peak memory usage
    pub peak_memory: AtomicUsize,
}

impl Default for MemoryStats {
    fn default() -> Self {
        Self {
            allocation_attempts: AtomicUsize::new(0),
            allocations: AtomicUsize::new(0),
            allocation_failures: AtomicUsize::new(0),
            pool_hits: AtomicUsize::new(0),
            pool_misses: AtomicUsize::new(0),
            bytes_allocated: AtomicUsize::new(0),
            bytes_deallocated: AtomicUsize::new(0),
            peak_memory: AtomicUsize::new(0),
        }
    }
}

impl MemoryStats {
    /// Create new statistics tracker
    pub fn new() -> Self {
        Self::default()
    }

    /// Record an allocation attempt
    pub fn record_allocation_attempt(&self) {
        self.allocation_attempts.fetch_add(1, Ordering::Relaxed);
    }

    /// Record a successful allocation
    pub fn record_allocation(&self, size: usize) {
        self.allocations.fetch_add(1, Ordering::Relaxed);
        let total = self.bytes_allocated.fetch_add(size, Ordering::Relaxed) + size;

        // Update peak memory if needed
        self.update_peak_memory(total);
    }

    /// Record a failed allocation
    pub fn record_allocation_failure(&self) {
        self.allocation_failures.fetch_add(1, Ordering::Relaxed);
    }

    /// Record a pool hit
    pub fn record_pool_hit(&self) {
        self.pool_hits.fetch_add(1, Ordering::Relaxed);
    }

    /// Record a pool miss
    pub fn record_pool_miss(&self) {
        self.pool_misses.fetch_add(1, Ordering::Relaxed);
    }

    /// Record a deallocation
    pub fn record_deallocation(&self, size: usize) {
        self.bytes_deallocated.fetch_add(size, Ordering::Relaxed);
    }

    /// Update peak memory usage
    fn update_peak_memory(&self, current: usize) {
        let mut peak = self.peak_memory.load(Ordering::Relaxed);
        while current > peak {
            match self.peak_memory.compare_exchange_weak(
                peak,
                current,
                Ordering::Relaxed,
                Ordering::Relaxed,
            ) {
                Ok(_) => break,
                Err(actual) => peak = actual,
            }
        }
    }

    /// Get a snapshot of current statistics
    pub fn snapshot(&self) -> MemoryStats {
        MemoryStats {
            allocation_attempts: AtomicUsize::new(self.allocation_attempts.load(Ordering::Relaxed)),
            allocations: AtomicUsize::new(self.allocations.load(Ordering::Relaxed)),
            allocation_failures: AtomicUsize::new(self.allocation_failures.load(Ordering::Relaxed)),
            pool_hits: AtomicUsize::new(self.pool_hits.load(Ordering::Relaxed)),
            pool_misses: AtomicUsize::new(self.pool_misses.load(Ordering::Relaxed)),
            bytes_allocated: AtomicUsize::new(self.bytes_allocated.load(Ordering::Relaxed)),
            bytes_deallocated: AtomicUsize::new(self.bytes_deallocated.load(Ordering::Relaxed)),
            peak_memory: AtomicUsize::new(self.peak_memory.load(Ordering::Relaxed)),
        }
    }

    /// Calculate pool hit rate as a percentage
    pub fn pool_hit_rate(&self) -> f64 {
        let hits = self.pool_hits.load(Ordering::Relaxed) as f64;
        let total = (self.pool_hits.load(Ordering::Relaxed)
            + self.pool_misses.load(Ordering::Relaxed)) as f64;

        if total > 0.0 {
            (hits / total) * 100.0
        } else {
            0.0
        }
    }

    /// Get allocation success rate as a percentage
    pub fn allocation_success_rate(&self) -> f64 {
        let successes = self.allocations.load(Ordering::Relaxed) as f64;
        let attempts = self.allocation_attempts.load(Ordering::Relaxed) as f64;

        if attempts > 0.0 {
            (successes / attempts) * 100.0
        } else {
            100.0
        }
    }

    /// Reset all statistics
    #[cfg(test)]
    pub fn reset(&self) {
        self.allocation_attempts.store(0, Ordering::Relaxed);
        self.allocations.store(0, Ordering::Relaxed);
        self.allocation_failures.store(0, Ordering::Relaxed);
        self.pool_hits.store(0, Ordering::Relaxed);
        self.pool_misses.store(0, Ordering::Relaxed);
        self.bytes_allocated.store(0, Ordering::Relaxed);
        self.bytes_deallocated.store(0, Ordering::Relaxed);
        self.peak_memory.store(0, Ordering::Relaxed);
    }
}

impl Clone for MemoryStats {
    fn clone(&self) -> Self {
        self.snapshot()
    }
}

/// Detailed memory diagnostics
#[derive(Debug, Clone)]
pub struct MemoryDiagnostics {
    /// Current memory usage in bytes
    pub memory_used: usize,
    /// Memory limit in bytes
    pub memory_limit: usize,
    /// Memory usage as a percentage
    pub usage_percent: f64,
    /// Whether memory usage is at warning level
    pub is_warning: bool,
    /// Whether memory usage is at critical level
    pub is_critical: bool,
    /// Memory statistics
    pub stats: MemoryStats,
    /// Information about each buffer pool
    pub pools: Vec<(BufferSize, PoolInfo)>,
}

impl MemoryDiagnostics {
    /// Get a human-readable summary of diagnostics
    pub fn summary(&self) -> String {
        let mut summary = String::new();

        // Memory usage
        summary.push_str(&format!(
            "Memory Usage: {:.1}% ({} / {} MB)\n",
            self.usage_percent,
            self.memory_used / (1024 * 1024),
            self.memory_limit / (1024 * 1024)
        ));

        // Status
        if self.is_critical {
            summary.push_str("Status: CRITICAL - Memory usage critical!\n");
        } else if self.is_warning {
            summary.push_str("Status: WARNING - Memory usage high\n");
        } else {
            summary.push_str("Status: OK\n");
        }

        // Pool statistics
        summary.push_str(&format!(
            "Pool Hit Rate: {:.1}%\n",
            self.stats.pool_hit_rate()
        ));

        summary.push_str(&format!(
            "Allocation Success Rate: {:.1}%\n",
            self.stats.allocation_success_rate()
        ));

        // Pool details
        summary.push_str("\nBuffer Pools:\n");
        for (size, info) in &self.pools {
            summary.push_str(&format!(
                "  {:?}: {} buffers, {} KB, {} reuses\n",
                size,
                info.pool_size,
                info.memory_used / 1024,
                info.total_reuses
            ));
        }

        summary
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_memory_stats() {
        let stats = MemoryStats::new();

        // Record some operations
        stats.record_allocation_attempt();
        stats.record_allocation(1024);
        stats.record_pool_hit();

        stats.record_allocation_attempt();
        stats.record_allocation(2048);
        stats.record_pool_miss();

        // Check statistics
        assert_eq!(stats.allocation_attempts.load(Ordering::Relaxed), 2);
        assert_eq!(stats.allocations.load(Ordering::Relaxed), 2);
        assert_eq!(stats.bytes_allocated.load(Ordering::Relaxed), 3072);
        assert_eq!(stats.pool_hits.load(Ordering::Relaxed), 1);
        assert_eq!(stats.pool_misses.load(Ordering::Relaxed), 1);

        // Check calculated rates
        assert_eq!(stats.pool_hit_rate(), 50.0);
        assert_eq!(stats.allocation_success_rate(), 100.0);
    }

    #[test]
    fn test_peak_memory_tracking() {
        let stats = MemoryStats::new();

        // Record allocations
        stats.record_allocation(1000);
        assert_eq!(stats.peak_memory.load(Ordering::Relaxed), 1000);

        stats.record_allocation(2000);
        assert_eq!(stats.peak_memory.load(Ordering::Relaxed), 3000);

        // Deallocation shouldn't affect peak
        stats.record_deallocation(1000);
        assert_eq!(stats.peak_memory.load(Ordering::Relaxed), 3000);
    }

    #[test]
    fn test_diagnostics_summary() {
        let stats = MemoryStats::new();
        stats.record_allocation_attempt();
        stats.record_allocation(1024 * 1024);
        stats.record_pool_hit();

        let diag = MemoryDiagnostics {
            memory_used: 50 * 1024 * 1024,
            memory_limit: 100 * 1024 * 1024,
            usage_percent: 50.0,
            is_warning: false,
            is_critical: false,
            stats,
            pools: vec![(
                BufferSize::Small,
                PoolInfo {
                    size_category: BufferSize::Small,
                    pool_size: 5,
                    memory_used: 20 * 1024,
                    total_reuses: 10,
                },
            )],
        };

        let summary = diag.summary();
        assert!(summary.contains("Memory Usage: 50.0%"));
        assert!(summary.contains("Status: OK"));
        assert!(summary.contains("Pool Hit Rate:"));
        assert!(summary.contains("Buffer Pools:"));
    }
}
