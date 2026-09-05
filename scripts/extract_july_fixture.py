#!/usr/bin/env python3
"""
Extract Alpaca activities from TestFiles/alpaca_activities.xls and write a
JSONL fixture to tests/fixtures/july_incident/activities.jsonl.

Usage (from repo root):
    python scripts/extract_july_fixture.py

The existing synthetic fixture is replaced only if the XLS exists and parses
correctly; otherwise it is left intact so CI still works.
"""

import json
import os
import sys
from pathlib import Path

XLS_PATH  = Path("TestFiles/alpaca_activities.xls")
OUT_PATH  = Path("tests/fixtures/july_incident/activities.jsonl")
OUT_PATH.parent.mkdir(parents=True, exist_ok=True)

if not XLS_PATH.exists():
    print(f"[SKIP] {XLS_PATH} not found — synthetic fixture unchanged.")
    sys.exit(0)

try:
    import xlrd
except ImportError:
    print("[SKIP] xlrd not installed — run: pip install xlrd==1.2.0")
    sys.exit(0)

wb = xlrd.open_workbook(str(XLS_PATH))
sh = wb.sheets()[0]

# Column layout inferred from analyze_alpaca.py:
#   0: description, 1: type, 2: qty, 3: amount, 4: date
# Alpaca activities export may also include a separate symbol column — check
# the header row and adapt column indices accordingly.
header = [str(sh.cell_value(0, c)).strip().lower() for c in range(sh.ncols)]
print(f"[INFO] Columns: {header}")

def col(name: str, fallback: int) -> int:
    """Return the index of column *name*, or *fallback* if not present."""
    for i, h in enumerate(header):
        if name in h:
            return i
    return fallback

COL_DESC   = col("desc",   0)
COL_TYPE   = col("type",   1)
COL_QTY    = col("qty",    2)
COL_AMT    = col("amount", 3)
COL_DATE   = col("date",   4)
COL_SYMBOL = col("symbol", -1)  # -1 = not present; extract from description

events = []
for r in range(1, sh.nrows):
    desc = str(sh.cell_value(r, COL_DESC)).strip()
    typ  = str(sh.cell_value(r, COL_TYPE)).strip()
    date = str(sh.cell_value(r, COL_DATE)).strip()

    if not typ or typ == "nan":
        continue

    try:
        qty_raw = str(sh.cell_value(r, COL_QTY)).strip()
        qty = float(qty_raw) if qty_raw else 0.0
    except (ValueError, TypeError):
        qty = 0.0

    try:
        amt_raw = str(sh.cell_value(r, COL_AMT)).strip()
        amt_clean = (amt_raw
                     .replace(",", "")
                     .replace("+$", "")
                     .replace("-$", "-")
                     .replace("$", "")
                     .replace("–", "0"))
        if amt_raw.startswith("+"):
            amt = abs(float(amt_clean))
        elif amt_raw.startswith("-"):
            amt = -abs(float(amt_clean))
        else:
            try:
                amt = float(amt_clean)
            except ValueError:
                amt = 0.0
    except (ValueError, TypeError):
        amt = 0.0

    # Extract symbol: use explicit column if present, else parse from description
    if COL_SYMBOL >= 0:
        sym = str(sh.cell_value(r, COL_SYMBOL)).strip()
    else:
        # Heuristic: look for known tickers or OCC-like tokens in the description
        sym = ""
        for tok in desc.split():
            tok = tok.strip("()")
            if len(tok) >= 16 and any(c in tok for c in "CP"):
                sym = tok  # likely OCC symbol
                break
        if not sym:
            for ticker in ["NVDA","QCOM","GLD","QQQ","GOOGL","AAPL","MSFT","META","SPY","IWM"]:
                if ticker in desc:
                    sym = ticker
                    break

    # Normalize timestamp (Alpaca dates vary: "Jul 14, 2026", ISO, etc.)
    ts = date if "T" in date else date + "T00:00:00Z"

    # Infer side from qty and type
    if typ == "FILL":
        side = "sell" if qty < 0 else "buy"
    elif typ == "OPTRD":
        side = "buy" if qty > 0 else "sell"
    elif typ == "OPASN":
        side = "sell"
    else:
        side = ""

    event = {
        "timestamp":           ts,
        "activity_type":       typ,
        "symbol":              sym,
        "qty":                 qty,
        "price":               abs(amt / qty) if qty != 0 else abs(amt),
        "side":                side,
        "order_id":            "",
        "client_order_id":     "",
        "equity_after":        0.0,   # not available in XLS — filled in manually or left 0
        "buying_power_after":  0.0,
        "description":         desc,
    }
    events.append(event)

with open(OUT_PATH, "w") as f:
    for ev in events:
        f.write(json.dumps(ev) + "\n")

print(f"[OK] Wrote {len(events)} events to {OUT_PATH}")
print("NOTE: equity_after fields are 0.0 — update manually if equity snapshots are available.")
