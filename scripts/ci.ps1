#!/usr/bin/env pwsh
# Local CI — the exact gate that .github/workflows/ci.yml enforces.
# Run before pushing:  ./scripts/ci.ps1
$ErrorActionPreference = 'Stop'

function Step($name, $block) {
    Write-Host "== $name ==" -ForegroundColor Cyan
    & $block
    if ($LASTEXITCODE -ne 0) {
        Write-Host "FAILED: $name (exit $LASTEXITCODE)" -ForegroundColor Red
        exit $LASTEXITCODE
    }
}

Step 'fmt'    { cargo fmt --all -- --check }
Step 'clippy' { cargo clippy --workspace --all-targets -- -D warnings }
Step 'test'   { cargo test --workspace }

Write-Host 'CI OK' -ForegroundColor Green
