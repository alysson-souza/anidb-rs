//! Buffer pool implementation for the memory manager

use super::BufferSize;
use std::collections::VecDeque;
use std::sync::Mutex;
use std::time::{Duration, Instant};

/// A buffer with tracking metadata
pub struct PooledBuffer {
    /// The actual buffer data
    pub data: Vec<u8>,
    /// When this buffer was last used
    pub last_used: Instant,
    /// Number of times this buffer has been reused
    pub reuse_count: usize,
}

/// Information about a buffer pool
#[derive(Debug, Clone)]
pub struct PoolInfo {
    /// Size category of this pool
    pub size_category: BufferSize,
    /// Number of buffers currently in pool
    pub pool_size: usize,
    /// Total memory used by this pool
    pub memory_used: usize,
    /// Number of times buffers were reused
    pub total_reuses: usize,
}

/// A pool of buffers for a specific size category
pub struct BufferPool {
    /// Size category for this pool
    size_category: BufferSize,
    /// Maximum number of buffers to keep in pool
    max_size: usize,
    /// The actual pool of buffers
    buffers: Mutex<VecDeque<PooledBuffer>>,
    /// Total number of reuses
    total_reuses: Mutex<usize>,
}

impl BufferPool {
    /// Create a new buffer pool for a specific size category
    pub fn new(size_category: BufferSize, max_size: usize) -> Self {
        Self {
            size_category,
            max_size,
            buffers: Mutex::new(VecDeque::with_capacity(max_size)),
            total_reuses: Mutex::new(0),
        }
    }

    /// Try to acquire a buffer from the pool
    pub fn try_acquire(&self) -> Option<Vec<u8>> {
        if let Ok(mut buffers) = self.buffers.try_lock()
            && let Some(mut pooled) = buffers.pop_front()
        {
            pooled.last_used = Instant::now();
            pooled.reuse_count += 1;

            // Update reuse counter
            if let Ok(mut reuses) = self.total_reuses.try_lock() {
                *reuses += 1;
            }

            return Some(pooled.data);
        }
        None
    }

    /// Release a buffer back to the pool
    pub fn release(&self, buffer: Vec<u8>) {
        if let Ok(mut buffers) = self.buffers.try_lock() {
            // Only add to pool if we haven't reached max size
            if buffers.len() < self.max_size {
                let pooled = PooledBuffer {
                    data: buffer,
                    last_used: Instant::now(),
                    reuse_count: 0,
                };
                buffers.push_back(pooled);
            }
            // Otherwise, let the buffer be dropped
        }
    }

    /// Check if we should pool a buffer
    pub fn should_pool(&self) -> bool {
        if let Ok(buffers) = self.buffers.try_lock() {
            buffers.len() < self.max_size
        } else {
            false
        }
    }

    /// Clear all buffers from the pool
    pub fn clear(&self) {
        if let Ok(mut buffers) = self.buffers.try_lock() {
            buffers.clear();
        }
    }

    /// Shrink the pool by removing buffers to free memory
    pub fn shrink(&self, bytes_to_free: usize) -> usize {
        if let Ok(mut buffers) = self.buffers.try_lock() {
            let mut freed = 0;

            while !buffers.is_empty() && freed < bytes_to_free {
                if let Some(pooled) = buffers.pop_front() {
                    freed += pooled.data.capacity();
                }
            }

            return freed;
        }
        0
    }

    /// Evict buffers that haven't been used recently
    pub fn evict_stale(&self, max_age: Duration) {
        if let Ok(mut buffers) = self.buffers.try_lock() {
            let now = Instant::now();

            // Remove stale buffers from the front (oldest)
            while let Some(front) = buffers.front() {
                if now.duration_since(front.last_used) > max_age {
                    buffers.pop_front();
                } else {
                    // Buffers are ordered by last_used, so we can stop here
                    break;
                }
            }
        }
    }

    /// Get information about this pool
    pub fn info(&self) -> PoolInfo {
        let (pool_size, memory_used) = if let Ok(buffers) = self.buffers.try_lock() {
            let size = buffers.len();
            let memory: usize = buffers.iter().map(|b| b.data.capacity()).sum();
            (size, memory)
        } else {
            (0, 0)
        };

        let total_reuses = self.total_reuses.try_lock().map(|r| *r).unwrap_or(0);

        PoolInfo {
            size_category: self.size_category,
            pool_size,
            memory_used,
            total_reuses,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_buffer_pool_basic() {
        let pool = BufferPool::new(BufferSize::Small, 5);

        // Pool should be empty initially
        assert!(pool.try_acquire().is_none());

        // Release a buffer to the pool
        let buffer = vec![0u8; 1024];
        pool.release(buffer);

        // Should be able to acquire it
        let acquired = pool.try_acquire();
        assert!(acquired.is_some());
        assert_eq!(acquired.unwrap().capacity(), 1024);

        // Pool should be empty again
        assert!(pool.try_acquire().is_none());
    }

    #[test]
    fn test_pool_max_size() {
        let pool = BufferPool::new(BufferSize::Medium, 3);

        // Add more buffers than max size
        for i in 0..5 {
            let mut buffer = vec![0u8; 1024];
            buffer[0] = i as u8; // Mark buffer for identification
            pool.release(buffer);
        }

        // Pool should only contain max_size buffers
        let info = pool.info();
        assert!(info.pool_size <= 3);
    }

    #[test]
    fn test_pool_eviction() {
        use std::thread;

        let pool = BufferPool::new(BufferSize::Large, 10);

        // Add some buffers
        for _ in 0..3 {
            pool.release(vec![0u8; 1024]);
        }

        // Wait a bit
        thread::sleep(Duration::from_millis(100));

        // Evict buffers older than 50ms
        pool.evict_stale(Duration::from_millis(50));

        // Pool should be empty
        let info = pool.info();
        assert_eq!(info.pool_size, 0);
    }

    #[test]
    fn test_pool_shrinking() {
        let pool = BufferPool::new(BufferSize::Small, 10);

        // Add buffers totaling ~4KB
        for _ in 0..4 {
            pool.release(vec![0u8; 1024]);
        }

        // Shrink by 2KB
        let freed = pool.shrink(2048);

        // Should have freed at least 2KB
        assert!(freed >= 2048);

        // Pool should have fewer buffers
        let info = pool.info();
        assert!(info.pool_size <= 2);
    }
}
