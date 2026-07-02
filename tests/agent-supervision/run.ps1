#!/usr/bin/env pwsh
# Agent-to-agent supervision harness.
# Supervisor seeds a wipe board with a spec ticket; a subordinate opencode agent
# discovers and completes it through the wipe CLI; the supervisor verifies.

[CmdletBinding()]
param(
    [string]$Model = $env:WIPE_SUBAGENT_MODEL,
    [int]$TimeoutSec = 240,
    [switch]$Keep,
    [string]$Work
)

$ErrorActionPreference = "Stop"
if (-not $Model) { $Model = "opencode/deepseek-v4-flash-free" }

# --- locate the wipe binary ------------------------------------------------
$repo = (Resolve-Path "$PSScriptRoot/../..").Path
$wipeExe = Join-Path $repo "target/debug/wipe.exe"
if (-not (Test-Path $wipeExe)) { $wipeExe = Join-Path $repo "target/debug/wipe" }
if (-not (Test-Path $wipeExe)) { throw "wipe binary not found; run 'cargo build' first ($wipeExe)" }
$wipeDir = Split-Path $wipeExe

# --- provision a throwaway git + wipe project ------------------------------
if (-not $Work) { $Work = Join-Path ([System.IO.Path]::GetTempPath()) ("wipe-supervision-" + [System.IO.Path]::GetRandomFileName().Substring(0,8)) }
if (Test-Path $Work) { Remove-Item -Recurse -Force $Work }
New-Item -ItemType Directory -Force $Work | Out-Null

Write-Host "==> work dir: $Work"
Write-Host "==> model:    $Model"

Push-Location $Work
try {
    git init -q
    git config user.email "supervisor@wipe.dev"
    git config user.name "Supervisor"

    & $wipeExe init . --name "Calc Service" | Out-Null
    $body = @"
Create a file calc.py at the repository root that defines a function add(a, b) returning a + b.

Acceptance criteria:
- calc.py exists at the repo root.
- It defines def add(a, b) returning a + b.

When finished: add a comment on this ticket summarizing what you did, then move this ticket to the done list.
"@
    & $wipeExe ticket create --title "Implement add(a, b) in calc.py" --list todo --body $body | Out-Null
    git add -A; git commit -q -m "chore: seed spec board for subordinate agent"
}
finally { Pop-Location }

# --- subordinate agent prompt ----------------------------------------------
$prompt = @"
You are a software worker agent. Your tasks are tracked in a CLI task board called `wipe`.
Coordinate ALL task state through the `wipe` command; never read or edit files under .wipe/ directly.

Do this now, in order:
1. Run: wipe skill        (learn the CLI)
2. Run: wipe ticket list --list todo --json     (find your assigned work)
3. Run: wipe ticket show T-1 --json             (read the spec)
4. Implement exactly what ticket T-1 asks, in this repository.
5. Run: wipe comment add T-1 --body "<one-line summary of what you did>"
6. Run: wipe ticket move T-1 --to done

Keep going until T-1 is in the done list. Be concise.
"@

# --- run the subordinate with a timeout ------------------------------------
Write-Host "==> launching subordinate agent (timeout ${TimeoutSec}s)..."
$job = Start-Job -ScriptBlock {
    param($work, $model, $prompt, $wipeDir)
    $env:PATH = "$wipeDir$([System.IO.Path]::PathSeparator)$env:PATH"
    Set-Location $work
    opencode run --dir $work --model $model --auto $prompt 2>&1
} -ArgumentList $Work, $Model, $prompt, $wipeDir

$completed = Wait-Job $job -Timeout $TimeoutSec
$agentOut = Receive-Job $job -ErrorAction SilentlyContinue
if (-not $completed) { Write-Warning "subordinate timed out after ${TimeoutSec}s"; Stop-Job $job -ErrorAction SilentlyContinue }
Remove-Job $job -Force -ErrorAction SilentlyContinue
$agentOut | Out-File -FilePath (Join-Path $Work "agent-output.log") -Encoding utf8

# --- supervisor verification -----------------------------------------------
$calc = Join-Path $Work "calc.py"
$fileExists = Test-Path $calc
$definesAdd = $fileExists -and ((Get-Content $calc -Raw) -match "def\s+add\s*\(")

$ticketJson = & $wipeExe -C $Work ticket show T-1 --json 2>$null | ConvertFrom-Json
$inDone = $ticketJson.list -eq "done"
$hasComment = ($ticketJson.comments | Measure-Object).Count -ge 1

$pass = $fileExists -and $definesAdd -and $inDone -and $hasComment

$result = [ordered]@{
    model        = $Model
    work         = $Work
    completed    = [bool]$completed
    file_exists  = [bool]$fileExists
    defines_add  = [bool]$definesAdd
    ticket_list  = $ticketJson.list
    in_done      = [bool]$inDone
    has_comment  = [bool]$hasComment
    comments     = @($ticketJson.comments | ForEach-Object { $_.body })
    pass         = [bool]$pass
}
$result | ConvertTo-Json -Depth 5 | Out-File -FilePath (Join-Path $Work "result.json") -Encoding utf8

Write-Host ""
Write-Host "==== supervision result ===="
$result | ConvertTo-Json -Depth 5
Write-Host "============================"
if ($pass) { Write-Host "PASS" -ForegroundColor Green } else { Write-Host "FAIL" -ForegroundColor Red }

if (-not $Keep) { Remove-Item -Recurse -Force $Work -ErrorAction SilentlyContinue }
if (-not $pass) { exit 1 }
