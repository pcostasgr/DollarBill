# Run Heston Stochastic Volatility Backtesting
# Advanced options strategy testing with realistic pricing

Write-Host 'DollarBill - Heston Backtesting Pipeline' -ForegroundColor Cyan
Write-Host '==============================================' -ForegroundColor Cyan
Write-Host ''

# Check if we're in the right directory
if (!(Test-Path 'Cargo.toml')) {
    Write-Host 'Error: Please run this script from the DollarBill project root directory' -ForegroundColor Red
    exit 1
}

Write-Host 'Step 1: Calibrating Heston parameters to live market data...' -ForegroundColor Yellow
Write-Host '   This fits kappa, theta, sigma, rho, v0 parameters to current options prices' -ForegroundColor Gray
Write-Host ''

try {
    cargo run --release --example calibrate_live_options
} catch {
    Write-Host 'Heston calibration failed. Check your internet connection and API keys.' -ForegroundColor Red
    exit 1
}

Write-Host ''
Write-Host 'Step 2: Running Heston backtesting...' -ForegroundColor Yellow
Write-Host '   Testing momentum-based options strategies with stochastic volatility' -ForegroundColor Gray
Write-Host ''

try {
    cargo run --release --example backtest_heston
} catch {
    Write-Host 'Heston backtesting failed. Check the error messages above.' -ForegroundColor Red
    exit 1
}

Write-Host ''
Write-Host 'Heston backtesting complete!' -ForegroundColor Green
Write-Host ''
Write-Host 'What just happened:' -ForegroundColor Cyan
Write-Host '   1. Calibrated Heston parameters to live market options' -ForegroundColor White
Write-Host '   2. Backtested momentum strategies using realistic option pricing' -ForegroundColor White
Write-Host '   3. Generated performance metrics for short/medium/long-term horizons' -ForegroundColor White
Write-Host ''
Write-Host 'Key advantages of Heston backtesting:' -ForegroundColor Cyan
Write-Host '   - Captures volatility smiles and skews' -ForegroundColor White
Write-Host '   - More realistic P&L than Black-Scholes' -ForegroundColor White
Write-Host '   - Professional-grade pricing model' -ForegroundColor White
Write-Host '   - Better edge detection for options trading' -ForegroundColor White
Write-Host ''
Write-Host 'Compare with Black-Scholes backtesting:' -ForegroundColor Yellow
Write-Host '   cargo run --release --example backtest_strategy' -ForegroundColor White
Write-Host ''
Write-Host 'Next steps:' -ForegroundColor Green
Write-Host '   - Review the backtest results above' -ForegroundColor White
Write-Host '   - Check docs/backtesting-guide.md for methodology' -ForegroundColor White
Write-Host '   - Consider paper trading the best-performing strategy' -ForegroundColor White