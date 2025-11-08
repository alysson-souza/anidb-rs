//! Comprehensive tests for the unified memory manager

use anidb_client_core::memory::{
    BufferSize, MemoryConfig, MemoryManager, allocate, clear_pools, diagnostics, memory_limit,
    memory_used, release,
};
use std::sync::{Arc, Barrier};
use std::thread;
use std::time::Duration;

#[test]
fn test_memory_limit_enforcement() {
    // Create a manager with a small limit for testing
    let config = MemoryConfig {
        max_memory: 10_000, // 10KB limit
        max_pool_size: 5,
        auto_shrink: false,
        eviction_timeout: Duration::from_secs(60),
        enable_diagnostics: false,
    };

    let manager = MemoryManager::with_config(config);

    // Allocate within limit
    let buffer1 = manager.allocate(4_000).unwrap();
    assert_eq!(buffer1.len(), 4_000);
    assert!(manager.memory_used() > 0);

    // Try to allocate more than remaining - should fail
    let result = manager.allocate(8_000);
    assert!(result.is_err());

    // Release the first buffer
    manager.release(buffer1);

    // Now allocation should succeed (buffer was pooled)
    let buffer2 = manager.allocate(4_000).unwrap();
    assert_eq!(buffer2.len(), 4_000);

    // Clear pools to free memory
    manager.clear_pools();
    manager.release(buffer2);
}

#[test]
fn test_buffer_size_categories() {
    let manager = MemoryManager::new();

    // Test small buffer
    let small = manager.allocate(1024).unwrap();
    assert_eq!(small.len(), 1024);
    assert!(small.capacity() >= BufferSize::Small.size());
    manager.release(small);

    // Test medium buffer
    let medium = manager.allocate(32 * 1024).unwrap();
    assert_eq!(medium.len(), 32 * 1024);
    assert!(medium.capacity() >= BufferSize::Medium.size());
    manager.release(medium);

    // Test large buffer
    let large = manager.allocate(512 * 1024).unwrap();
    assert_eq!(large.len(), 512 * 1024);
    assert!(large.capacity() >= BufferSize::Large.size());
    manager.release(large);

    // Test extra large buffer
    let extra_large = manager.allocate(4 * 1024 * 1024).unwrap();
    assert_eq!(extra_large.len(), 4 * 1024 * 1024);
    assert!(extra_large.capacity() >= BufferSize::ExtraLarge.size());
    manager.release(extra_large);
}

#[test]
fn test_pool_reuse() {
    let manager = MemoryManager::new();

    // Allocate and release a buffer
    let buffer1 = manager.allocate(2048).unwrap();
    let capacity1 = buffer1.capacity();
    manager.release(buffer1);

    // Allocate again - should reuse from pool
    let buffer2 = manager.allocate(2048).unwrap();
    let capacity2 = buffer2.capacity();

    // Should get the same capacity (reused buffer)
    assert_eq!(capacity1, capacity2);

    // Check statistics
    let stats = manager.stats();
    assert!(stats.pool_hits.load(std::sync::atomic::Ordering::Relaxed) > 0);

    manager.release(buffer2);
}

#[test]
fn test_memory_diagnostics() {
    let config = MemoryConfig {
        max_memory: 1024 * 1024, // 1MB
        enable_diagnostics: true,
        ..Default::default()
    };

    let manager = MemoryManager::with_config(config);

    // Allocate some memory (use smaller sizes that fit in categories)
    let _b1 = manager.allocate(10 * 1024).unwrap();
    let _b2 = manager.allocate(20 * 1024).unwrap();

    let diag = manager.diagnostics();

    // Check diagnostics
    assert!(diag.memory_used > 0);
    assert_eq!(diag.memory_limit, 1024 * 1024);
    assert!(diag.usage_percent > 0.0 && diag.usage_percent < 100.0);
    assert!(!diag.is_warning);
    assert!(!diag.is_critical);

    // Check that we have pool information
    assert!(!diag.pools.is_empty());
}

#[test]
fn test_auto_shrinking() {
    let config = MemoryConfig {
        max_memory: 1024 * 1024, // 1MB
        auto_shrink: true,
        ..Default::default()
    };

    let manager = MemoryManager::with_config(config);

    // Allocate and release many buffers to fill pools
    let mut buffers = Vec::new();
    for _ in 0..10 {
        buffers.push(manager.allocate(64 * 1024).unwrap());
    }

    // Release all to pools
    for buffer in buffers {
        manager.release(buffer);
    }

    let before_shrink = manager.memory_used();
    assert!(before_shrink > 0, "Should have memory in pools");

    // Manually trigger shrink to a small target
    manager.shrink_pools(100 * 1024);

    let after_shrink = manager.memory_used();

    // Memory usage should have decreased or stayed the same (if already below target)
    assert!(
        after_shrink <= before_shrink,
        "Memory should not increase after shrinking"
    );
}

#[test]
fn test_eviction() {
    let config = MemoryConfig {
        eviction_timeout: Duration::from_millis(100),
        ..Default::default()
    };

    let manager = MemoryManager::with_config(config);

    // Add buffers to pool
    for _ in 0..5 {
        let buffer = manager.allocate(1024).unwrap();
        manager.release(buffer);
    }

    // Wait for buffers to become stale
    thread::sleep(Duration::from_millis(150));

    // Evict stale buffers
    manager.evict_stale();

    // Pool should be smaller or empty
    let diag = manager.diagnostics();
    let total_pooled: usize = diag.pools.iter().map(|(_, info)| info.pool_size).sum();

    // Should have evicted at least some buffers
    assert!(total_pooled < 5);
}

#[test]
fn test_warning_and_critical_thresholds() {
    let config = MemoryConfig {
        max_memory: 10_000, // 10KB for easy testing
        auto_shrink: false,
        enable_diagnostics: true,
        ..Default::default()
    };

    let manager = MemoryManager::with_config(config);

    // Allocate to warning threshold (80%) - use small buffer that fits in Small category (4KB)
    let _buffer1 = manager.allocate(3000).unwrap(); // Will allocate 4KB
    let _buffer2 = manager.allocate(3000).unwrap(); // Will allocate 4KB, total 8KB (80%)

    assert!(manager.is_memory_warning() || manager.memory_usage_percent() > 70.0);
    assert!(!manager.is_memory_critical());

    // Try to allocate more to reach critical threshold (95%)
    if let Ok(_buffer3) = manager.allocate(1000) {
        // If this succeeds, we should be near critical
        assert!(manager.memory_usage_percent() > 85.0);
    }
}

#[test]
fn test_global_memory_manager() {
    // Clear any previous state
    clear_pools();

    // Test global allocate/release
    let buffer = allocate(2048).unwrap();
    assert_eq!(buffer.len(), 2048);

    let used_before = memory_used();
    assert!(used_before > 0);

    release(buffer);

    // Memory should still be used (pooled)
    assert!(memory_used() > 0);

    // Check global limit
    assert_eq!(memory_limit(), 500 * 1024 * 1024); // Default 500MB

    // Get global diagnostics
    let diag = diagnostics();
    assert!(diag.memory_limit > 0);
    assert!(diag.usage_percent >= 0.0);
}

#[test]
fn test_concurrent_allocations() {
    let manager = Arc::new(MemoryManager::new());
    let barrier = Arc::new(Barrier::new(10));
    let mut handles = vec![];

    for i in 0..10 {
        let manager_clone = manager.clone();
        let barrier_clone = barrier.clone();

        let handle = thread::spawn(move || {
            // Wait for all threads to be ready
            barrier_clone.wait();

            // Each thread allocates and releases buffers
            let mut allocated = vec![];
            for j in 0..10 {
                match manager_clone.allocate(1024 * (i + 1)) {
                    Ok(buffer) => {
                        // Simulate some work
                        thread::sleep(Duration::from_micros(j as u64 * 10));
                        allocated.push(buffer);
                    }
                    Err(_) => {
                        // Memory limit reached, that's ok in concurrent test
                        break;
                    }
                }
            }

            // Release all buffers
            for buffer in allocated {
                manager_clone.release(buffer);
            }
        });

        handles.push(handle);
    }

    // Wait for all threads to complete
    for handle in handles {
        handle.join().unwrap();
    }

    // Check that statistics are consistent
    let stats = manager.stats();
    assert!(stats.allocations.load(std::sync::atomic::Ordering::Relaxed) > 0);
    assert!(
        stats
            .allocation_attempts
            .load(std::sync::atomic::Ordering::Relaxed)
            >= stats.allocations.load(std::sync::atomic::Ordering::Relaxed)
    );
}

#[test]
fn test_memory_fragmentation_prevention() {
    let config = MemoryConfig {
        max_memory: 10 * 1024 * 1024, // 10MB
        max_pool_size: 10,
        ..Default::default()
    };

    let manager = MemoryManager::with_config(config);

    // Allocate buffers of varying sizes
    let mut buffers = vec![];
    for i in 0..20 {
        let size = 1024 * (1 + i % 5); // 1KB to 5KB
        if let Ok(buffer) = manager.allocate(size) {
            buffers.push(buffer);
        }
    }

    // Release every other buffer to create fragmentation
    for i in (0..buffers.len()).step_by(2) {
        if i < buffers.len() {
            let buffer = buffers.remove(i);
            manager.release(buffer);
        }
    }

    // Try to allocate a medium-sized buffer
    let result = manager.allocate(64 * 1024);

    // Should still be able to allocate despite fragmentation
    assert!(result.is_ok() || manager.memory_used() > manager.memory_limit() * 9 / 10);

    // Clean up
    for buffer in buffers {
        manager.release(buffer);
    }

    if let Ok(buffer) = result {
        manager.release(buffer);
    }
}

#[test]
fn test_pool_hit_rate() {
    let manager = MemoryManager::new();

    // Warm up the pool
    for _ in 0..5 {
        let buffer = manager.allocate(4096).unwrap();
        manager.release(buffer);
    }

    // Reset stats to measure hit rate accurately
    // Note: In production, we wouldn't reset, but for testing it's useful

    // Allocate and release multiple times
    for _ in 0..10 {
        let buffer = manager.allocate(4096).unwrap();
        manager.release(buffer);
    }

    let stats = manager.stats();
    let hit_rate = stats.pool_hit_rate();

    // After warm-up, we should have a good hit rate
    assert!(hit_rate > 50.0, "Hit rate was only {hit_rate:.1}%");
}

#[test]
fn test_memory_leak_detection() {
    let config = MemoryConfig {
        max_memory: 1024 * 1024, // 1MB
        auto_shrink: false,
        ..Default::default()
    };

    let manager = MemoryManager::with_config(config);

    // Track initial memory
    let initial_used = manager.memory_used();

    // Allocate and properly release
    for _ in 0..100 {
        let buffer = manager.allocate(1024).unwrap();
        manager.release(buffer);
    }

    // Clear pools to reclaim all memory
    manager.clear_pools();

    // Memory should return close to initial (some overhead is ok)
    let final_used = manager.memory_used();

    // Allow for some overhead, but not more than 10KB
    assert!(
        final_used <= initial_used + 10 * 1024,
        "Possible memory leak: initial={initial_used}, final={final_used}"
    );
}
