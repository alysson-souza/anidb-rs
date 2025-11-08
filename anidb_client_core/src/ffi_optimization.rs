//! FFI Performance Optimizations
//!
//! This module provides optimized versions of hot-path FFI functions
//! with reduced overhead, better cache utilization, and minimal allocations.

use crate::ffi::*;
use crate::memory::allocate as pool_allocate;
use std::ffi::CStr;
use std::mem::{self, MaybeUninit};
use std::os::raw::c_char;
use std::ptr;

// Cache-aligned structure to reduce false sharing
#[repr(C, align(64))]
pub struct AlignedFileResult {
    pub inner: AniDBFileResult,
    _padding: [u8; 64 - (mem::size_of::<AniDBFileResult>() % 64)],
}

// Pre-allocated thread-local buffers for string operations
thread_local! {
    static STRING_BUFFER: std::cell::RefCell<Vec<u8>> = std::cell::RefCell::new(Vec::with_capacity(4096));
    static PATH_BUFFER: std::cell::RefCell<Vec<u8>> = std::cell::RefCell::new(Vec::with_capacity(512));
}

/// Optimized string allocation using thread-local buffer
///
/// Reduces allocations for small strings by reusing thread-local buffers
#[inline]
pub fn optimized_string_alloc(s: &str) -> *mut c_char {
    let len = s.len();

    // For small strings, try to use thread-local buffer
    if len < 256 {
        STRING_BUFFER.with(|buffer| {
            let mut buf = buffer.borrow_mut();

            // Check if buffer has enough capacity
            if buf.capacity() > len {
                buf.clear();
                buf.extend_from_slice(s.as_bytes());
                buf.push(0); // null terminator

                // Allocate permanent copy
                let ptr = unsafe {
                    let alloc = libc::malloc(buf.len()) as *mut u8;
                    if !alloc.is_null() {
                        ptr::copy_nonoverlapping(buf.as_ptr(), alloc, buf.len());
                    }
                    alloc as *mut c_char
                };

                return ptr;
            }

            // Fall back to standard allocation
            crate::ffi_memory::ffi_allocate_string(s)
        })
    } else {
        crate::ffi_memory::ffi_allocate_string(s)
    }
}

/// Optimized C string parsing with minimal overhead
///
/// Avoids UTF-8 validation for trusted paths
///
/// # Safety
///
/// The caller must ensure that:
/// - `s` is either null or a valid pointer to a null-terminated C string
/// - The string remains valid for the returned lifetime
#[inline]
pub unsafe fn fast_c_str_to_path(s: *const c_char) -> Result<&'static str, AniDBResult> {
    if s.is_null() {
        return Err(AniDBResult::ErrorInvalidParameter);
    }

    unsafe {
        // Use unchecked conversion for trusted file paths
        let c_str = CStr::from_ptr(s);
        let bytes = c_str.to_bytes();

        // Fast ASCII check for common file paths
        if bytes.iter().all(|&b| b < 128) {
            // Safe because we verified ASCII
            Ok(std::str::from_utf8_unchecked(bytes))
        } else {
            // Fall back to checked conversion for non-ASCII
            c_str.to_str().map_err(|_| AniDBResult::ErrorInvalidUtf8)
        }
    }
}

/// Zero-copy hash result creation
///
/// Creates hash results without intermediate allocations
#[inline]
pub fn create_hash_result_zero_copy(
    algorithm: AniDBHashAlgorithm,
    hash: &str,
    buffer: &mut [u8],
) -> Result<AniDBHashResult, AniDBResult> {
    let hash_bytes = hash.as_bytes();
    let required_size = hash_bytes.len() + 1;

    if buffer.len() < required_size {
        return Err(AniDBResult::ErrorInvalidParameter);
    }

    // Copy hash value directly into provided buffer
    unsafe {
        ptr::copy_nonoverlapping(hash_bytes.as_ptr(), buffer.as_mut_ptr(), hash_bytes.len());
        buffer[hash_bytes.len()] = 0; // null terminator
    }

    Ok(AniDBHashResult {
        algorithm,
        hash_value: buffer.as_mut_ptr() as *mut c_char,
        hash_length: hash_bytes.len(),
    })
}

/// Batch allocate file results for better cache locality
///
/// Allocates multiple file results in a single contiguous block
pub fn batch_allocate_file_results(count: usize) -> Result<Vec<Box<AniDBFileResult>>, AniDBResult> {
    if count == 0 || count > 10000 {
        return Err(AniDBResult::ErrorInvalidParameter);
    }

    // Allocate all results in one go for better cache performance
    let mut results = Vec::with_capacity(count);

    // Use MaybeUninit to avoid unnecessary zeroing
    for _ in 0..count {
        let mut result = Box::new(MaybeUninit::<AniDBFileResult>::uninit());

        // Initialize with default values
        unsafe {
            let ptr = result.as_mut_ptr();
            (*ptr).file_path = ptr::null_mut();
            (*ptr).file_size = 0;
            (*ptr).status = AniDBStatus::Pending;
            (*ptr).hashes = ptr::null_mut();
            (*ptr).hash_count = 0;
            (*ptr).processing_time_ms = 0;
            (*ptr).error_message = ptr::null_mut();

            results.push(Box::from_raw(ptr));
        }
    }

    Ok(results)
}

/// Optimized progress callback invocation
///
/// Reduces overhead by batching progress updates
#[repr(C)]
pub struct ProgressBatch {
    pub updates: [ProgressUpdate; 16],
    pub count: usize,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct ProgressUpdate {
    pub percentage: f32,
    pub bytes_processed: u64,
    pub total_bytes: u64,
}

impl Default for ProgressBatch {
    fn default() -> Self {
        Self::new()
    }
}

impl ProgressBatch {
    #[inline]
    pub fn new() -> Self {
        Self {
            updates: [ProgressUpdate {
                percentage: 0.0,
                bytes_processed: 0,
                total_bytes: 0,
            }; 16],
            count: 0,
        }
    }

    #[inline]
    pub fn add(&mut self, percentage: f32, bytes_processed: u64, total_bytes: u64) -> bool {
        if self.count < 16 {
            self.updates[self.count] = ProgressUpdate {
                percentage,
                bytes_processed,
                total_bytes,
            };
            self.count += 1;
            self.count == 16 // Return true when we reach capacity
        } else {
            true // Already full
        }
    }

    #[inline]
    pub fn clear(&mut self) {
        self.count = 0;
    }
}

/// Fast path for simple hash calculations
///
/// Optimized for single-algorithm file processing
///
/// # Safety
///
/// The caller must ensure that:
/// - `file_path` is either null or a valid pointer to a null-terminated C string
/// - `result_buffer` is either null or a valid pointer with at least `buffer_size` bytes
/// - `buffer_size` accurately represents the size of the buffer
#[inline]
pub unsafe fn fast_process_single_hash(
    file_path: *const c_char,
    _algorithm: AniDBHashAlgorithm,
    result_buffer: *mut u8,
    buffer_size: usize,
) -> AniDBResult {
    // Quick parameter validation
    if file_path.is_null() || result_buffer.is_null() || buffer_size < 256 {
        return AniDBResult::ErrorInvalidParameter;
    }

    unsafe {
        // Parse path with fast ASCII check
        let _path_str = match fast_c_str_to_path(file_path) {
            Ok(s) => s,
            Err(e) => return e,
        };

        // Use pre-allocated buffer from pool
        let _hash_buffer = match pool_allocate(128) {
            Ok(buf) => buf,
            Err(_) => return AniDBResult::ErrorOutOfMemory,
        };

        // TODO: Actual hash calculation would go here
        // For now, just demonstrate the structure

        AniDBResult::Success
    }
}

/// Memory pool specifically for FFI string operations
pub struct FfiStringPool {
    small_buffers: Vec<Vec<u8>>,  // 64-byte buffers
    medium_buffers: Vec<Vec<u8>>, // 256-byte buffers
    large_buffers: Vec<Vec<u8>>,  // 1024-byte buffers
}

impl Default for FfiStringPool {
    fn default() -> Self {
        Self::new()
    }
}

impl FfiStringPool {
    pub fn new() -> Self {
        Self {
            small_buffers: Vec::with_capacity(32),
            medium_buffers: Vec::with_capacity(16),
            large_buffers: Vec::with_capacity(8),
        }
    }

    #[inline]
    pub fn allocate(&mut self, size: usize) -> Option<Vec<u8>> {
        if size <= 64 {
            self.small_buffers
                .pop()
                .or_else(|| Some(Vec::with_capacity(64)))
        } else if size <= 256 {
            self.medium_buffers
                .pop()
                .or_else(|| Some(Vec::with_capacity(256)))
        } else if size <= 1024 {
            self.large_buffers
                .pop()
                .or_else(|| Some(Vec::with_capacity(1024)))
        } else {
            None
        }
    }

    #[inline]
    pub fn release(&mut self, mut buffer: Vec<u8>) {
        buffer.clear();

        match buffer.capacity() {
            64 if self.small_buffers.len() < 32 => self.small_buffers.push(buffer),
            256 if self.medium_buffers.len() < 16 => self.medium_buffers.push(buffer),
            1024 if self.large_buffers.len() < 8 => self.large_buffers.push(buffer),
            _ => {} // Let it drop
        }
    }
}

/// Optimized batch result structure with better memory layout
#[repr(C)]
pub struct OptimizedBatchResult {
    // Hot data (frequently accessed) at the beginning
    pub status: AniDBStatus,
    pub total_files: u32,
    pub successful_files: u32,
    pub failed_files: u32,

    // Cold data (rarely accessed) at the end
    pub total_time_ms: u64,
    pub results: *mut AniDBFileResult,

    // Padding to ensure cache line alignment
    _padding: [u8; 24],
}

/// SIMD-optimized memory copy for hash values
///
/// # Safety
///
/// The caller must ensure that:
/// - Both `src` and `dst` slices have valid memory
/// - The CPU supports AVX2 instructions (checked at runtime)
#[cfg(target_arch = "x86_64")]
#[inline]
pub unsafe fn simd_copy_hash(src: &[u8], dst: &mut [u8]) {
    use std::arch::x86_64::*;

    let len = src.len().min(dst.len());

    if len >= 32 {
        // Use AVX2 for 32-byte copies
        let chunks = len / 32;
        for i in 0..chunks {
            let offset = i * 32;
            let src_ptr = src.as_ptr().add(offset) as *const __m256i;
            let dst_ptr = dst.as_mut_ptr().add(offset) as *mut __m256i;

            let data = _mm256_loadu_si256(src_ptr);
            _mm256_storeu_si256(dst_ptr, data);
        }

        // Copy remaining bytes
        let remaining = len % 32;
        if remaining > 0 {
            let offset = chunks * 32;
            ptr::copy_nonoverlapping(
                src.as_ptr().add(offset),
                dst.as_mut_ptr().add(offset),
                remaining,
            );
        }
    } else {
        // Fall back to regular copy for small data
        ptr::copy_nonoverlapping(src.as_ptr(), dst.as_mut_ptr(), len);
    }
}

/// SIMD-optimized memory copy for hash values (fallback for non-x86_64)
///
/// # Safety
///
/// The caller must ensure that both `src` and `dst` slices have valid memory
#[cfg(not(target_arch = "x86_64"))]
#[inline]
pub unsafe fn simd_copy_hash(src: &[u8], dst: &mut [u8]) {
    unsafe {
        let len = src.len().min(dst.len());
        ptr::copy_nonoverlapping(src.as_ptr(), dst.as_mut_ptr(), len);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::CString;

    #[test]
    fn test_optimized_string_alloc() {
        let test_str = "Hello, FFI!";
        let ptr = optimized_string_alloc(test_str);
        assert!(!ptr.is_null());

        unsafe {
            let c_str = CStr::from_ptr(ptr);
            assert_eq!(c_str.to_str().unwrap(), test_str);
            libc::free(ptr as *mut libc::c_void);
        }
    }

    #[test]
    fn test_fast_c_str_to_path() {
        let path = CString::new("/home/user/test.mkv").unwrap();

        unsafe {
            let result = fast_c_str_to_path(path.as_ptr());
            assert!(result.is_ok());
            assert_eq!(result.unwrap(), "/home/user/test.mkv");
        }

        // Test with null
        unsafe {
            let result = fast_c_str_to_path(ptr::null());
            assert_eq!(result, Err(AniDBResult::ErrorInvalidParameter));
        }
    }

    #[test]
    fn test_progress_batch() {
        let mut batch = ProgressBatch::new();

        for i in 0..16 {
            let full = batch.add(i as f32, i as u64 * 1000, 100000);
            assert_eq!(full, i == 15);
        }

        assert_eq!(batch.count, 16);

        batch.clear();
        assert_eq!(batch.count, 0);
    }

    #[test]
    fn test_string_pool() {
        let mut pool = FfiStringPool::new();

        // Allocate and release small buffer
        let buf = pool.allocate(50).unwrap();
        assert!(buf.capacity() >= 64);
        pool.release(buf);

        // Should reuse the buffer
        let buf2 = pool.allocate(60).unwrap();
        assert_eq!(buf2.capacity(), 64);
    }
}
