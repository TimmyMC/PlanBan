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
turns on GitHub's native auto-merge. When all required checks go green the PR is
**squash-merged into trunk and its branch deleted**. If a check fails, the PR just
sits until you push a fix.

## What gates a merge

Required status checks (enforced by branch protection on `trunk`):

| Check (CI job name)       | What it enforces                                            |
| ------------------------- | ---------------------------------------------------------- |
| `fmt · clippy · test`     | `cargo fmt --check`, `clippy -D warnings`, full test suite |
| `coverage (>=75% lines)`  | `cargo llvm-cov --fail-under-lines 75`                     |
| `build · e2e`             | frontend `tsc`+`vite` build, Biome lint, Playwright e2e    |
| `tauri shell`             | compiles the Tauri desktop shell + `clippy -D warnings`    |

The Tauri shell lives outside the cargo workspace (WebView2 system deps), so the
workspace `cargo test --workspace` deliberately doesn't touch it — the `tauri shell`
job is what gates that crate so it can't silently stop compiling.

The coverage bar is **75% line coverage**, measured across the workspace by
`cargo-llvm-cov`. Baseline when the gate landed was ~79%; the 75% floor leaves a
small buffer for churn. Raise it as coverage improves — bump `--fail-under-lines`
in `.github/workflows/ci.yml` and update the table above.

## One-time setup (repo settings)

Branch protection and the "allow auto-merge" toggle are GitHub *settings*, not
files in the repo, so they aren't applied by cloning. Set them up once:

```sh
gh auth login                      # if not already authenticated
./scripts/setup-branch-protection.sh   # idempotent; defaults to TimmyMC/PlanBan
```

That script enables auto-merge (squash-only, auto-delete merged branches) and
protects `trunk` with the three required checks above. Prefer clicking? In the
GitHub UI: **Settings → General → Pull Requests** → enable *Allow auto-merge* and
*Automatically delete head branches*; then **Settings → Branches → Add rule** for
`trunk` → require pull requests and require the three status checks (strict).

`enforce_admins` is left off, so you can still push a direct hotfix to `trunk` in a
pinch; day to day, go through a PR.

## Dependabot

Dependabot PRs are auto-enabled for merge too, so routine dependency bumps merge
themselves once green. The bumps still face the exact same gates as any other PR.
