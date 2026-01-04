# Fetch historical stock data for multiple symbols from Yahoo Finance
# Saves each symbol as {symbol}_one_year.csv
# Run: python fetch_multi_stocks.py

import yfinance as yf
import pandas as pd
from datetime import datetime, timedelta

# Configure symbols and time period
SYMBOLS = ["TSLA", "AAPL", "NVDA", "MSFT", "META", "GOOGL", "AMZN"]
PERIOD = "1y"  # Options: 1d, 5d, 1mo, 3mo, 6mo, 1y, 2y, 5y, 10y, ytd, max
OUTPUT_DIR = ""  # Leave empty for current directory, or set to "data/"

print("=" * 70)
print("MULTI-SYMBOL STOCK DATA FETCHER")
print(f"Fetching {PERIOD} of historical data for {len(SYMBOLS)} symbols")
print("=" * 70)

results = []

for symbol in SYMBOLS:
    try:
        print(f"\n📊 Fetching {symbol}...", end=" ")
        
        ticker = yf.Ticker(symbol)
        history = ticker.history(period=PERIOD)
        
        if history.empty:
            print(f"❌ No data available")
            results.append({"symbol": symbol, "status": "FAILED", "reason": "No data"})
            continue
        
        # Save to CSV
        filename = f"{OUTPUT_DIR}{symbol.lower()}_one_year.csv"
        history.to_csv(filename)
        
        # Get current price and stats
        current_price = history["Close"].iloc[-1]
        data_points = len(history)
        date_range = f"{history.index[0].strftime('%Y-%m-%d')} to {history.index[-1].strftime('%Y-%m-%d')}"
        
        print(f"✓ {data_points} days | ${current_price:.2f} | {filename}")
        
        results.append({
            "symbol": symbol,
            "status": "SUCCESS",
            "filename": filename,
            "data_points": data_points,
            "current_price": current_price,
            "date_range": date_range
        })
        
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
for r in successful:
    print(f"  {r['symbol']:6} → {r['filename']:30} ({r['data_points']} days)")

if failed:
    print(f"\n❌ Failed: {len(failed)}")
    for r in failed:
        print(f"  {r['symbol']:6} → {r.get('reason', 'Unknown error')}")

print("\n" + "=" * 70)
print("Done! Stock data saved to CSV files.")
print("=" * 70)
