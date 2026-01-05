# Alpaca Paper Trading Integration

Complete API client for Alpaca's free paper trading platform. Trade stocks in real-time with $100,000 virtual cash.

## 🚀 Quick Start

### 1. Get Free API Keys

1. Sign up at [Alpaca Paper Trading](https://app.alpaca.markets/paper/dashboard/overview) (100% free, no credit card)
2. Go to API Keys section
3. Generate new paper trading keys
4. Copy your API Key and Secret Key

### 2. Set Environment Variables

**PowerShell (Windows):**
```powershell
$env:ALPACA_API_KEY="PK..."
$env:ALPACA_API_SECRET="..."
```

**Bash (Linux/Mac):**
```bash
export ALPACA_API_KEY="PK..."
export ALPACA_API_SECRET="..."
```

### 3. Run Demo

```bash
cargo run --example alpaca_demo
```

## 📖 Features

### ✅ Account Management
- Get account balance, buying power, portfolio value
- Track equity and cash positions
- Monitor day trade count and pattern day trader status

### ✅ Position Tracking
- View all open positions
- Get real-time P&L for each position
- Close individual or all positions

### ✅ Order Execution
- Submit market, limit, stop, and stop-limit orders
- Track order status and fills
- Cancel pending orders

### ✅ Live Market Data
- Real-time quotes (bid/ask)
- Latest trades
- Historical bars (1Min, 5Min, 15Min, 1Hour, 1Day)
- Market snapshots combining all data

## 📝 Code Examples

### Get Account Info
```rust
use black_scholes_rust::alpaca::AlpacaClient;

let client = AlpacaClient::from_env()?;
let account = client.get_account().await?;

println!("Buying Power: ${}", account.buying_power);
println!("Portfolio Value: ${}", account.portfolio_value);
```

### Get Live Price
```rust
let snapshot = client.get_snapshot("TSLA").await?;
if let Some(trade) = snapshot.latest_trade {
    println!("TSLA: ${:.2}", trade.price);
}
```

### Submit Market Order
```rust
use black_scholes_rust::alpaca::{OrderRequest, OrderSide, OrderType, TimeInForce};

let order = OrderRequest {
    symbol: "TSLA".to_string(),
    qty: 10.0,
    side: OrderSide::Buy,
    r#type: OrderType::Market,
    time_in_force: TimeInForce::Day,
    limit_price: None,
    stop_price: None,
    extended_hours: None,
    client_order_id: None,
};

let result = client.submit_order(&order).await?;
println!("Order ID: {}", result.id);
```

### Submit Limit Order
```rust
let order = OrderRequest {
    symbol: "NVDA".to_string(),
    qty: 5.0,
    side: OrderSide::Buy,
    r#type: OrderType::Limit,
    time_in_force: TimeInForce::Day,
    limit_price: Some(180.00),  // Only buy at $180 or below
    stop_price: None,
    extended_hours: None,
    client_order_id: None,
};

let result = client.submit_order(&order).await?;
```

### Get All Positions
```rust
let positions = client.get_positions().await?;
for pos in positions {
    println!("{}: {} shares @ ${} | P&L: ${}",
        pos.symbol,
        pos.qty,
        pos.avg_entry_price,
        pos.unrealized_pl
    );
}
```

### Close a Position
```rust
let order = client.close_position("TSLA").await?;
println!("Position closed, order ID: {}", order.id);
```

### Get Historical Data
```rust
let bars = client.get_bars(
    "TSLA",
    "1Day",
    "2024-01-01T00:00:00Z",
    None,
    Some(30)  // Last 30 days
).await?;

for bar in bars {
    println!("Date: {} | Close: ${:.2} | Volume: {}",
        bar.t, bar.c, bar.v);
}
```

## 🎯 Next Steps

### Paper Trading Strategy Example

See `examples/paper_trading.rs` for a complete example that:
- Runs your backtested strategies on live data
- Generates buy/sell signals in real-time
- Executes paper trades automatically
- Tracks performance vs backtests

### Run Your Strategies Live

1. Modify `examples/paper_trading.rs` with your strategy
2. Run: `cargo run --example paper_trading`
3. Watch trades execute in real-time
4. View results at https://app.alpaca.markets/paper/dashboard

## 📚 API Reference

### AlpacaClient Methods

**Account:**
- `get_account()` - Get account information

**Positions:**
- `get_positions()` - Get all positions
- `get_position(symbol)` - Get specific position
- `close_position(symbol)` - Close position
- `close_all_positions()` - Close all positions

**Orders:**
- `submit_order(order)` - Submit new order
- `get_orders(status)` - Get orders (filtered by status)
- `get_order(id)` - Get specific order
- `cancel_order(id)` - Cancel order
- `cancel_all_orders()` - Cancel all orders

**Market Data:**
- `get_latest_quote(symbol)` - Get bid/ask
- `get_latest_trade(symbol)` - Get last trade
- `get_snapshot(symbol)` - Get full market snapshot
- `get_bars(symbol, timeframe, start, end, limit)` - Get historical OHLCV

## ⚠️ Important Notes

- **Paper Trading Only**: This uses Alpaca's paper trading endpoint
- **Market Hours**: Live data only available during market hours (9:30 AM - 4:00 PM ET)
- **Rate Limits**: 200 requests per minute
- **Data Feed**: Free tier has 15-minute delayed data (real-time requires paid plan)

## 🔗 Resources

- [Alpaca Dashboard](https://app.alpaca.markets/paper/dashboard)
- [Alpaca API Docs](https://alpaca.markets/docs/)
- [Market Hours](https://www.alpaca.markets/support/market-hours)
