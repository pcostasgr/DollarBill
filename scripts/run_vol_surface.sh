#!/usr/bin/env bash
# Volatility surface analysis and visualisation pipeline
# Equivalent to scripts/run_vol_surface.ps1
# Usage: ./scripts/run_vol_surface.sh

set -euo pipefail

cd "$(dirname "$0")/.."

echo ""
echo "============================================================"
echo "  Volatility Surface Analysis Pipeline"
echo "============================================================"
echo ""

echo "Step 1: Extracting volatility surfaces from options data..."
cargo run --release --example vol_surface_analysis

echo ""
echo "Step 2: Generating interactive visualisations..."
python py/plot_vol_surface.py

echo ""
echo "============================================================"
echo "Complete! Open the HTML files in your browser:"
echo "  tsla_vol_surface_3d.html"
echo "  tsla_vol_smile.html"
echo "  tsla_term_structure.html"
echo "  (and similar for other symbols)"
echo "============================================================"
echo ""
