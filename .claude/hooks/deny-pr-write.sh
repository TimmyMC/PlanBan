#!/usr/bin/env bash
# PreToolUse guard (Bash + PowerShell tools): hard-deny approving or merging a PR
# via the CLI, matching the dangerous call ANYWHERE in the command — so the
# `bash -c '...'` wrappers, aliases, and `gh api .../merge` calls are all caught,
# not just a bare `gh pr merge`. This is the sole harness guard: it supersedes a
# prefix-matched permissions.deny list (which couldn't see the wrapped/API forms),
# so that list was removed. Returns a deny decision with a reason so a well-meaning
# agent understands why and stops.
#
# Zero-trust note (docs/agent-pipeline.md): this is defense-in-depth, NOT the
# boundary. An injected/adversarial agent isn't "dissuaded" — it will probe other
# paths. The real control is that the agent's identity/token can't cast a *counting*
# approval (not in OWNERS/REVIEWER_LOGINS) and can't merge. Keep this guard cheap and
# treat credential separation as the actual security.
#
# No jq dependency: the command text appears verbatim in the raw payload (the
# patterns contain no JSON-escaped characters), so we grep the whole stdin.

payload="$(cat)"

if printf '%s' "$payload" | grep -Eqi 'gh[[:space:]]+pr[[:space:]]+(merge|review)|pulls/[0-9]+/merge'; then
  cat <<'JSON'
{"hookSpecificOutput":{"hookEventName":"PreToolUse","permissionDecision":"deny","permissionDecisionReason":"Blocked: approving or merging a PR via the CLI is reserved for a human owner reviewing in the GitHub UI (zero-trust, docs/agent-pipeline.md). No agent may clear a gate or merge. Retrying with a wrapped command, an alias, or the REST API will also be blocked and is a policy violation - stop and ask the human to review."}}
JSON
fi
