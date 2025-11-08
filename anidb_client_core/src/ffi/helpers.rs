//! Utility functions and conversions for FFI
//!
//! This module provides helper functions for FFI operations including
//! panic catching, validation, string conversion, and callback invocation.

use crate::ffi::handles::{CallbackRegistration, NEXT_HANDLE_ID};
use crate::ffi::types::{AniDBCallbackType, AniDBHashAlgorithm, AniDBResult};
use crate::ffi_memory::ffi_allocate_string;
use crate::{Error, HashAlgorithm};
use std::collections::HashMap;
use std::ffi::{CStr, c_char};
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::atomic::Ordering;
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

/// Macro to wrap FFI functions with panic catching
#[macro_export]
macro_rules! ffi_catch_panic {
    ($($body:tt)*) => {{
        match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| { $($body)* })) {
            Ok(result) => result,
            Err(_) => $crate::ffi::types::AniDBResult::ErrorUnknown,
        }
    }};
}

/// Validate a C string pointer
#[inline]
pub(crate) fn validate_c_str(ptr: *const c_char) -> bool {
    !ptr.is_null()
}

/// Validate a pointer is not null
#[inline]
pub(crate) fn validate_ptr<T>(ptr: *const T) -> bool {
    !ptr.is_null()
}

/// Validate a mutable pointer is not null
#[inline]
pub(crate) fn validate_mut_ptr<T>(ptr: *mut T) -> bool {
    !ptr.is_null()
}

/// Validate buffer parameters
#[inline]
pub(crate) fn validate_buffer(buffer: *mut c_char, size: usize) -> bool {
    !buffer.is_null() && size > 0
}

/// Convert Rust error to FFI result code
pub(crate) fn error_to_result(error: &Error) -> AniDBResult {
    use crate::error::{InternalError, IoErrorKind, ProtocolError, ValidationError};

    match error {
        Error::Io(io_err) => match io_err.kind {
            IoErrorKind::FileNotFound => AniDBResult::ErrorFileNotFound,
            IoErrorKind::PermissionDenied => AniDBResult::ErrorPermissionDenied,
            _ => AniDBResult::ErrorProcessing,
        },
        Error::Protocol(proto_err) => match proto_err {
            ProtocolError::NetworkOffline => AniDBResult::ErrorNetwork,
            _ => AniDBResult::ErrorNetwork,
        },
        Error::Validation(val_err) => match val_err {
            ValidationError::InvalidConfiguration { .. } => AniDBResult::ErrorInvalidParameter,
            _ => AniDBResult::ErrorInvalidParameter,
        },
        Error::Internal(int_err) => match int_err {
            InternalError::MemoryLimitExceeded { .. } => AniDBResult::ErrorOutOfMemory,
            _ => AniDBResult::ErrorProcessing,
        },
    }
}

/// Convert C string to Rust string with safety checks
pub(crate) fn c_str_to_string(s: *const c_char) -> Result<String, AniDBResult> {
    if !validate_c_str(s) {
        return Err(AniDBResult::ErrorInvalidParameter);
    }

    // Wrap in catch_unwind to handle potential panics from invalid memory
    match catch_unwind(AssertUnwindSafe(|| unsafe {
        CStr::from_ptr(s)
            .to_str()
            .map(|s| s.to_string())
            .map_err(|_| AniDBResult::ErrorInvalidUtf8)
    })) {
        Ok(result) => result,
        Err(_) => Err(AniDBResult::ErrorInvalidParameter),
    }
}

/// Convert Rust string to C string with memory tracking
pub(crate) fn string_to_c_string(s: &str) -> *mut c_char {
    ffi_allocate_string(s)
}

/// Convert FFI hash algorithm to internal type
pub(crate) fn convert_hash_algorithm(
    algo: AniDBHashAlgorithm,
) -> Result<HashAlgorithm, AniDBResult> {
    match algo {
        AniDBHashAlgorithm::ED2K => Ok(HashAlgorithm::ED2K),
        AniDBHashAlgorithm::CRC32 => Ok(HashAlgorithm::CRC32),
        AniDBHashAlgorithm::MD5 => Ok(HashAlgorithm::MD5),
        AniDBHashAlgorithm::SHA1 => Ok(HashAlgorithm::SHA1),
        AniDBHashAlgorithm::TTH => Ok(HashAlgorithm::TTH),
    }
}

/// Convert internal hash algorithm to FFI type
pub(crate) fn convert_hash_algorithm_to_ffi(algo: &HashAlgorithm) -> AniDBHashAlgorithm {
    match algo {
        HashAlgorithm::ED2K => AniDBHashAlgorithm::ED2K,
        HashAlgorithm::CRC32 => AniDBHashAlgorithm::CRC32,
        HashAlgorithm::MD5 => AniDBHashAlgorithm::MD5,
        HashAlgorithm::SHA1 => AniDBHashAlgorithm::SHA1,
        HashAlgorithm::TTH => AniDBHashAlgorithm::TTH,
    }
}

/// Generate a new handle ID
pub(crate) fn generate_handle_id() -> usize {
    NEXT_HANDLE_ID.fetch_add(1, Ordering::SeqCst)
}

/// Get current timestamp in milliseconds since epoch
pub(crate) fn get_timestamp_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

/// Invoke callbacks of a specific type
pub(crate) fn invoke_callbacks(
    callbacks: &Arc<Mutex<HashMap<u64, CallbackRegistration>>>,
    callback_type: AniDBCallbackType,
    invoke_fn: impl Fn(&CallbackRegistration),
) {
    if let Ok(callbacks_map) = callbacks.lock() {
        for registration in callbacks_map.values() {
            if registration.callback_type == callback_type {
                invoke_fn(registration);
            }
        }
    }
}

/// Check if there are any progress callbacks registered
pub(crate) fn has_progress_callbacks(
    callbacks: &Arc<Mutex<HashMap<u64, CallbackRegistration>>>,
) -> bool {
    if let Ok(map) = callbacks.lock() {
        map.values()
            .any(|reg| reg.callback_type == AniDBCallbackType::Progress)
    } else {
        false
    }
}
