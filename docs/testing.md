# Testing taxonomy

How Clabby proves it works. This is the vocabulary every test slots into, and — more
importantly — **which gate catches what**, so a red CI run tells you exactly where the missing
test belongs. It exists because [Constitution §9](../CONSTITUTION.md) ("regressions are guarded
by tests, not vigilance") demands that coverage be *machine-enforced*, not remembered. Prose
documents; CI enforces. Nothing below depends on someone reading this file — the gates run
whether or not anyone does.

## The layers

| Layer | Proves | Lives in | Tooling |
| --- | --- | --- | --- |
| **CLI use-case docs** | a command's real input → output, *as a usage example* | `crates/cli/tests/cmd/*.md` | `trycmd` (snapshot) |
| **CLI behavior tests** | flags, edge cases, failure exit codes, filesystem effects | `crates/cli/tests/cli_blackbox.rs` | `assert_cmd` / `assert_fs` |
| **Engine unit tests** | pure module logic | `crates/core/src/*.rs` `#[cfg(test)]` | std test |
| **Engine pipeline tests** | the command-template pipeline against offline fakes | `crates/core/tests/e2e.rs` | std test + temp git/SQLite |
| **Frontend component tests** | components + the `api` seam, headless | `ui/src/**/*.test.ts(x)` | Vitest |
| **Frontend UX e2e tests** *(priority)* | real user flows in a browser | `ui/tests/*.spec.ts` | Playwright |
| **Fitness functions** | architectural invariants + use-case coverage | `crates/core/tests/architecture.rs`, the coverage gates | std test / Vitest |

Two deliberate emphases, both from the Constitution:

- **Black-box first (§9).** A user-facing CLI use case is proven by driving the *compiled
  binary* — args/stdin in, exit code + stdout/stderr out — knowing nothing about the internals.
  Because these bind only to the CLI contract, the engine can be rewritten freely (§7) and the
  tests still hold.
- **Playwright UX e2e is the priority frontend layer (§10).** The GUI has shipped (M3), so "UX
  is a priority, not a polish phase" means the browser-level flow tests — not component
  rendering alone — are where frontend correctness is asserted.

## Which gate catches what

These run *inside* the existing `Lint & test` and `Frontend` CI jobs — no separate required
check, no ruleset dependency. Each turns a value into deterministic teeth.

- **CLI command + flag coverage** — `crates/cli/tests/use_case_coverage.rs` walks the clap
  command tree (the single source of truth, exposed via `clabby::use_case_keys()`) and **fails
  the build** if any leaf command or flag is missing a row in
  [`docs/use-cases.md`](./use-cases.md), or if a row names a `cli_blackbox::<fn>` test that no
  longer exists. A command can't ship undocumented *or* untested.
- **UI api-method + flow coverage** — `ui/src/use-case-coverage.test.ts` parses the `Api`
  interface in `ui/src/api/index.ts` by brace-matching (not a fragile regex — a `}` inside a
  return type would otherwise truncate the parse and silently pass), and **fails** if any method
  lacks an `@usecase:api/<method>` tagged Playwright spec, or any registry flow lacks an
  `@usecase:flow/<id>` one.
- **Architectural invariants** — `crates/core/tests/architecture.rs` is a fitness function that
  fails the moment `clabby-core` gains a UI/desktop, network-client, or vendor-SDK dependency or
  `use` import (Constitution §7/§11). The engine shells out through rendered command templates;
  it never links a vendor.
- **Doc drift** — `trycmd` runs the `crates/cli/tests/cmd/*.md` transcripts as snapshot tests,
  so the living docs fail the build when command output drifts. Regenerate intentionally with
  `TRYCMD=overwrite cargo test -p clabby --test cli_docs`.
- **Depth backstops** — line-coverage gates (Rust and frontend) keep the above from being
  satisfied by shallow tests.

### The meta-gate: don't loosen a gate to go green

The `Gate integrity` CI job (`scripts/gate-guard.sh`) sits above all of these. It fails a PR
that *weakens* the test/gate surface — deleting a test file, net-removing assertions, adding
skip / focus / ignore markers, lowering a coverage or strictness threshold, dropping
warnings-as-errors or frozen-lockfile enforcement, editing the gate machinery itself, or
removing a use-case row. The fix is to make the code pass, not to remove the check. A
*deliberate* gate change is a human decision: the owner adds the `gate-change-approved` label.
See the "Gate guard" section of [`CLAUDE.md`](../CLAUDE.md) for the full rule set and the local
run (`scripts/gate-guard.sh origin/trunk`).

## The one honest residual

Everything enumerable from code is gated above. The single thing no CI can enumerate is **UX
flow completeness**: which user journeys *deserve* a test isn't derivable from the source. So
the registry ([`docs/use-cases.md`](./use-cases.md)) lists flows with stable ids, every spec is
tagged `@usecase:<id>`, and the UI gate asserts that every *listed* flow has a tagged test — but
the completeness of the *list itself* is the one human-judgment point in the whole scheme. This
is stated plainly rather than dressed up as full automation: when you add a genuinely new user
flow, you add its row; the gate then forces the test.

## Writing Playwright UX tests

The board specs live in `ui/tests/*.spec.ts`. Patterns worth reusing (the
[`ui-testing`](../.claude/skills/ui-testing/SKILL.md) skill has the operational details):

- **Tag every spec** with `{ tag: ["@usecase:api/<method>", "@usecase:flow/<id>"] }` — this is
  what the UI coverage gate reads. An untagged new flow or api method reddens CI.
- **Dragging:** use the `dragCardToColumn` helper (a real pointer drag past dnd-kit's activation
  distance, in steps). dnd-kit installs a one-shot capture-phase listener that swallows the
  *first* click after a drag, so a click immediately following a drag must be retried until it
  lands — wrap it in `expect(async () => { … }).toPass()` rather than a bare click.
- **Accessibility:** scan with axe across the *conditional* states, not just the resting board —
  the override banner is the only place the destructive/accent tokens actually render, so a
  resting-only scan would never see them. The board spec asserts zero violations at rest, with
  the banner shown, and after the override resolves.
- **Reliability:** the Playwright config sets one retry with `trace: on-first-retry`, and CI
  uploads the trace/report on failure — so a flake fails loudly and debuggably instead of
  silently.

## Running

```sh
cargo test                      # everything: core + e2e + CLI black-box + trycmd docs
cargo test -p clabby-core       # the fast inner loop (§8)
cargo test -p clabby            # black-box CLI (needs node + git on PATH)
pnpm --dir ui test:cov          # Vitest unit + coverage (includes the UI coverage gate)
pnpm --dir ui test:e2e          # Playwright UX e2e
scripts/gate-guard.sh origin/trunk   # the meta-gate, locally, before pushing
```
