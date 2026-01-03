// Quick test to fetch live TSLA options from Yahoo Finance
use black_scholes_rust::market_data::real_option_data_yahoo::{
    fetch_yahoo_options, 
    get_available_expirations,
    display_options_summary,
    fetch_liquid_options,
};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let symbol = "TSLA";
    
    println!("==============================================");
    println!("LIVE OPTIONS DATA FROM YAHOO FINANCE");
    println!("==============================================\n");
    
    // 1. Get available expiration dates
    println!("Fetching available expirations for {}...", symbol);
    let expirations = get_available_expirations(symbol).await?;
    println!("✓ Found {} expirations:", expirations.len());
    for (i, date) in expirations.iter().take(5).enumerate() {
        println!("  [{}] {}", i, date);
    }
    println!();
    
    // 2. Fetch nearest expiration options chain
    println!("Fetching options chain for nearest expiration...");
    let all_options = fetch_yahoo_options(symbol, 0).await?;
    display_options_summary(&all_options);
    println!();
    
    // 3. Filter for liquid options only
    println!("Filtering for liquid options (volume >= 50, spread <= 10%)...");
    let liquid_options = fetch_liquid_options(symbol, 0, 50, 10.0).await?;
    display_options_summary(&liquid_options);
    println!();
    
    // 4. Show sample of calls
    println!("Sample Call Options (first 10):");
    println!("{:<10} {:<10} {:<10} {:<10} {:<10}", "Strike", "Bid", "Ask", "Mid", "Volume");
    println!("{:-<60}", "");
    
    let calls: Vec<_> = liquid_options
        .iter()
        .filter(|o| matches!(o.option_type, black_scholes_rust::calibration::market_option::OptionType::Call))
        .take(10)
        .collect();
    
    for call in calls {
        println!(
            "${:<9.2} ${:<9.2} ${:<9.2} ${:<9.2} {:>10}",
            call.strike,
            call.bid,
            call.ask,
            call.mid_price(),
            call.volume
        );
    }
    
    println!("\n✅ Yahoo options scraper working!");
    println!("\nNext steps:");
    println!("  1. Use these options in calibration");
    println!("  2. Compare model prices to market prices");
    println!("  3. Find mispricings and generate trade signals");
    
    Ok(())
}
