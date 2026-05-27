#!/usr/bin/env bash
# Heston preparation: data fetch -> Heston backtest -> personality models
# Equivalent to scripts/heston_preparation.ps1
# Usage: ./scripts/heston_preparation.sh

set -euo pipefail

cd "$(dirname "$0")/.."

echo "=========================================="
echo "  DollarBill - Heston Preparation"
echo "  Data -> Heston -> Personality Models"
echo "=========================================="
echo ""

if [[ ! -f Cargo.toml ]]; then
    echo "Error: Run this script from the DollarBill project root." >&2
    exit 1
fi

# Activate venv if present
if [[ -f .venv/bin/activate ]]; then
    echo "Activating Python virtual environment..."
    # shellcheck disable=SC1091
    source .venv/bin/activate
    echo "Virtual environment activated."
    echo ""
else
    echo "No .venv found — proceeding without activation."
    echo "  Consider: python -m venv .venv && source .venv/bin/activate && pip install yfinance pandas"
    echo ""
fi

# ── Step 1: Fetch historical stock data ───────────────────────────────────────
echo "=========================================="
echo "Step 1: Fetching Historical Stock Data"
echo "=========================================="
echo ""

python py/fetch_multi_stocks.py
echo "Historical stock data fetched."
echo ""

# ── Step 2: Fetch live options data ───────────────────────────────────────────
echo "=========================================="
echo "Step 2: Fetching Live Options Data"
echo "=========================================="
echo ""

python py/fetch_multi_options.py
echo "Live options data fetched."
echo ""

# ── Step 3: Heston calibration + backtest ─────────────────────────────────────
echo "=========================================="
echo "Step 3: Heston Calibration + Backtest"
echo "=========================================="
echo ""

echo "Calibrating Heston parameters to live market data..."
cargo run --release --example calibrate_live_options
echo ""

echo "Running Heston backtest..."
cargo run --release --example backtest_heston
echo ""
echo "Heston backtesting complete."
echo ""

# ── Step 4: Enhanced personality analysis ─────────────────────────────────────
echo "=========================================="
echo "Step 4: Enhanced Personality Analysis"
echo "=========================================="
echo ""

cargo run --release --example enhanced_personality_analysis
cargo run --release --example personality_driven_pipeline
echo ""
echo "Personality models trained."
echo ""

echo "=========================================="
echo "Heston preparation complete!"
echo ""
echo "Next steps:"
echo "  Step 5: ./target/release/dollarbill trade --dry-run"
echo "  Step 6: ./scripts/start_bot.sh"
echo "=========================================="
echo ""
