# Task runner for Clabby. Install with `cargo install just`, then `just <recipe>`.
# Running `just` with no args runs the full CI gate.
set windows-shell := ["powershell.exe", "-NoLogo", "-Command"]

# The full local CI gate (same as .github/workflows/ci.yml).
default: ci
ci: fmt-check lint test

# Auto-format the whole tree.
fmt:
    cargo fmt --all

# Fail if anything is unformatted (CI uses this).
fmt-check:
    cargo fmt --all -- --check

# Clippy with warnings as errors.
lint:
    cargo clippy --workspace --all-targets -- -D warnings

# Full test suite.
test:
    cargo test --workspace
