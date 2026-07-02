#!/usr/bin/env pwsh
# Provision a realistic demo wipe board for manual UI / daemon testing.
# Usage: pwsh tests/mock-projects/seed.ps1 [-Dir <path>] [-Serve]
[CmdletBinding()]
param(
    [string]$Dir,
    [switch]$Serve
)
$ErrorActionPreference = "Stop"
$repo = (Resolve-Path "$PSScriptRoot/../..").Path
$wipe = Join-Path $repo "target/debug/wipe.exe"
if (-not (Test-Path $wipe)) { $wipe = Join-Path $repo "target/debug/wipe" }
if (-not (Test-Path $wipe)) { throw "build first: cargo build" }

if (-not $Dir) { $Dir = Join-Path $repo "tests\sandbox\demo" }
if (Test-Path $Dir) { Remove-Item -Recurse -Force $Dir }
New-Item -ItemType Directory -Force $Dir | Out-Null

Push-Location $Dir
try {
    git init -q; git config user.email "demo@wipe.dev"; git config user.name "Demo"
    & $wipe init . --name "Payments Service" | Out-Null

    & $wipe label create backend --color "#3b82f6" | Out-Null
    & $wipe label create urgent --color "#ef4444" | Out-Null

    & $wipe ticket create --title "Design ledger schema" --type spec --priority high --list backlog | Out-Null
    & $wipe ticket create --title "Implement /charge endpoint" --type feature --priority high --list todo --label backend | Out-Null
    & $wipe ticket create --title "Add idempotency keys" --type feature --list todo --label backend | Out-Null
    & $wipe ticket create --title "Fix rounding in refunds" --type bug --priority urgent --list in-progress --label urgent --label backend | Out-Null
    & $wipe ticket create --title "Set up CI" --type chore --list done | Out-Null
    git add -A; git commit -q -m "chore: seed initial board" | Out-Null

    & $wipe comment add T-4 --body "Repro: refund of 0.10 loses a cent due to float math." | Out-Null
    & $wipe ticket move T-2 --to in-progress | Out-Null
    git add -A; git commit -q -m "feat: start /charge endpoint, note refund bug" | Out-Null

    Write-Host "Seeded demo board at: $Dir"
    & $wipe -C $Dir status
}
finally { Pop-Location }

if ($Serve) { & $wipe -C $Dir serve }
