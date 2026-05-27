#!/usr/bin/env bash
# Set up the Python virtual environment and install dependencies
# Equivalent to scripts/setup_python.bat
# Usage: ./scripts/setup_python.sh

set -euo pipefail

cd "$(dirname "$0")/.."

echo ""
echo "============================================================"
echo "  DollarBill Python Environment Setup"
echo "============================================================"
echo ""

if ! command -v python3 &>/dev/null; then
    echo "Error: python3 not found. Install it via your package manager." >&2
    echo "  Ubuntu/Debian: sudo apt install python3 python3-venv python3-pip" >&2
    echo "  Fedora/RHEL:   sudo dnf install python3" >&2
    echo "  macOS:         brew install python3  (or use the installer from python.org)" >&2
    exit 1
fi

echo "Python version: $(python3 --version)"
echo ""

# Create venv if it doesn't exist
if [[ ! -d .venv ]]; then
    echo "Creating virtual environment at .venv ..."
    python3 -m venv .venv
    echo "Virtual environment created."
else
    echo "Virtual environment already exists at .venv — skipping creation."
fi
echo ""

# Activate
# shellcheck disable=SC1091
source .venv/bin/activate

echo "Installing required packages..."
pip install --upgrade pip --quiet
pip install yfinance pandas plotly

echo ""
echo "Python environment ready."
echo ""
echo "To activate in your shell:"
echo "  source .venv/bin/activate"
echo ""
echo "Installed packages:"
pip show yfinance pandas plotly | grep -E "^(Name|Version):"
echo ""
