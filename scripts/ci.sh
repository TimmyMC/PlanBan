#!/usr/bin/env bash
# Local CI — the exact gate that .github/workflows/ci.yml enforces.
# Run before pushing:  ./scripts/ci.sh
set -euo pipefail

echo "== fmt =="
cargo fmt --all -- --check

echo "== clippy (warnings are errors) =="
cargo clippy --workspace --all-targets -- -D warnings

echo "== test =="
cargo test --workspace

echo "CI OK"
