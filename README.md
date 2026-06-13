# Clabby

A **local command center** for agent-assisted software work: a dashboard that keeps
an overview of many concurrent work streams (multiple agents, terminals, git
worktrees, issues in flight), synced to your issue tracker, with a deterministic
workflow engine that guarantees the mandatory steps happen — while you stay in the
loop.

Clabby is **not** an autonomous agent runner. It enforces your workflow; you do the
thinking. See [`CONSTITUTION.md`](./CONSTITUTION.md) for the principles that govern
every change.

## Status: Milestone 1 (headless command center)

Working today, fully headless via the CLI:

- **Tracker sync** — pull issues via a configurable command (your `acli`/`jira`/`gh`),
  with the tracker as the source of truth. External changes are detected and flagged
  as **diverged** rather than silently overwritten; your own pushes reconcile cleanly.
- **Sessions** — *managed* runs that Clabby spawns and streams, plus *external*
  interactive sessions you run yourself and attach for the overview.
- **Worktrees** — create per-issue git worktrees and surface their live git state.
- **Overview** — `clabby status` joins issues × sessions × git state into one view.
- **Cron** — keep the overview fresh on a schedule.

Everything external is a **command template** — no tracker, VCS host, or agent is
hardcoded (Constitution §5, §11). The engine (`clabby-core`) has no UI dependency;
the CLI is a thin driver, and a Tauri + React board will be another (Milestone 3).

## Quickstart (offline demo, no credentials)

Requires the Rust toolchain and Node.js (the demo's fake tracker is a node script).

```sh
cargo build
cd examples/jira

# Pull issues from the (fake) tracker and show the board
../../target/debug/clabby sync
../../target/debug/clabby status

# Transition an issue from Clabby (pushes back to the tracker)
../../target/debug/clabby issue set-status PROJ-12 "In Review"

# Run a managed agent for an issue, streaming its output
../../target/debug/clabby session spawn PROJ-12 --agent echo

# Simulate someone else changing the tracker, then re-sync to see divergence
node ../tracker.mjs issues.json transition PROJ-44 "Done"
../../target/debug/clabby sync
../../target/debug/clabby status        # PROJ-44 shows "! diverged(->Done)"
```

`examples/github/` is the **same engine** driven by a deliberately different tracker
shape (top-level array, flat `state`, label objects) — proof that adopting a new
workflow needs config changes only, never code (Constitution §11).

## Commands

```
clabby sync                               Pull + reconcile issues from the tracker
clabby status [--watch]                   The overview dashboard
clabby issue set-status <KEY> <STATUS>    Push a status change to the tracker
clabby session spawn <KEY> --agent <A>    Spawn + stream a managed agent run
clabby session attach <KEY> --worktree P  Register an external interactive session
clabby session list                       List sessions
clabby worktree add <KEY> [--branch B]    Create a per-issue git worktree
clabby worktree list                      List recorded worktrees
clabby logs tail <SESSION_ID>             Recent log lines for a session
clabby cron run [--once]                  Run scheduled jobs (or each once)
```

Config is discovered as `clabby.toml` from the current directory upward, or passed
with `--config`. See the examples for a documented schema.

## Roadmap

- **M2 — deterministic workflow engine:** column transitions run ordered
  command-template steps with **gate-with-override** (a required step failure blocks
  the transition; overrides require a logged reason — the regulatory audit trail).
- **M3 — Tauri + React board:** drag-to-transition, live session panels, with
  comprehensive Playwright end-to-end coverage.
- **Future — `clabby init --from-jira`:** bootstrap config from a live instance's
  custom statuses and labels.
