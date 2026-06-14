#!/usr/bin/env bash
# SessionStart hook: surface the project skill library so every agent session knows
# what self-improvement assets exist and can extend them. Output is injected as
# context. Stays silent (exit 0) if there are no skills yet.
set -euo pipefail

root="$(git rev-parse --show-toplevel 2>/dev/null || pwd)"
dir="$root/.claude/skills"
[ -d "$dir" ] || exit 0

found=0
for skill in "$dir"/*/SKILL.md; do
  [ -f "$skill" ] || continue
  if [ "$found" -eq 0 ]; then
    echo "Project skills (.claude/skills/) — invoke or extend these; capture reusable"
    echo "learnings as new/updated skills via the 'self-improve' skill:"
    found=1
  fi
  name="$(sed -n 's/^name:[[:space:]]*//p' "$skill" | head -1)"
  desc="$(sed -n 's/^description:[[:space:]]*//p' "$skill" | head -1)"
  echo "- ${name:-$(basename "$(dirname "$skill")")}: ${desc}"
done

exit 0
