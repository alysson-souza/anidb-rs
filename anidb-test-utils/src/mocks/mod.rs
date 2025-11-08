//! Mock implementations for testing

mod client;
mod filesystem;

pub use client::{MockAniDBClient, MockHashCalculator};
pub use filesystem::MockFileSystem;
