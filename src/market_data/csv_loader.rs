// src/csv_loader.rs
// Generic CSV loader for Yahoo Finance format (Date,Open,High,Low,Close,Volume,Dividends,Stock Splits)
// Returns vector of HistoricalDay with closes, newest first

use std::error::Error;
use std::fs::File;
use csv::ReaderBuilder;

#[derive(Debug, Clone)]
pub struct HistoricalDay {
    pub date: String,
    pub close: f64,
}

/// Load closes from any Yahoo-style CSV
/// filename: &str — path to CSV (e.g., "data/tesla_one_year.csv")
pub fn load_csv_closes(filename: &str) -> Result<Vec<HistoricalDay>, Box<dyn Error>> {
    let file = File::open(filename)?;
    let mut rdr = ReaderBuilder::new()
        .flexible(true)      // Handles varying column counts
        .trim(csv::Trim::All)  // Trim whitespace
        .has_headers(true)
        .from_reader(file);

    let mut days = Vec::new();
    for result in rdr.records() {
        let record = result?;
        if record.len() < 5 { continue; }  // Skip short rows

        let date = record[0].trim().to_string();
        let close_str = record[4].trim();

        if close_str.is_empty() || close_str == "null" || close_str == "N/A" {
            continue;  // Skip bad data
        }

        let close: f64 = match close_str.parse() {
            Ok(v) => v,
            Err(_) => {
                eprintln!("Skipping invalid Close '{}' on {}", close_str, date);
                continue;
            }
        };

        days.push(HistoricalDay { date, close });
    }

    days.reverse();  // Newest first
    if days.is_empty() {
        return Err("No valid data parsed — check CSV format".into());
    }
    Ok(days)
}

/// Optional test function
pub fn test_load_csv() -> Result<(), Box<dyn Error>> {
    let filename = "tesla_one_year.csv";
    let history = load_csv_closes(filename)?;
    
    println!("Loaded {} trading days from {}", history.len(), filename);
    if let Some(first) = history.first() {
        println!("Most recent close: {:.2} on {}", first.close, first.date);
    }
    Ok(())
}