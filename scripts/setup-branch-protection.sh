#!/usr/bin/env bash
# Configure GitHub for trunk-based development with coverage-gated auto-merge.
#
# These are *repo settings*, not files, so they can't be committed — run this
# once (needs `gh` authenticated: `gh auth login`). Re-running is idempotent.
#
#   ./scripts/setup-branch-protection.sh [owner/repo]
#
# After this, the flow is: branch off trunk -> open PR -> CI runs -> the PR
# merges itself once all required checks pass (see .github/workflows/automerge.yml).
set -euo pipefail

REPO="${1:-TimmyMC/PlanBan}"
echo "Configuring $REPO ..."

# 1. Repo-level merge settings: enable native auto-merge, squash-only, and
#    auto-delete merged branches (keeps the short-lived-branch model tidy).
gh api -X PATCH "repos/$REPO" \
  -F allow_auto_merge=true \
  -F allow_squash_merge=true \
  -F allow_merge_commit=false \
  -F allow_rebase_merge=false \
  -F delete_branch_on_merge=true >/dev/null
echo "  - auto-merge + squash-only + auto-delete branches: on"

# 2. Branch protection on trunk. The `contexts` MUST match the CI job `name:`
#    fields in .github/workflows/ci.yml — these are the gates auto-merge waits
#    on. `strict` requires the branch be up to date with trunk before merging.
#    No required reviews: this is a solo repo, and a required review with no
#    second approver would deadlock auto-merge — checks alone are the gate.
gh api -X PUT "repos/$REPO/branches/trunk/protection" --input - >/dev/null <<'JSON'
{
  "required_status_checks": {
    "strict": true,
    "contexts": [
      "Lint & test",
      "Coverage",
      "Frontend",
      "Desktop app"
    ]
  },
  "enforce_admins": false,
  "required_pull_request_reviews": null,
  "restrictions": null,
  "required_linear_history": true,
  "allow_force_pushes": false,
  "allow_deletions": false
}
JSON
echo "  - trunk protected: PR + 3 required checks, strict, linear history"
echo "Done. New work: branch -> PR -> auto-merges when green."
