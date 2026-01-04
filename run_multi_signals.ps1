# Run multi-symbol trade signals example
Write-Host ""
Write-Host "============================================================"
Write-Host "  Multi-Symbol Trade Signal Generator"
Write-Host "  Processing multiple symbols in parallel"
Write-Host "============================================================"
Write-Host ""

cargo run --release --example multi_symbol_signals

Write-Host ""
Write-Host "Execution complete."
Read-Host "Press Enter to exit"
