#!/bin/bash
# Run options signal generator pipeline
# Usage: ./run_signals.sh

echo "================================================"
echo "TSLA OPTIONS SIGNAL GENERATOR"
echo "================================================"
echo ""

echo "Step 1: Fetching live options data from Yahoo Finance..."
python py/fetch_options.py

if [ $? -ne 0 ]; then
    echo "Error: Failed to fetch options data"
    exit 1
fi

echo ""
echo "Step 2: Running Heston calibration and generating signals..."
cargo run --example trade_signals --release

echo ""
echo "================================================"
echo "Done! Check signals above."
echo "================================================"
