//! FFI (Foreign Function Interface) for the AniDB Client Core Library
//!
//! This module provides C-compatible bindings for external language consumption.
//! It implements the API defined in include/anidb.h using an opaque handle pattern
//! for safety and versioned API for compatibility.
//!
//! # Safety
//!
//! All FFI functions implement comprehensive safety checks:
//! - Null pointer validation for all pointer parameters
//! - Buffer overflow prevention with size validation
//! - Panic catching at FFI boundary using catch_unwind
//! - Proper memory cleanup on all error paths
//! - Thread safety through interior mutability patterns
//!
//! No Rust panics can cross the FFI boundary - all are caught and
//! converted to appropriate error codes.

#![allow(clippy::not_unsafe_ptr_arg_deref)]

// Module declarations
pub mod callbacks;
pub mod events;
pub mod handles;
pub mod helpers;
pub mod memory;
pub mod operations;
pub mod results;
pub mod types;

// Re-export all public FFI functions and types
pub use callbacks::*;
pub use events::*;
pub use handles::*;
pub use memory::*;
pub use operations::*;
pub use results::*;
pub use types::*;

// Import helpers

// Re-export the macro at module level for internal use
pub(crate) use crate::ffi_catch_panic;

use std::ffi::c_char;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::atomic::Ordering;

// Version constants matching the header
#[allow(dead_code)]
const VERSION_MAJOR: u32 = 0;
#[allow(dead_code)]
const VERSION_MINOR: u32 = 1;
#[allow(dead_code)]
const VERSION_PATCH: u32 = 0;
const VERSION_STRING: &str = "0.1.0-alpha\0";
const ABI_VERSION: u32 = 1;

/* ========================================================================== */
/*                          Library Initialization                             */
/* ========================================================================== */

/// Initialize the AniDB client library
#[unsafe(no_mangle)]
pub extern "C" fn anidb_init(abi_version: u32) -> AniDBResult {
    ffi_catch_panic!({
        if abi_version != ABI_VERSION {
            return AniDBResult::ErrorVersionMismatch;
        }

        // Always reset memory state on init to ensure clean state between tests
        // This is critical for test isolation when tests run in the same process
        crate::buffer::reset_memory_state_for_tests();

        // Initialize only once
        if handles::INITIALIZED
            .compare_exchange(0, 1, Ordering::SeqCst, Ordering::SeqCst)
            .is_ok()
        {
            // Perform any one-time initialization here
            AniDBResult::Success
        } else {
            // Already initialized
            AniDBResult::Success
        }
    })
}

/// Cleanup the AniDB client library
#[unsafe(no_mangle)]
pub extern "C" fn anidb_cleanup() {
    let _ = catch_unwind(AssertUnwindSafe(|| {
        if handles::INITIALIZED
            .compare_exchange(1, 0, Ordering::SeqCst, Ordering::SeqCst)
            .is_ok()
        {
            // Clear all handles with proper error handling
            if let Ok(mut clients) = handles::CLIENTS.write() {
                clients.clear();
            }
            if let Ok(mut operations) = handles::OPERATIONS.write() {
                operations.clear();
            }
            if let Ok(mut batches) = handles::BATCHES.write() {
                batches.clear();
            }

            // Reset memory state on cleanup to ensure clean state
            crate::buffer::reset_memory_state_for_tests();
        }
    }));
}

/// Get library version string
#[unsafe(no_mangle)]
pub extern "C" fn anidb_get_version() -> *const c_char {
    VERSION_STRING.as_ptr() as *const c_char
}

/// Get library ABI version
#[unsafe(no_mangle)]
pub extern "C" fn anidb_get_abi_version() -> u32 {
    ABI_VERSION
}
