# DollarBill Release Builder
Write-Host ""
Write-Host "============================================================"
Write-Host "  DollarBill Release Builder"
Write-Host "  Pre-compile all examples for fast execution"
Write-Host "============================================================"
Write-Host ""

Write-Host "Building all examples in release mode..."
Write-Host "This will create optimized binaries for fast execution"
Write-Host ""

try {
    cargo build --release --examples
    Write-Host ""
    Write-Host "Release build completed successfully!"
    Write-Host ""
    Write-Host "Binaries created:"
    Write-Host "  target\release\examples\multi_symbol_signals.exe"
    Write-Host "  target\release\examples\paper_trading.exe"
    Write-Host "  target\release\examples\trading_bot.exe"
    Write-Host "  target\release\examples\backtest_strategy.exe"
    Write-Host "  target\release\examples\vol_surface_analysis.exe"
    Write-Host "  target\release\examples\calibrate_live_options.exe"
    Write-Host ""
    Write-Host "You can now run the pipeline quickly with:"
    Write-Host "  .\scripts\run_release_pipeline.ps1"
    Write-Host ""
} catch {
    Write-Host "Error during build: $_"
    exit 1
}

Read-Host "Press Enter to exit"