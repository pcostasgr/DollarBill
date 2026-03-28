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
            let hist = store.get_trade_history(20).await.unwrap_or_default();
            self.trades = hist.into_iter().filter(|t| t.action != "tick").collect();
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
