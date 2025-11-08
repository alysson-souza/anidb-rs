//! Result marshalling and conversion for FFI
//!
//! This module handles error result conversion, error strings,
//! and hash algorithm information functions.

use crate::ffi::types::{AniDBHashAlgorithm, AniDBResult};
use std::ffi::c_char;

/// Get human-readable error description
#[unsafe(no_mangle)]
pub extern "C" fn anidb_error_string(error: AniDBResult) -> *const c_char {
    let msg = match error {
        AniDBResult::Success => "Success\0",
        AniDBResult::ErrorInvalidHandle => "Invalid handle\0",
        AniDBResult::ErrorInvalidParameter => "Invalid parameter\0",
        AniDBResult::ErrorFileNotFound => "File not found\0",
        AniDBResult::ErrorProcessing => "Processing error\0",
        AniDBResult::ErrorOutOfMemory => "Out of memory\0",
        AniDBResult::ErrorIo => "I/O error\0",
        AniDBResult::ErrorNetwork => "Network error\0",
        AniDBResult::ErrorCancelled => "Operation cancelled\0",
        AniDBResult::ErrorInvalidUtf8 => "Invalid UTF-8\0",
        AniDBResult::ErrorVersionMismatch => "Version mismatch\0",
        AniDBResult::ErrorTimeout => "Operation timeout\0",
        AniDBResult::ErrorPermissionDenied => "Permission denied\0",
        AniDBResult::ErrorBusy => "Resource busy\0",
        AniDBResult::ErrorUnknown => "Unknown error\0",
    };
    msg.as_ptr() as *const c_char
}

/// Get hash algorithm name
#[unsafe(no_mangle)]
pub extern "C" fn anidb_hash_algorithm_name(algorithm: AniDBHashAlgorithm) -> *const c_char {
    let name = match algorithm {
        AniDBHashAlgorithm::ED2K => "ED2K\0",
        AniDBHashAlgorithm::CRC32 => "CRC32\0",
        AniDBHashAlgorithm::MD5 => "MD5\0",
        AniDBHashAlgorithm::SHA1 => "SHA1\0",
        AniDBHashAlgorithm::TTH => "TTH\0",
    };
    name.as_ptr() as *const c_char
}

/// Get required hash buffer size
#[unsafe(no_mangle)]
pub extern "C" fn anidb_hash_buffer_size(algorithm: AniDBHashAlgorithm) -> usize {
    match algorithm {
        AniDBHashAlgorithm::ED2K => 33, // 32 hex chars + null
        AniDBHashAlgorithm::CRC32 => 9, // 8 hex chars + null
        AniDBHashAlgorithm::MD5 => 33,  // 32 hex chars + null
        AniDBHashAlgorithm::SHA1 => 41, // 40 hex chars + null
        AniDBHashAlgorithm::TTH => 40,  // 39 base32 chars + null
    }
}
