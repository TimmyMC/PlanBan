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
directly. Branch off, open a PR, and it auto-merges once CI is green. CI gates:
`Lint & test`, `Coverage`, `Frontend`, and `Desktop app`. See
[`docs/trunk-based-development.md`](docs/trunk-based-development.md) and the broader plan in
[`docs/quality-roadmap.md`](docs/quality-roadmap.md).

**Quality gates worth knowing.** The toolchain is pinned in `rust-toolchain.toml` (bump it
*and* the `dtolnay/rust-toolchain@<ver>` refs in `ci.yml` together). `crates/core/tests/architecture.rs`
is a fitness function that fails if `clabby-core` gains a UI/vendor/network dependency or
import (Constitution §7/§11) — if you're tempted to add one to core, it belongs in a
command template or driver instead.

**Gate guard — don't loosen gates to go green.** The `Gate integrity` CI job
(`scripts/gate-guard.sh`) fails the build when a PR *weakens* the test/gate surface:
deleting a test file, net-removing tests/assertions, adding `#[ignore]`/`.skip`/`.only`/
`continue-on-error`, lowering a coverage/strictness threshold, removing `-D warnings`/
`--frozen-lockfile`, editing the gate machinery itself, or dropping a `docs/use-cases.md`
row. The fix is to make the code pass, not to remove the check. A *deliberate* gate change
is a human decision: the owner adds the `gate-change-approved` label, which re-runs CI and
clears the guard (CODEOWNERS marks the same paths for optional required review). Run it
locally before pushing: `scripts/gate-guard.sh origin/trunk`.

**Skills & self-improvement.** Reusable, repo-specific know-how lives in `.claude/skills/`;
a `SessionStart` hook lists them each session. When you solve something non-obvious that
will recur, capture it via the `self-improve` skill (add/update a `SKILL.md`).

Lints: crate roots set `#![warn(clippy::all)]` with the doc/style nags allowed; CI runs
`clippy -D warnings`. Don't enable `clippy::pedantic` broadly — it's high-noise; the
allows in `lib.rs`/`main.rs` keep it opt-in if individual pedantic lints are wanted later.
Deferred work (incl. moving sqlx to compile-time `query!`) is tracked as GitHub issues
(labels `roadmap` / `tech-debt`), not in-repo.

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

**Test taxonomy — where a new test belongs, and which gate fails without it.** Full version in
[`docs/testing.md`](docs/testing.md); the short map:

| Layer | Lives in | Tooling |
| --- | --- | --- |
| CLI use-case docs | `crates/cli/tests/cmd/*.md` | `trycmd` snapshots |
| CLI behavior tests | `crates/cli/tests/cli_blackbox.rs` | `assert_cmd` / `assert_fs` |
| Engine unit tests | `crates/core/src/*.rs` `#[cfg(test)]` | std test |
| Engine pipeline tests | `crates/core/tests/e2e.rs` | std test + temp git/SQLite |
| Frontend component tests | `ui/src/**/*.test.ts(x)` | Vitest |
| Frontend UX e2e *(priority)* | `ui/tests/*.spec.ts` | Playwright |
| Fitness functions | `crates/core/tests/architecture.rs` + the coverage gates | std test / Vitest |

Which gate reddens when a use case is added without a test:

- New CLI command/flag, no row + test → `crates/cli/tests/use_case_coverage.rs` (walks the clap
  tree; needs a [`docs/use-cases.md`](docs/use-cases.md) row naming a real `cli_blackbox::<fn>`).
- New `Api` seam method or UX flow, no tagged spec → `ui/src/use-case-coverage.test.ts` (needs
  an `@usecase:api/<method>` / `@usecase:flow/<id>` tag on a Playwright spec).
- Vendor/UI/network dep or import leaks into core → `crates/core/tests/architecture.rs` (§7/§11).
- Command output drifts → `trycmd` (`crates/cli/tests/cmd/*.md`).

The **Gate guard** (below) is the meta-gate over all of these: it stops them being silently
weakened to go green.

## Status / roadmap

Milestones 1–2 are complete: the headless command center (sync, sessions, worktrees,
overview, cron) plus the deterministic workflow engine. M2 lives in `engine.rs` (the
`Engine` struct: `transition` runs a transition's steps under gate-with-override;
`override_for_issue` resumes a blocked one) and `audit.rs` (override reasons →
`audit_log`); transitions/steps/hooks are config in `clabby.toml`. CLI: `clabby move`
and `clabby override`. M3 (Tauri + React board, Playwright e2e) is in too. Planned and
deferred work lives in GitHub issues (labels `roadmap` / `tech-debt`), not in-repo.
