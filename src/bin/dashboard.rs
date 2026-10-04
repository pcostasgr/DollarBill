//! DollarBill live dashboard — `cargo build --release && target\release\dashboard`
//!
//! Reads `data/bot_status.json` (written by the live bot every tick) and
//! `data/trades.db` (SQLite) to render a real-time terminal UI.
//!
//! Layout
//! ┌─ Header ─ mode / circuit-breaker / daily-loss ─────────────────────┐
//! ├─ Open Positions ─────────────────┬─ Last Signals ───────────────────┤
//! ├─ Portfolio Greeks ──────────────────────────────────────────────────┤
//! ├─ Recent Orders ────────────────────────────────────────────────────┤
//! └─ Footer ─ keybindings / last-updated ──────────────────────────────┘

use std::{
    io,
    time::{Duration, Instant},
};

use crossterm::{
    event::{self, Event, KeyCode},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Cell, Paragraph, Row, Table},
    Terminal,
};

use dollarbill::persistence::{BotStatus, PositionRecord, TradeRecord, TradeStore};

// ─── App state ────────────────────────────────────────────────────────────

struct App {
    status:    BotStatus,
    positions: Vec<PositionRecord>,
    trades:    Vec<TradeRecord>,
    last_poll: Instant,
    db_path:   String,
}

impl App {
    fn new(db_path: String) -> Self {
        Self {
            status:    BotStatus::default(),
            positions: vec![],
            trades:    vec![],
            last_poll: Instant::now() - Duration::from_secs(5),
            db_path,
        }
    }

    /// Refresh data from JSON status file and SQLite.
    async fn refresh(&mut self) {
        self.last_poll = Instant::now();

        // JSON status (best-effort)
        if let Some(s) = BotStatus::read() {
            self.status = s;
        }

        // SQLite positions + recent orders
        if let Ok(store) = TradeStore::new(&self.db_path).await {
            self.positions = store.get_open_positions().await.unwrap_or_default();
            self.trades = store.get_recent_orders(20).await.unwrap_or_default();
        }
    }
}

// ─── Rendering ────────────────────────────────────────────────────────────

fn render(f: &mut ratatui::Frame, app: &App) {
    let area = f.area();

    // Outer layout: header / middle / greeks / trades / footer
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),  // header
            Constraint::Min(6),     // positions + signals (side by side)
            Constraint::Length(3),  // greeks
            Constraint::Length(8),  // recent orders
            Constraint::Length(1),  // footer
        ])
        .split(area);

    render_header(f, app, chunks[0]);
    render_middle(f, app, chunks[1]);
    render_greeks(f, app, chunks[2]);
    render_orders(f, app, chunks[3]);
    render_footer(f, app, chunks[4]);
}

// ── Header ─────────────────────────────────────────────────────────────────

fn render_header(f: &mut ratatui::Frame, app: &App, area: Rect) {
    let s = &app.status;

    let mode_style = if s.dry_run {
        Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)
    };
    let mode_str = if s.dry_run { "DRY-RUN" } else { "LIVE" };

    let cb_style = if s.circuit_broken {
        Style::default().fg(Color::Red).add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(Color::Green)
    };
    let cb_str = if s.circuit_broken { "🔴 TRIPPED" } else { "✅ OK" };

    let loss_pct = if s.max_daily_loss > 0.0 {
        s.estimated_daily_loss / s.max_daily_loss * 100.0
    } else {
        0.0
    };
    let loss_color = if loss_pct >= 80.0 { Color::Red } else if loss_pct >= 50.0 { Color::Yellow } else { Color::Green };

    let line = Line::from(vec![
        Span::raw("  Mode: "),
        Span::styled(mode_str, mode_style),
        Span::raw("   CB: "),
        Span::styled(cb_str, cb_style),
        Span::raw(format!("   Daily Loss: ")),
        Span::styled(
            format!("${:.2} / ${:.2}  ({:.0}%)", s.estimated_daily_loss, s.max_daily_loss, loss_pct),
            Style::default().fg(loss_color),
        ),
        Span::raw(format!("   Equity: ${:.2}   Positions: {}   Orders: {}",
            s.equity, s.open_position_count, s.session_orders)),
    ]);

    let block = Block::default()
        .title(" DollarBill Dashboard ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan));

    let p = Paragraph::new(line).block(block);
    f.render_widget(p, area);
}

// ── Middle: open positions (left) + last signals (right) ──────────────────

fn render_middle(f: &mut ratatui::Frame, app: &App, area: Rect) {
    let cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(55), Constraint::Percentage(45)])
        .split(area);

    render_positions(f, app, cols[0]);
    render_signals(f, app, cols[1]);
}

fn render_positions(f: &mut ratatui::Frame, app: &App, area: Rect) {
    let header = Row::new(vec!["Symbol", "Qty", "Entry $", "Strategy", "Expires"])
        .style(Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD))
        .height(1);

    let rows: Vec<Row> = app.positions.iter().map(|p| {
        Row::new(vec![
            Cell::from(p.symbol.clone()),
            Cell::from(format!("{:.0}", p.qty)),
            Cell::from(format!("{:.2}", p.entry_price)),
            Cell::from(p.strategy.as_deref().unwrap_or("—").to_string()),
            Cell::from(p.expires_at.as_deref().unwrap_or("—").to_string()),
        ])
    }).collect();

    let widths = [
        Constraint::Length(6),
        Constraint::Length(5),
        Constraint::Length(9),
        Constraint::Min(14),
        Constraint::Length(12),
    ];

    let table = Table::new(rows, widths)
        .header(header)
        .block(Block::default()
            .title(format!(" Open Positions ({}) ", app.positions.len()))
            .borders(Borders::ALL))
        .row_highlight_style(Style::default().add_modifier(Modifier::REVERSED));

    f.render_widget(table, area);
}

fn render_signals(f: &mut ratatui::Frame, app: &App, area: Rect) {
    let header = Row::new(vec!["Symbol", "Last Signal"])
        .style(Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD))
        .height(1);

    let mut entries: Vec<(&String, &String)> = app.status.last_signals.iter().collect();
    entries.sort_by_key(|(k, _)| k.as_str());

    let rows: Vec<Row> = entries.iter().map(|(sym, desc)| {
        Row::new(vec![
            Cell::from(sym.to_string()),
            Cell::from(desc.to_string()),
        ])
    }).collect();

    let widths = [Constraint::Length(7), Constraint::Min(20)];

    let table = Table::new(rows, widths)
        .header(header)
        .block(Block::default()
            .title(" Last Signals ")
            .borders(Borders::ALL));

    f.render_widget(table, area);
}

// ── Portfolio Greeks bar ───────────────────────────────────────────────────

fn render_greeks(f: &mut ratatui::Frame, app: &App, area: Rect) {
    let s = &app.status;

    let delta_color = if s.portfolio_delta.abs() > 3.0 { Color::Yellow } else { Color::Green };

    let line = Line::from(vec![
        Span::raw("  Portfolio Greeks:  "),
        Span::raw("Δ "),
        Span::styled(format!("{:+.3}", s.portfolio_delta),
            Style::default().fg(delta_color).add_modifier(Modifier::BOLD)),
        Span::raw("  |  Γ "),
        Span::styled(format!("{:.4}", s.portfolio_gamma), Style::default().fg(Color::Cyan)),
        Span::raw("  |  Vega "),
        Span::styled(format!("${:.0}", s.portfolio_vega), Style::default().fg(Color::Magenta)),
        Span::raw("  |  Θ "),
        Span::styled(format!("${:.0}/day", s.portfolio_theta), Style::default().fg(Color::Red)),
    ]);

    let block = Block::default()
        .title(" Greeks ")
        .borders(Borders::ALL);

    f.render_widget(Paragraph::new(line).block(block), area);
}

// ── Recent orders ─────────────────────────────────────────────────────────

fn render_orders(f: &mut ratatui::Frame, app: &App, area: Rect) {
    let header = Row::new(vec!["Time", "Symbol", "Action", "Qty", "Price $", "Status", "Strategy"])
        .style(Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD))
        .height(1);

    let rows: Vec<Row> = app.trades.iter().map(|t| {
        let ts = if t.timestamp.len() >= 19 { &t.timestamp[11..19] } else { &t.timestamp };
        let status_color = match t.fill_status.as_deref() {
            Some("filled")    => Color::Green,
            Some("submitted") => Color::Yellow,
            Some("error") | Some("rejected") => Color::Red,
            _ => Color::White,
        };
        Row::new(vec![
            Cell::from(ts.to_string()),
            Cell::from(t.symbol.clone()),
            Cell::from(t.action.clone()),
            Cell::from(format!("{:.0}", t.quantity)),
            Cell::from(format!("{:.2}", t.price)),
            Cell::from(t.fill_status.as_deref().unwrap_or("—").to_string())
                .style(Style::default().fg(status_color)),
            Cell::from(t.strategy.as_deref().unwrap_or("—").to_string()),
        ])
    }).collect();

    let widths = [
        Constraint::Length(9),
        Constraint::Length(7),
        Constraint::Length(8),
        Constraint::Length(5),
        Constraint::Length(9),
        Constraint::Length(11),
        Constraint::Min(14),
    ];

    let table = Table::new(rows, widths)
        .header(header)
        .block(Block::default()
            .title(" Recent Orders (last 20) ")
            .borders(Borders::ALL));

    f.render_widget(table, area);
}

// ── Footer ────────────────────────────────────────────────────────────────

fn render_footer(f: &mut ratatui::Frame, app: &App, area: Rect) {
    let updated = if app.status.updated_at.len() >= 19 {
        &app.status.updated_at[11..19]
    } else {
        &app.status.updated_at
    };

    let line = Line::from(vec![
        Span::styled(" [q] ", Style::default().fg(Color::Yellow)),
        Span::raw("quit  "),
        Span::styled("[r] ", Style::default().fg(Color::Yellow)),
        Span::raw("refresh now  "),
        Span::raw(format!("  ↻ auto-refresh 1s   last bot write: {}", updated)),
    ]);

    f.render_widget(Paragraph::new(line), area);
}

// ─── Main ─────────────────────────────────────────────────────────────────

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let db_path = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "data/trades.db".to_string());

    // Setup terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let mut app = App::new(db_path);
    app.refresh().await;

    let tick = Duration::from_secs(1);
    let mut last_tick = Instant::now();

    loop {
        terminal.draw(|f| render(f, &app))?;

        // Poll for input with a short timeout so the loop stays snappy
        let timeout = tick.saturating_sub(last_tick.elapsed());
        if event::poll(timeout)? {
            if let Event::Key(key) = event::read()? {
                match key.code {
                    KeyCode::Char('q') | KeyCode::Char('Q') | KeyCode::Esc => break,
                    KeyCode::Char('r') | KeyCode::Char('R') => {
                        app.refresh().await;
                    }
                    _ => {}
                }
            }
        }

        if last_tick.elapsed() >= tick {
            app.refresh().await;
            last_tick = Instant::now();
        }
    }

    // Restore terminal
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use dollarbill::execution::store::EventStore;
    use ratatui::backend::TestBackend;

    fn trade(action: &str) -> TradeRecord {
        TradeRecord {
            symbol: "AAPL".into(), action: action.into(), quantity: 2.0, price: 195.25,
            order_id: Some("dashboard-contract".into()), fill_status: Some("filled".into()),
            strategy: Some("Momentum".into()), error_message: None,
            timestamp: "2026-10-04T09:30:00Z".into(), spot_price: Some(195.25),
            iv_at_fill: None, delta_at_fill: None, vega_at_fill: None, theta_at_fill: None,
        }
    }

    fn screen(app: &App, width: u16, height: u16) -> String {
        let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
        terminal.draw(|f| render(f, app)).unwrap();
        terminal.backend().buffer().content.iter().map(|cell| cell.symbol()).collect()
    }

    #[tokio::test]
    async fn legacy_dashboard_reads_positions_and_orders_with_execution_schema_present() {
        let path = std::env::temp_dir().join(format!("dashboard-contract-{}-{}.db",
            std::process::id(), std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
        let db_path = path.to_str().unwrap();
        // The new append-only journal must coexist with the dashboard's legacy tables.
        let journal = EventStore::open(db_path).await.unwrap();
        let store = TradeStore::new(db_path).await.unwrap();
        store.upsert_position(&PositionRecord {
            symbol: "AAPL".into(), qty: 2.0, entry_price: 195.25,
            entry_date: "2026-10-04".into(), strategy: Some("Momentum".into()),
            expires_at: None, premium_collected: None, occ_symbol: None,
            roll_count: 0, legs: vec![],
        }).await.unwrap();
        store.insert_trade(&trade("buy")).await.unwrap();
        for _ in 0..25 {
            store.insert_trade(&trade("tick")).await.unwrap();
        }
        let history = store.get_trade_history(20).await.unwrap();
        assert_eq!(history.len(), 20);
        assert!(history.iter().all(|record| record.action == "tick"));
        let mut app = App::new(db_path.into());
        app.refresh().await;
        assert_eq!(app.positions.len(), 1);
        assert_eq!(app.positions[0].qty, 2.0);
        assert_eq!(app.trades.len(), 1);
        assert_eq!(app.trades[0].action, "buy");
        let rendered = screen(&app, 160, 35);
        assert!(rendered.contains("AAPL"));
        assert!(rendered.contains("195.25"));
        assert!(rendered.contains("filled"));
        store.close_position("AAPL").await.unwrap();
        app.refresh().await;
        assert!(app.positions.is_empty());
        drop(store);
        journal.close().await;
        // This uniquely-created scratch file is the only file removed.
        std::fs::remove_file(path).unwrap();
    }

    #[test]
    fn bot_status_contract_renders_risk_signals_and_orders() {
        let status = BotStatus {
            updated_at: "2026-10-04T09:30:00Z".into(), dry_run: true,
            circuit_broken: true, estimated_daily_loss: 80.0, max_daily_loss: 100.0,
            equity: 10000.0, open_position_count: 1, session_orders: 1,
            last_signals: [("AAPL".into(), "Momentum buy".into())].into(),
            portfolio_delta: 2.0, portfolio_gamma: 0.125,
            portfolio_vega: 15.0, portfolio_theta: -3.0,
        };
        let json = serde_json::to_string(&status).unwrap();
        let mut app = App::new(":memory:".into());
        app.status = serde_json::from_str(&json).unwrap();
        app.trades.push(trade("buy"));
        let rendered = screen(&app, 180, 35);
        for expected in ["DRY-RUN", "TRIPPED", "$80.00 / $100.00", "Momentum buy",
            "0.1250", "09:30:00", "filled"] {
            assert!(rendered.contains(expected), "missing {expected}");
        }
    }

    #[test]
    fn empty_dashboard_renders_at_small_terminal_sizes() {
        let app = App::new(":memory:".into());
        for (width, height) in [(160, 35), (80, 24), (30, 10), (1, 1)] {
            screen(&app, width, height);
        }
    }
}
