# FFI Architecture

## Structure

The FFI code is split into two parts:

### 1. Main FFI Interface (`src/ffi/`)

- **`mod.rs`** - Public C function exports, library init/cleanup
- **`types.rs`** - C structs and enums
- **`handles.rs`** - Handle registry for clients/operations/batches
- **`memory.rs`** - Free functions (`anidb_free_*`), memory stats
- **`callbacks.rs`** - Callback registration/invocation
- **`events.rs`** - Event queue and thread management
- **`operations.rs`** - File processing, hashing, cache, identification
- **`results.rs`** - Error code conversion, error strings
- **`helpers.rs`** - `ffi_catch_panic!` macro, validation, string conversion

### 2. FFI Support Modules (`src/ffi_*.rs`)

- **`ffi_memory.rs`** - Allocation tracking, `ALLOCATION_TRACKER`, memory pressure
- **`ffi_buffer_pool.rs`** - Size-classed buffer pools (64B-1MB), reuse
- **`ffi_optimization.rs`** - ASCII paths, zero-copy, SIMD operations
- **`ffi_inline.rs`** - Inline directives, constant strings, branch hints

## Dependencies

```
anidb.h (C header)
    ↓
ffi/mod.rs
    ↓
ffi/ modules ──→ ffi_memory.rs
             ──→ ffi_buffer_pool.rs  
             ──→ ffi_optimization.rs
             ──→ ffi_inline.rs
```

- Main FFI modules use support modules for memory/performance
- No circular dependencies
- `ffi_catch_panic!` macro exported from helpers.rs

## Files

```
src/
├── ffi/
│   ├── mod.rs
│   ├── types.rs
│   ├── handles.rs
│   ├── memory.rs
│   ├── callbacks.rs
│   ├── events.rs
│   ├── operations.rs
│   ├── results.rs
│   └── helpers.rs
├── ffi_memory.rs
├── ffi_buffer_pool.rs
├── ffi_optimization.rs
└── ffi_inline.rs
```

## Tests

- `tests/ffi_tests.rs`
- `tests/ffi_memory_tests.rs`
- `tests/ffi_safety_tests.rs`
- `benches/ffi_performance.rs`