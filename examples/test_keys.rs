use std::error::Error;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    println!("=== Testing Alpaca API Keys ===\n");
    
    let api_key = std::env::var("ALPACA_API_KEY")
        .map_err(|_| "ALPACA_API_KEY not set")?;
    let api_secret = std::env::var("ALPACA_API_SECRET")
        .map_err(|_| "ALPACA_API_SECRET not set")?;
    
    println!("API Key: {}", api_key);
    println!("API Secret: {}...{}", &api_secret[..10], &api_secret[api_secret.len()-5..]);
    
    let client = black_scholes_rust::alpaca::AlpacaClient::new(api_key, api_secret);
    let account = client.get_account().await?;
    
    println!("\n📊 Account Info:");
    println!("   Cash: ${}", account.cash);
    println!("   Buying Power: ${}", account.buying_power);
    println!("   Portfolio Value: ${}", account.portfolio_value);
    
    Ok(())
}
