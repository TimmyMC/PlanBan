# Trunk-based development

Clabby uses **trunk-based development**: `trunk` is the single long-lived branch
and the GitHub default. All work happens on short-lived branches that open a pull
request into `trunk`, and **a PR merges itself once every required check passes** —
build, lint, tests, and a code-coverage floor.

## The loop

```
git switch trunk && git pull
git switch -c fix/some-thing      # short-lived branch
# ...commit work (the pre-commit hook auto-formats)...
git push -u origin HEAD
gh pr create --fill               # or open the PR in the browser
```

From there it's hands-off: CI runs on the PR, and `.github/workflows/automerge.yml`
**waits for the whole CI workflow to succeed, then squash-merges the PR and deletes
the branch**. If any job fails, the PR just sits until you push a fix.

To **hold** a PR for human review, add the `do-not-merge` label — CI still runs, but
auto-merge skips it until you remove the label (the human-in-the-loop escape hatch, §1).

**Staleness is automatic too.** The ruleset is strict (a PR must be up to date with
trunk to merge), so when trunk advances `.github/workflows/auto-update-prs.yml` updates
every open, conflict-free PR and re-runs its CI — PRs merge in a self-healing cascade.
The only things that need a human are a **conflict** or a **CI failure**. This needs a
`AUTOMERGE_PAT` secret (a branch update by the default token wouldn't re-trigger CI);
without it the workflow no-ops. Stale *running* CI is cancelled by `ci-complete.yml`'s
`concurrency` block.

> **Why gate on the CI workflow, not GitHub "auto-merge"?** GitHub's native
> auto-merge only waits for checks that *branch protection* lists as required — so
> if protection isn't set up (or is set up after the fact), a PR can merge *before
> checks finish*. The auto-merge workflow instead triggers on `workflow_run` of CI
> completing `success`, so by the time it runs, all four jobs below are already
> green. No dependency on settings being configured in the right order.

## What gates a merge

The auto-merge job starts only after **every** CI job passes:

| Check (CI job name) | What it enforces                                           |
| ------------------- | ---------------------------------------------------------- |
| `Lint & test`       | `cargo fmt --check`, `clippy -D warnings`, full test suite |
| `Coverage`          | `cargo llvm-cov --fail-under-lines 80`                     |
| `Frontend`          | frontend `tsc`+`vite` build, Biome lint, Playwright e2e    |
| `Desktop app`       | compiles the Tauri desktop shell + `clippy -D warnings`    |

The desktop shell lives outside the cargo workspace (WebView2 system deps), so the
workspace `cargo test --workspace` deliberately doesn't touch it — the `Desktop app`
job is what gates that crate so it can't silently stop compiling.

The coverage bar is **80% line coverage**, measured across the workspace by
`cargo-llvm-cov`. Measured coverage is ~82%; the 80% floor leaves a small buffer
for churn. Raise it as coverage improves — bump `--fail-under-lines` in
`.github/workflows/ci-complete.yml` and update the table above.

## One-time setup (recommended, but not required for the wait)

The auto-merge workflow above is self-contained — it needs no repo settings to
gate correctly. Branch protection is still worth adding as a **second layer** that
blocks direct pushes to `trunk` and requires the checks at the GitHub level. It's a
*setting*, not a file, so it isn't applied by cloning:

**Settings → Branches → Add rule** for `trunk` → require pull requests and require
the `CI complete` status check (strict). `enforce_admins` is left off, so you can
still push a direct hotfix to `trunk` in a pinch.

Do **not** also enable GitHub's native "Allow auto-merge" and click *Enable
auto-merge* on a PR before protection lists the required checks — that path merges
immediately. The workflow is the gate; let it do the merging.

## Dependabot

Dependabot PRs merge themselves once green too, facing the exact same CI gates as
any other PR.
