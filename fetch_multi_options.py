# Fetch live options data for multiple symbols from Yahoo Finance
# Saves each symbol as {symbol}_options_live.json
# Run: python fetch_multi_options.py

import yfinance as yf
import json
from datetime import datetime
import time

# Configure symbols and options parameters
SYMBOLS = ["TSLA", "AAPL", "NVDA", "MSFT"]
EXPIRATION_INDEX = 2  # 0 = nearest, 1 = next week, 2 = ~2-3 weeks out
OUTPUT_DIR = ""  # Leave empty for current directory, or set to "options_data/"
DELAY_SECONDS = 1  # Delay between API calls to avoid rate limiting

print("=" * 70)
print("MULTI-SYMBOL OPTIONS DATA FETCHER")
print(f"Fetching options chains for {len(SYMBOLS)} symbols")
print(f"Target expiration: Index {EXPIRATION_INDEX}")
print("=" * 70)

results = []

for i, symbol in enumerate(SYMBOLS):
    try:
        print(f"\n[{i+1}/{len(SYMBOLS)}] 📊 Fetching {symbol} options...", end=" ")
        
        ticker = yf.Ticker(symbol)
        spot_price = ticker.history(period="1d")["Close"].iloc[-1]
        
        # Get available expirations
        expirations = ticker.options
        if len(expirations) <= EXPIRATION_INDEX:
            print(f"❌ Not enough expirations (only {len(expirations)} available)")
            results.append({"symbol": symbol, "status": "FAILED", "reason": f"Only {len(expirations)} expirations"})
            continue
        
        # Fetch selected expiration
        expiration_date = expirations[EXPIRATION_INDEX]
        options_chain = ticker.option_chain(expiration_date)
        calls = options_chain.calls
        puts = options_chain.puts
        
        # Calculate time to expiry
        exp_datetime = datetime.strptime(expiration_date, "%Y-%m-%d")
        days_to_expiry = (exp_datetime - datetime.now()).days
        time_to_expiry = days_to_expiry / 365.0
        
        # Build JSON structure
        data = {
            "symbol": symbol,
            "spot_price": float(spot_price),
            "expiration_date": expiration_date,
            "time_to_expiry": time_to_expiry,
            "fetched_at": datetime.now().isoformat(),
            "options": []
        }
        
        # Add calls
        for _, row in calls.iterrows():
            if row["bid"] > 0 and row["ask"] > 0:
                data["options"].append({
                    "strike": float(row["strike"]),
                    "bid": float(row["bid"]),
                    "ask": float(row["ask"]),
                    "volume": int(row["volume"]) if row["volume"] == row["volume"] else 0,
                    "open_interest": int(row["openInterest"]) if row["openInterest"] == row["openInterest"] else 0,
                    "option_type": "Call"
                })
        
        # Add puts
        for _, row in puts.iterrows():
            if row["bid"] > 0 and row["ask"] > 0:
                data["options"].append({
                    "strike": float(row["strike"]),
                    "bid": float(row["bid"]),
                    "ask": float(row["ask"]),
                    "volume": int(row["volume"]) if row["volume"] == row["volume"] else 0,
                    "open_interest": int(row["openInterest"]) if row["openInterest"] == row["openInterest"] else 0,
                    "option_type": "Put"
                })
        
        # Save to JSON
        filename = f"{OUTPUT_DIR}{symbol.lower()}_options_live.json"
        with open(filename, "w") as f:
            json.dump(data, f, indent=2)
        
        call_count = sum(1 for o in data["options"] if o["option_type"] == "Call")
        put_count = sum(1 for o in data["options"] if o["option_type"] == "Put")
        
        print(f"✓ {len(data['options'])} options | ${spot_price:.2f} | {expiration_date} ({days_to_expiry}d)")
        
        results.append({
            "symbol": symbol,
            "status": "SUCCESS",
            "filename": filename,
            "spot_price": spot_price,
            "expiration_date": expiration_date,
            "days_to_expiry": days_to_expiry,
            "total_options": len(data["options"]),
            "calls": call_count,
            "puts": put_count
        })
        
        # Delay to avoid rate limiting (except for last symbol)
        if i < len(SYMBOLS) - 1:
            time.sleep(DELAY_SECONDS)
        
    except Exception as e:
        print(f"❌ Error: {str(e)}")
        results.append({"symbol": symbol, "status": "FAILED", "reason": str(e)})

# Summary
print("\n" + "=" * 70)
print("SUMMARY")
print("=" * 70)

successful = [r for r in results if r["status"] == "SUCCESS"]
failed = [r for r in results if r["status"] == "FAILED"]

print(f"\n✓ Successful: {len(successful)}/{len(SYMBOLS)}")
if successful:
    print(f"\n{'Symbol':<8} {'Spot':>10} {'Expiry':<12} {'Days':>6} {'Calls':>7} {'Puts':>7} {'Total':>7} {'File'}")
    print("-" * 90)
    for r in successful:
        print(f"{r['symbol']:<8} ${r['spot_price']:>9.2f} {r['expiration_date']:<12} {r['days_to_expiry']:>6} "
              f"{r['calls']:>7} {r['puts']:>7} {r['total_options']:>7} {r['filename']}")

if failed:
    print(f"\n❌ Failed: {len(failed)}")
    for r in failed:
        print(f"  {r['symbol']:6} → {r.get('reason', 'Unknown error')}")

print("\n" + "=" * 70)
print("Done! Options data saved to JSON files.")
print("Use these files with multi_symbol_signals example:")
print("  cargo run --release --example multi_symbol_signals")
print("=" * 70)
