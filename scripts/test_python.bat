@echo off
echo Testing Python Environment
echo ==========================

REM Activate virtual environment
if exist ".venv\Scripts\activate.bat" (
    call .venv\Scripts\activate.bat
    echo Python environment activated
) else (
    echo Virtual environment not found, using system Python
)

echo.
echo Python version:
python --version

echo.
echo Installed packages:
pip list | findstr "yfinance pandas plotly"

echo.
echo Testing imports:
python -c "import yfinance; import pandas; import json; print('✓ All imports successful')" 2>nul
if errorlevel 1 (
    echo ❌ Import test failed
    echo Installing missing packages...
    pip install yfinance pandas plotly
) else (
    echo ✓ Import test passed
)

echo.
echo Testing Yahoo Finance connection:
python -c "import yfinance as yf; ticker = yf.Ticker('AAPL'); info = ticker.info; print(f'✓ Yahoo Finance working - AAPL price: ${info.get(\"currentPrice\", \"N/A\")}')" 2>nul
if errorlevel 1 (
    echo ❌ Yahoo Finance test failed
) else (
    echo ✓ Yahoo Finance connection working
)

pause