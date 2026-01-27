@echo off
echo Setting up Python environment for DollarBill...

REM Check if virtual environment exists
if not exist ".venv" (
    echo Creating Python virtual environment...
    python -m venv .venv
    if errorlevel 1 (
        echo ERROR: Failed to create virtual environment
        echo Make sure Python is installed and in PATH
        pause
        exit /b 1
    )
)

REM Activate virtual environment
echo Activating virtual environment...
call .venv\Scripts\activate.bat

REM Upgrade pip
echo Upgrading pip...
python -m pip install --upgrade pip

REM Install required packages
echo Installing required Python packages...
pip install yfinance pandas plotly scikit-learn tensorflow numpy matplotlib seaborn

if errorlevel 1 (
    echo WARNING: Some packages failed to install
    echo Continuing with basic packages...
    pip install yfinance pandas plotly numpy
)

echo.
echo Python environment setup complete!
echo.
echo Installed packages:
pip list

echo.
echo To use Python scripts, run: call .venv\Scripts\activate.bat
pause