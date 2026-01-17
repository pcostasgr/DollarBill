@echo off
REM Run multi-symbol trade signals example
echo.
echo ============================================================
echo  Multi-Symbol Trade Signal Generator
echo  Processing multiple symbols in parallel
echo ============================================================
echo.

cargo run --release --example multi_symbol_signals

echo.
echo Execution complete.
pause
