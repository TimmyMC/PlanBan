# Point git at the committed hooks in scripts/hooks. Run once per clone:
#   ./scripts/install-hooks.ps1
$ErrorActionPreference = "Stop"
git config core.hooksPath scripts/hooks
Write-Host "hooks installed: core.hooksPath -> scripts/hooks"
