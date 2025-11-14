---
applyTo: "anidb_client_core/src/ffi/**"
---
# FFI Change Rules

The following scoped instructions apply only when working in paths matching `anidb_client_core/src/ffi/**`.

## Invariants
- Preserve handle lifecycle; never expose raw pointers directly.
- Keep allocation + deallocation symmetric; every `new_*` requires a documented corresponding `free_*`.
- Avoid panics crossing FFI boundary; convert to error codes / strings.
- Do not introduce global mutable state; keep operations stateless and reentrant.
- Maintain memory safety: validate buffer lengths and null pointers before use.
- Any struct layout change must be reflected in `include/anidb.h` and accompanied by an ABI version bump note.

## Testing Requirements
- Add/adjust tests in `ffi_tests.rs`, `ffi_memory_tests.rs`, and `ffi_safety_tests.rs` for new or changed FFI surfaces.
- Include negative tests (invalid handles, double free attempts, null callbacks).
- Benchmark critical changes in `benches/ffi_performance.rs` only if performance impact is expected.

## Error Handling
- Use explicit error codes or mapped messages; never leak Rust panic text.
- Provide contextual error strings via existing conversion helpers.

## Performance
- Prefer existing buffer pools / trackers; do not allocate large temporary vectors repeatedly.
- Keep per-call allocations O(1) relative to payload size; stream large data.

## Documentation
- Update `docs/ffi_*` guides if public behavior changes.
- Ensure examples remain valid; compile C examples after significant changes.

## Anti-Patterns (Reject)
- Hidden caching or session state in FFI layer.
- Blocking sleeps for rate limiting (use protocol abstractions instead).
- Unsafe pointer arithmetic without bounds checks.

## Review Checklist
1. Layout / ABI compatibility maintained?
2. Symmetric allocation/deallocation present?
3. Null pointer paths covered in tests?
4. No panics escaping FFI boundary?
5. `include/anidb.h` updated if needed?
6. Benchmarks added or explicitly not required?

## Commit Message Addendum (When FFI changes)
Add a body section:
```
* FFI: <summary of handle/type changes>
* ABI: <unchanged|bumped to X>
* Tests: ffi_tests, ffi_safety_tests updated
```

## Prompt Examples
- "extend FFI to expose batch hashing without introducing state"
- "add test for double free of file handle"
- "benchmark ffi memory copy path for large buffers"
