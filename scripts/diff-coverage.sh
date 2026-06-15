#!/usr/bin/env bash
#
# Per-PR diff coverage (Tier 2 of docs/quality-roadmap.md). The global floor
# (cargo-llvm-cov --fail-under-lines, Vitest thresholds) catches the *aggregate*
# dropping; this catches a PR adding *new* under-tested lines while the aggregate
# stays green. It gates only the lines the PR changed.
#
# Wraps `diff-cover` (pip): it intersects an lcov report with `git diff <base>`
# and fails when changed-line coverage is below the bar. Same invocation locally
# and in CI, so a red PR reproduces with one command.
#
#   Usage:  scripts/diff-coverage.sh <lcov> <base-ref> <fail-under> [path-prefix]
#     <lcov>        path to the lcov report (cargo-llvm-cov / Vitest v8)
#     <base-ref>    branch to diff against, e.g. origin/trunk
#     <fail-under>  minimum % of changed lines that must be covered
#     [path-prefix] prepended to report paths so they match git's repo-relative
#                   paths. Vitest emits ui-relative `src/...` → pass `ui/`.
#                   cargo-llvm-cov is already repo-relative → omit.
#
#   Run locally before pushing, e.g.:
#     scripts/diff-coverage.sh ui/coverage/lcov.info origin/trunk 90 ui/
#     scripts/diff-coverage.sh lcov.info             origin/trunk 90
set -euo pipefail

LCOV="${1:?usage: diff-coverage.sh <lcov> <base-ref> <fail-under> [path-prefix]}"
BASE="${2:?missing <base-ref>}"
FAIL_UNDER="${3:?missing <fail-under>}"
PREFIX="${4:-}"

if [ ! -f "$LCOV" ]; then
  echo "diff-coverage: lcov report not found: $LCOV" >&2
  exit 1
fi

# Normalize report paths to repo-relative POSIX so diff-cover can match them to
# `git diff` output: Windows V8 lcov uses backslashes and ui-relative paths.
normalized="$(mktemp)"
trap 'rm -f "$normalized"' EXIT
sed -e 's/\\/\//g' -e "s#^SF:#SF:${PREFIX}#" "$LCOV" > "$normalized"

echo "diff-coverage: $LCOV vs $BASE (fail-under ${FAIL_UNDER}%, prefix '${PREFIX:-<none>}')"
diff-cover "$normalized" --compare-branch "$BASE" --fail-under "$FAIL_UNDER"
