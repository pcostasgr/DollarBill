// Example: Alpaca Paper Trading Demo
// 
// Setup:
// 1. Sign up for free paper trading at https://app.alpaca.markets/paper/dashboard/overview
// 2. Get your API keys from the dashboard
// 3. Set environment variables:
//    $env:ALPACA_API_KEY="your_key_here"
//    $env:ALPACA_API_SECRET="your_secret_here"
// 4. Run: cargo run --example alpaca_demo

use dollarbill::alpaca::{AlpacaClient, OrderRequest, OrderSide, OrderType, TimeInForce};
use std::error::Error;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    println!("\n🚀 Alpaca Paper Trading Demo");
    println!("{}", "=".repeat(60));
    
    // Create client from environment variables
    let client = AlpacaClient::from_env()?;
    println!("✅ Connected to Alpaca Paper Trading");
    
    // 1. Get account information
    println!("\n📊 Account Information:");
    println!("{}", "-".repeat(60));
    let account = client.get_account().await?;
    println!("Account ID: {}", account.account_number);
    println!("Status: {}", account.status);
    println!("Buying Power: ${}", account.buying_power);
    println!("Cash: ${}", account.cash);
    println!("Portfolio Value: ${}", account.portfolio_value);
    println!("Equity: ${}", account.equity);
    
    // 2. Get current positions
    println!("\n💼 Current Positions:");
    println!("{}", "-".repeat(60));
    let positions = client.get_positions().await?;
    if positions.is_empty() {
        println!("No open positions");
    } else {
        for pos in &positions {
            println!("{} | Qty: {} | Avg Price: ${} | Current: ${} | P&L: ${} ({}%)",
                pos.symbol,
                pos.qty,
                pos.avg_entry_price,
                pos.current_price,
                pos.unrealized_pl,
                pos.unrealized_plpc
            );
        }
    }
    
    // 3. Get live market data for TSLA
    println!("\n📈 Live Market Data:");
    println!("{}", "-".repeat(60));
    
    let symbols = vec!["TSLA", "NVDA", "AAPL"];
    for symbol in symbols {
        match client.get_snapshot(symbol).await {
            Ok(snapshot) => {
                if let Some(trade) = snapshot.latest_trade {
                    println!("{}: ${:.2} (last trade)", symbol, trade.price);
                }
                if let Some(quote) = snapshot.latest_quote {
                    println!("  Bid: ${:.2} x {} | Ask: ${:.2} x {}",
                        quote.bid, quote.bid_size, quote.ask, quote.ask_size);
                }
            }
            Err(e) => println!("{}: Error - {}", symbol, e),
        }
    }
    
    // 4. Example: Submit a market order (commented out for safety)
    println!("\n📝 Example Order (not executed):");
    println!("{}", "-".repeat(60));
    let _example_order = OrderRequest {
        symbol: "TSLA".to_string(),
        qty: 1.0,
        side: OrderSide::Buy,
        r#type: OrderType::Market,
        time_in_force: TimeInForce::Day,
        limit_price: None,
        stop_price: None,
        extended_hours: None,
        client_order_id: None,
    };
    println!("Order: Buy 1 share of TSLA at market price");
    println!("(Executing order...)");
    
    // Uncomment to actually submit the order:
    // let order = client.submit_order(&example_order).await?;
    // println!("✅ Order submitted: ID {}", order.id);
    
    // 5. Get open orders
    println!("\n📋 Open Orders:");
    println!("{}", "-".repeat(60));
    let orders = client.get_orders(Some("open")).await?;
    if orders.is_empty() {
        println!("No open orders");
    } else {
        for order in &orders {
            println!("{} | {} {} @ {} | Status: {}",
                order.symbol,
                order.side,
                order.qty,
                order.order_type,
                order.status
            );
        }
    }
    
    println!("\n✅ Demo complete!");
    println!("\nNext steps:");
    println!("  1. Uncomment the submit_order line to place a real paper trade");
    println!("  2. Check examples/paper_trading.rs for live trading strategy");
    println!("  3. Visit https://app.alpaca.markets/paper/dashboard to view your trades\n");
    
    Ok(())
}
