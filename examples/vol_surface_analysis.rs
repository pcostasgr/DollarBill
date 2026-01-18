// Volatility surface analysis for multiple symbols
// Generates volatility smile visualizations and CSV exports

use dollarbill::market_data::options_json_loader::load_options_from_json;
use dollarbill::utils::vol_surface::{extract_vol_surface, save_vol_surface_csv, print_vol_smile};
use dollarbill::market_data::symbols::load_enabled_stocks;
use rayon::prelude::*;
use std::time::Instant;
use serde::Deserialize;
use std::fs;

#[derive(Debug, Deserialize)]
struct VolSurfaceConfig {
    volatility_surface: VolSurfaceAnalysis,
}

#[derive(Debug, Deserialize)]
struct VolSurfaceAnalysis {
    risk_free_rate: f64,
    analysis: AnalysisParams,
    calibration: CalibrationParams,
}

#[derive(Debug, Deserialize)]
struct AnalysisParams {
    min_strikes_around_atm: usize,
    max_strikes_around_atm: usize,
    moneyness_tolerance: f64,
}

#[derive(Debug, Deserialize)]
struct CalibrationParams {
    tolerance: f64,
    max_iterations: usize,
    initial_vol_guess: f64,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("===============================================================");
    println!("VOLATILITY SURFACE ANALYZER");
    println!("Extract and visualize implied volatility surfaces");
    println!("===============================================================\n");

    // Load configuration
    let config_content = fs::read_to_string("config/vol_surface_config.json")
        .map_err(|e| format!("Failed to read vol surface config file: {}", e))?;
    let config: VolSurfaceConfig = serde_json::from_str(&config_content)
        .map_err(|e| format!("Failed to parse vol surface config file: {}", e))?;

    println!("📋 Loaded volatility surface configuration from config/vol_surface_config.json");

    // Load enabled symbols from stocks.json
    let symbols = load_enabled_stocks().expect("Failed to load stocks from config/stocks.json");
    let rate = config.volatility_surface.risk_free_rate;
    
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
