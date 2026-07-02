/// Evaluate Alpaca paper trading performance.
///
/// Usage:
///   cargo run --example evaluate_paper_trading
///
/// Reads config/paper_trading_config.json for initial_balance and
/// commission_per_trade, then fetches live account state and order
/// history from Alpaca to produce a full performance report.
///
/// Required env vars:
///   ALPACA_API_KEY     — Alpaca paper trading key
///   ALPACA_API_SECRET  — Alpaca paper trading secret
#![allow(dead_code)]

use dollarbill::alpaca::AlpacaClient;
use serde::Deserialize;
use std::{collections::HashMap, error::Error, fs};

// ── Config ────────────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
struct PaperTradingConfig {
    paper_trading: PaperTradingSettings,
    trading: TradingConfig,
}

#[derive(Debug, Deserialize)]
struct PaperTradingSettings {
    initial_balance: f64,
    commission_per_trade: f64,
}

#[derive(Debug, Deserialize)]
struct TradingConfig {
    #[allow(dead_code)]
    position_size_shares: f64,
}

// ── Trade record (a matched buy→sell pair) ────────────────────────────────────

#[derive(Debug)]
struct ClosedTrade {
    symbol: String,
    qty: f64,
    entry_price: f64,
    exit_price: f64,
    entry_time: String,
    exit_time: String,
    gross_pnl: f64,
    commission: f64,
    net_pnl: f64,
}

impl ClosedTrade {
    fn pnl_pct(&self) -> f64 {
        let cost = self.entry_price * self.qty;
        if cost == 0.0 {
            0.0
        } else {
            (self.net_pnl / cost) * 100.0
        }
    }
    fn is_winner(&self) -> bool {
        self.net_pnl > 0.0
    }
}

// ── Helpers ───────────────────────────────────────────────────────────────────

fn calc_sharpe(returns: &[f64]) -> f64 {
    if returns.len() < 2 {
        return 0.0;
    }
    let mean = returns.iter().sum::<f64>() / returns.len() as f64;
    let variance = returns
        .iter()
        .map(|r| (r - mean).powi(2))
        .sum::<f64>()
        / returns.len() as f64;
    let std_dev = variance.sqrt();
    if std_dev == 0.0 {
        0.0
    } else {
        // Annualise assuming each return is one trade (not daily)
        mean / std_dev * (returns.len() as f64).sqrt()
    }
}

fn calc_max_drawdown(equity_series: &[f64]) -> (f64, f64) {
    let mut peak = 0.0f64;
    let mut max_dd = 0.0f64;
    let mut max_dd_pct = 0.0f64;
    for &eq in equity_series {
        if eq > peak {
            peak = eq;
        }
        if peak > 0.0 {
            let dd = peak - eq;
            let dd_pct = dd / peak * 100.0;
            if dd > max_dd {
                max_dd = dd;
                max_dd_pct = dd_pct;
            }
        }
    }
    (max_dd, max_dd_pct)
}

// ── Main ──────────────────────────────────────────────────────────────────────

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    println!("{}", "=".repeat(80));
    println!("  ALPACA PAPER TRADING — PERFORMANCE EVALUATION");
    println!("{}", "=".repeat(80));
    println!();

    // ── Load config ───────────────────────────────────────────────────────────
    let config_content = fs::read_to_string("config/paper_trading_config.json")
        .map_err(|e| format!("Cannot read config/paper_trading_config.json: {}", e))?;
    let config: PaperTradingConfig = serde_json::from_str(&config_content)
        .map_err(|e| format!("Cannot parse paper_trading_config.json: {}", e))?;

    let initial_balance = config.paper_trading.initial_balance;
    let commission = config.paper_trading.commission_per_trade;

    println!("Config  : initial_balance = ${:.2}, commission = ${:.2}/trade",
             initial_balance, commission);
    println!();

    // ── Connect to Alpaca ─────────────────────────────────────────────────────
    let client = AlpacaClient::from_env()
        .map_err(|e| format!("Alpaca credentials missing: {}", e))?;

    // ── Account snapshot ──────────────────────────────────────────────────────
    let account = client.get_account().await
        .map_err(|e| format!("get_account failed: {}", e))?;

    let portfolio_value: f64 = account.portfolio_value.parse().unwrap_or(0.0);
    let cash: f64 = account.cash.parse().unwrap_or(0.0);
    let equity: f64 = account.equity.parse().unwrap_or(0.0);
    let long_market_value: f64 = account.long_market_value.parse().unwrap_or(0.0);

    // ── Open positions ────────────────────────────────────────────────────────
    let open_positions = client.get_positions().await
        .map_err(|e| format!("get_positions failed: {}", e))?;

    let total_unrealized_pnl: f64 = open_positions
        .iter()
        .filter_map(|p| p.unrealized_pl.parse::<f64>().ok())
        .sum();

    // ── All historical orders ─────────────────────────────────────────────────
    let orders = client.get_all_orders(500).await
        .map_err(|e| format!("get_all_orders failed: {}", e))?;

    // Keep only filled orders, sorted chronologically by fill time
    let mut filled: Vec<_> = orders
        .into_iter()
        .filter(|o| o.status == "filled" && o.filled_at.is_some())
        .collect();
    filled.sort_by(|a, b| a.filled_at.cmp(&b.filled_at));

    // ── Match BUY→SELL pairs (FIFO per symbol) ────────────────────────────────
    // open_lots[symbol] = queue of (qty, fill_price, fill_time) buy lots
    let mut open_lots: HashMap<String, Vec<(f64, f64, String)>> = HashMap::new();
    let mut closed_trades: Vec<ClosedTrade> = Vec::new();

    for order in &filled {
        let qty: f64 = order.filled_qty.parse().unwrap_or(0.0);
        let price: f64 = order
            .filled_avg_price
            .as_deref()
            .and_then(|s| s.parse().ok())
            .unwrap_or(0.0);
        let time = order.filled_at.clone().unwrap_or_default();
        let symbol = order.symbol.clone();

        if order.side == "buy" {
            open_lots
                .entry(symbol)
                .or_default()
                .push((qty, price, time));
        } else if order.side == "sell" {
            // Match against oldest open lot(s) for this symbol (FIFO)
            let lots = open_lots.entry(symbol.clone()).or_default();
            let mut remaining_sell = qty;

            while remaining_sell > 1e-9 && !lots.is_empty() {
                let lot = lots.first_mut().unwrap();
                let matched = remaining_sell.min(lot.0);

                let gross = (price - lot.1) * matched;
                let net = gross - commission * 2.0; // buy + sell commission

                closed_trades.push(ClosedTrade {
                    symbol: symbol.clone(),
                    qty: matched,
                    entry_price: lot.1,
                    exit_price: price,
                    entry_time: lot.2.clone(),
                    exit_time: time.clone(),
                    gross_pnl: gross,
                    commission: commission * 2.0,
                    net_pnl: net,
                });

                lot.0 -= matched;
                remaining_sell -= matched;

                if lot.0 < 1e-9 {
                    lots.remove(0);
                }
            }
        }
    }

    // ── Portfolio history (equity curve + drawdown) ───────────────────────────
    let history = client
        .get_portfolio_history(Some("all"), Some("1D"))
        .await
        .ok(); // non-fatal — compute fallback from trades if unavailable

    let (max_dd_abs, max_dd_pct) = match &history {
        Some(h) if !h.is_empty() => {
            let dd_pct = h.max_drawdown_pct() * 100.0;
            let dd_abs = dd_pct / 100.0 * h.base_value;
            (dd_abs, dd_pct)
        }
        _ => {
            // Fallback: reconstruct equity curve from closed trades
            let mut cumulative = initial_balance;
            let equity_series: Vec<f64> = std::iter::once(initial_balance)
                .chain(closed_trades.iter().map(|t| {
                    cumulative += t.net_pnl;
                    cumulative
                }))
                .collect();
            calc_max_drawdown(&equity_series)
        }
    };

    let sharpe = match &history {
        Some(h) if !h.is_empty() => h.sharpe_ratio(),
        _ => {
            // Fallback: compute from per-trade returns
            let returns: Vec<f64> = closed_trades.iter().map(|t| t.pnl_pct()).collect();
            calc_sharpe(&returns)
        }
    };

    // ── Aggregate trade statistics ────────────────────────────────────────────
    let n_closed = closed_trades.len();
    let winners: Vec<&ClosedTrade> = closed_trades.iter().filter(|t| t.is_winner()).collect();
    let losers: Vec<&ClosedTrade> = closed_trades.iter().filter(|t| !t.is_winner()).collect();

    let n_winners = winners.len();
    let n_losers = losers.len();
    let win_rate = if n_closed > 0 {
        n_winners as f64 / n_closed as f64 * 100.0
    } else {
        0.0
    };

    let gross_profit: f64 = winners.iter().map(|t| t.net_pnl).sum();
    let gross_loss: f64 = losers.iter().map(|t| t.net_pnl.abs()).sum();
    let total_net_pnl: f64 = closed_trades.iter().map(|t| t.net_pnl).sum();
    let total_commissions: f64 = closed_trades.iter().map(|t| t.commission).sum();

    let avg_win = if n_winners > 0 { gross_profit / n_winners as f64 } else { 0.0 };
    let avg_loss = if n_losers > 0 { gross_loss / n_losers as f64 } else { 0.0 };
    let profit_factor = if gross_loss > 0.0 { gross_profit / gross_loss } else { f64::INFINITY };

    let largest_win = winners.iter().map(|t| t.net_pnl).fold(0.0f64, f64::max);
    let largest_loss = losers.iter().map(|t| t.net_pnl.abs()).fold(0.0f64, f64::max);

    // Expectancy: average $ per trade
    let expectancy = if n_closed > 0 { total_net_pnl / n_closed as f64 } else { 0.0 };

    // ── Open position count ───────────────────────────────────────────────────
    let n_open_positions = open_positions.len();

    // Open lots still queued (buys with no matching sell yet)
    let n_open_lots: usize = open_lots.values().map(|v| v.len()).sum();

    // ── Return vs initial balance ─────────────────────────────────────────────
    let total_return_pct = (portfolio_value - initial_balance) / initial_balance * 100.0;
    let realised_return_pct = total_net_pnl / initial_balance * 100.0;

    // ── Print report ──────────────────────────────────────────────────────────

    println!("ACCOUNT SNAPSHOT");
    println!("{}", "-".repeat(60));
    println!("  Initial Balance        : ${:>14.2}", initial_balance);
    println!("  Portfolio Value (now)  : ${:>14.2}", portfolio_value);
    println!("  Cash                   : ${:>14.2}", cash);
    println!("  Long Market Value      : ${:>14.2}", long_market_value);
    println!("  Total Equity           : ${:>14.2}", equity);
    println!("  Unrealised P&L (open)  : ${:>+14.2}", total_unrealized_pnl);
    println!("  Total Return           : {:>+13.2}%", total_return_pct);
    println!();

    println!("PERFORMANCE METRICS  (closed trades)");
    println!("{}", "-".repeat(60));
    println!("  Closed Trades          : {:>14}", n_closed);
    println!("  Open Positions         : {:>14}", n_open_positions);
    println!("  Open Unmatched Lots    : {:>14}", n_open_lots);
    println!();
    println!("  Realised Net P&L       : ${:>+14.2}  ({:>+.2}%)",
             total_net_pnl, realised_return_pct);
    println!("  Total Commissions      : ${:>14.2}", total_commissions);
    println!("  Sharpe Ratio           : {:>14.2}", sharpe);
    println!("  Max Drawdown           : ${:>14.2}  ({:.2}%)",
             max_dd_abs, max_dd_pct);
    println!();

    println!("TRADE STATISTICS");
    println!("{}", "-".repeat(60));
    println!("  Win Rate               : {:>13.1}%", win_rate);
    println!("  Winners / Losers       : {:>7} / {:>5}", n_winners, n_losers);
    println!("  Average Win            : ${:>+14.2}", avg_win);
    println!("  Average Loss           : ${:>14.2}", avg_loss);
    println!("  Largest Win            : ${:>+14.2}", largest_win);
    println!("  Largest Loss           : ${:>14.2}", largest_loss);
    println!("  Profit Factor          : {:>14.2}", profit_factor);
    println!("  Expectancy / Trade     : ${:>+14.2}", expectancy);
    println!();

    // ── Per-symbol breakdown ──────────────────────────────────────────────────
    if !closed_trades.is_empty() {
        println!("PER-SYMBOL BREAKDOWN  (closed trades only)");
        println!("{}", "-".repeat(60));
        println!("{:<8}  {:>6}  {:>9}  {:>8}  {:>8}",
                 "Symbol", "Trades", "Net P&L", "Win%", "Avg P&L");
        println!("{}", "-".repeat(60));

        let mut symbol_stats: HashMap<String, (usize, usize, f64)> = HashMap::new();
        for t in &closed_trades {
            let entry = symbol_stats.entry(t.symbol.clone()).or_insert((0, 0, 0.0));
            entry.0 += 1;
            if t.is_winner() { entry.1 += 1; }
            entry.2 += t.net_pnl;
        }

        let mut symbols: Vec<_> = symbol_stats.iter().collect();
        symbols.sort_by(|a, b| b.1.2.partial_cmp(&a.1.2).unwrap());

        for (sym, (n, wins, pnl)) in symbols {
            let wr = *wins as f64 / *n as f64 * 100.0;
            let avg = pnl / *n as f64;
            println!("{:<8}  {:>6}  {:>+9.2}  {:>7.1}%  {:>+8.2}",
                     sym, n, pnl, wr, avg);
        }
        println!();
    }

    // ── Recent trades (last 10) ───────────────────────────────────────────────
    if !closed_trades.is_empty() {
        let recent_n = 10.min(closed_trades.len());
        println!("LAST {} CLOSED TRADES", recent_n);
        println!("{}", "-".repeat(80));
        println!("{:<8}  {:>6}  {:>9}  {:>9}  {:>10}  {:>8}",
                 "Symbol", "Qty", "Entry", "Exit", "Net P&L", "Result");
        println!("{}", "-".repeat(80));

        for t in closed_trades.iter().rev().take(recent_n) {
            let result = if t.is_winner() { "WIN" } else { "LOSS" };
            println!("{:<8}  {:>6.0}  {:>9.2}  {:>9.2}  {:>+10.2}  {:>8}",
                     t.symbol, t.qty, t.entry_price, t.exit_price, t.net_pnl, result);
        }
        println!();
    }

    // ── Open positions table ──────────────────────────────────────────────────
    if !open_positions.is_empty() {
        println!("OPEN POSITIONS");
        println!("{}", "-".repeat(80));
        println!("{:<8}  {:>6}  {:>9}  {:>9}  {:>12}  {:>8}",
                 "Symbol", "Qty", "Avg Entry", "Cur Price", "Unreal P&L", "P&L %");
        println!("{}", "-".repeat(80));

        let mut open_sorted = open_positions.clone();
        open_sorted.sort_by(|a, b| {
            let pa: f64 = a.unrealized_pl.parse().unwrap_or(0.0);
            let pb: f64 = b.unrealized_pl.parse().unwrap_or(0.0);
            pb.partial_cmp(&pa).unwrap()
        });

        for pos in &open_sorted {
            let qty: f64 = pos.qty.parse().unwrap_or(0.0);
            let entry: f64 = pos.avg_entry_price.parse().unwrap_or(0.0);
            let cur: f64 = pos.current_price.parse().unwrap_or(0.0);
            let upl: f64 = pos.unrealized_pl.parse().unwrap_or(0.0);
            let uplpct: f64 = pos.unrealized_plpc.parse().unwrap_or(0.0) * 100.0;
            println!("{:<8}  {:>6.0}  {:>9.2}  {:>9.2}  {:>+12.2}  {:>+7.1}%",
                     pos.symbol, qty, entry, cur, upl, uplpct);
        }
        println!();
    }

    println!("View live dashboard: https://app.alpaca.markets/paper/dashboard");

    Ok(())
}
