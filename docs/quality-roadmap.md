# Quality & velocity roadmap

Guardrails to keep quality and velocity high past 50+ features. The principle:
**entropy enters one PR at a time**, so each invariant we care about should be a *gate*
that fails CI, not a habit we hope holds. Tracked here; checked off as landed.

Status legend: ✅ done · 🚧 in progress · ⬜ planned

## Already in place (baseline)

- ✅ `Lint & test` (cargo fmt/clippy `-D warnings`/test)
- ✅ `Coverage` floor (`cargo-llvm-cov --fail-under-lines`; floors centralized in `.github/coverage-thresholds.json`)
- ✅ `Frontend` (tsc + Biome + Playwright) and a `Desktop app` build gate
- ✅ Panic discipline in core (`unwrap_used`/`expect_used`/`dbg_macro` denied outside tests)
- ✅ Trunk-based auto-merge gated on the full CI workflow
- ✅ Auto-format pre-commit hook (cargo fmt + Biome)

## Tier 1 — highest leverage

- ✅ **Architectural fitness function** (`crates/core/tests/architecture.rs`) — fails the
  build if `clabby-core` gains a UI/vendor/network dependency or imports a vendor crate
  (locks Constitution §7 & §11).
- ✅ **Pinned toolchain** — `rust-toolchain.toml` (channel + components) so local and CI
  build the same compiler and clippy upgrades are deliberate PRs, not surprise breakage.
- ✅ **Dependency updates** — `.github/dependabot.yml` covers `cargo` (workspace +
  `src-tauri`), `npm` (`ui`), and `github-actions`.
- ❌ **Supply-chain gate (`cargo-deny`)** — *removed as overkill for this project.* The
  RUSTSEC-advisory/license/ban gate added too much friction for the value at this scale;
  Dependabot covers dependency hygiene. Revisit if compliance ever requires a hard gate.

## Tier 1b — correctness, its own PR (bigger)

- ✅ **Compile-time SQL + migrations** (#20) — moved `db.rs` onto **Diesel**: queries are
  checked at build time against `crates/core/src/schema.rs`, and the inline `CREATE TABLE`
  block became versioned migrations in `crates/core/migrations/`. Diesel was chosen over
  sqlx's `query!` macros specifically to avoid sqlx's weak SQLite nullability inference (the
  per-column `AS "col!"` override tax) — and it needs **no `.sqlx` offline cache**, so there's
  nothing to keep in sync and no CI staleness step. Schema drift is now a compile error.

## Tier 2 — test depth (coverage ≠ confidence)

- ✅ **Coverage floors + per-PR visibility** — the global line floors are ratcheted
  (Rust 80 → 85) and centralized in `.github/coverage-thresholds.json` (one place to
  raise; read by both `ui/vite.config.ts` and the Rust gate in `ci-complete.yml`). Each
  run's coverage table is posted to `$GITHUB_STEP_SUMMARY` (Checks → job → Summary), so the
  actual level is visible on every PR without Codecov. *Considered and dropped:* a per-PR
  *diff*-coverage gate (`diff-cover`) — YAGNI at 85–90% floors, where the absolute floor
  already catches new under-tested code.
- ✅ **Frontend unit/component tests** — Vitest (jsdom + Testing Library) covers the
  `mockApi` seam, the components, and the `useBoard` handler logic; the coverage gate
  (`vite.config.ts` thresholds, sourced from `.github/coverage-thresholds.json`) and
  `axe-core` a11y assertions (asserted across resting / gated-banner / post-override
  states) both ship. Coverage is tracked **per test type**: Vitest measures units only,
  while Playwright e2e has a separate, report-only track (`ui/coverage-e2e`) that is never
  merged into the unit number.
- 🚧 **Mutation testing** (#23) — a scheduled (weekly) `cargo-mutants` run on `core` (too
  slow per-PR), in `.github/workflows/mutants.yml` with tuning in `.cargo/mutants.toml`.
  Validates that tests *catch* bugs, not just execute. v1 is report-only: results land as a
  `$GITHUB_STEP_SUMMARY` table plus an uploaded `mutants.out` artifact. Auto-filing a
  tracking issue from the report is the planned follow-up.
- ⬜ **Secret scanning** — `gitleaks` in CI and pre-commit.

## Tier 3 — velocity at scale (keep the gates fast)

- ✅ **Path-filtered jobs** (#49) — a `changes` job (`dorny/paths-filter@v3`) maps changed
  paths to `rust` / `frontend` / `desktop` / `any_code` (filters in `.github/filters.yml`);
  each downstream job gates on its output, so docs-only PRs skip the Rust compile, coverage,
  and Playwright suite. The `CI complete` aggregator passes when every job succeeded or was
  skipped, giving branch protection one stable required check.
- ⬜ **Faster tests + caching** — `cargo-nextest` (speed + native flaky-retry + JUnit),
  `sccache`; Playwright `retries` + trace-on-failure artifact upload.
- ✅ **CI concurrency** — `concurrency: { group: <ref>, cancel-in-progress: true }` so
  superseded runs stop and can't race auto-merge.
- ✅ **Auto-merge escape hatch** — a `do-not-merge` label the auto-merge workflow
  respects (Constitution §1): CI still runs, but the PR won't merge until it's removed.

## Agent self-improvement

- 🚧 **Skills library + self-improve loop** — `.claude/skills/` holds reusable skills, a
  `self-improve` meta-skill documents how an agent captures a learning as a new/updated
  skill, and a `SessionStart` hook surfaces the available skills each session so the loop
  is visible. See `.claude/skills/self-improve/SKILL.md`.
