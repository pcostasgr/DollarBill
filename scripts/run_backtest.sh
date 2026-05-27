#!/usr/bin/env bash
# Backtesting pipeline — Black-Scholes strategy backtest
# Equivalent to scripts/run_backtest.ps1
# Usage: ./scripts/run_backtest.sh

set -euo pipefail

cd "$(dirname "$0")/.."

echo "================================"
echo "OPTIONS BACKTESTING FRAMEWORK"
echo "================================"
echo ""

cargo run --release --example backtest_strategy

echo ""
echo "Backtest complete!"
