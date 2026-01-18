# DollarBill Release Build and Pipeline Runner
Write-Host ""
Write-Host "============================================================"
Write-Host "  DollarBill Release Build + Pipeline Runner"
Write-Host "  Fast execution with pre-compiled binaries"
Write-Host "============================================================"
Write-Host ""

# Step 1: Build release version
Write-Host "============================================================"
Write-Host "Step 1: Building Release Version"
Write-Host "============================================================"
Write-Host ""

Write-Host "Building Rust project in release mode..."
Write-Host "This may take a few minutes..."
Write-Host ""

try {
    cargo build --release
    Write-Host "Release build completed successfully!"
} catch {
    Write-Host "Error during build: $_"
    exit 1
}

Write-Host ""

# Check if binaries exist
$multiSignalsBinary = "target\release\examples\multi_symbol_signals.exe"
$paperTradingBinary = "target\release\examples\paper_trading.exe"

if (-not (Test-Path $multiSignalsBinary)) {
    Write-Host "Error: multi_symbol_signals binary not found at $multiSignalsBinary"
    exit 1
}

if (-not (Test-Path $paperTradingBinary)) {
    Write-Host "Error: paper_trading binary not found at $paperTradingBinary"
    exit 1
}

Write-Host "✓ All required binaries found"
Write-Host ""

# Step 2: Activate Python environment
Write-Host "============================================================"
Write-Host "Step 2: Setting up Python Environment"
Write-Host "============================================================"
Write-Host ""

$venvPath = ".\.venv\Scripts\Activate.ps1"
if (Test-Path $venvPath) {
    Write-Host "Activating Python virtual environment..."
    & $venvPath
    Write-Host "Virtual environment activated"
    Write-Host ""
} else {
    Write-Host "Virtual environment not found at $venvPath"
    Write-Host "Proceeding without activation..."
    Write-Host ""
}

# Step 3: Fetch historical stock data
Write-Host "============================================================"
Write-Host "Step 3: Fetching Historical Stock Data"
Write-Host "============================================================"
Write-Host ""

try {
    Write-Host "Running fetch_multi_stocks.py..."
    python py/fetch_multi_stocks.py
    Write-Host "Stock data fetch completed"
} catch {
    Write-Host "Error fetching stock data: $_"
    Read-Host "Press Enter to continue anyway"
}

Write-Host ""

# Step 4: Fetch live options data
Write-Host "============================================================"
Write-Host "Step 4: Fetching Live Options Data"
Write-Host "============================================================"
Write-Host ""

try {
    Write-Host "Running fetch_multi_options.py..."
    python py/fetch_multi_options.py
    Write-Host "Options data fetch completed"
} catch {
    Write-Host "Error fetching options data: $_"
    Read-Host "Press Enter to continue anyway"
}

Write-Host ""

# Step 5: Generate trade signals (includes Heston calibration)
Write-Host "============================================================"
Write-Host "Step 5: Generating Trade Signals (with Calibration)"
Write-Host "============================================================"
Write-Host ""

try {
    Write-Host "Running pre-compiled multi_symbol_signals..."
    Write-Host "This will calibrate Heston models and generate trade signals..."
    Write-Host ""
    & $multiSignalsBinary
    Write-Host "Trade signals generated"
} catch {
    Write-Host "Error generating signals: $_"
    Read-Host "Press Enter to continue anyway"
}

Write-Host ""

# Step 6: Paper trading
Write-Host "============================================================"
Write-Host "Step 6: Paper Trading"
Write-Host "============================================================"
Write-Host ""

# Check for API keys
if (-not $env:ALPACA_API_KEY -or -not $env:ALPACA_API_SECRET) {
    Write-Host "Alpaca API keys not set in environment variables"
    Write-Host ""
    Write-Host "Please set your Alpaca paper trading API credentials:"
    Write-Host "Example:"
    Write-Host '$env:ALPACA_API_KEY="your-api-key"'
    Write-Host '$env:ALPACA_API_SECRET="your-api-secret"'
    Write-Host ""

    $setKeys = Read-Host "Do you want to set them now? (y/n)"
    if ($setKeys -eq "y" -or $setKeys -eq "Y") {
        $apiKey = Read-Host "Enter your Alpaca API Key"
        $apiSecret = Read-Host "Enter your Alpaca API Secret"

        $env:ALPACA_API_KEY = $apiKey
        $env:ALPACA_API_SECRET = $apiSecret

        Write-Host "API keys set for this session"
    } else {
        Write-Host "Skipping paper trading - API keys required"
        Write-Host ""
        Write-Host "Pipeline completed (data fetch and signal generation only)"
        Read-Host "Press Enter to exit"
        exit
    }
}

Write-Host ""
Write-Host "Running paper trading with Alpaca..."
Write-Host "This will execute trades based on the generated signals..."
Write-Host ""

try {
    & $paperTradingBinary
    Write-Host "Paper trading completed"
} catch {
    Write-Host "Error during paper trading: $_"
}

Write-Host ""
Write-Host "============================================================"
Write-Host "  Pipeline Complete!"
Write-Host "============================================================"
Write-Host ""
Write-Host "Summary:"
Write-Host "Built release binaries"
Write-Host "Fetched historical stock data"
Write-Host "Fetched live options data"
Write-Host "Calibrated Heston models"
Write-Host "Generated trade signals with Greeks"
Write-Host "Executed paper trades (if API keys provided)"
Write-Host ""

Read-Host "Press Enter to exit"