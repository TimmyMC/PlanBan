#!/usr/bin/env bash
# Generic session-budget pre-flight, reusable by any agent routine/action.
#
# Reads the real Claude Code plan usage that the statusline sees — the 5-hour
# session window and the 7-day (weekly) window — and short-circuits before a
# routine starts work if there isn't enough budget left.
#
# Source of truth: Claude Code delivers `rate_limits` ONLY as stdin JSON to the
# statusline command (no CLI flag, no standalone file). The statusline persists a
# snapshot to ~/.claude/rate-limit-snapshot.json on every refresh; this script
# reads that snapshot. It FAILS CLOSED when the snapshot is missing or stale so a
# routine never starts blind.
#
# Dependency: `node` (already required by the project; no standalone jq needed).
#
# Exit codes:
#   0  enough budget        (prints "5h: N% · 7d: M% · ok" on stderr)
#   1  5-hour window over the threshold
#   2  7-day (weekly) window over the threshold
#   3  snapshot missing / unreadable / stale  (fail-closed)
#   4  bad usage / missing dependency
#
# All human-readable output goes to stderr; exit code is the machine signal.
set -euo pipefail

max_5h=85
max_7d=90
max_stale_min=20
snapshot="${HOME}/.claude/rate-limit-snapshot.json"

while [ $# -gt 0 ]; do
  case "$1" in
    --max-5h-pct)    max_5h="${2:?--max-5h-pct needs a value}"; shift 2 ;;
    --max-7d-pct)    max_7d="${2:?--max-7d-pct needs a value}"; shift 2 ;;
    --max-stale-min) max_stale_min="${2:?--max-stale-min needs a value}"; shift 2 ;;
    --snapshot)      snapshot="${2:?--snapshot needs a value}"; shift 2 ;;
    -h|--help)
      cat >&2 <<'USAGE'
check-session-budget.sh — generic Claude Code plan-budget pre-flight
  --max-5h-pct N      fail (exit 1) if 5-hour window used% >= N   (default 85)
  --max-7d-pct N      fail (exit 2) if weekly window used% >= N    (default 90)
  --max-stale-min N   fail-closed (exit 3) if snapshot older than N min (default 20)
  --snapshot PATH     snapshot file (default ~/.claude/rate-limit-snapshot.json)
exit: 0 ok · 1 5h over · 2 7d over · 3 missing/stale · 4 bad usage
USAGE
      exit 0 ;;
    *) echo "check-session-budget: unknown arg '$1'" >&2; exit 4 ;;
  esac
done

command -v node >/dev/null 2>&1 || { echo "check-session-budget: node not found on PATH" >&2; exit 4; }

[ -r "$snapshot" ] || {
  echo "BUDGET: snapshot missing — open an interactive Claude Code turn to refresh it ($snapshot)" >&2
  exit 3
}

# Pull the fields in one pass via node; missing values print as the literal "null"
# so the bash logic below can detect them. Order: capturedAt 5h% 7d% 5hReset 7dReset.
fields=$(node -e '
  try {
    const d = JSON.parse(require("fs").readFileSync(process.argv[1], "utf8"));
    const g = v => (v === undefined || v === null ? "null" : v);
    process.stdout.write([g(d.capturedAt),
      g(d.five_hour && d.five_hour.used_percentage), g(d.seven_day && d.seven_day.used_percentage),
      g(d.five_hour && d.five_hour.resets_at),       g(d.seven_day && d.seven_day.resets_at)].join(" "));
  } catch (e) { process.exit(7); }
' "$snapshot") || {
  echo "BUDGET: snapshot unreadable / malformed — refresh it ($snapshot)" >&2
  exit 3
}
read -r captured five7 seven7 five_reset seven_reset <<< "$fields"

now=$(date +%s)

# Staleness: a snapshot with no/invalid capturedAt, or older than the window, fails closed.
case "$captured" in
  ''|null|*[!0-9]*)
    echo "BUDGET: snapshot has no capture time — refresh it ($snapshot)" >&2
    exit 3 ;;
esac
age_min=$(( (now - captured) / 60 ))
if [ "$age_min" -gt "$max_stale_min" ]; then
  echo "BUDGET: snapshot is ${age_min}m old (> ${max_stale_min}m) — open an interactive Claude Code turn to refresh it" >&2
  exit 3
fi

# Both windows must carry a usable number; absence = can't confirm = fail closed.
for v in "$five7" "$seven7"; do
  case "$v" in
    ''|null) echo "BUDGET: rate_limits not populated yet (Pro/Max, after first API response) — refresh it" >&2; exit 3 ;;
  esac
done

# Round to whole percents for comparison and display (values arrive as e.g. 23.5).
to_int() { awk -v x="$1" 'BEGIN{printf("%d", x+0.5)}'; }
fmt_reset() {
  case "$1" in ''|null|*[!0-9]*) echo "unknown" ;; *) date -d "@$1" '+%Y-%m-%d %H:%M' 2>/dev/null || echo "@$1" ;; esac
}
five_i=$(to_int "$five7")
seven_i=$(to_int "$seven7")

if [ "$five_i" -ge "$max_5h" ]; then
  echo "BUDGET: 5h window ${five_i}% used (>= ${max_5h}%) — resets $(fmt_reset "$five_reset"). Skipping run." >&2
  exit 1
fi
if [ "$seven_i" -ge "$max_7d" ]; then
  echo "BUDGET: 7d window ${seven_i}% used (>= ${max_7d}%) — resets $(fmt_reset "$seven_reset"). Skipping run." >&2
  exit 2
fi

echo "5h: ${five_i}% · 7d: ${seven_i}% · ok" >&2
exit 0
