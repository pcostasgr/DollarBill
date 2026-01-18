// Volatility surface analysis for multiple symbols
// Generates volatility smile visualizations and CSV exports

use dollarbill::market_data::options_json_loader::load_options_from_json;
use dollarbill::utils::vol_surface::{extract_vol_surface, save_vol_surface_csv, print_vol_smile};
use rayon::prelude::*;
use std::time::Instant;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("===============================================================");
    println!("VOLATILITY SURFACE ANALYZER");
    println!("Extract and visualize implied volatility surfaces");
    println!("===============================================================\n");
    
    let symbols = vec!["TSLA", "AAPL", "NVDA", "MSFT"];
    let rate = 0.05;
    
    println!("Processing {} symbols...\n", symbols.len());
    let start = Instant::now();
    
    // Process all symbols in parallel
    let results: Vec<_> = symbols
        .par_iter()
        .map(|symbol| {
            let json_file = format!("data/{}_options_live.json", symbol.to_lowercase());
            
            match load_options_from_json(&json_file) {
                Ok((spot, options)) => {
                    let surface_points = extract_vol_surface(&options, spot, rate);
                    Ok((symbol.to_string(), spot, surface_points))
                }
                Err(e) => Err(format!("{}: {}", symbol, e))
            }
        })
        .collect();
    
    let elapsed = start.elapsed();
    
    // Process results
    let mut success_count = 0;
    
    for result in results {
        match result {
            Ok((symbol, spot, surface_points)) => {
                success_count += 1;
                
                println!("✓ {} - Extracted {} volatility points (spot: ${:.2})", 
                    symbol, surface_points.len(), spot);
                
                // Save to CSV
                let csv_filename = format!("data/{}_vol_surface.csv", symbol.to_lowercase());
                if let Err(e) = save_vol_surface_csv(&surface_points, &symbol, &csv_filename) {
                    println!("  ⚠ Failed to save CSV: {}", e);
                } else {
                    println!("  → Saved to {}", csv_filename);
                }
                
                // Print volatility smile
                print_vol_smile(&surface_points, &symbol);
            }
            Err(e) => {
                println!("✗ {}", e);
            }
        }
    }
    
    println!("\n===============================================================");
    println!("SUMMARY");
    println!("===============================================================");
    println!("Processed: {}/{} symbols", success_count, symbols.len());
    println!("Time: {} ms", elapsed.as_millis());
    
    if success_count > 0 {
        println!("\n📊 CSV files generated:");
        for symbol in &symbols {
            println!("  data/{}_vol_surface.csv", symbol.to_lowercase());
        }
        
        println!("\n💡 Next steps:");
        println!("  1. Import CSV files into Python/Excel");
        println!("  2. Plot 3D surface: Strike vs Time vs IV");
        println!("  3. Analyze volatility skew and term structure");
        println!("  4. Compare implied vs historical volatility");
        
        println!("\n🐍 Python visualization example:");
        println!("  import pandas as pd");
        println!("  import plotly.graph_objects as go");
        println!("  df = pd.read_csv('data/tsla_vol_surface.csv')");
        println!("  fig = go.Figure(data=[go.Surface(");
        println!("      x=df['Strike'], y=df['TimeToExpiry'], z=df['ImpliedVol']");
        println!("  )])");
        println!("  fig.show()");
    }
    
    Ok(())
}
