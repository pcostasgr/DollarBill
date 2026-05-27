#!/usr/bin/env bash
# Complete pipeline: data fetch -> calibration -> signals -> paper trading
# Equivalent to scripts/run_full_pipeline.ps1
# Usage: ./scripts/run_full_pipeline.sh

set -uo pipefail   # -e intentionally omitted so steps can continue on failure

cd "$(dirname "$0")/.."

echo ""
echo "============================================================"
echo "  DollarBill Full Pipeline Runner"
echo "  Data -> Calibration -> Signals -> Paper Trading"
echo "============================================================"
echo ""

# Activate venv if present
if [[ -f .venv/bin/activate ]]; then
    echo "Activating Python virtual environment..."
    # shellcheck disable=SC1091
    source .venv/bin/activate
    echo "Virtual environment activated."
    echo ""
fi

# ── Step 1: Historical stock data ─────────────────────────────────────────────
echo "============================================================"
echo "Step 1: Fetching Historical Stock Data"
echo "============================================================"
echo ""

if python py/fetch_multi_stocks.py; then
    echo "Stock data fetch completed."
else
    echo "Warning: stock data fetch failed. Continuing with cached data if available."
fi
echo ""

# ── Step 2: Live options data ─────────────────────────────────────────────────
echo "============================================================"
echo "Step 2: Fetching Live Options Data"
echo "============================================================"
echo ""

if python py/fetch_multi_options.py; then
    echo "Options data fetch completed."
else
    echo "Warning: options data fetch failed. Continuing with cached data if available."
fi
echo ""

# ── Step 3: Trade signals ─────────────────────────────────────────────────────
echo "============================================================"
echo "Step 3: Generating Trade Signals (with Calibration)"
echo "============================================================"
echo ""

if cargo run --release --example multi_symbol_signals; then
    echo "Trade signals generated."
else
    echo "Warning: signal generation failed."
fi
echo ""

# ── Step 4: Paper trading ─────────────────────────────────────────────────────
echo "============================================================"
echo "Step 4: Paper Trading"
echo "============================================================"
echo ""

if [[ -z "${ALPACA_API_KEY:-}" || -z "${ALPACA_API_SECRET:-}" ]]; then
    echo "ALPACA_API_KEY / ALPACA_API_SECRET not set."
    echo "Load them from .env or export them before running:"
    echo "  export ALPACA_API_KEY=your-key"
    echo "  export ALPACA_API_SECRET=your-secret"
    echo ""
    echo "Skipping paper trading. Pipeline completed (data + signals only)."
    exit 0
fi

echo "Running paper trading with Alpaca..."
cargo run --release --example paper_trading

echo ""
echo "============================================================"
echo "  Pipeline Complete!"
echo "============================================================"
echo ""
echo "  Fetched historical stock data"
echo "  Fetched live options data"
echo "  Calibrated Heston models and generated signals"
echo "  Executed paper trades"
echo ""
