#!/usr/bin/env bash
# Start the DollarBill trading bot, loading credentials from .env
# Equivalent to scripts/start_bot.ps1
#
# Usage:
#   ./scripts/start_bot.sh            # paper trade (default)
#   ./scripts/start_bot.sh --dry-run  # validate without placing orders
#   ./scripts/start_bot.sh --live     # real money (requires APCA_LIVE=1 in .env)

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
ENV_FILE="$ROOT/.env"
BINARY="$ROOT/target/release/dollarbill"

cd "$ROOT"

# ── Parse arguments ───────────────────────────────────────────────────────────
MODE="--live"
for arg in "$@"; do
    case "$arg" in
        --dry-run) MODE="--dry-run" ;;
        --live)    MODE="--live" ;;
        *) echo "Unknown argument: $arg" >&2; exit 1 ;;
    esac
done

# ── Check binary ──────────────────────────────────────────────────────────────
if [[ ! -x "$BINARY" ]]; then
    echo "Error: binary not found at $BINARY" >&2
    echo "Run 'cargo build --release' first." >&2
    exit 1
fi

# ── Load .env ─────────────────────────────────────────────────────────────────
if [[ -f "$ENV_FILE" ]]; then
    echo "[start_bot] Loading credentials from $ENV_FILE"
    while IFS='=' read -r key value; do
        # Skip comments and blank lines
        [[ "$key" =~ ^[[:space:]]*# ]] && continue
        [[ -z "$key" ]] && continue
        key="${key// /}"   # strip spaces
        value="${value%%#*}"   # strip inline comments
        value="${value#"${value%%[![:space:]]*}"}"  # ltrim
        value="${value%"${value##*[![:space:]]}"}"  # rtrim
        export "$key=$value"
    done < "$ENV_FILE"
else
    echo "[start_bot] Warning: .env not found at $ENV_FILE — relying on existing environment."
fi

# ── Validate required credentials ─────────────────────────────────────────────
for var in ALPACA_API_KEY ALPACA_API_SECRET; do
    if [[ -z "${!var:-}" ]]; then
        echo "Error: required environment variable '$var' is not set." >&2
        echo "Create a .env file (see .env.example)." >&2
        exit 1
    fi
done

# ── Set up log file ───────────────────────────────────────────────────────────
LOG_DIR="$ROOT/data/logs"
mkdir -p "$LOG_DIR"
LOG_FILE="$LOG_DIR/bot_$(date +%Y%m%d_%H%M%S).log"

echo "[start_bot] Starting: $BINARY trade $MODE"
echo "[start_bot] Log:      $LOG_FILE"
echo "[start_bot] Press Ctrl+C to stop (the bot will cancel open orders before exit)."
echo ""

# ── Launch — tee to console and log ───────────────────────────────────────────
"$BINARY" trade "$MODE" 2>&1 | tee "$LOG_FILE"
