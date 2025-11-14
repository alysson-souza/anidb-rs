---
applyTo: "**"
---

# AniDB Client Codebase Instructions

## Response Style & Workflow
- **Tone**: Technical, concise, evidence-based; avoid hype or emojis unless requested
- **Structure**: For multi-step tasks, state plan briefly then execute with minimal diffs
- **References**: Wrap file/symbol names in backticks; use KaTeX for math (e.g., ED2K chunk size $9728000$)
- **Performance claims**: Require benchmark evidence from `cargo bench`
- **Tasks**: Build TODO list, update incrementally, never skip tests
- **Scope**: Keep patches minimal; defer large refactors unless explicitly requested

## Code Review Focus
Verify:
- Architectural compliance: stateless core, streaming, error context, FFI stability
- No caching or persistent state introduced in `anidb_client_core`
- Error context uses helpers (e.g., `Error::file_not_found(path)`)
- Progress reporting present for new file I/O paths

## FFI Changes
Before altering FFI exports:
1. Update `include/anidb.h` to match internal Rust changes
2. Add conversion tests in `ffi_tests.rs` or related test file
3. Validate memory model (no ownership leaks, proper deallocation)
4. Document ABI impact and migration notes in commit body

## Prompt Patterns
Use terse imperative phrases:
- "add ed2k multi-chunk test reaching 3 chunks"
- "refactor buffer pool to reduce lock contention"
- "explain md4 finalization step in ed2k"