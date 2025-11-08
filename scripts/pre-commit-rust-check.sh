#!/usr/bin/env bash
# Pre-commit hook that only runs Rust checks if .rs files are staged

set -e

# Check if there are any staged .rs files
if ! git diff --cached --name-only --diff-filter=ACM | grep -q '\.rs$'; then
    echo "No Rust files staged, skipping Rust checks"
    exit 0
fi

echo "Running Rust checks on staged files..."

# Run cargo fmt check
echo "→ cargo fmt --all -- --check"
cargo fmt --all -- --check

# Run clippy
echo "→ cargo clippy --workspace --all-targets --all-features -- -D warnings"
cargo clippy --workspace --all-targets --all-features -- -D warnings

# Run tests
echo "→ cargo test --workspace --all-targets --all-features"
cargo test --workspace --all-targets --all-features

echo "✓ All Rust checks passed!"
