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

# Run the Tauri desktop app (Vite dev server + hot reload) against a project
# directory — defaults to the offline Jira example. Usage: `just desktop` or
# `just desktop docs/examples/github`.
desktop dir="docs/examples/jira":
    $env:CLABBY_PROJECT = (Resolve-Path "{{dir}}").Path; & ./ui/node_modules/.bin/tauri.CMD dev
