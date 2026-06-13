#!/usr/bin/env bash
# Point git at the committed hooks in scripts/hooks. Run once per clone:
#   ./scripts/install-hooks.sh
set -euo pipefail
git config core.hooksPath scripts/hooks
chmod +x scripts/hooks/* 2>/dev/null || true
echo "hooks installed: core.hooksPath -> scripts/hooks"
