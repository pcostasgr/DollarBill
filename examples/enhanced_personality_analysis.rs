// Enhanced Personality Analysis Example
// Demonstrates the new advanced stock classification system
// Run with: cargo run --example enhanced_personality_analysis

use dollarbill::analysis::stock_classifier::StockClassifier;
use std::collections::HashMap;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("🚀 DollarBill Enhanced Stock Personality Analysis");
    println!("===============================================");
    println!();

    // Create enhanced classifier
    let mut classifier = StockClassifier::new();
    
    // Load stocks from configuration file
    let config_content = std::fs::read_to_string("config/stocks.json")?;
    let config: serde_json::Value = serde_json::from_str(&config_content)?;
    
    let mut stocks = Vec::new();
    if let Some(stock_array) = config["stocks"].as_array() {
        for stock in stock_array {
            if let (Some(symbol), Some(sector), Some(enabled)) = (
                stock["symbol"].as_str(),
                stock["sector"].as_str(), 
                stock["enabled"].as_bool()
            ) {
                if enabled {
                    stocks.push((symbol, sector));
                }
            }
        }
    }
    
    println!("🧠 Analyzing {} stocks with advanced multi-dimensional features...", stocks.len());
    println!();

    let mut results = Vec::new();
    let mut sector_summary: HashMap<String, Vec<String>> = HashMap::new();

    for (symbol, sector) in stocks {
        println!("📊 Analyzing {} ({})...", symbol, sector);
        
        match classifier.classify_stock_enhanced(symbol, sector) {
            Ok(profile) => {
                println!("   ✅ Classification complete!");
                println!("   🎯 Best strategies: {:?}", profile.best_strategies);
                println!("   ❌ Avoid strategies: {:?}", profile.worst_strategies);
                
                // Group by sector
                sector_summary.entry(sector.to_string())
                    .or_insert_with(Vec::new)
                    .push(format!("{}: {:?}", symbol, profile.personality));
                
                results.push((symbol, profile));
                println!();
            }
            Err(e) => {
                println!("   ❌ Error analyzing {}: {}", symbol, e);
                println!();
            }
        }
    }

    // Print sector summary
    println!("📈 SECTOR PERSONALITY BREAKDOWN");
    println!("==============================");
    for (sector, stocks_in_sector) in sector_summary {
        println!("🏢 {}:", sector);
        for stock_info in stocks_in_sector {
            println!("   • {}", stock_info);
        }
        println!();
    }

    // Compare with legacy system
    println!("🔄 COMPARISON: Enhanced vs Legacy Classification");
    println!("===============================================");
    
    for (symbol, enhanced_profile) in &results {
        // Run legacy classification for comparison
        let legacy_profile = classifier.classify_stock(
            symbol,
            enhanced_profile.avg_volatility,
            enhanced_profile.trend_strength,
            enhanced_profile.mean_reversion_tendency,
            enhanced_profile.momentum_sensitivity,
        );
        
        let personality_match = enhanced_profile.personality == legacy_profile.personality;
        let status_emoji = if personality_match { "✅" } else { "🔄" };
        
        println!("{} {}: Enhanced={:?} | Legacy={:?}", 
                 status_emoji, symbol, enhanced_profile.personality, legacy_profile.personality);
    }
    
    println!();
    println!("🎯 ANALYSIS COMPLETE!");
    println!("📊 Enhanced system provides:");
    println!("   • Multi-dimensional feature analysis");
    println!("   • Percentile-based volatility thresholds"); 
    println!("   • Market regime detection");
    println!("   • Sector-normalized metrics");
    println!("   • Confidence scoring");
    println!("   • Time-weighted analysis");
    println!();
    println!("⚠️  Legacy system uses fixed 25%/50% thresholds (deprecated)");

    Ok(())
}