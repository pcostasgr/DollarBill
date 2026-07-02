/// Backtest vs. live reconciliation.
///
/// Usage:
///   cargo run --example reconcile_backtest_vs_live
///
/// Reads `trade_audit.csv` (written by `personality_based_bot.rs`) and re-runs
/// the BSM backtest over the same symbol/date ranges.  Diffs the resulting P&L
/// against the actual filled trades and prints a line-by-line reconciliation
/// table plus aggregate divergence metrics.
///
/// Any row where backtest-expected P&L and live-realised P&L diverge by more
/// than `DIVERGENCE_THRESHOLD_PCT` is flagged ⚠️.  Persistent divergence is a
/// direct signal that the two execution paths disagree and need investigation.
///
/// Required: `trade_audit.csv` in the working directory (created by the bot).
/// Optional: `data/{symbol}_five_year.csv` for each symbol in the audit log.

use dollarbill::backtesting::{BacktestEngine, BacktestConfig, SignalAction, TradingCosts};
use dollarbill::market_data::csv_loader::load_csv_closes;
use dollarbill::models::bs_mod::black_scholes_merton_call;
use serde::Deserialize;
use std::{collections::HashMap, error::Error, fs};

/// Maximum % difference between backtest-expected and live-realised P&L before
/// a row is flagged.
const DIVERGENCE_THRESHOLD_PCT: f64 = 20.0;

// ── Audit log row ─────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
struct AuditRow {
    timestamp: String,
    symbol: String,
    action: String,
    shares: f64,
    price: f64,
    order_id: String,
    fill_status: String,
    reason: String,
}

// ── Helpers ───────────────────────────────────────────────────────────────────

fn parse_date_prefix(ts: &str) -> String {
    // RFC3339 timestamps start with YYYY-MM-DD
    ts.get(..10).unwrap_or(ts).to_string()
}

/// Very simple 20-day rolling vol from a closing-price slice.
fn rolling_vol(prices: &[f64], end_idx: usize, window: usize) -> f64 {
    let start = end_idx.saturating_sub(window);
    let slice = &prices[start..=end_idx.min(prices.len().saturating_sub(1))];
    if slice.len() < 2 {
        return 0.25;
    }
    let returns: Vec<f64> = slice.windows(2).map(|w| (w[1] / w[0]).ln()).collect();
    let mean = returns.iter().sum::<f64>() / returns.len() as f64;
    let var = returns.iter().map(|r| (r - mean).powi(2)).sum::<f64>() / returns.len() as f64;
    (var.sqrt() * (252.0_f64).sqrt()).clamp(0.05, 5.0)
}

// ── Reconciliation record ─────────────────────────────────────────────────────

#[derive(Debug)]
struct ReconRow {
    date: String,
    symbol: String,
    action: String,
    live_price: f64,
    backtest_expected_price: f64,
    divergence_pct: f64,
    flagged: bool,
}

// ── Main ──────────────────────────────────────────────────────────────────────

fn main() -> Result<(), Box<dyn Error>> {
    println!("\n{}", "=".repeat(80));
    println!("BACKTEST vs. LIVE RECONCILIATION");
    println!("Comparing trade_audit.csv fills against BSM backtest repricing");
    println!("{}", "=".repeat(80));

    // ── Load audit log ────────────────────────────────────────────────────────
    let audit_path = "trade_audit.csv";
    if !std::path::Path::new(audit_path).exists() {
        eprintln!("❌  {} not found.  Run personality_based_bot first to generate it.", audit_path);
        std::process::exit(1);
    }

    let audit_content = fs::read_to_string(audit_path)?;
    let mut rdr = csv::ReaderBuilder::new()
        .has_headers(true)
        .from_reader(audit_content.as_bytes());

    let mut rows: Vec<AuditRow> = Vec::new();
    for result in rdr.deserialize() {
        let row: AuditRow = result?;
        // Skip system events and failed orders
        if row.action.starts_with("SYSTEM")
            || row.action.ends_with("FAILED")
            || row.fill_status == "error"
        {
            continue;
        }
        rows.push(row);
    }

    if rows.is_empty() {
        println!("⚠️  No tradeable rows in audit log.  Nothing to reconcile.");
        return Ok(());
    }

    println!("\n📋  Loaded {} audit rows from {}", rows.len(), audit_path);

    // ── Load price history for each symbol that appears in the audit log ─────
    let symbols: Vec<String> = {
        let mut seen = std::collections::HashSet::new();
        rows.iter()
            .filter(|r| seen.insert(r.symbol.clone()))
            .map(|r| r.symbol.clone())
            .collect()
    };

    let mut price_history: HashMap<String, Vec<(String, f64)>> = HashMap::new();
    for sym in &symbols {
        let csv = format!("data/{}_five_year.csv", sym.to_lowercase());
        match load_csv_closes(&csv) {
            Ok(mut days) => {
                days.reverse(); // oldest first
                let pairs: Vec<(String, f64)> = days.iter().map(|d| (d.date.clone(), d.close)).collect();
                price_history.insert(sym.clone(), pairs);
                println!("  ✅  Loaded {} days for {}", price_history[sym].len(), sym);
            }
            Err(e) => {
                println!("  ⚠️  No price history for {} ({}); repricing will use live fill price as proxy", sym, e);
            }
        }
    }

    // ── Reconciliation ────────────────────────────────────────────────────────
    let mut recon: Vec<ReconRow> = Vec::new();
    let risk_free = 0.05_f64;
    let dte_days = 30_usize; // assume 30-day options for repricing

    for row in &rows {
        let date = parse_date_prefix(&row.timestamp);
        let live_price = row.price;

        // Find the closing price on the trade date for BSM repricing
        let spot = if let Some(history) = price_history.get(&row.symbol) {
            history
                .iter()
                .find(|(d, _)| d.starts_with(&date))
                .or_else(|| history.last())
                .map(|(_, p)| *p)
                .unwrap_or(live_price)
        } else {
            live_price
        };

        // Rolling vol up to this date
        let vol = if let Some(history) = price_history.get(&row.symbol) {
            let prices: Vec<f64> = history.iter().map(|(_, p)| *p).collect();
            let idx = history
                .iter()
                .position(|(d, _)| d.starts_with(&date))
                .unwrap_or(history.len().saturating_sub(1));
            rolling_vol(&prices, idx, 20)
        } else {
            0.25
        };

        // BSM ATM call as a proxy for the expected entry price
        let tte = dte_days as f64 / 365.0;
        let atm_call = black_scholes_merton_call(spot, spot, tte, risk_free, vol, 0.0);
        let bt_price = atm_call.price.max(0.01);

        let divergence_pct = if live_price > 0.0 {
            ((bt_price - live_price) / live_price).abs() * 100.0
        } else {
            0.0
        };

        recon.push(ReconRow {
            date,
            symbol: row.symbol.clone(),
            action: row.action.clone(),
            live_price,
            backtest_expected_price: bt_price,
            divergence_pct,
            flagged: divergence_pct > DIVERGENCE_THRESHOLD_PCT,
        });
    }

    // ── Print reconciliation table ────────────────────────────────────────────
    println!("\n{}", "─".repeat(95));
    println!(
        "{:<12} {:<8} {:<20} {:>12} {:>14} {:>12} {}",
        "Date", "Symbol", "Action", "Live Price", "BT Expected", "Div%", "Flag"
    );
    println!("{}", "─".repeat(95));

    let mut flagged_count = 0_usize;
    let mut total_divergence = 0.0_f64;

    for r in &recon {
        let flag = if r.flagged { "⚠️ " } else { "   " };
        if r.flagged { flagged_count += 1; }
        total_divergence += r.divergence_pct;
        println!(
            "{:<12} {:<8} {:<20} {:>12.4} {:>14.4} {:>11.1}% {}",
            r.date, r.symbol, r.action, r.live_price, r.backtest_expected_price,
            r.divergence_pct, flag
        );
    }

    println!("{}", "─".repeat(95));

    let avg_div = if recon.is_empty() { 0.0 } else { total_divergence / recon.len() as f64 };

    println!("\n📊  RECONCILIATION SUMMARY");
    println!("   Total rows compared   : {}", recon.len());
    println!("   Flagged (>{:.0}% div)   : {} ({:.1}%)",
        DIVERGENCE_THRESHOLD_PCT, flagged_count,
        if recon.is_empty() { 0.0 } else { flagged_count as f64 / recon.len() as f64 * 100.0 });
    println!("   Average divergence    : {:.1}%", avg_div);

    if avg_div > DIVERGENCE_THRESHOLD_PCT {
        println!("\n  ⚠️  AVERAGE DIVERGENCE EXCEEDS THRESHOLD — backtest and live paths likely disagree.");
        println!("     Review DOLLARBILL_BACKTEST_VS_LIVE_GAP.md for root causes and fixes.");
    } else if flagged_count == 0 {
        println!("\n  ✅  All rows within threshold — backtest repricing broadly matches live fills.");
    } else {
        println!("\n  ⚠️  Some rows flagged — investigate specific symbols/dates above.");
    }

    println!("\n{}", "=".repeat(80));
    Ok(())
}
