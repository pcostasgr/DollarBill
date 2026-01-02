# scrape_yfinance.py
import yfinance as yf
import json

TICKER = "TSLA"

print(f"Scraping {TICKER}...")

ticker = yf.Ticker(TICKER)
info = ticker.info
history = ticker.history(period="1y")

# FIX: Convert history to list of dicts (serializable)
history_dict = history.reset_index().to_dict(orient="records")

data = {
    "ticker": TICKER,
    "current_info": info,
    "historical_prices": history_dict
}

with open(f"{TICKER.lower()}_data.json", "w") as f:
    json.dump(data, f, indent=4, default=str)

print(f"{TICKER.lower()}_data.json saved without errors!")