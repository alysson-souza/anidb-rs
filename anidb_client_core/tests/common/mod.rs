//! Common test utilities for integration tests
//!
//! This module provides shared test infrastructure used across multiple
//! integration tests in the AniDB client codebase.

use std::sync::Mutex;

pub mod test_harness;

// Re-export commonly used types for convenience
pub use test_harness::FileOperationsTestHarness;

// Global mutex to ensure FFI tests don't interfere with each other
// This is necessary because the FFI layer uses global state (CLIENTS, INITIALIZED, etc.)
lazy_static::lazy_static! {
    pub static ref FFI_TEST_MUTEX: Mutex<()> = Mutex::new(());
}
