# Quality & velocity roadmap

Guardrails to keep quality and velocity high past 50+ features. The principle:
**entropy enters one PR at a time**, so each invariant we care about should be a *gate*
that fails CI, not a habit we hope holds. Tracked here; checked off as landed.

Status legend: ✅ done · 🚧 in progress · ⬜ planned

## Already in place (baseline)

- ✅ `Lint & test` (cargo fmt/clippy `-D warnings`/test)
- ✅ `Coverage` floor (`cargo-llvm-cov --fail-under-lines 80`)
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

- ⬜ **Diff coverage** — gate on changed-line coverage per PR (not just the global floor),
  and ratchet the global floor upward over time.
- 🚧 **Frontend unit/component tests** — Vitest is wired (jsdom + Testing Library),
  covering the `mockApi` seam and `IssueCard`, and runs in the `Frontend` CI job (`pnpm
  test`). *Still to do:* a frontend coverage gate and `axe-core` a11y assertions in the
  Playwright suite.
- ⬜ **Mutation testing** — a scheduled (weekly) `cargo-mutants` run on `core` (too slow
  per-PR), surfaced as a report/issue. Validates that tests *catch* bugs, not just execute.
- ⬜ **Secret scanning** — `gitleaks` in CI and pre-commit.

## Tier 3 — velocity at scale (keep the gates fast)

- ⬜ **Path-filtered jobs** — run `Desktop app` / `Frontend` only when `ui/**` or
  `src-tauri/**` change, so pure-Rust PRs aren't taxed by the frontend/Tauri build.
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
