# Backlog

Work we've consciously deferred, captured so it isn't lost. This is *not* the roadmap
(see `README.md` for milestones) — it's the list of "good idea, not now".

## Tooling / robustness

- **Compile-time-checked SQL via `sqlx::query!` macros.** Move `crates/core/src/db.rs`
  from the runtime `sqlx::query` API to the compile-time-checked `query!`/`query_as!`
  macros with a committed offline cache (`cargo sqlx prepare` → `.sqlx/`).
  - *Gain:* every SQL statement is verified against the real schema at build time.
  - *Cost (why deferred):* requires a `DATABASE_URL` or an offline cache at build time
    and a regeneration step on every SQL change — friction against clean-clone build
    velocity (Constitution §8). For now, query correctness is covered end-to-end by the
    integration tests (`crates/core/tests/e2e.rs`) and the CLI black-box suite.
  - *Wanted eventually* — flagged by the maintainer for "one day".

- **CI: add an `ubuntu-latest` matrix leg.** The workflow currently runs only on
  `windows-latest`; an ubuntu leg would exercise the non-Windows `sh -c` path in
  `runner::shell`, which is otherwise unverified.

## Engine

- **Batch the overview's per-issue queries.** `overview::build` does N+1 DB lookups plus
  a few `git` subprocesses per worktree. Fine for a single user with dozens of issues;
  worth batching if boards grow large.
