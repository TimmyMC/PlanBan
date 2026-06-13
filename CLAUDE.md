# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## What this is

`clabby/` is **Clabby** — a local, harness-agnostic command center for agent-assisted
software work, synced to an issue tracker, with a deterministic workflow engine. It is
explicitly **not** an autonomous agent runner: it enforces the workflow; the human
decides. Read [`CONSTITUTION.md`](CONSTITUTION.md) first — its articles
govern every change. If a change violates an article, the change is wrong.

## Commands (run from the repo root)

```sh
cargo build                       # build the workspace
cargo test                        # all tests (core unit + e2e + CLI black-box + trycmd docs)
cargo test -p clabby-core         # core only — the fast inner loop (§8)
cargo test -p clabby-core full_pipeline   # the engine end-to-end integration test
cargo test -p clabby              # black-box CLI tests (require node + git on PATH)
cargo build --bin clabby          # build just the CLI

# Regenerate the trycmd living-doc snapshots after an intentional CLI output change:
$env:TRYCMD="overwrite"; cargo test -p clabby --test cli_docs   # PowerShell

# Run the full CI gate locally before pushing (fmt + clippy -D warnings + test):
./scripts/ci.ps1                  # or ./scripts/ci.sh, or `just`

# One-time per clone: enable the auto-format pre-commit hook (cargo fmt + Biome):
./scripts/install-hooks.ps1       # or ./scripts/install-hooks.sh
```

The pre-commit hook (`scripts/hooks/pre-commit`, enabled via `core.hooksPath`) runs
`cargo fmt` on staged Rust files and Biome `check --write` on staged frontend files,
re-staging anything it reformats — so the CI fmt/lint gates never fail on formatting alone.

**Trunk-based development.** `trunk` is the protected default branch — don't push to it
directly. Branch off, open a PR, and it auto-merges once CI is green (fmt/clippy/test,
≥75% line coverage via `cargo llvm-cov`, frontend build/lint/e2e). See
[`docs/trunk-based-development.md`](docs/trunk-based-development.md).

Lints: crate roots set `#![warn(clippy::all)]` with the doc/style nags allowed; CI runs
`clippy -D warnings`. Don't enable `clippy::pedantic` broadly — it's high-noise; the
allows in `lib.rs`/`main.rs` keep it opt-in if individual pedantic lints are wanted later.
Deferred work (incl. moving sqlx to compile-time `query!`) lives in `BACKLOG.md`.

The Rust toolchain is MSVC (`stable-x86_64-pc-windows-msvc`); the build compiles a
vendored SQLite via `sqlx`, so a C toolchain (VS Build Tools) is required. Node.js is
needed only to run the offline example tracker (`examples/tracker.mjs`).

Run the CLI against an example without installing:
`./target/debug/clabby --config examples/jira/clabby.toml status` (or `cd` into the
example dir; config is discovered from cwd upward).

## Architecture

A Cargo workspace with a strict separation that mirrors Constitution §7 (core is UI-free)
and §11 (generic core, workflow in config):

- **`crates/core` (`clabby-core`)** — the engine. No UI/Tauri dependency. Vocabulary is
  domain-neutral (`Issue`, `status`, `Session`, `Worktree`, `agent`) — nothing names
  Jira/Azure/Claude. Key modules:
  - `config.rs` — parses `clabby.toml`; the *entire workflow* lives here, not in code.
  - `sync.rs` — tracker pull/push + the **divergence rule** (`reconcile`): the tracker
    always wins on value; a change is flagged `diverged` only when it wasn't our own push.
  - `runner.rs` — the one place we shell out. Every external action is a rendered command
    template run via the platform shell. **Windows note:** commands go through
    `cmd /C "<line>"` with the whole line wrapped in one quote pair (see `shell()`); this
    is load-bearing for preserving quoted args with spaces — there's a regression test.
  - `session.rs` — hybrid sessions: `run_managed` (spawn + stream) and `attach` (observe).
  - `git.rs`, `db.rs` (sqlx/SQLite, runtime queries — no compile-time `query!` macros),
    `template.rs` (minijinja), `jsonpath.rs` (reads issues out of arbitrary tracker JSON),
    `overview.rs`, `events.rs`.
- **`crates/cli` (`clabby`)** — a thin driver over core: `clap` commands + a live log
  printer that subscribes to the core event bus. Presentation (the `status` table) lives
  here in `render.rs`, never in core.

Drivers (CLI, cron, future Tauri) all call the same engine functions, so behavior can't
diverge by entry point.

## Conventions

- **Add a new tracker/agent/VCS by editing config, not code.** If you find yourself
  adding a vendor-specific type to `core`, it belongs in a command template instead (§5,
  §11). `examples/github/` exists to prove the engine runs unchanged on a different
  tracker shape — keep that true.
- **Tests guard behavior (§9), black-box first.** New *user-facing* behavior gets a
  black-box test driving the compiled binary in `crates/cli/tests/` — `assert_cmd`/
  `assert_fs` for functional + failure paths, `trycmd` markdown transcripts for living
  docs (they fail when output drifts; regenerate with `TRYCMD=overwrite`). New *engine*
  behavior gets a unit test or extends `crates/core/tests/e2e.rs` (real command-template
  pipeline against offline fakes — a JSON file + a temp git repo, no network/credentials).
- Timestamps are stored as RFC3339 text and booleans as 0/1 integers in SQLite, mapped to
  domain types in `db.rs`.

## Status / roadmap

Milestones 1–2 are complete: the headless command center (sync, sessions, worktrees,
overview, cron) plus the deterministic workflow engine. M2 lives in `engine.rs` (the
`Engine` struct: `transition` runs a transition's steps under gate-with-override;
`override_for_issue` resumes a blocked one) and `audit.rs` (override reasons →
`audit_log`); transitions/steps/hooks are config in `clabby.toml`. CLI: `clabby move`
and `clabby override`. Next: M3 Tauri + React board with Playwright tests. See
`README.md` and `BACKLOG.md`.
