//! Inline optimizations for FFI hot paths
//!
//! This module provides inline hints and optimizations for frequently
//! called FFI functions to reduce overhead.

use crate::ffi::*;
use std::hint::black_box;

/// Force inline for version string access
#[inline(always)]
pub fn get_version_inline() -> *const std::os::raw::c_char {
    const VERSION_STRING: &str = "0.1.0-alpha\0";
    VERSION_STRING.as_ptr() as *const std::os::raw::c_char
}

/// Force inline for ABI version
#[inline(always)]
pub const fn get_abi_version_inline() -> u32 {
    1
}

/// Optimized error string lookup with compile-time strings
#[inline(always)]
pub fn error_string_inline(error: AniDBResult) -> *const std::os::raw::c_char {
    // Use a match with compile-time constant strings
    const ERROR_STRINGS: &[&str] = &[
        "Success\0",
        "Invalid handle\0",
        "Invalid parameter\0",
        "File not found\0",
        "Processing error\0",
        "Out of memory\0",
        "I/O error\0",
        "Network error\0",
        "Operation cancelled\0",
        "Invalid UTF-8\0",
        "Version mismatch\0",
        "Operation timeout\0",
        "Permission denied\0",
        "Resource busy\0",
        "Unknown error\0",
    ];

    let index = match error {
        AniDBResult::Success => 0,
        AniDBResult::ErrorInvalidHandle => 1,
        AniDBResult::ErrorInvalidParameter => 2,
        AniDBResult::ErrorFileNotFound => 3,
        AniDBResult::ErrorProcessing => 4,
        AniDBResult::ErrorOutOfMemory => 5,
        AniDBResult::ErrorIo => 6,
        AniDBResult::ErrorNetwork => 7,
        AniDBResult::ErrorCancelled => 8,
        AniDBResult::ErrorInvalidUtf8 => 9,
        AniDBResult::ErrorVersionMismatch => 10,
        AniDBResult::ErrorTimeout => 11,
        AniDBResult::ErrorPermissionDenied => 12,
        AniDBResult::ErrorBusy => 13,
        AniDBResult::ErrorUnknown => 14,
    };

    ERROR_STRINGS[index].as_ptr() as *const std::os::raw::c_char
}

/// Optimized hash algorithm name lookup
#[inline(always)]
pub fn hash_algorithm_name_inline(algorithm: AniDBHashAlgorithm) -> *const std::os::raw::c_char {
    const NAMES: &[&str] = &["ED2K\0", "CRC32\0", "MD5\0", "SHA1\0", "TTH\0"];

    let index = match algorithm {
        AniDBHashAlgorithm::ED2K => 0,
        AniDBHashAlgorithm::CRC32 => 1,
        AniDBHashAlgorithm::MD5 => 2,
        AniDBHashAlgorithm::SHA1 => 3,
        AniDBHashAlgorithm::TTH => 4,
    };

    NAMES[index].as_ptr() as *const std::os::raw::c_char
}

/// Optimized validation functions
#[inline(always)]
pub fn validate_handle_inline(handle: *mut std::ffi::c_void) -> bool {
    !handle.is_null() && (handle as usize) != 0 && (handle as usize) < usize::MAX / 2
}

#[inline(always)]
pub const fn validate_ptr_inline<T>(ptr: *const T) -> bool {
    !ptr.is_null()
}

#[inline(always)]
pub const fn validate_mut_ptr_inline<T>(ptr: *mut T) -> bool {
    !ptr.is_null()
}

/// Fast parameter validation for process_file
#[inline(always)]
pub fn validate_process_params_inline(
    handle: *mut std::ffi::c_void,
    file_path: *const std::os::raw::c_char,
    options: *const AniDBProcessOptions,
    result: *mut *mut AniDBFileResult,
) -> Result<(), AniDBResult> {
    if !validate_handle_inline(handle) {
        return Err(AniDBResult::ErrorInvalidHandle);
    }

    if file_path.is_null() || options.is_null() || result.is_null() {
        return Err(AniDBResult::ErrorInvalidParameter);
    }

    Ok(())
}

/// Memory barrier optimization for callback invocations
#[inline(always)]
pub fn memory_fence_callback() {
    std::sync::atomic::fence(std::sync::atomic::Ordering::AcqRel);
}

/// Optimized handle conversion
#[inline(always)]
pub fn handle_to_id(handle: *mut std::ffi::c_void) -> usize {
    handle as usize
}

#[inline(always)]
pub fn id_to_handle(id: usize) -> *mut std::ffi::c_void {
    id as *mut std::ffi::c_void
}

/// Branch prediction hints for common paths
#[inline(always)]
pub fn likely(b: bool) -> bool {
    if b { black_box(true) } else { black_box(false) }
}

#[inline(always)]
pub fn unlikely(b: bool) -> bool {
    if b { black_box(true) } else { black_box(false) }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_inline_functions() {
        // Test version
        let version = get_version_inline();
        assert!(!version.is_null());

        // Test ABI version
        assert_eq!(get_abi_version_inline(), 1);

        // Test error strings
        let error_str = error_string_inline(AniDBResult::Success);
        assert!(!error_str.is_null());

        // Test hash algorithm names
        let name = hash_algorithm_name_inline(AniDBHashAlgorithm::ED2K);
        assert!(!name.is_null());
    }

    #[test]
    fn test_validation_functions() {
        assert!(!validate_handle_inline(std::ptr::null_mut()));
        assert!(!validate_handle_inline(std::ptr::null_mut::<
            std::ffi::c_void,
        >()));
        assert!(validate_handle_inline(
            std::ptr::NonNull::dangling().as_ptr()
        ));

        assert!(!validate_ptr_inline::<u8>(std::ptr::null()));
        assert!(validate_ptr_inline(&42u8));
    }
}
