#!/usr/bin/env bash
# Fast pipeline using pre-built release binaries
# Run build_release.sh first, then this for no recompilation delay.
# Equivalent to scripts/run_release_pipeline.ps1
# Usage: ./scripts/run_release_pipeline.sh

set -uo pipefail

cd "$(dirname "$0")/.."

echo ""
echo "============================================================"
echo "  DollarBill Release Pipeline Runner"
echo "  Fast execution with pre-compiled binaries"
echo "============================================================"
echo ""

# Verify required binaries
MULTI_SIGNALS="target/release/examples/multi_symbol_signals"
DOLLARBILL="target/release/dollarbill"

for bin in "$MULTI_SIGNALS" "$DOLLARBILL"; do
    if [[ ! -x "$bin" ]]; then
        echo "Error: binary not found: $bin" >&2
        echo "Run './scripts/build_release.sh' first." >&2
        exit 1
    fi
done
echo "All required binaries found."
echo ""

# Activate venv if present
if [[ -f .venv/bin/activate ]]; then
    # shellcheck disable=SC1091
    source .venv/bin/activate
fi

# ── Fetch data ────────────────────────────────────────────────────────────────
echo "Fetching historical stock data..."
python py/fetch_multi_stocks.py || echo "Warning: stock fetch failed."
echo ""

echo "Fetching live options data..."
python py/fetch_multi_options.py || echo "Warning: options fetch failed."
echo ""

# ── Signals ───────────────────────────────────────────────────────────────────
echo "Generating trade signals..."
"$MULTI_SIGNALS"
echo ""

# ── Paper trading ─────────────────────────────────────────────────────────────
if [[ -z "${ALPACA_API_KEY:-}" || -z "${ALPACA_API_SECRET:-}" ]]; then
    echo "ALPACA_API_KEY / ALPACA_API_SECRET not set — skipping trading step."
    echo "Load from .env or export them to include paper trading."
else
    echo "Starting dry-run paper trade..."
    "$DOLLARBILL" trade --dry-run
fi

echo ""
echo "============================================================"
echo "  Pipeline complete!"
echo "============================================================"
echo ""
