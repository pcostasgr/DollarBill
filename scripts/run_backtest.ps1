# Run backtesting framework - tests multiple strategies on historical data
# Compares performance metrics across different approaches

Write-Host "================================" -ForegroundColor Cyan
Write-Host "OPTIONS BACKTESTING FRAMEWORK" -ForegroundColor Cyan
Write-Host "================================" -ForegroundColor Cyan
Write-Host ""

cargo run --release --example backtest_strategy

Write-Host ""
Write-Host "Backtest complete!" -ForegroundColor Green
