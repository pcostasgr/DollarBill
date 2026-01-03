// src/symbols.rs
// Popular and viral stock symbols for analysis

/// Array of 20 viral and popular stocks for trading analysis
pub const VIRAL_STOCKS: [&str; 20] = [
    "TSLA",  // Tesla - Electric vehicles, most traded stock
    "NVDA",  // NVIDIA - AI chips, data centers
    "AAPL",  // Apple - Consumer electronics
    "MSFT",  // Microsoft - Cloud computing, AI
    "GOOGL", // Alphabet (Google) - Search, cloud, AI
    "META",  // Meta (Facebook) - Social media
    "AMZN",  // Amazon - E-commerce, cloud (AWS)
    "AMD",   // Advanced Micro Devices - Semiconductors
    "GME",   // GameStop - Original meme stock
    "AMC",   // AMC Entertainment - Meme stock
    "PLTR",  // Palantir - Big data analytics
    "RIVN",  // Rivian - Electric vehicles
    "COIN",  // Coinbase - Cryptocurrency exchange
    "SOFI",  // SoFi - Fintech
    "NIO",   // Nio - Chinese EV maker
    "SNAP",  // Snap Inc - Social media
    "RBLX",  // Roblox - Gaming platform
    "UBER",  // Uber - Ride sharing
    "LCID",  // Lucid Motors - Luxury EVs
    "F",     // Ford - Traditional auto + EVs
];

/// Get a stock symbol by index
pub fn get_symbol(index: usize) -> Option<&'static str> {
    VIRAL_STOCKS.get(index).copied()
}

/// Get all symbols as a Vec
pub fn all_symbols() -> Vec<&'static str> {
    VIRAL_STOCKS.to_vec()
}

/// Check if a symbol is in the viral stocks list
pub fn is_viral_stock(symbol: &str) -> bool {
    VIRAL_STOCKS.contains(&symbol)
}

/// Print all symbols with descriptions
pub fn print_all_symbols() {
    println!("=== 20 Viral Stocks ===");
    for (i, symbol) in VIRAL_STOCKS.iter().enumerate() {
        println!("{}. {}", i + 1, symbol);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_array_length() {
        assert_eq!(VIRAL_STOCKS.len(), 20);
    }

    #[test]
    fn test_tesla_included() {
        assert!(VIRAL_STOCKS.contains(&"TSLA"));
    }

    #[test]
    fn test_get_symbol() {
        assert_eq!(get_symbol(0), Some("TSLA"));
        assert_eq!(get_symbol(20), None);
    }

    #[test]
    fn test_is_viral_stock() {
        assert!(is_viral_stock("TSLA"));
        assert!(is_viral_stock("GME"));
        assert!(!is_viral_stock("XYZ"));
    }
}
