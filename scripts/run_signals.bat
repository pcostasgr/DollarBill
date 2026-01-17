@echo off
REM Batch file to run options signal generator
REM Usage: run_signals.bat

echo ================================================
echo TSLA OPTIONS SIGNAL GENERATOR
echo ================================================
echo.

echo Step 1: Fetching live options data from Yahoo Finance...
python py/fetch_options.py

if %errorlevel% neq 0 (
    echo Error: Failed to fetch options data
    exit /b 1
)

echo.
echo Step 2: Running Heston calibration and generating signals...
cargo run --example trade_signals --release

echo.
echo ================================================
echo Done! Check signals above.
echo ================================================
