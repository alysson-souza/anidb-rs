//! Progress reporting module for the CLI
//!
//! This module provides progress reporting infrastructure for the CLI,
//! including providers and renderers.

pub mod provider;
pub mod renderer;
pub mod utils;

// Re-export main helpers
#[allow(unused_imports)]
pub use provider::create_progress_infrastructure;
#[allow(unused_imports)]
pub use renderer::render_progress;
#[allow(unused_imports)]
pub use utils::{format_bytes, format_duration, format_throughput};
