// Short options signal scanner
//
// Demonstrates signal generation for short-side strategies:
//   • SellCall / SellPut via ShortStrangleStrategy
//   • Iron Condor, Credit Call Spread via SpreadConfig
//   • Covered Call via detect_covered_calls
//
// Usage:
//   cargo run --example short_options_signals
//   cargo run --example short_options_signals -- --symbol NVDA

use dollarbill::market_data::csv_loader::load_csv_closes;
use dollarbill::market_data::symbols::load_enabled_stocks;
use dollarbill::models::bs_mod::compute_historical_vol;
use dollarbill::strategies::short_strangle::ShortStrangleStrategy;
use dollarbill::strategies::spreads::{generate_spread_signals, SpreadConfig};
use dollarbill::strategies::{SignalAction, StrategyRegistry};
use std::env;

fn main() {
    // ── Symbol list ──────────────────────────────────────────────────────────
    let cli_symbol = env::args().skip(1).find(|a| a != "--symbol").or_else(|| {
        let mut it = env::args().skip(1);
        while let Some(a) = it.next() {
            if a == "--symbol" {
                return it.next();
            }
        }
        None
    });

    let symbols: Vec<String> = if let Some(sym) = cli_symbol {
        vec![sym.to_uppercase()]
    } else {
        load_enabled_stocks()
            .unwrap_or_else(|_| vec!["AAPL".into(), "TSLA".into(), "NVDA".into()])
    };

    // ── Strategy registry ────────────────────────────────────────────────────
    let strangle = ShortStrangleStrategy {
        min_iv_rank: 0.40,    // enter when vol ≥ 40th percentile
        max_delta: 0.25,      // OTM options only
        min_days_to_expiry: 7,
        max_days_to_expiry: 45,
        profit_target_pct: 50.0,
        stop_loss_pct: 200.0,
    };
    let mut registry = StrategyRegistry::new();
    registry.register(Box::new(strangle));

    // ── Spread config ────────────────────────────────────────────────────────
    let spread_config = SpreadConfig {
        min_premium_threshold: 0.30,
        max_spread_width_pct: 20.0,
        min_days_to_expiry: 7,
        max_days_to_expiry: 45,
        min_volume: 5,
        max_spread_pct: 30.0,
        risk_free_rate: 0.045,
        iv_rank_threshold: 0.0, // gate disabled — scanner mode
    };

    println!("╔══════════════════════════════════════════════════════════════╗");
    println!("║              SHORT OPTIONS SIGNAL SCANNER                   ║");
    println!("╚══════════════════════════════════════════════════════════════╝");
    println!();

    let mut total = 0usize;

    for symbol in &symbols {
        let csv_path = format!("data/{}_five_year.csv", symbol.to_lowercase());
        let history = match load_csv_closes(&csv_path) {
            Ok(h) if h.len() >= 30 => h,
            _ => {
                eprintln!("[SKIP] {}: no CSV data", symbol);
                continue;
            }
        };

        let closes: Vec<f64> = history.iter().map(|d| d.close).collect();
        let spot = *closes.last().unwrap();
        let hv = compute_historical_vol(&closes);
        // Use a mild IV premium over historical vol as a proxy market IV
        let market_iv = hv * 1.15;

        println!("┌─ {} ─────────────────────────────────────────────────", symbol);
        println!("│  Spot: ${:.2}   HV-21: {:.1}%   proxy market IV: {:.1}%",
            spot, hv * 100.0, market_iv * 100.0);

        // ── Strategy-based signals (SellCall / SellPut) ──────────────────────
        let strategy_signals = registry.generate_all_signals(
            symbol, spot, market_iv, hv, hv,
        );

        let actionable: Vec<_> = strategy_signals
            .iter()
            .filter(|s| s.confidence >= 0.45 && !matches!(s.action, SignalAction::NoAction))
            .collect();

        if actionable.is_empty() {
            println!("│  [strategy] No signals above 45% confidence");
        }
        for sig in &actionable {
            print_signal(symbol, sig.confidence, sig.edge, &sig.action, &sig.strategy_name);
            total += 1;
        }

        // ── Spread signals (iron condor, credit spread, covered call) ─────
        match generate_spread_signals(symbol, &spread_config) {
            Ok(spread_signals) => {
                if spread_signals.is_empty() {
                    println!("│  [spreads]  No spread signals (no options data?)");
                }
                for sig in &spread_signals {
                    print_signal(symbol, sig.confidence, sig.edge, &sig.action, &sig.strategy_name);
                    total += 1;
                }
            }
            Err(e) => println!("│  [spreads]  {}", e),
        }

        println!("└──────────────────────────────────────────────────────────");
        println!();
    }

    println!("══════════════════════════════════════════════════════════════");
    println!("  Total short signals generated: {}", total);
    println!("══════════════════════════════════════════════════════════════");
}

fn print_signal(symbol: &str, confidence: f64, edge: f64, action: &SignalAction, strategy: &str) {
    let desc = describe_action(action);
    println!("│  [{:>5.1}%] {:<28}  edge: ${:.2}  ({})",
        confidence * 100.0, desc, edge, strategy);
    let _ = symbol; // available if caller needs it
}

fn describe_action(action: &SignalAction) -> String {
    match action {
        SignalAction::SellCall { strike, days_to_expiry, .. } =>
            format!("SELL CALL  K={:.0}  DTE={}", strike, days_to_expiry),
        SignalAction::SellPut { strike, days_to_expiry, .. } =>
            format!("SELL PUT   K={:.0}  DTE={}", strike, days_to_expiry),
        SignalAction::IronCondor { sell_call_strike, sell_put_strike, days_to_expiry, .. } =>
            format!("IRON CONDOR {:.0}/{:.0}  DTE={}", sell_put_strike, sell_call_strike, days_to_expiry),
        SignalAction::CreditCallSpread { sell_strike, buy_strike, days_to_expiry } =>
            format!("CREDIT CALL {:.0}/{:.0}  DTE={}", sell_strike, buy_strike, days_to_expiry),
        SignalAction::CreditPutSpread { sell_strike, buy_strike, days_to_expiry } =>
            format!("CREDIT PUT  {:.0}/{:.0}  DTE={}", sell_strike, buy_strike, days_to_expiry),
        SignalAction::CoveredCall { sell_strike, days_to_expiry } =>
            format!("COVERED CALL K={:.0}  DTE={}", sell_strike, days_to_expiry),
        SignalAction::SellStraddle { strike, days_to_expiry } =>
            format!("SELL STRADDLE K={:.0}  DTE={}", strike, days_to_expiry),
        SignalAction::CashSecuredPut { strike, days_to_expiry } =>
            format!("CASH-SEC PUT K={:.0}  DTE={}", strike, days_to_expiry),
        other => format!("{:?}", other),
    }
}
