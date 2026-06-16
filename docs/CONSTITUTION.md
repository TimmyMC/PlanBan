# Clabby Constitution

A short, durable statement of intent. Every plan, PR, and agent action is checked
against it. **If a change violates an article, the change is wrong — not the
constitution.** Amendments are deliberate and rare, and must be made here explicitly.

Clabby is a **local command center** for agent-assisted software work: a dashboard
that keeps an overview of many concurrent work streams (multiple agents, terminals,
git worktrees, and issues in flight), synced to an external issue tracker, with a
deterministic workflow engine that guarantees mandatory steps happen — while a human
stays in the loop.

---

## Articles

### 1. Human-in-the-loop, never autonomous
Clabby orchestrates and *enforces*; the human thinks and decides. Agents are
constrained subprocesses inside guardrails — never the driver. This is the line that
separates Clabby from autonomous agent runners.

### 2. Determinism over cleverness
Mandatory workflow steps run exactly and every time. No skipped steps, no "the agent
usually does it." Failures gate the workflow; overriding a gate is an explicit action
that records a reason — for debugging and operational clarity ("why was this bypassed?"),
not as a regulatory compliance trail. Logging exists to be useful, not to certify.

### 3. Overview first
The primary job is situational awareness across many concurrent work streams. If a
feature doesn't help you understand or control what's in flight, it's secondary.

### 4. The issue tracker is the source of truth
Clabby mirrors and may write, but never silently diverges. Conflicts are surfaced and
reconciled transparently; on conflict, the tracker wins and the divergence is flagged.

### 5. Harness- and tool-agnostic
Every external action (agent CLI, issue tracker, VCS host, git) is a configurable
command template. No vendor or agent is hardcoded. Config and hooks are first-class
and editable without code changes.

### 6. Local-first, single-user
Runs on your machine; your data stays local. No cloud dependency, no telemetry.

### 7. Core stays UI-free
The engine is a library with no UI/Tauri dependency. CLI, cron, and GUI are thin,
interchangeable drivers over one engine — so behavior never diverges by entry point.

### 8. Rapid feedback loops are a feature
The system must be verifiable in seconds, headless, without the GUI. Slow or GUI-only
verification is a defect.

### 9. Regressions are guarded by tests, not vigilance
Untested behavior is considered broken. Coverage is *machine-enforced*, not remembered:
deterministic gates fail the build when a use case is added without a test. The test taxonomy
and the "which gate catches what" map live in [`docs/testing.md`](docs/testing.md). The
strategy is layered:

- **Black-box first.** Every user-facing use case has a test that drives the *compiled
  binary* — args/stdin in, exit code + stdout/stderr out — knowing nothing about the
  internals. These tests double as documentation and must read as usage examples.
  Realized with the assert-rs stack: `trycmd` runs documented command/output transcripts
  as snapshot tests (living docs that fail the build when output drifts), `assert_cmd`
  exercises flags, edge cases, and failure exit codes, and `assert_fs` sandboxes
  filesystem state. Because they bind only to the CLI contract, the engine can be
  rewritten freely (§7) and the tests still hold.
- **Core unit/integration.** Engine logic (sync/divergence, templating, persistence, the
  M2 gate) is covered directly against temp SQLite and temp git repos, fully offline (§8).
- **UI end-to-end (the priority frontend layer).** The GUI has shipped, so comprehensive
  Playwright tests covering the user *flows* — not just rendering — are how frontend
  correctness is asserted. A coverage gate fails the build if an `api` seam method or a listed
  UX flow has no tagged e2e test (see `docs/testing.md`).

Tests run on every commit; the trunk stays green (§12).

### 10. UX is a priority, not a polish phase
The tool must reduce cognitive load. Friction, ambiguity, or noise in the overview is
a bug.

### 11. Generic core, specific workflow
The engine encodes *no* particular workflow. A user's highly-defined process (their
statuses, worktree convention, tracker/VCS steps) lives entirely in config and hooks —
never in core code. The test of every core abstraction: *could a different team adopt
Clabby for a completely different workflow by editing only config and hook scripts,
touching no Rust?* If a workflow assumption leaks into the engine, that's the bug.
Extensibility and configurability are first-class design constraints — new step types,
integrations, and triggers must be addable without forking core.

### 12. Trunk-based development
Work integrates into a single trunk (`trunk`) continuously. Branches are short-lived
(hours to a day) and merge back fast; long-lived feature branches are not allowed — they
defeat the rapid feedback loops of §8 and let drift accumulate against §11. The trunk is
always releasable: every commit keeps the build green and the tests passing (§9).
Incomplete work ships behind config/flags (§5) rather than lingering on a branch. Commit
in small, coherent increments so the history itself tells the story.

---

## How to use this document

- **Designing a feature?** Name the articles it serves. If it fights one, redesign.
- **Reviewing a change?** A violation of an article is sufficient grounds to reject it.
- **Tempted to hardcode a vendor, status, or workflow assumption in `core`?** See §5
  and §11 — it belongs in config, a command template, or a hook.
