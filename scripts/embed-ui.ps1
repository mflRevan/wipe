#!/usr/bin/env pwsh
# Build the desktop UI and stage it into the daemon's embed directory so
# `cargo build` bakes the real board UI into the `wipe` binary.
[CmdletBinding()]
param([switch]$SkipInstall)
$ErrorActionPreference = "Stop"

$repo = (Resolve-Path "$PSScriptRoot/..").Path
$ui = Join-Path $repo "apps/desktop"
$assets = Join-Path $repo "crates/wipe-daemon/assets"

if (-not (Test-Path (Join-Path $ui "package.json"))) {
    Write-Warning "apps/desktop not present yet; nothing to embed."
    exit 0
}

Push-Location $ui
try {
    if (-not $SkipInstall) { pnpm install }
    pnpm build
}
finally { Pop-Location }

$build = Join-Path $ui "build"
if (-not (Test-Path $build)) { throw "expected SvelteKit static output at $build" }

# Clear staged assets (keep .gitkeep) and copy the fresh build in.
Get-ChildItem $assets -Force | Where-Object { $_.Name -ne ".gitkeep" } | Remove-Item -Recurse -Force
Copy-Item (Join-Path $build "*") $assets -Recurse -Force
Write-Host "Embedded desktop UI into $assets"
