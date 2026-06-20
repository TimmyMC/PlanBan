#!/usr/bin/env bash
# PreToolUse guard: deny PR approve/merge via CLI (incl. wrapped/aliased/REST forms).
# Defense-in-depth only; the real control is credential separation (docs/agent-pipeline.md).

payload="$(cat)"

if printf '%s' "$payload" | grep -Eqi 'gh[[:space:]]+pr[[:space:]]+(merge|review)|pulls/[0-9]+/merge'; then
  cat <<'JSON'
{"hookSpecificOutput":{"hookEventName":"PreToolUse","permissionDecision":"deny","permissionDecisionReason":"Blocked: approving/merging a PR via CLI is reserved for a human owner (zero-trust, docs/agent-pipeline.md). Wrapped/alias/REST retries are also blocked — stop and ask the human to review."}}
JSON
fi
