#!/usr/bin/env bash
# Deterministically select the next trusted, ready-to-implement issue.
#
# Same trust + priority + age logic as .claude/commands/implement-next.md §1.
# This is convenience, NOT a security gate (see SKILL.md security model): trusted
# authors are write collaborators, fetched live from the GitHub API.
#
# Output (stdout): "<number> <complexity-label>"  e.g. "42 complexity:sonnet"
#   — or nothing if no trusted, ready, complexity-labelled issue exists.
# With an argument: hard-verify THAT issue against every rule; any failure aborts (exit 1).
set -euo pipefail

repo=$(gh repo view --json nameWithOwner -q .nameWithOwner)

# Trusted authors = collaborators with WRITE (push) access — read-only/outside
# collaborators excluded. Fetched live (same boundary as ready-author-guard.yml),
# lowercased + space-padded for a case-insensitive substring match.
collabs_lc=" $(gh api --paginate "repos/$repo/collaborators?permission=push" \
  --jq '.[].login' | tr '[:upper:]' '[:lower:]' | tr '\n' ' ') "
is_trusted() { case "$collabs_lc" in *" $(printf '%s' "$1" | tr '[:upper:]' '[:lower:]') "*) return 0;; *) return 1;; esac; }

# Print "<number> <complexity-label>" for an issue, given its labels CSV.
complexity_of() {
  printf '%s' "$1" | tr ',' '\n' | grep -m1 '^complexity:' || true
}

ARG="${1:-}"
if [ -n "$ARG" ]; then
  # Targeted: hard-verify the named issue against every rule. Any failure aborts.
  # Fetch first so a missing issue / API error reports itself rather than being
  # misread downstream as "author lacks write access".
  view=$(gh issue view "$ARG" --json author,labels \
    --jq '"\(.author.login) \([.labels[].name] | join(","))"') \
    || { echo "REJECTED #$ARG: cannot read issue (no such issue, or gh/API error)." >&2; exit 1; }
  read -r login names <<< "$view"
  is_trusted "$login" || { echo "REJECTED #$ARG: author @$login lacks write access." >&2; exit 1; }
  case ",$names," in *",status:ready,"*) : ;; *) echo "REJECTED #$ARG: not status:ready." >&2; exit 1;; esac
  case ",$names," in *",status:in-progress,"*) echo "REJECTED #$ARG: already in progress." >&2; exit 1;; esac
  case ",$names," in *",complexity:"*) : ;; *) echo "REJECTED #$ARG: no complexity label." >&2; exit 1;; esac
  echo "$ARG $(complexity_of "$names")"
  exit 0
fi

# Ready issues with a complexity label and not already claimed, ordered priority
# then oldest, as "number login complexity" lines:
candidates=$(gh issue list --state open --label "status:ready" \
  --json number,author,labels,createdAt --jq '
    map({number, login: .author.login, created: .createdAt, names: [.labels[].name]})
    | map(select((.names | index("status:in-progress") | not)
                 and (.names | map(startswith("complexity:")) | any)))
    | map(.prank = ({"priority:critical":0,"priority:high":1,"priority:medium":2,"priority:low":3}
                    [([.names[] | select(startswith("priority:"))][0]) // "priority:medium"] // 2))
    | map(.cx = ([.names[] | select(startswith("complexity:"))][0]))
    | sort_by(.prank, .created) | .[] | "\(.number) \(.login) \(.cx)"')

# First candidate whose author is a write collaborator:
while read -r num login cx; do
  [ -z "${num:-}" ] && continue
  if is_trusted "$login"; then echo "$num $cx"; exit 0; fi
done <<< "$candidates"

# Nothing eligible — print nothing, exit 0.
exit 0
