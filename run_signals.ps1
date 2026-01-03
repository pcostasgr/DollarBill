# PowerShell script to run options signal generator
# Usage: .\run_signals.ps1

Write-Host "================================================" -ForegroundColor Cyan
Write-Host "TSLA OPTIONS SIGNAL GENERATOR" -ForegroundColor Cyan
Write-Host "================================================" -ForegroundColor Cyan
Write-Host ""

Write-Host "Step 1: Fetching live options data from Yahoo Finance..." -ForegroundColor Yellow
python fetch_options.py

if ($LASTEXITCODE -ne 0) {
    Write-Host "Error: Failed to fetch options data" -ForegroundColor Red
    exit 1
}

Write-Host ""
Write-Host "Step 2: Running Heston calibration and generating signals..." -ForegroundColor Yellow
cargo run --example trade_signals --release

Write-Host ""
Write-Host "================================================" -ForegroundColor Cyan
Write-Host "Done! Check signals above." -ForegroundColor Green
Write-Host "================================================" -ForegroundColor Cyan
