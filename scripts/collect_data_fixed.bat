@echo off
echo DollarBill Data Collection Pipeline
echo ====================================

REM Activate Python virtual environment
if exist ".venv\Scripts\activate.bat" (
    echo Activating Python virtual environment...
    call .venv\Scripts\activate.bat
) else (
    echo ERROR: Virtual environment not found!
    echo Please run setup_python.bat first
    pause
    exit /b 1
)

REM Check if yfinance is installed
python -c "import yfinance" 2>nul
if errorlevel 1 (
    echo ERROR: yfinance not installed
    echo Installing required packages...
    pip install yfinance pandas plotly
)

echo.
echo Step 1: Fetching historical stock data...
python py\fetch_multi_stocks.py
if errorlevel 1 (
    echo WARNING: Stock data fetch had errors
)

echo.
echo Step 2: Fetching live options data...
python py\fetch_multi_options.py
if errorlevel 1 (
    echo WARNING: Options data fetch had errors
)

echo.
echo Step 3: Running enhanced personality analysis...
cargo run --release --example enhanced_personality_analysis

echo.
echo ====================================
echo Data collection and analysis complete!
echo ====================================
pause