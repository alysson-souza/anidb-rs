//! Test utilities for the AniDB client
//!
//! This crate provides mock implementations, test builders, and fixtures
//! for testing AniDB client functionality.

pub mod builders;
pub mod mocks;
pub mod performance;

// Re-export commonly used types
pub use builders::{TestDataBuilder, TestFileBuilder};
pub use mocks::{MockAniDBClient, MockFileSystem};
pub use performance::{CoverageReporter, PerformanceTracker, TestHarness};
