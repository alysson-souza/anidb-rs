//! Memory management and deallocation for FFI
//!
//! This module handles all memory-related FFI functions including
//! deallocation, memory statistics, and garbage collection.

use crate::ffi::helpers::validate_mut_ptr;
use crate::ffi::types::{AniDBBatchResult, AniDBFileResult, AniDBHashResult, AniDBResult};
use crate::ffi_catch_panic;
use crate::ffi_memory::{
    MemoryPressure, check_memory_pressure, ffi_free_string, ffi_release_buffer, get_memory_stats,
};
use std::ffi::c_char;
use std::panic::{AssertUnwindSafe, catch_unwind};

/// Free a string allocated by the library
#[unsafe(no_mangle)]
pub extern "C" fn anidb_free_string(str: *mut c_char) {
    let _ = catch_unwind(AssertUnwindSafe(|| {
        if validate_mut_ptr(str) {
            unsafe {
                ffi_free_string(str);
            }
        }
    }));
}

/// Free a file result structure
#[unsafe(no_mangle)]
pub extern "C" fn anidb_free_file_result(result: *mut AniDBFileResult) {
    let _ = catch_unwind(AssertUnwindSafe(|| {
        if !validate_mut_ptr(result) {
            return;
        }

        unsafe {
            let result_box = Box::from_raw(result);

            // Free file path
            if validate_mut_ptr(result_box.file_path) {
                ffi_free_string(result_box.file_path);
            }

            // Free error message
            if validate_mut_ptr(result_box.error_message) {
                ffi_free_string(result_box.error_message);
            }

            // Free hashes with bounds checking
            if validate_mut_ptr(result_box.hashes) && result_box.hash_count > 0 {
                // Limit hash count to prevent excessive iteration
                let hash_count = result_box.hash_count.min(100);
                let hash_slice = std::slice::from_raw_parts_mut(result_box.hashes, hash_count);

                for hash in hash_slice {
                    if validate_mut_ptr(hash.hash_value) {
                        ffi_free_string(hash.hash_value);
                    }
                }

                // Reconstruct the buffer to properly release it
                let hash_results_size =
                    result_box.hash_count * std::mem::size_of::<AniDBHashResult>();
                let buffer = Vec::from_raw_parts(
                    result_box.hashes as *mut u8,
                    hash_results_size,
                    hash_results_size,
                );
                ffi_release_buffer(buffer);
            }

            // Box automatically deallocates
        }
    }));
}

/// Free a batch result structure
#[unsafe(no_mangle)]
pub extern "C" fn anidb_free_batch_result(result: *mut AniDBBatchResult) {
    let _ = catch_unwind(AssertUnwindSafe(|| {
        if !validate_mut_ptr(result) {
            return;
        }

        unsafe {
            let result_box = Box::from_raw(result);

            // Free individual results with bounds checking
            if validate_mut_ptr(result_box.results) && result_box.total_files > 0 {
                // Limit file count to prevent excessive iteration
                let file_count = result_box.total_files.min(10000);
                let results_slice = std::slice::from_raw_parts_mut(result_box.results, file_count);

                for file_result in results_slice {
                    // Manually free each file result's contents
                    if validate_mut_ptr(file_result.file_path) {
                        ffi_free_string(file_result.file_path);
                    }

                    if validate_mut_ptr(file_result.error_message) {
                        ffi_free_string(file_result.error_message);
                    }

                    if validate_mut_ptr(file_result.hashes) && file_result.hash_count > 0 {
                        let hash_count = file_result.hash_count.min(100);
                        let hash_slice =
                            std::slice::from_raw_parts_mut(file_result.hashes, hash_count);

                        for hash in hash_slice {
                            if validate_mut_ptr(hash.hash_value) {
                                ffi_free_string(hash.hash_value);
                            }
                        }

                        // Reconstruct the buffer to properly release it
                        let hash_results_size =
                            file_result.hash_count * std::mem::size_of::<AniDBHashResult>();
                        let buffer = Vec::from_raw_parts(
                            file_result.hashes as *mut u8,
                            hash_results_size,
                            hash_results_size,
                        );
                        ffi_release_buffer(buffer);
                    }
                }

                // Reconstruct the results buffer to properly release it
                let results_size = result_box.total_files * std::mem::size_of::<AniDBFileResult>();
                let buffer =
                    Vec::from_raw_parts(result_box.results as *mut u8, results_size, results_size);
                ffi_release_buffer(buffer);
            }

            // Box automatically deallocates
        }
    }));
}

/// Memory statistics structure
#[repr(C)]
pub struct AniDBMemoryStats {
    pub total_memory_used: u64,
    pub ffi_allocated: u64,
    pub ffi_peak: u64,
    pub pool_memory: u64,
    pub pool_hits: u64,
    pub pool_misses: u64,
    pub active_allocations: u64,
    pub memory_limit: u64,
    pub memory_pressure: u32, // 0=Low, 1=Medium, 2=High, 3=Critical
}

/// Get current memory statistics
#[unsafe(no_mangle)]
pub extern "C" fn anidb_get_memory_stats(stats: *mut AniDBMemoryStats) -> AniDBResult {
    ffi_catch_panic!({
        if !validate_mut_ptr(stats) {
            return AniDBResult::ErrorInvalidParameter;
        }

        let mem_stats = get_memory_stats();
        let pressure = match check_memory_pressure() {
            MemoryPressure::Low => 0,
            MemoryPressure::Medium => 1,
            MemoryPressure::High => 2,
            MemoryPressure::Critical => 3,
        };

        unsafe {
            (*stats).total_memory_used = mem_stats.total_memory_used as u64;
            (*stats).ffi_allocated = mem_stats.ffi_allocated as u64;
            (*stats).ffi_peak = mem_stats.ffi_peak as u64;
            (*stats).pool_memory = mem_stats.pool_memory as u64;
            (*stats).pool_hits = mem_stats.pool_hits as u64;
            (*stats).pool_misses = mem_stats.pool_misses as u64;
            (*stats).active_allocations = mem_stats.active_allocations as u64;
            (*stats).memory_limit = mem_stats.memory_limit as u64;
            (*stats).memory_pressure = pressure;
        }

        AniDBResult::Success
    })
}

/// Force garbage collection of unused buffers
#[unsafe(no_mangle)]
pub extern "C" fn anidb_memory_gc() -> AniDBResult {
    ffi_catch_panic!({
        use crate::memory::shrink_pools;

        // Shrink pool to 50% of current usage
        let stats = get_memory_stats();
        shrink_pools(stats.pool_memory / 2);

        AniDBResult::Success
    })
}

/// Check for memory leaks (debug builds only)
#[unsafe(no_mangle)]
pub extern "C" fn anidb_check_memory_leaks(
    leak_count: *mut u64,
    total_leaked_bytes: *mut u64,
) -> AniDBResult {
    ffi_catch_panic!({
        if !validate_mut_ptr(leak_count) || !validate_mut_ptr(total_leaked_bytes) {
            return AniDBResult::ErrorInvalidParameter;
        }

        #[cfg(debug_assertions)]
        {
            use crate::ffi_memory::ALLOCATION_TRACKER;
            let leaks = ALLOCATION_TRACKER.check_leaks();
            let count = leaks.len() as u64;
            let total_bytes: u64 = leaks.iter().map(|l| l.size as u64).sum();

            unsafe {
                *leak_count = count;
                *total_leaked_bytes = total_bytes;
            }

            // Log leaks in debug mode
            for leak in leaks {
                eprintln!(
                    "Memory leak detected: {:?} ({} bytes) at 0x{:x}",
                    leak.alloc_type, leak.size, leak.address
                );
            }
        }

        #[cfg(not(debug_assertions))]
        unsafe {
            *leak_count = 0;
            *total_leaked_bytes = 0;
        }

        AniDBResult::Success
    })
}
