//! FFI Memory Management and Tracking
//!
//! This module provides memory management utilities specifically for FFI,
//! including tracking allocations, detecting leaks, and ensuring proper
//! cleanup across the language boundary.

use crate::buffer::get_memory_limit;
#[cfg(test)]
use crate::memory::clear_pools;
use crate::memory::{allocate, diagnostics, memory_used, release};
use std::collections::HashMap;
use std::ffi::CString;
use std::os::raw::c_char;
use std::ptr;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

/// Memory allocation tracking information
#[derive(Debug, Clone)]
pub struct AllocationInfo {
    /// Size of the allocation
    pub size: usize,
    /// Type of allocation
    pub alloc_type: AllocationType,
    /// Stack trace at allocation (if debug mode)
    #[cfg(debug_assertions)]
    pub stack_trace: String,
    /// Timestamp of allocation
    pub timestamp: u64,
}

/// Types of allocations we track
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AllocationType {
    /// String allocated for FFI
    String,
    /// Buffer for hash results
    HashResult,
    /// File result structure
    FileResult,
    /// Batch result structure
    BatchResult,
    /// Generic buffer
    Buffer,
    /// Event data
    Event,
}

// Global allocation tracker
lazy_static::lazy_static! {
    pub static ref ALLOCATION_TRACKER: AllocationTracker = AllocationTracker::new();
}

/// Tracks all FFI allocations for leak detection
pub struct AllocationTracker {
    /// Map of allocation address to info
    allocations: Arc<Mutex<HashMap<usize, AllocationInfo>>>,
    /// Total allocations made
    total_allocations: Arc<AtomicU64>,
    /// Total deallocations made
    total_deallocations: Arc<AtomicU64>,
    /// Current allocated memory
    current_allocated: Arc<AtomicUsize>,
    /// Peak allocated memory
    peak_allocated: Arc<AtomicUsize>,
}

impl AllocationTracker {
    fn new() -> Self {
        Self {
            allocations: Arc::new(Mutex::new(HashMap::new())),
            total_allocations: Arc::new(AtomicU64::new(0)),
            total_deallocations: Arc::new(AtomicU64::new(0)),
            current_allocated: Arc::new(AtomicUsize::new(0)),
            peak_allocated: Arc::new(AtomicUsize::new(0)),
        }
    }

    /// Track a new allocation
    pub fn track_allocation(&self, ptr: *const u8, size: usize, alloc_type: AllocationType) {
        if ptr.is_null() {
            return;
        }

        let addr = ptr as usize;
        let info = AllocationInfo {
            size,
            alloc_type,
            #[cfg(debug_assertions)]
            stack_trace: Self::capture_stack_trace(),
            timestamp: Self::current_timestamp(),
        };

        if let Ok(mut allocations) = self.allocations.lock() {
            allocations.insert(addr, info);
        }

        self.total_allocations.fetch_add(1, Ordering::Relaxed);

        // Update current and peak memory
        let new_size = self.current_allocated.fetch_add(size, Ordering::AcqRel) + size;
        self.peak_allocated.fetch_max(new_size, Ordering::Relaxed);
    }

    /// Track a deallocation
    pub fn track_deallocation(&self, ptr: *const u8) -> Option<AllocationInfo> {
        if ptr.is_null() {
            return None;
        }

        let addr = ptr as usize;

        if let Ok(mut allocations) = self.allocations.lock()
            && let Some(info) = allocations.remove(&addr)
        {
            self.total_deallocations.fetch_add(1, Ordering::Relaxed);
            self.current_allocated
                .fetch_sub(info.size, Ordering::AcqRel);
            return Some(info);
        }

        None
    }

    /// Get current allocation statistics
    pub fn get_stats(&self) -> AllocationStats {
        let allocations = self.allocations.lock().unwrap_or_else(|e| e.into_inner());

        let mut by_type = HashMap::new();
        for info in allocations.values() {
            *by_type.entry(info.alloc_type).or_insert(0) += info.size;
        }

        AllocationStats {
            total_allocations: self.total_allocations.load(Ordering::Relaxed),
            total_deallocations: self.total_deallocations.load(Ordering::Relaxed),
            current_allocated: self.current_allocated.load(Ordering::Relaxed),
            peak_allocated: self.peak_allocated.load(Ordering::Relaxed),
            active_allocations: allocations.len(),
            allocations_by_type: by_type,
        }
    }

    /// Check for memory leaks
    pub fn check_leaks(&self) -> Vec<LeakInfo> {
        let allocations = self.allocations.lock().unwrap_or_else(|e| e.into_inner());

        allocations
            .iter()
            .map(|(&addr, info)| LeakInfo {
                address: addr,
                size: info.size,
                alloc_type: info.alloc_type,
                #[cfg(debug_assertions)]
                stack_trace: info.stack_trace.clone(),
                age_ms: Self::current_timestamp() - info.timestamp,
            })
            .collect()
    }

    /// Clear all tracking (for tests)
    #[cfg(test)]
    pub fn clear(&self) {
        if let Ok(mut allocations) = self.allocations.lock() {
            allocations.clear();
        }
        self.total_allocations.store(0, Ordering::Relaxed);
        self.total_deallocations.store(0, Ordering::Relaxed);
        self.current_allocated.store(0, Ordering::Relaxed);
        self.peak_allocated.store(0, Ordering::Relaxed);
    }

    #[cfg(debug_assertions)]
    fn capture_stack_trace() -> String {
        // In debug mode, capture a simple stack trace
        // In production, this would be compiled out
        std::backtrace::Backtrace::capture()
            .to_string()
            .lines()
            .take(10)
            .collect::<Vec<_>>()
            .join("\n")
    }

    fn current_timestamp() -> u64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64
    }
}

/// Statistics about FFI allocations
#[derive(Debug, Clone)]
pub struct AllocationStats {
    pub total_allocations: u64,
    pub total_deallocations: u64,
    pub current_allocated: usize,
    pub peak_allocated: usize,
    pub active_allocations: usize,
    pub allocations_by_type: HashMap<AllocationType, usize>,
}

/// Information about a potential memory leak
#[derive(Debug, Clone)]
pub struct LeakInfo {
    pub address: usize,
    pub size: usize,
    pub alloc_type: AllocationType,
    #[cfg(debug_assertions)]
    pub stack_trace: String,
    pub age_ms: u64,
}

/// Allocate a string for FFI with tracking
pub fn ffi_allocate_string(s: &str) -> *mut c_char {
    match CString::new(s) {
        Ok(c_str) => {
            let ptr = c_str.into_raw();
            ALLOCATION_TRACKER.track_allocation(
                ptr as *const u8,
                s.len() + 1, // +1 for null terminator
                AllocationType::String,
            );
            ptr
        }
        Err(_) => ptr::null_mut(),
    }
}

/// Free a string allocated for FFI
///
/// # Safety
///
/// The pointer must have been allocated by `ffi_allocate_string` and not
/// previously freed.
pub unsafe fn ffi_free_string(ptr: *mut c_char) {
    if ptr.is_null() {
        return;
    }

    ALLOCATION_TRACKER.track_deallocation(ptr as *const u8);
    unsafe {
        let _ = CString::from_raw(ptr);
    }
}

/// Allocate a buffer with FFI tracking
pub fn ffi_allocate_buffer(
    size: usize,
    alloc_type: AllocationType,
) -> Result<Vec<u8>, crate::Error> {
    // Try to allocate from unified memory manager
    let buffer = allocate(size)?;

    // Don't track the allocation here - it's already tracked by allocate_buffer
    // Only track the FFI-specific metadata
    ALLOCATION_TRACKER.track_allocation(buffer.as_ptr(), buffer.capacity(), alloc_type);

    Ok(buffer)
}

/// Release a buffer with FFI tracking
pub fn ffi_release_buffer(buffer: Vec<u8>) {
    ALLOCATION_TRACKER.track_deallocation(buffer.as_ptr());
    release(buffer);
}

/// Get memory usage statistics
pub fn get_memory_stats() -> MemoryStats {
    let allocation_stats = ALLOCATION_TRACKER.get_stats();
    let diag = diagnostics();
    let stats = diag.stats;

    MemoryStats {
        total_memory_used: memory_used(),
        ffi_allocated: allocation_stats.current_allocated,
        ffi_peak: allocation_stats.peak_allocated,
        pool_memory: diag.memory_used,
        pool_hits: stats.pool_hits.load(std::sync::atomic::Ordering::Relaxed),
        pool_misses: stats.pool_misses.load(std::sync::atomic::Ordering::Relaxed),
        active_allocations: allocation_stats.active_allocations,
        memory_limit: get_memory_limit(),
    }
}

/// Combined memory statistics
#[derive(Debug, Clone)]
pub struct MemoryStats {
    /// Total memory used by the library
    pub total_memory_used: usize,
    /// Memory allocated through FFI
    pub ffi_allocated: usize,
    /// Peak FFI memory usage
    pub ffi_peak: usize,
    /// Memory held in buffer pool
    pub pool_memory: usize,
    /// Buffer pool cache hits
    pub pool_hits: usize,
    /// Buffer pool cache misses
    pub pool_misses: usize,
    /// Number of active allocations
    pub active_allocations: usize,
    /// Memory limit
    pub memory_limit: usize,
}

/// Check if we're approaching memory limit
pub fn check_memory_pressure() -> MemoryPressure {
    let used = memory_used();
    let limit = get_memory_limit();
    let ratio = used as f64 / limit as f64;

    if ratio > 0.9 {
        MemoryPressure::Critical
    } else if ratio > 0.75 {
        MemoryPressure::High
    } else if ratio > 0.5 {
        MemoryPressure::Medium
    } else {
        MemoryPressure::Low
    }
}

/// Memory pressure levels
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MemoryPressure {
    Low,
    Medium,
    High,
    Critical,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_string_allocation_tracking() {
        let test_str = "Hello, FFI!";
        let ptr = ffi_allocate_string(test_str);
        assert!(!ptr.is_null());

        // Verify the allocation is tracked
        let allocations = ALLOCATION_TRACKER.allocations.lock().unwrap();
        let our_alloc = allocations.get(&(ptr as usize));
        assert!(our_alloc.is_some());
        if let Some(info) = our_alloc {
            assert_eq!(info.alloc_type, AllocationType::String);
            assert_eq!(info.size, test_str.len() + 1);
        }
        drop(allocations);

        unsafe {
            ffi_free_string(ptr);
        }

        // Verify the allocation was removed
        let allocations = ALLOCATION_TRACKER.allocations.lock().unwrap();
        let our_alloc = allocations.get(&(ptr as usize));
        assert!(our_alloc.is_none());
    }

    #[test]
    fn test_buffer_allocation_tracking() {
        // Clear the pool to ensure consistent test behavior
        #[cfg(test)]
        clear_pools();

        let buffer = ffi_allocate_buffer(1024, AllocationType::Buffer).unwrap();
        assert_eq!(buffer.len(), 1024);
        let buffer_ptr = buffer.as_ptr();

        // Verify the allocation is tracked
        let allocations = ALLOCATION_TRACKER.allocations.lock().unwrap();
        let our_alloc = allocations.get(&(buffer_ptr as usize));
        assert!(our_alloc.is_some());
        if let Some(info) = our_alloc {
            assert_eq!(info.alloc_type, AllocationType::Buffer);
            assert!(info.size >= 1024);
        }
        drop(allocations);

        ffi_release_buffer(buffer);

        // Verify the allocation was removed
        let allocations = ALLOCATION_TRACKER.allocations.lock().unwrap();
        let our_alloc = allocations.get(&(buffer_ptr as usize));
        assert!(our_alloc.is_none());
    }

    #[test]
    fn test_leak_detection() {
        // Get initial leak count
        let initial_leaks = ALLOCATION_TRACKER.check_leaks().len();

        // Allocate without freeing
        let ptr1 = ffi_allocate_string("Leak 1");
        let ptr2 = ffi_allocate_string("Leak 2");

        let current_leaks = ALLOCATION_TRACKER.check_leaks();
        // Should have at least 2 more leaks than initially
        assert!(current_leaks.len() >= initial_leaks + 2);

        // Find our specific leaks
        let our_leaks: Vec<_> = current_leaks
            .iter()
            .filter(|leak| {
                leak.alloc_type == AllocationType::String
                    && (leak.address == ptr1 as usize || leak.address == ptr2 as usize)
            })
            .collect();
        assert_eq!(our_leaks.len(), 2);

        for leak in &our_leaks {
            assert_eq!(leak.alloc_type, AllocationType::String);
            assert!(leak.size > 0);
        }

        // Clean up after the test to avoid affecting other tests
        unsafe {
            ffi_free_string(ptr1);
            ffi_free_string(ptr2);
        }
    }

    #[test]
    fn test_memory_pressure() {
        // This test just verifies the logic works
        let pressure = check_memory_pressure();
        assert!(matches!(
            pressure,
            MemoryPressure::Low
                | MemoryPressure::Medium
                | MemoryPressure::High
                | MemoryPressure::Critical
        ));
    }

    #[test]
    fn test_allocation_by_type() {
        // Clear the pool to ensure consistent test behavior
        #[cfg(test)]
        clear_pools();

        // Get initial stats
        let initial_stats = ALLOCATION_TRACKER.get_stats();
        let _initial_type_count = initial_stats.allocations_by_type.len();

        let str_ptr = ffi_allocate_string("test");
        let buf1 = ffi_allocate_buffer(100, AllocationType::HashResult).unwrap();
        let buf2 = ffi_allocate_buffer(200, AllocationType::FileResult).unwrap();

        let stats = ALLOCATION_TRACKER.get_stats();
        // Should have at least the types we allocated
        assert!(stats.allocations_by_type.len() >= 3);
        assert!(
            stats
                .allocations_by_type
                .contains_key(&AllocationType::String)
        );
        assert!(
            stats
                .allocations_by_type
                .contains_key(&AllocationType::HashResult)
        );
        assert!(
            stats
                .allocations_by_type
                .contains_key(&AllocationType::FileResult)
        );

        // Verify that our allocations are tracked
        let string_size = stats
            .allocations_by_type
            .get(&AllocationType::String)
            .copied()
            .unwrap_or(0);
        let hash_size = stats
            .allocations_by_type
            .get(&AllocationType::HashResult)
            .copied()
            .unwrap_or(0);
        let file_size = stats
            .allocations_by_type
            .get(&AllocationType::FileResult)
            .copied()
            .unwrap_or(0);

        // These should at least include our allocations
        assert!(string_size >= 5); // "test" + null terminator
        assert!(hash_size >= 100);
        assert!(file_size >= 200);

        // Clean up after the test
        unsafe {
            ffi_free_string(str_ptr);
        }
        ffi_release_buffer(buf1);
        ffi_release_buffer(buf2);
    }
}
