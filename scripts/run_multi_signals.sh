#!/usr/bin/env bash
# Multi-symbol trade signal generator
# Equivalent to scripts/run_multi_signals.ps1
# Usage: ./scripts/run_multi_signals.sh

set -euo pipefail

cd "$(dirname "$0")/.."

echo ""
echo "============================================================"
echo "  Multi-Symbol Trade Signal Generator"
echo "  Processing multiple symbols in parallel"
echo "============================================================"
echo ""

cargo run --release --example multi_symbol_signals

echo ""
echo "Execution complete."
