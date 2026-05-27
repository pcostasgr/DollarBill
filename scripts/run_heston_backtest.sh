#!/usr/bin/env bash
# Heston backtest pipeline: calibrate -> backtest
# Equivalent to scripts/run_heston_backtest.ps1
# Usage: ./scripts/run_heston_backtest.sh

set -euo pipefail

cd "$(dirname "$0")/.."

echo "DollarBill - Heston Backtesting Pipeline"
echo "=============================================="
echo ""

if [[ ! -f Cargo.toml ]]; then
    echo "Error: Run this script from the DollarBill project root." >&2
    exit 1
fi

echo "Step 1: Calibrating Heston parameters to live market data..."
echo "   Fits kappa, theta, sigma, rho, v0 to current options prices"
echo ""

cargo run --release --example calibrate_live_options

echo ""
echo "Step 2: Running Heston backtesting..."
echo "   Testing momentum-based options strategies with stochastic volatility"
echo ""

cargo run --release --example backtest_heston

echo ""
echo "Heston backtesting complete!"
echo ""
echo "What just happened:"
echo "   1. Calibrated Heston parameters to live market options"
echo "   2. Backtested momentum strategies using realistic option pricing"
echo "   3. Generated performance metrics for short/medium/long-term horizons"
