#Requires -Version 5.1
<#
.SYNOPSIS
    Start the DollarBill trading bot, loading credentials from a .env file.

.DESCRIPTION
    Reads KEY=VALUE pairs from .env (in the same directory as this script),
    exports them as environment variables, then launches the bot binary.

    Designed to be called by Windows Task Scheduler or run manually.

.EXAMPLE
    # Dry-run (no real orders)
    .\scripts\start_bot.ps1 -DryRun

    # Live mode (default)
    .\scripts\start_bot.ps1
#>

param(
    [switch]$DryRun,
    [string]$LogFile = ""
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

# ── Resolve paths ─────────────────────────────────────────────────────────────
$Root    = Split-Path -Parent $PSScriptRoot        # repo root (parent of scripts/)
$EnvFile = Join-Path $Root ".env"
$Binary  = Join-Path $Root "target\release\dollarbill.exe"

if (-not (Test-Path $Binary)) {
    Write-Error "Binary not found: $Binary`nRun 'cargo build --release' first."
    exit 1
}

# ── Load .env ─────────────────────────────────────────────────────────────────
if (Test-Path $EnvFile) {
    Get-Content $EnvFile | Where-Object { $_ -match "^\s*[^#]\w+=.+" } | ForEach-Object {
        $parts = $_ -split "=", 2
        $key   = $parts[0].Trim()
        $value = $parts[1].Trim()
        [System.Environment]::SetEnvironmentVariable($key, $value, "Process")
    }
    Write-Host "[start_bot] Loaded credentials from $EnvFile"
} else {
    Write-Warning "[start_bot] .env not found at $EnvFile — relying on existing environment variables."
}

# ── Validate required credentials ─────────────────────────────────────────────
foreach ($var in @("ALPACA_API_KEY", "ALPACA_API_SECRET")) {
    if (-not [System.Environment]::GetEnvironmentVariable($var, "Process")) {
        Write-Error "Required environment variable '$var' is not set.`nCreate a .env file (see .env.example)."
        exit 1
    }
}

# ── Build command ──────────────────────────────────────────────────────────────
$Mode = if ($DryRun) { "--dry-run" } else { "--live" }
$Cmd  = @($Binary, "trade", $Mode)

# ── Optional log file ─────────────────────────────────────────────────────────
if ($LogFile -eq "") {
    $LogDir  = Join-Path $Root "data\logs"
    if (-not (Test-Path $LogDir)) { New-Item -ItemType Directory -Path $LogDir | Out-Null }
    $LogFile = Join-Path $LogDir ("bot_" + (Get-Date -Format "yyyyMMdd_HHmmss") + ".log")
}

Write-Host "[start_bot] Starting: $($Cmd -join ' ')"
Write-Host "[start_bot] Log:      $LogFile"
Write-Host "[start_bot] Press Ctrl+C to stop (the bot will cancel open orders before exit)."
Write-Host ""

# ── Launch ─────────────────────────────────────────────────────────────────────
# Tee output to both console and log file.
& $Cmd[0] $Cmd[1..($Cmd.Length - 1)] 2>&1 | Tee-Object -FilePath $LogFile

exit $LASTEXITCODE
