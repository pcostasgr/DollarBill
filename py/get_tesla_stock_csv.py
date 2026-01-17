# download_tsla_csv.py
# Hardcore yfinance downloader — 1 year TSLA daily data → tesla_one_year.csv
# No fluff, no JSON — pure CSV blood for your Rust beast

import yfinance as yf
import pandas as pd

TICKER = "TSLA"
PERIOD = "5y"  # 5 years for comprehensive backtesting

print(f"Downloading {PERIOD} of {TICKER} data from Yahoo Finance...")

# Download the data
data = yf.download(TICKER, period=PERIOD)

# Reset index to make Date a column (Yahoo format match)
data = data.reset_index()

# Select and order columns exactly like your uploaded CSV
data = data[['Date', 'Open', 'High', 'Low', 'Close', 'Volume']]

# Dump to CSV — no row index numbers
data.to_csv("data/tsla_five_year.csv", index=False)

print("data/tsla_five_year.csv saved in data directory!")
print(f"Rows downloaded: {len(data)}")
print("Last 5 rows:")
print(data.tail(5))