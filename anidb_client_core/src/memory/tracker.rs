//! Memory tracking implementation

use crate::{Error, Result, error::InternalError};
use std::sync::atomic::{AtomicUsize, Ordering};

/// Global memory tracker for enforcing memory limits
pub struct MemoryTracker {
    /// Current memory usage in bytes
    used: AtomicUsize,
    /// Maximum memory limit in bytes
    limit: usize,
}

impl MemoryTracker {
    /// Create a new memory tracker with the specified limit
    pub fn new(limit: usize) -> Self {
        Self {
            used: AtomicUsize::new(0),
            limit,
        }
    }

    /// Get current memory usage
    pub fn used(&self) -> usize {
        self.used.load(Ordering::Acquire)
    }

    /// Get memory limit
    pub fn limit(&self) -> usize {
        self.limit
    }

    /// Try to allocate memory, returning error if it would exceed limit
    pub fn try_allocate(&self, size: usize) -> Result<()> {
        // Use a CAS loop for atomic check and update
        let mut current = self.used.load(Ordering::Acquire);

        loop {
            let new_used = current + size;

            // Check if allocation would exceed limit
            if new_used > self.limit {
                return Err(Error::Internal(InternalError::memory_limit_exceeded(
                    self.limit, new_used,
                )));
            }

            // Try to update atomically
            match self.used.compare_exchange_weak(
                current,
                new_used,
                Ordering::AcqRel,
                Ordering::Acquire,
            ) {
                Ok(_) => return Ok(()),
                Err(actual) => {
                    // Someone else modified it, retry with new value
                    current = actual;
                }
            }
        }
    }

    /// Deallocate memory (called when buffers are freed)
    pub fn deallocate(&self, size: usize) {
        // Use saturating subtraction to prevent underflow
        self.used
            .fetch_update(Ordering::AcqRel, Ordering::Acquire, |current| {
                Some(current.saturating_sub(size))
            })
            .ok();
    }

    /// Reset memory tracking (mainly for tests)
    #[cfg(test)]
    pub fn reset(&self) {
        self.used.store(0, Ordering::Release);
    }

    /// Get memory usage as a percentage of the limit
    pub fn usage_percent(&self) -> f64 {
        (self.used() as f64 / self.limit as f64) * 100.0
    }

    /// Check if memory usage is above a threshold percentage
    pub fn is_above_threshold(&self, threshold_percent: f64) -> bool {
        self.usage_percent() >= threshold_percent
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_memory_tracker_basic() {
        let tracker = MemoryTracker::new(1024 * 1024); // 1MB limit

        // Initial state
        assert_eq!(tracker.used(), 0);
        assert_eq!(tracker.limit(), 1024 * 1024);

        // Allocate some memory
        tracker.try_allocate(1024).unwrap();
        assert_eq!(tracker.used(), 1024);

        // Deallocate
        tracker.deallocate(1024);
        assert_eq!(tracker.used(), 0);
    }

    #[test]
    fn test_memory_limit_enforcement() {
        let tracker = MemoryTracker::new(10_000);

        // Allocate within limit
        tracker.try_allocate(5_000).unwrap();
        assert_eq!(tracker.used(), 5_000);

        // Try to allocate more than remaining
        let result = tracker.try_allocate(6_000);
        assert!(result.is_err());
        assert_eq!(tracker.used(), 5_000); // Should not have changed

        // Allocate exactly remaining
        tracker.try_allocate(5_000).unwrap();
        assert_eq!(tracker.used(), 10_000);
    }

    #[test]
    fn test_concurrent_allocation() {
        use std::sync::Arc;
        use std::thread;

        let tracker = Arc::new(MemoryTracker::new(100_000));
        let mut handles = vec![];

        // Spawn multiple threads trying to allocate
        for _ in 0..10 {
            let tracker_clone = tracker.clone();
            let handle = thread::spawn(move || {
                let mut allocated = 0;
                for _ in 0..100 {
                    if tracker_clone.try_allocate(100).is_ok() {
                        allocated += 100;
                    }
                }
                allocated
            });
            handles.push(handle);
        }

        // Wait for all threads and sum allocations
        let total_allocated: usize = handles.into_iter().map(|h| h.join().unwrap()).sum();

        // Total allocated should match tracker's used memory
        assert_eq!(tracker.used(), total_allocated);
        // And should not exceed limit
        assert!(total_allocated <= 100_000);
    }

    #[test]
    fn test_usage_percentage() {
        let tracker = MemoryTracker::new(1000);

        tracker.try_allocate(250).unwrap();
        assert_eq!(tracker.usage_percent(), 25.0);

        tracker.try_allocate(250).unwrap();
        assert_eq!(tracker.usage_percent(), 50.0);

        assert!(!tracker.is_above_threshold(60.0));
        assert!(tracker.is_above_threshold(50.0));
        assert!(tracker.is_above_threshold(40.0));
    }

    #[test]
    fn test_saturating_deallocation() {
        let tracker = MemoryTracker::new(1000);

        // Deallocate more than allocated (should saturate at 0)
        tracker.deallocate(100);
        assert_eq!(tracker.used(), 0);

        // Allocate and then over-deallocate
        tracker.try_allocate(50).unwrap();
        tracker.deallocate(100);
        assert_eq!(tracker.used(), 0);
    }
}
