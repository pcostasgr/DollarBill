#!/usr/bin/env bash
# Build all DollarBill release binaries
# Usage: ./scripts/build_release.sh

set -euo pipefail

cd "$(dirname "$0")/.."

echo ""
echo "============================================================"
echo "  DollarBill Release Builder"
echo "  Pre-compile all examples for fast execution"
echo "============================================================"
echo ""

if ! command -v cargo &>/dev/null; then
    echo "Error: cargo not found. Install Rust from https://rustup.rs" >&2
    exit 1
fi

echo "Building all examples in release mode..."
echo "This will create optimised binaries for fast execution."
echo ""

cargo build --release --examples

echo ""
echo "Release build completed successfully!"
echo ""
echo "Binaries created:"
echo "  target/release/examples/multi_symbol_signals"
echo "  target/release/examples/calibrate_live_options"
echo "  target/release/examples/backtest_strategy"
echo "  target/release/examples/backtest_heston"
echo "  target/release/examples/vol_surface_analysis"
echo "  target/release/examples/personality_based_bot"
echo ""
echo "Run the full pipeline with:"
echo "  ./scripts/run_release_pipeline.sh"
echo ""
