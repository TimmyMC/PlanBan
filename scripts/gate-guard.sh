#!/usr/bin/env bash
#
# Gate guard (Constitution §9). Fails a PR that *weakens* the test/gate surface
# unless the change is explicitly approved by the owner via the
# `gate-change-approved` label. The point is to stop an agent from making CI
# green by deleting a test, disabling a check, or loosening a threshold instead
# of fixing the code — and to route any deliberate strictness change through a
# human decision rather than letting it slip in silently.
#
# Detection is heuristic and deliberately errs toward flagging: a false positive
# costs one label; a false negative lets a gate erode unnoticed. It runs on the
# raw diff, so it needs no toolchain and is identical locally and in CI.
#
#   Usage:  scripts/gate-guard.sh <base-ref>      # e.g. origin/trunk
#   Env:    GATE_CHANGE_APPROVED=true             # set by CI from the PR label
#
#   Exit 0 = clean, or loosening present but owner-approved.
#   Exit 1 = loosening detected and not approved.
#
# Run it locally before pushing:  scripts/gate-guard.sh origin/trunk
set -uo pipefail

BASE="${1:-origin/trunk}"
APPROVED="${GATE_CHANGE_APPROVED:-false}"

base_commit="$(git merge-base "$BASE" HEAD 2>/dev/null || echo "$BASE")"
range="${base_commit}...HEAD"
# Line-level heuristics scan everything *except* this script — otherwise the
# pattern literals it defines (`.only(`, `fail-under-lines`, …) match themselves.
# Rule 6 still flags edits to it by filename.
scan_diff="$(git diff "$range" -- ':(exclude)scripts/gate-guard.sh')"
removed_lines="$(printf '%s\n' "$scan_diff" | grep -E '^-[^-]' || true)"   # without the '---' header
added_lines="$(printf '%s\n' "$scan_diff"   | grep -E '^\+[^+]' || true)"  # without the '+++' header
changed_files="$(git diff --name-only "$range" || true)"
deleted_files="$(git diff --diff-filter=D --name-only "$range" || true)"

findings=()
add() { findings+=("$1"); }
cnt() { printf '%s\n' "$1" | grep -cE "$2" 2>/dev/null || true; }

# 1) Deleted test files — the bluntest way to make a failing test "pass".
while IFS= read -r f; do
  [ -z "$f" ] && continue
  case "$f" in
    *.spec.ts|*.test.ts|*.test.tsx|*_test.rs) add "deleted test file: $f" ;;
    */tests/*|*test*.rs)                       add "deleted test file: $f" ;;
  esac
done <<< "$deleted_files"

# 2) Net removal of tests/assertions across the diff (removed more than added).
markers='#\[(tokio::)?test\]|\bfn test_|\bit\(|\btest\(|\bassert(_eq|_ne)?!|\bexpect\(|toEqual|toBe\('
r=$(cnt "$removed_lines" "$markers"); a=$(cnt "$added_lines" "$markers")
if [ "$r" -gt "$a" ]; then add "net removal of tests/assertions (-$r added back +$a)"; fi

# 3) Added skip / disable / focus markers (a focused `.only` silently skips the rest).
skips='#\[ignore\]|\.skip\(|\bit\.skip|\btest\.skip|describe\.skip|xfail|\.only\(|continue-on-error:[[:space:]]*true|if:[[:space:]]*false'
s=$(cnt "$added_lines" "$skips")
if [ "$s" -gt 0 ]; then add "added $s test-skip/disable marker(s): ignore/.skip/.only/continue-on-error/if:false"; fi

# 4) Coverage / strictness thresholds touched — either direction; the owner verifies it wasn't lowered.
if printf '%s\n' "$scan_diff" | grep -qE '^[-+].*(fail-under-lines|--fail-under|fail_under|coverageThreshold|thresholds:)'; then
  add "a coverage/strictness threshold was changed — confirm it was not lowered"
fi

# 5) Removed strictness flags (warnings-as-errors, format/lint checks).
if [ "$(cnt "$removed_lines" '\-D warnings|deny\(warnings\)|--check|--frozen-lockfile')" -gt 0 ]; then
  add "removed a strictness flag (-D warnings / deny(warnings) / --check / --frozen-lockfile)"
fi

# 6) The gate machinery itself was modified (self-guard): any edit to a gate test,
#    the CI workflows, or this script needs an explicit owner sign-off — strengthening
#    is fine, but only a human can tell strengthening from weakening here.
guard='crates/cli/tests/use_case_coverage\.rs|ui/src/use-case-coverage\.test\.ts|crates/core/tests/architecture\.rs|crates/cli/tests/cli_docs\.rs|scripts/gate-guard\.sh|^\.github/workflows/'
while IFS= read -r f; do
  [ -z "$f" ] && continue
  printf '%s\n' "$f" | grep -qE "$guard" && add "gate/CI machinery modified: $f"
done <<< "$changed_files"

# 7) Use-case registry rows removed (every removed row drops a tracked use case).
if [ "$(cnt "$removed_lines" '^-[[:space:]]*\|.*`(flow/|api/|[a-z_]+::)')" -gt 0 ]; then
  add "use-case registry row(s) removed from docs/use-cases.md"
fi

if [ "${#findings[@]}" -eq 0 ]; then
  echo "Gate guard: no test/gate weakening detected."
  exit 0
fi

echo "Gate guard findings:"
printf '  - %s\n' "${findings[@]}"

if [ "$APPROVED" = "true" ]; then
  echo
  echo "✔ 'gate-change-approved' is set — the owner accepted this gate change. Allowing."
  exit 0
fi

cat <<'MSG'

❌ This PR changes the test/gate surface in a way that could weaken it.
   If the change is intentional, the repository owner must review it and add the
   `gate-change-approved` label to this PR (re-running CI), which clears this gate.
   Otherwise, restore the test/gate that was removed or loosened.
MSG
exit 1
