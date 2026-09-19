# DollarBill — Paper Trading Runbook (Ubuntu server)

Single-step runbook for an AI CLI agent to bring DollarBill up in **paper trading**
mode on an Ubuntu server (e.g. Contabo VPS). Run each step in order from the
project root. Stop and report back if any step's exit code is non-zero.

**Scope:** paper trading only (fake money, Alpaca's `paper-api.alpaca.markets`
endpoint). Do **not** set `APCA_LIVE=1` anywhere in this runbook.

---

## ⚠️ Security note before you start

`scripts/run_paper_trading.sh` (if present in this checkout) contains a
**hardcoded Alpaca API key/secret**. It is `.gitignore`d and was never
committed to git, but treat that key as compromised anyway — do not use that
script, and rotate/revoke that key in the Alpaca dashboard if it's still
active. This runbook uses the supported `.env`-based flow instead
(`scripts/start_bot.sh`), which never hardcodes secrets.

---

## Step 0 — Locate the project and confirm prerequisites

```bash
cd ~/DollarBill   # adjust to wherever the repo was cloned
test -f Cargo.toml && echo OK || echo "FAIL: not the project root"
rustc --version && cargo --version
```
If `rustc`/`cargo` are missing:
```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
source "$HOME/.cargo/env"
```

## Step 1 — Install system build dependencies

```bash
sudo apt-get update
sudo apt-get install -y build-essential pkg-config libssl-dev libsqlite3-dev git python3-venv
```

## Step 2 — Build the release binaries

```bash
cargo build --release --bins
ls -la target/release/dollarbill target/release/dashboard
```

## Step 3 — (Optional) Set up Python for data fetching

```bash
bash scripts/setup_python.sh
source .venv/bin/activate
```

## Step 4 — Configure the watchlist

Edit `config/stocks.json` and set `"enabled": true` only for the symbols you
want to trade. Skip this step to keep the defaults already in the repo.
```bash
cat config/stocks.json
```

## Step 5 — Fetch market data

```bash
source .venv/bin/activate 2>/dev/null || true
python py/fetch_multi_stocks.py
python py/fetch_multi_options.py
```

## Step 6 — Calibrate Heston parameters + build the performance matrix

```bash
bash scripts/run_heston_backtest.sh
```
This populates `data/*_heston_params.json` and the performance matrix the bot
uses to weight strategies. Required before live/paper trading for realistic
behavior.

## Step 7 — Train personality models (recommended, not strictly optional)

`dollarbill trade` loads `models/stock_classifier.json` and
`models/performance_matrix.json` at startup to drive personality-based
strategy selection. Without them it still trades, but falls back to generic
defaults and logs `"performance_matrix.json not found — strategy filtering
disabled"`. Only the pipeline command below actually writes those files:
```bash
cargo run --release --example personality_driven_pipeline
```
`enhanced_personality_analysis` (seen in some docs) is a print-only demo —
it saves nothing and does not affect paper trading, so it's skipped here.

## Step 8 — Configure Alpaca **paper** credentials

```bash
cp .env.example .env
```
Edit `.env` and set:
```
ALPACA_API_KEY=your-paper-api-key-here
ALPACA_API_SECRET=your-paper-api-secret-here
```
Leave `APCA_LIVE` commented out — paper keys already route to the fake-money
paper endpoint regardless.

```bash
chmod 600 .env
```

## Step 9 — Dry-run validation (no orders submitted)

```bash
bash scripts/start_bot.sh --dry-run
```
Let it run for one iteration (watch for `"Options session closed"` /
`"SIGNAL"` log lines), then `Ctrl+C`. Confirm no errors before proceeding.

## Step 10 — Start paper trading

Run in `tmux` (or `screen`) so it survives an SSH disconnect:
```bash
sudo apt-get install -y tmux   # if not already installed
tmux new -d -s dollarbill 'bash scripts/start_bot.sh'
tmux ls   # confirm the "dollarbill" session is running
```
To attach and watch live output: `tmux attach -t dollarbill` (detach again
with `Ctrl+b d` — do not close the terminal with the bot still needing to run).

## Step 11 — Verify it's running

```bash
tail -n 50 data/logs/bot_*.log
cat data/bot_status.json
```
Optional live TUI dashboard (run in a second tmux window):
```bash
tmux new -d -s dashboard './target/release/dashboard'
tmux attach -t dashboard
```

## Step 12 — Stop the bot safely

```bash
tmux send-keys -t dollarbill C-c
tmux wait-for -S dollarbill-stopped &
```
Confirm it exited cleanly (it cancels open orders on SIGINT/SIGTERM):
```bash
tail -n 20 data/logs/bot_*.log
```

---

## Optional: persistent systemd service (auto-restart on crash/reboot)

Only do this once you've validated paper trading manually via Steps 9–12.

```bash
sudo useradd -r -s /usr/sbin/nologin dollarbill || true
sudo mkdir -p /opt/dollarbill /etc/dollarbill
sudo cp -r . /opt/dollarbill
sudo cp target/release/dollarbill /usr/local/bin/dollarbill
sudo bash -c 'cat > /etc/dollarbill/secrets.env' <<'EOF'
ALPACA_API_KEY=your-paper-api-key-here
ALPACA_API_SECRET=your-paper-api-secret-here
RUST_LOG=info
EOF
sudo chmod 600 /etc/dollarbill/secrets.env
sudo chown -R dollarbill:dollarbill /opt/dollarbill /etc/dollarbill
sudo cp deploy/dollarbill.service /etc/systemd/system/
sudo systemctl daemon-reload
sudo systemctl enable --now dollarbill
sudo systemctl status dollarbill
journalctl -u dollarbill -f
```
`deploy/dollarbill.service` runs `dollarbill trade --live`, which — with
**paper** keys in `secrets.env` and no `APCA_LIVE=1` — still only trades on
the paper endpoint.

---

## Checklist for the AI CLI agent

- [ ] Step 0–2: build succeeds, both binaries exist
- [ ] Step 6: Heston calibration completes without error
- [ ] Step 8: `.env` contains **paper** keys only, `chmod 600`
- [ ] Step 9: dry-run produces signal logs, no crash
- [ ] Step 10: `tmux ls` shows the `dollarbill` session alive
- [ ] Step 11: `data/bot_status.json` updates and log file is growing
- [ ] Never set `APCA_LIVE=1` unless the user explicitly asks to go live
