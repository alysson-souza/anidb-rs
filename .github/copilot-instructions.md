# Copilot Instructions for AniDB Client Codebase

## Critical Architecture: Stateless Core + Stateful CLI

This Rust workspace follows a strict separation: **`anidb_client_core` is completely stateless**, while **`anidb_cli` owns all state** (caching, config persistence, orchestration).

### Core Library (Stateless)
`anidb_client_core` performs pure computations without maintaining state between calls:
- **No caching** - all caching is CLI responsibility
- **No persistent connections** - each operation is independent
- **No session management** - FFI consumers must implement their own state
- All operations are thread-safe and can be called in any order
- See `anidb_client_core/src/api.rs` for the stateless API surface

### CLI Application (Stateful)
`anidb_cli` handles all stateful operations:
- **Cache ownership**: `anidb_cli/src/cache/` - SQLite/memory/file-based caching
- **Orchestration**: `anidb_cli/src/orchestrators/` - coordinates cache + core calls
- **Pattern**: Check cache → call stateless core → store result
- Example: `identify_orchestrator.rs` demonstrates the correct pattern

```rust
// CORRECT: CLI orchestrates caching around stateless core
let cache = Arc::new(SqliteCache::new(cache_dir)?);
let core_client = Arc::new(AniDBClient::new(config)?);

if let Some(cached) = cache.get(&key).await? {
    return Ok(cached);  // Cache hit
}
let result = core_client.process_file(path, options).await?;  // Stateless call
cache.put(&key, &result).await?;  // CLI stores result
```

- `cache/service.rs` (`HashCacheService`) merges partial cache hits with fresh hashes and records cache stats
- `progress/provider.rs` + `renderer.rs` form the bridge between the core `ProgressProvider` trait and the CLI renderer; always feed progress through them rather than inventing new channels
- `file_discovery/` wraps `walkdir` + glob filtering so commands never load entire directory trees eagerly
- `sync/` owns AniDB MyList queue processing and must respect the built-in 0.5 req/s rate limiter inside `ProtocolClient`
- `output/` contains the canonical text/json/jsonl/csv/template formatters; reuse them for any new command output

## Critical Hash Implementation: ED2K Algorithm

ED2K has special chunking requirements that affect all file processing:
- **Chunk size**: Exactly **9,728,000 bytes** (9.5MB) - see `hashing/algorithms/ed2k.rs`
- **Files ≤9.5MB**: Direct MD4 hash
- **Files >9.5MB**: Split into 9.5MB chunks → hash each → concatenate hashes → hash result
- **Red vs Blue variants**: `Ed2kVariant::Red` (default) vs `Ed2kVariant::Blue` (AniDB v2)
- All parallel processing respects ED2K chunk boundaries

## Memory Architecture: Streaming Everything

Designed to process 100GB+ files with constant memory (<500MB):
- **Never** `fs::read()` entire files - always use streaming processors
- `FileProcessor` uses streaming pipeline (`pipeline/` module)
- `BufferPool` in `memory/` for buffer reuse
- Platform-specific: Linux can use mmap for files <1GB (`platform/build_config.rs`)
- Memory tracking enforced throughout - see `buffer.rs` for limits

## Developer Workflows

**Pre-commit hooks via Prek** (enforces fmt, clippy, tests):
```bash
# Setup once
cargo install prek  # or: brew install prek
prek install

# Manual run
prek run

# Key commands
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace --all-targets
```
- These hooks ultimately invoke `scripts/pre-commit-rust-check.sh`; keep that script aligned with CI whenever you add new checks

**Testing philosophy**: Test-driven development required
- Write failing tests first, then implement
- Unit tests: `tests/*_tests.rs` (one per module)
- Integration tests: `tests/integration_*_tests.rs`
- Mock utilities: `anidb-test-utils` crate provides `MockAniDBClient`, `MockFileSystem`
- Benchmarks: `benches/` for performance regression detection

**Common patterns**:
```bash
cargo test --workspace ed2k        # Test ED2K implementation
cargo test --workspace cache       # Test CLI cache (not in core!)
cargo bench --bench hash_performance  # Benchmark hashing
```

## FFI Architecture

Multi-language bindings in `bindings/` (C#, Node.js, Python, Swift):
- **C API**: `anidb_client_core/include/anidb.h` and `src/ffi/`
- **Stateless FFI**: FFI consumers must implement their own caching
- **Handle-based**: Opaque handles with registry in `ffi/handles.rs`
- **Callback system**: Events, progress, memory in respective `ffi/*.rs` files
- **Documentation**: Extensive docs in `anidb_client_core/docs/ffi_*.md`

## AniDB Protocol Details

UDP-based protocol with strict requirements (`protocol/mod.rs`):
- **Rate limit**: 0.5 requests/second (1 request per 2 seconds) - enforced in `client.rs`
- **Server**: `api.anidb.net:9000` (UDP)
- **Session timeout**: 30 minutes
- **Fragmentation**: Packets limited to 1400 bytes (PPPoE-safe) - see `codec/`

## Platform-Specific Code

All OS-specific logic isolated in `platform/`:
- Windows: Long path support (`\\?\` prefix)
- Linux: mmap optimization for <1GB files
- All platforms: UTF-8 path handling required
- `build_config.rs` selects optimal I/O strategies per platform

## Key Conventions

1. **Error context**: Always use `Error::file_not_found(path)` not `Error::FileNotFound`
2. **Progress reporting**: Required for all file I/O (even if null provider)
3. **Rust edition**: 2024 edition, minimum Rust 1.91
4. **Documentation**: All public APIs must have doc comments
5. **No cache in core**: If adding features, caching goes in CLI, not core
6. **FFI/bindings**: Any API or struct that crosses the FFI boundary must continue to match `anidb_client_core/include/anidb.h` and stay compatible with the bindings under `bindings/`

## Critical Files to Reference

- `AGENTS.md`: Comprehensive development guide with TDD approach
- `anidb_client_core/src/api.rs`: Stateless public API
- `anidb_cli/src/cache/service.rs`: How CLI wraps core with caching
- `anidb_client_core/src/hashing/algorithms/ed2k.rs`: ED2K implementation
- `anidb_client_core/src/file_io.rs`: Streaming pipeline usage
- `anidb_client_core/src/protocol/mod.rs`: AniDB protocol constants
- `anidb_client_core/include/anidb.h`: Canonical C header mirrored by every binding
