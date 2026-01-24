@echo off
REM DollarBill Heston Preparation Script
REM Combines market data fetching, Heston backtesting, and personality model training
REM Steps 3-5 from the getting started guide

echo ==========================================
echo   DollarBill - Heston Preparation
echo   Data → Heston → Personality Models
echo ==========================================
echo.

REM Check if we're in the right directory
if not exist "Cargo.toml" (
    echo ❌ Error: Please run this script from the DollarBill project root directory
    pause
    exit /b 1
)

REM Check if virtual environment exists and activate it
if exist ".\.venv\Scripts\activate.bat" (
    echo 🐍 Activating Python virtual environment...
    call .\.venv\Scripts\activate.bat
    echo ✅ Virtual environment activated
    echo.
) else (
    echo ⚠️  Virtual environment not found, proceeding without activation
    echo.
)

REM Step 3: Fetch Market Data
echo ==========================================
echo Step 3: Fetching Market Data
echo ==========================================
echo.

echo 📊 Fetching historical stock data...
python py/fetch_multi_stocks.py
if %errorlevel% neq 0 (
    echo ❌ Error fetching historical stock data
    pause
    exit /b 1
)
echo ✅ Historical stock data fetched
echo.

echo 📈 Fetching live options data...
python py/fetch_multi_options.py
if %errorlevel% neq 0 (
    echo ❌ Error fetching live options data
    pause
    exit /b 1
)
echo ✅ Live options data fetched
echo.

REM Step 4: Run Heston Backtesting
echo ==========================================
echo Step 4: Heston Backtesting
echo ==========================================
echo.

echo 🔬 Running Heston backtesting...
echo    This calibrates parameters to live market data
echo    and builds the performance matrix for live trading
echo.

call scripts\run_heston_backtest.ps1
if %errorlevel% neq 0 (
    echo ❌ Heston backtesting failed
    pause
    exit /b 1
)
echo ✅ Heston backtesting complete
echo    Performance matrix updated with realistic data
echo.

REM Step 5: Train Personality Models
echo ==========================================
echo Step 5: Training Personality Models
echo ==========================================
echo.

echo 🧠 Training personality models...
echo    Analyzing stock behaviors and matching optimal strategies
echo.

cargo run --release --example personality_driven_pipeline
if %errorlevel% neq 0 (
    echo ❌ Personality training failed
    pause
    exit /b 1
)
echo ✅ Personality models trained and saved
echo.

REM Success Summary
echo ==========================================
echo 🎉 Heston Preparation Complete!
echo ==========================================
echo.
echo 📋 What was accomplished:
echo    ✅ Market data fetched and updated
echo    ✅ Heston parameters calibrated to live markets
echo    ✅ Performance matrix built with realistic data
echo    ✅ Personality models trained and optimized
echo    ✅ Stock classifier and strategy matcher ready
echo.
echo 🚀 Next steps:
echo    • Test with: cargo run --example personality_based_bot -- --dry-run
echo    • Go live with: cargo run --example personality_based_bot -- --continuous 5
echo.
echo 💡 Your personality-driven trading system is now ready!
echo.
pause