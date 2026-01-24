# DollarBill Heston Preparation Script
# Combines market data fetching, Heston backtesting, and personality model training
# Steps 3-5 from the getting started guide

Write-Host "==========================================" -ForegroundColor Cyan
Write-Host "  DollarBill - Heston Preparation" -ForegroundColor Cyan
Write-Host "  Data -> Heston -> Personality Models" -ForegroundColor Cyan
Write-Host "==========================================" -ForegroundColor Cyan
Write-Host ""

# Check if we're in the right directory
if (!(Test-Path "Cargo.toml")) {
    Write-Host "X Error: Please run this script from the DollarBill project root directory" -ForegroundColor Red
    exit 1
}

# Check if virtual environment exists and activate it
$venvPath = ".\.venv\Scripts\Activate.ps1"
if (Test-Path $venvPath) {
    Write-Host "Activating Python virtual environment..." -ForegroundColor Yellow
    & $venvPath
    Write-Host "Virtual environment activated" -ForegroundColor Green
    Write-Host ""
} else {
    Write-Host "Virtual environment not found, proceeding without activation" -ForegroundColor Yellow
    Write-Host ""
}

# Step 3: Fetch Market Data
Write-Host "==========================================" -ForegroundColor Cyan
Write-Host "Step 3: Fetching Market Data" -ForegroundColor Cyan
Write-Host "==========================================" -ForegroundColor Cyan
Write-Host ""

Write-Host "Fetching historical stock data..." -ForegroundColor Yellow
python py/fetch_multi_stocks.py
if ($LASTEXITCODE -ne 0) {
    Write-Host "X Error fetching historical stock data" -ForegroundColor Red
    exit 1
}
Write-Host "Historical stock data fetched" -ForegroundColor Green
Write-Host ""

Write-Host "Fetching live options data..." -ForegroundColor Yellow
python py/fetch_multi_options.py
if ($LASTEXITCODE -ne 0) {
    Write-Host "X Error fetching live options data" -ForegroundColor Red
    exit 1
}
Write-Host "Live options data fetched" -ForegroundColor Green
Write-Host ""

# Step 4: Run Heston Backtesting
Write-Host "==========================================" -ForegroundColor Cyan
Write-Host "Step 4: Heston Backtesting" -ForegroundColor Cyan
Write-Host "==========================================" -ForegroundColor Cyan
Write-Host ""

Write-Host "Running Heston backtesting..." -ForegroundColor Yellow
Write-Host "   This calibrates parameters to live market data" -ForegroundColor Gray
Write-Host "   and builds the performance matrix for live trading" -ForegroundColor Gray
Write-Host ""

.\scripts\run_heston_backtest.ps1
if ($LASTEXITCODE -ne 0) {
    Write-Host "X Heston backtesting failed" -ForegroundColor Red
    exit 1
}
Write-Host "Heston backtesting complete" -ForegroundColor Green
Write-Host "   Performance matrix updated with realistic data" -ForegroundColor Gray
Write-Host ""

# Step 5: Train Personality Models
Write-Host "==========================================" -ForegroundColor Cyan
Write-Host "Step 5: Training Personality Models" -ForegroundColor Cyan
Write-Host "==========================================" -ForegroundColor Cyan
Write-Host ""

Write-Host "Training personality models..." -ForegroundColor Yellow
Write-Host "   Analyzing stock behaviors and matching optimal strategies" -ForegroundColor Gray
Write-Host ""

cargo run --release --example personality_driven_pipeline
if ($LASTEXITCODE -ne 0) {
    Write-Host "X Personality training failed" -ForegroundColor Red
    exit 1
}
Write-Host "Personality models trained and saved" -ForegroundColor Green
Write-Host ""

# Success Summary
Write-Host "==========================================" -ForegroundColor Green
Write-Host "Heston Preparation Complete!" -ForegroundColor Green
Write-Host "==========================================" -ForegroundColor Green
Write-Host ""
Write-Host "What was accomplished:" -ForegroundColor Cyan
Write-Host "   * Market data fetched and updated" -ForegroundColor White
Write-Host "   * Heston parameters calibrated to live markets" -ForegroundColor White
Write-Host "   * Performance matrix built with realistic data" -ForegroundColor White
Write-Host "   * Personality models trained and optimized" -ForegroundColor White
Write-Host "   * Stock classifier and strategy matcher ready" -ForegroundColor White
Write-Host ""
Write-Host "Next steps:" -ForegroundColor Green
Write-Host "   * Test with: cargo run --example personality_based_bot -- --dry-run" -ForegroundColor White
Write-Host "   * Go live with: cargo run --example personality_based_bot -- --continuous 5" -ForegroundColor White
Write-Host ""
Write-Host "Your personality-driven trading system is now ready!" -ForegroundColor Cyan
Write-Host ""