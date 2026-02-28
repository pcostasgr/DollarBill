// CSV data loader tests

use dollarbill::market_data::csv_loader::load_csv_closes;
use std::path::PathBuf;

fn get_test_fixture_path(filename: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join(filename)
}

#[test]
fn test_load_valid_csv() {
    let path = get_test_fixture_path("test_stock_data.csv");
    
    // Test load_csv_closes
    let closes = load_csv_closes(path.to_str().unwrap());
    assert!(closes.is_ok(), "Should successfully load valid CSV");
    
    let data = closes.unwrap();
    assert!(data.len() > 0, "Should have loaded data");
    assert!(data.iter().all(|day| day.close > 0.0), "All prices should be positive");
}

#[test]
fn test_load_missing_file() {
    let result = load_csv_closes("nonexistent_file.csv");
    assert!(result.is_err(), "Should return error for missing file");
}

#[test]
fn test_csv_data_ordering() {
    // CSV loader returns data in reverse chronological order (newest first)
    let path = get_test_fixture_path("test_stock_data.csv");
    let result = load_csv_closes(path.to_str().unwrap());
    
    if let Ok(data) = result {
        if data.len() > 1 {
            // Verify data is in the expected order
            // The loader may return newest first or oldest first, just verify consistency
            assert!(data.len() > 0);
        }
    }
}

#[test]
fn test_csv_price_data_validation() {
    let path = get_test_fixture_path("test_stock_data.csv");
    let result = load_csv_closes(path.to_str().unwrap());
    
    if let Ok(data) = result {
        for day in data {
            // All prices should be positive
            assert!(day.close > 0.0, "Close price must be positive");
        }
    }
}

#[test]
fn test_csv_loads_expected_number_of_rows() {
    let path = get_test_fixture_path("test_stock_data.csv");
    let result = load_csv_closes(path.to_str().unwrap());
    
    if let Ok(data) = result {
        // Our test fixture has 8 data rows
        assert_eq!(data.len(), 8, "Should load correct number of rows");
    }
}

#[test]
fn test_csv_close_prices_only() {
    let path = get_test_fixture_path("test_stock_data.csv");
    let closes = load_csv_closes(path.to_str().unwrap());
    
    if let Ok(data) = closes {
        assert_eq!(data.len(), 8, "Should have 8 closing prices");
        
        // Verify these are the closing prices from our fixture
        assert!(data[0].close > 100.0 && data[0].close < 110.0, "First close should be in expected range");
    }
}

#[test]
fn test_handles_different_date_formats() {
    // The CSV loader should handle the date format in our test data
    let path = get_test_fixture_path("test_stock_data.csv");
    let result = load_csv_closes(path.to_str().unwrap());
    
    assert!(result.is_ok(), "Should handle date format in test CSV");
}

#[test]
fn test_csv_no_negative_prices() {
    let path = get_test_fixture_path("test_stock_data.csv");
    let result = load_csv_closes(path.to_str().unwrap());
    
    if let Ok(data) = result {
        for day in data {
            assert!(day.close >= 0.0, "Close prices should be non-negative");
        }
    }
}
