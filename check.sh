#!/usr/bin/env bash
# Run all CI checks locally before submitting

set -e

echo "Running cargo fmt --check..."
cargo fmt --check

echo "Running cargo clippy..."
cargo clippy -- -D warnings

echo "Running cargo test..."
cargo test

echo "All checks passed!"
