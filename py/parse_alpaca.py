import pandas as pd
import re

df = pd.read_excel(r'C:\Users\Costas\Downloads\activities_al.xlsx')

def parse_amt(x):
    s = str(x).strip()
    if s in ['–', 'nan', '']:
        return 0.0
    neg = s.startswith('-')
    s2 = s.replace('+$','').replace('-$','').replace(',','').replace('$','')
    try:
        v = float(s2)
        return -v if neg else v
    except:
        return 0.0

df['amt'] = df['Amount'].apply(parse_amt)

fills = df[df['Type'] == 'FILL'].copy()
fees  = df[df['Type'] == 'FEE'].copy()

def parse_date(d):
    s = str(d).strip()
    # "Jun 03, 2026, 04:39:29 PM" or "Jun 03, 2026"
    m = re.match(r'(\w+ \d+, \d+)', s)
    if m:
        try:
            return pd.to_datetime(m.group(1), format='%b %d, %Y')
        except:
            return pd.NaT
    return pd.NaT

fills = fills.copy()
fees  = fees.copy()
fills['date'] = fills['Date'].apply(parse_date)
fees['date']  = fees['Date'].apply(parse_date)

def extract_sym(desc):
    m = re.search(r'(QCOM|GLD|GOOGL|SPY|AAPL|NVDA|TSLA|IWM|META|MSFT|AMZN|PLTR|COIN|QQQ)', str(desc))
    return m.group(1) if m else 'OTHER'

fills['symbol'] = fills['Description'].apply(extract_sym)

print("=== NET P&L BY DATE / SYMBOL ===")
summary = fills.groupby([fills['date'].dt.date, 'symbol'])['amt'].sum().reset_index()
summary.columns = ['Date', 'Symbol', 'Net_Credit']
print(summary.to_string(index=False))
print()
print(f"Total FILL net:  ${fills['amt'].sum():>10,.2f}")
print(f"Total FEE  net:  ${fees['amt'].sum():>10,.2f}")
print(f"GRAND NET:       ${df['amt'].sum():>10,.2f}")
print()

print("=== INDIVIDUAL TRADE LEGS ===")
print(f"{'Date':12s}  {'Sym':6s}  {'Dir':12s}  {'Qty':3s}  {'Strike/Type':30s}  {'Amount':>10s}")
print("-" * 90)
for _, row in fills.iterrows():
    desc = str(row['Description'])
    # direction
    if 'Sell_short' in desc:
        direction = 'SELL (credit)'
    else:
        direction = 'BUY  (debit) '
    # parse OCC symbol
    m = re.search(r'(\w+260\w+|[A-Z]+\d{6}[CP]\d+)', desc)
    occ = m.group(1) if m else desc[-30:]
    qty = str(row['Qty'])
    date_str = str(row['date'])[:10]
    sym = str(row['symbol'])
    amt = row['amt']
    print(f"{date_str:12s}  {sym:6s}  {direction}  {qty:3s}  {occ:30s}  {amt:>+10.2f}")

print()
print("=== IRON CONDOR GROUPING (by timestamp) ===")
fills['ts'] = fills['Date'].apply(lambda x: str(x)[:20])
for ts, grp in fills.groupby('ts'):
    net = grp['amt'].sum()
    legs = len(grp)
    sym = grp['symbol'].iloc[0]
    print(f"  {ts:22s}  {sym:6s}  {legs} legs  net: ${net:>+9.2f}")

print()
print("=== OPEXP / Other ===")
other = df[~df['Type'].isin(['FILL','FEE'])]
print(other[['Description','Type','Qty','Amount','Date']].to_string(index=False))
