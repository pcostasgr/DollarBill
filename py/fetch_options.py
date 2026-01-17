# Fetch live TSLA options using yfinance and save to JSON
# Run: python fetch_options.py

import yfinance as yf
import json
from datetime import datetime

SYMBOL = "TSLA"
OUTPUT_FILE = "data/tsla_options_live.json"
EXPIRATION_INDEX = 2  # 0 = nearest, 1 = next week, 2 = ~2-3 weeks out

print(f"Fetching options chain for {SYMBOL}...")

ticker = yf.Ticker(SYMBOL)
spot_price = ticker.history(period="1d")["Close"].iloc[-1]

# Get options chain for nearest expiration
try:
    expirations = ticker.options
    print(f"Available expirations: {len(expirations)}")
    print(f"  First 5: {expirations[:5]}")
    
    # Fetch selected expiration (change EXPIRATION_INDEX at top of file)
    expiration_date = expirations[EXPIRATION_INDEX]
    print(f"\nFetching options for expiration: {expiration_date} (index {EXPIRATION_INDEX})")
    
    options_chain = ticker.option_chain(expiration_date)
    calls = options_chain.calls
    puts = options_chain.puts
    
    # Calculate time to expiry in years
    exp_datetime = datetime.strptime(expiration_date, "%Y-%m-%d")
    days_to_expiry = (exp_datetime - datetime.now()).days
    time_to_expiry = days_to_expiry / 365.0
    
    print(f"Days to expiry: {days_to_expiry}")
    print(f"Time to expiry: {time_to_expiry:.4f} years")
    
    # Build JSON structure
    data = {
        "symbol": SYMBOL,
        "spot_price": float(spot_price),
        "expiration_date": expiration_date,
        "time_to_expiry": time_to_expiry,
        "fetched_at": datetime.now().isoformat(),
        "options": []
    }
    
    # Add calls
    print(f"\nProcessing {len(calls)} call options...")
    for _, row in calls.iterrows():
        if row["bid"] > 0 and row["ask"] > 0:  # Skip zero bids/asks
            data["options"].append({
                "strike": float(row["strike"]),
                "bid": float(row["bid"]),
                "ask": float(row["ask"]),
                "volume": int(row["volume"]) if row["volume"] == row["volume"] else 0,  # Handle NaN
                "open_interest": int(row["openInterest"]) if row["openInterest"] == row["openInterest"] else 0,
                "option_type": "Call"
            })
    
    # Add puts
    print(f"Processing {len(puts)} put options...")
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
    with open(OUTPUT_FILE, "w") as f:
        json.dump(data, f, indent=2)
    
    print(f"\n✓ Saved {len(data['options'])} options to {OUTPUT_FILE}")
    print(f"  Spot price: ${spot_price:.2f}")
    print(f"  Expiration: {expiration_date}")
    print(f"  Calls: {sum(1 for o in data['options'] if o['option_type'] == 'Call')}")
    print(f"  Puts: {sum(1 for o in data['options'] if o['option_type'] == 'Put')}")
    
except Exception as e:
    print(f"Error: {e}")
    import traceback
    traceback.print_exc()
