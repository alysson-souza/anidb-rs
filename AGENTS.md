# AGENTS.md

This file provides guidance when working with code in this repository.

## Key Commands

**Dependencies**: Use `cargo add/rm` to manage dependencies (keeps Cargo.lock in sync).

**Pre-commit hooks**: Configured via Prek to enforce formatting, linting, and tests before commits.

To set up locally:
  - Install Prek: `cargo install prek` or `brew install prek` (macOS/Linux) or `pip install prek`
  - Install hooks: `prek install`
  - Run hooks manually if needed: `prek run`
  - Uninstall hooks: `prek uninstall`

### Building

```bash
# Build entire workspace
cargo build

# Build release version
cargo build --release

# Build specific crate
cargo build -p anidb_client_core
```

### Testing

```bash
# Run all tests
cargo test --workspace --all-targets

# Run tests with output
cargo test --workspace --all-targets -- --nocapture

# Run specific test
cargo test --workspace test_ed2k_multichunk_file

# Run tests for specific crate
cargo test -p anidb_client_core

# Run tests matching pattern
cargo test --workspace ed2k

# Run benchmarks
cargo bench --bench hash_performance
cargo bench --bench memory_performance
```

### Code Quality

```bash
# Format code
cargo fmt --all

# Check formatting (used in pre-commit hooks)
cargo fmt --all -- --check

# Run linter
cargo clippy --workspace --all-targets -- -D warnings

# Security audit
cargo audit
```

## Development Approach

This codebase follows test-driven development. When implementing new functionality:

1. Write failing tests that define the expected behavior
2. Implement the minimal code to make tests pass
3. Refactor while keeping tests green

Every module has a corresponding test file in `tests/` with test coverage. Test utilities are provided by the
`anidb-test-utils` crate, enabling testing without external dependencies.

### Communication Style

Use clear, technical language in all code artifacts:

- **Comments**: Explain what and why, not how. Focus on technical accuracy.
- **Commit messages**: Describe specific changes and their purpose
- **Documentation**: Write for developers who need to understand the system
- **Error messages**: Provide actionable information for debugging

Avoid marketing language, superlatives, or subjective descriptions. The code should communicate through clarity and
precision, not persuasion.

## Architecture

This is a Rust workspace with a **stateless** core library (`anidb_client_core`) that handles file processing, hashing, and AniDB
protocol communication. The architecture prioritizes streaming processing to handle very large files (100GB+) with
constant memory usage. All caching and state management is handled at the application layer (`anidb_cli`), not in the core.

### Core Library Modules (Stateless)

The core library (`anidb_client_core`) is completely stateless - it performs pure computations without maintaining any state between calls.

**`api.rs`** - Public API surface

- `AniDBClient`: Main client interface (stateless operations only)
- `ProcessOptions` / `BatchOptions`: Configuration types
- `FileResult` / `AnimeIdentification`: Result types
- Note: `ClientConfig` no longer has a `cache_dir` field

**`hashing.rs`** - Hash calculation engine

- Implements ED2K, CRC32, MD5, SHA1, TTH algorithms
- ED2K uses 9728000-byte chunks (9.5MB)
- For files >9.5MB: chunk hashes are concatenated then hashed again
- Supports parallel calculation of multiple algorithms

**`file_io.rs`** - Streaming file processor

- `StreamingReader`: Processes files with constant memory usage
- Double-buffering for performance
- Progress reporting via async channels
- Never loads entire files into memory

**`memory/`** - Memory management

- `BufferPool`: Reusable buffer allocation
- `MemoryTracker`: Tracks and limits memory usage
- Enforces <500MB memory limit for any file size

**`platform/`** - OS-specific code

- Path normalization (Windows long paths)
- I/O strategy selection
- Platform-specific optimizations (Linux mmap for <1GB files)

**`error.rs`** - Error handling

- Custom error types with context
- Distinguishes transient vs permanent errors
- All public APIs return `Result<T, Error>`

**`identification/`** - Anime identification

- Query management for AniDB lookups
- Types for identification results
- Service interfaces for identification operations

**`ffi/`** - Foreign Function Interface modules

- `mod.rs`: Public FFI exports and library initialization
- `types.rs`: C-compatible type definitions
- `handles.rs`: Handle registry and lifecycle management
- `memory.rs`: Memory management and deallocation
- `callbacks.rs`: Callback registration system
- `events.rs`: Event queue and notification system
- `operations.rs`: Core file processing operations (stateless)
- `results.rs`: Result conversion and error strings
- `helpers.rs`: Common utility functions
- Note: FFI users must implement their own caching if needed

### CLI Application Modules (Stateful)

The CLI application (`anidb_cli`) handles all stateful operations including caching, configuration persistence, and orchestration.

**`cache/`** - Cache implementation (CLI-owned)

- `mod.rs`: Cache module exports and types
- `traits.rs`: Cache trait definitions
- `sqlite_cache.rs`: SQLite-based persistent cache
- `memory_cache.rs`: In-memory cache implementation
- `file_cache.rs`: File-based cache operations
- `service.rs`: Cache service wrapper that adds caching to core operations
- `factory.rs`: Cache instance creation and configuration
- Thread-safe operations with automatic expiration

**`orchestrators/`** - High-level operation coordination

- `identify_orchestrator.rs`: Manages identification with caching
- Demonstrates the correct pattern: check cache → call stateless core → store result

**`progress/`** - Progress infrastructure

- `provider.rs`: Channel-backed implementation of the core `ProgressProvider` trait
- `renderer.rs`: TUI-friendly rendering loop used by CLI commands and orchestrators
- `utils.rs`: Shared helpers such as human-readable byte/throughput formatting

**`file_discovery/`** - File enumeration helpers

- Default include/exclude extension sets (`extensions.rs`)
- Glob filtering and walker utilities that feed both hash and identify commands
- Always stream directory traversal to avoid loading entire directory trees

**`sync/`** - AniDB MyList synchronization

- `service.rs`: Wraps repositories, protocol client, and credential store to process the sync queue
- `mod.rs`: Clap integration plus `SyncCommand` parser working with `SyncOrchestrator`
- Uses AniDB UDP client with enforced 0.5 req/s throttle, so never bypass the rate limiter

**`cache/service.rs`** - Hash cache orchestration

- `HashCacheService` sits between CLI commands and the stateless core
- Handles partial cache hits, merges cached + freshly computed hashes, and records cache stats
- Works with `CacheFactory` to swap between file, memory, noop, or layered cache implementations

**`progress`, `terminal.rs`, and `output/` together define CLI UX**

- Terminal utilities detect CI/non-interactive environments before rendering progress bars
- Output formatters (text/json/jsonl/csv/template) live under `output/` and must be used for batch results

**Pattern for Using Stateless Core with Cache:**

```rust
// CLI owns the cache
let cache = Arc::new(SqliteCache::new(cache_dir)?);
let core_client = Arc::new(AniDBClient::new(config)?);

// CLI checks cache BEFORE calling core
if let Some(cached) = cache.get(&key).await? {
    return Ok(cached);
}

// CLI calls stateless core function
let result = core_client.process_file(path, options).await?;

// CLI stores result in cache
cache.put(&key, &result).await?;
```

### Test Utilities Crate

**`anidb-test-utils`** - Dedicated test infrastructure crate

- `MockAniDBClient`: Mock client implementation
- `MockFileSystem`: In-memory file system
- `TestDataBuilder`: Test scenario creation
- `PerformanceTracker`: Performance measurement
- Located in separate crate to keep production code clean

### Cross-language Bindings and FFI Usage

- C ABI lives in `anidb_client_core/include/anidb.h` and mirrors `src/ffi`, including versioned constants
- Language bindings in `bindings/python`, `bindings/csharp`, `bindings/nodejs`, and `bindings/swift` call the same handle-based API
- FFI calls are stateless; consumers must implement their own caching/progress plumbing just like the CLI
- Extensive docs are under `anidb_client_core/docs/ffi_*.md`; reference them before adjusting any exported symbols

### Tooling Scripts

- `scripts/pre-commit-rust-check.sh` is invoked by Prek to gate commits; keep it aligned with CI expectations
- When adding new checks, update both the script and pre-commit configuration so developers get the same signal locally

## Important Implementation Details

### Stateless Core Architecture

The core library is designed to be completely stateless:

- No caching within the core - all caching is handled by the CLI layer
- No persistent connections or sessions maintained between calls
- Each operation is independent and can be called in any order
- FFI consumers must implement their own caching layer if needed
- This design enables better testability, predictability, and concurrent usage

### ED2K Hash Algorithm

The ED2K implementation requires special handling:

- Files ≤9.5MB: Direct MD4 hash
- Files >9.5MB: Split into 9.5MB chunks, hash each chunk, concatenate hashes, hash the result
- Chunk size is exactly 9728000 bytes

### Memory Streaming

All file operations use streaming to maintain constant memory usage:

```rust
// Never do this:
let data = fs::read(path)?;

// Always use streaming:
let reader = StreamingReader::new(path)?;
```

### Error Context

Always include relevant context in errors:

```rust
Error::file_not_found(path)  // Good - includes path
Error::FileNotFound           // Bad - no context
```

### Platform Differences

- Windows: Use `\\?\` prefix for long paths
- Linux: Can use mmap for files <1GB
- All platforms: UTF-8 path handling required

### AniDB Protocol

When implementing AniDB communication:

- Rate limit: 0.5 requests/second maximum
- UDP port: 9000
- Session timeout: 30 minutes
- Always handle offline scenarios with queuing

## Testing Infrastructure

The test suite is structured to support test-driven development:

### Test Organization

- **Unit tests**: `tests/*_tests.rs` - One test file per module
- **Integration tests**: `tests/integration_*_tests.rs` - End-to-end workflows
- **Benchmarks**: `benches/` - Performance validation
- **Test utilities**: `tests/testing_infrastructure_tests.rs` - Shared test helpers

### Test Patterns

```rust
// Example: Adding a new feature
// 1. Start with a failing test
#[test]
fn test_new_hash_algorithm() {
    let result = HashCalculator::new()
        .calculate_bytes(HashAlgorithm::NewAlgo, b"data")
        .unwrap();
    assert_eq!(result.hash, "expected_hash");
}

// 2. Implement minimal code to pass
// 3. Add edge cases and error scenarios
// 4. Refactor with confidence
```

### Running Tests

```bash
cargo test hashing    # Hash algorithm tests
cargo test file_io    # File processing tests
cargo test platform   # Platform-specific tests
cargo test cache      # Cache tests (in CLI only)

# Run tests continuously during development
cargo watch -x test
```

### Mock Usage

Test utilities are provided by the `anidb-test-utils` crate:

```rust
use anidb_test_utils::mocks::{MockAniDBClient, MockFileSystem};
use anidb_test_utils::builders::TestDataBuilder;
```

- `MockAniDBClient` - Test without network calls
- `MockFileSystem` - Test without real files
- `TestDataBuilder` - Create test scenarios easily

## Performance Considerations

Key performance patterns:

- Use `BufferPool` for buffer reuse
- Enable platform-specific optimizations
- Stream processing for all file operations
- Parallel hash calculation when possible
