#!/usr/bin/env bash
# PreToolUse guard (incl. wrapped/aliased/REST forms). Defense-in-depth only; the real
# control is credential separation (docs/agent-pipeline.md).
#
#  * Merging a PR via CLI is NEVER allowed for an agent — merges go through
#    automerge.yml + branch protection, or a human.
#  * Approving/reviewing a PR is allowed ONLY for the installed-App reviewer running in
#    the trusted CI reviewer context (agent-review.yml sets CLABBY_REVIEW_CONTEXT=
#    agent-review), where submitting the review IS the reviewer's job. Everywhere else —
#    the local implementer, interactive sessions — it stays blocked.

payload="$(cat)"

deny() {
  printf '{"hookSpecificOutput":{"hookEventName":"PreToolUse","permissionDecision":"deny","permissionDecisionReason":"%s"}}\n' "$1"
  exit 0
}

# Merge: never via an agent CLI.
if printf '%s' "$payload" | grep -Eqi 'gh[[:space:]]+pr[[:space:]]+merge|pulls/[0-9]+/merge'; then
  deny "Blocked: merging a PR via CLI is reserved for automation/owner (zero-trust, docs/agent-pipeline.md). Wrapped/alias/REST retries are also blocked — stop and ask the human."
fi

# Review/approve: only the trusted CI reviewer context may do this.
if printf '%s' "$payload" | grep -Eqi 'gh[[:space:]]+pr[[:space:]]+review|pulls/[0-9]+/reviews'; then
  if [ "${CLABBY_REVIEW_CONTEXT:-}" = "agent-review" ]; then
    exit 0
  fi
  deny "Blocked: approving a PR via CLI is reserved for the CI reviewer/owner (zero-trust, docs/agent-pipeline.md). Wrapped/alias/REST retries are also blocked — stop and ask the human to review."
fi
