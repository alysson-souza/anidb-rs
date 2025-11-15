//! Legacy-compatible FFI test guard.
//!
//! Historically the FFI layer relied on a global registry that required explicit
//! isolation between tests. The production codebase now scopes registries per
//! context, but the tests still construct a `TestContextGuard`. This module
//! provides a lightweight replacement so existing tests continue to compile
//! without pulling additional infrastructure into the production crate.

use std::sync::{Mutex, MutexGuard};

static TEST_GUARD_MUTEX: Mutex<()> = Mutex::new(());

/// RAII guard retained for backwards compatibility with older tests.
///
/// The guard serializes FFI tests by taking a global mutex and triggers a
/// cleanup when dropped to keep state isolated between test cases.
pub struct TestContextGuard {
    _lock: MutexGuard<'static, ()>,
}

impl TestContextGuard {
    /// Create a new guard instance.
    pub fn new() -> Self {
        let lock = TEST_GUARD_MUTEX
            .lock()
            .expect("test context guard mutex poisoned");
        Self { _lock: lock }
    }
}

impl Default for TestContextGuard {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for TestContextGuard {
    fn drop(&mut self) {
        anidb_client_core::ffi::anidb_cleanup(std::ptr::null_mut());
    }
}
