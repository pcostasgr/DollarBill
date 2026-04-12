//! Daily regime + sizing audit log.
//!
//! Every time the pre-trade pipeline runs it appends one
//! [`RegimeSizingAuditEntry`] to the [`AuditLog`].  The log can be exported
//! as structured JSON or CSV.
//!
//! Printed format:
//! ```text
//! 2025-02-28 | Regime:        HighVol | Multiplier: 0.35 | Portfolio Vega: $-2840 | Max DD projected: 1.76%
//! ```

// ─── Entry ───────────────────────────────────────────────────────────────────

/// One trading-day snapshot produced by the pre-trade pipeline.
#[derive(Debug, Clone)]
pub struct RegimeSizingAuditEntry {
    /// Calendar date ("YYYY-MM-DD").
    pub date: String,
    /// `MarketRegime` rendered as a string, e.g. `"HighVol"`.
    pub regime: String,
    /// `RegimeDetector::sizing_multiplier(regime)`.
    pub multiplier: f64,
    /// Net portfolio vega in option-dollar units (`$` per +1 % move in IV).
    pub portfolio_vega: f64,
    /// Net portfolio delta.
    pub portfolio_delta: f64,
    /// Regime-adjusted contract count from `calculate_size_with_regime`.
    pub net_contracts: i32,
    /// Running portfolio equity (normalised to 1.0 at inception).
    pub equity: f64,
    /// `true` when a limit breach triggered auto-flatten on this day.
    pub auto_derisk: bool,
    /// Heuristic worst-case DD: |net_vega| × Δvol(1 %) / equity, as a %.
    pub projected_max_dd_pct: f64,
    /// Reason a new entry was suppressed (e.g. `"HighVol regime: no new entries"`).
    /// `None` when entry was permitted or there was no pending entry.
    pub skip_reason: Option<String>,
}

impl RegimeSizingAuditEntry {
    /// One-liner matching the spec format.
    pub fn summary_line(&self) -> String {
        let skip = match &self.skip_reason {
            Some(r) => format!(" | Action: SKIPPED | Reason: {r}"),
            None    => String::new(),
        };
        format!(
            "{} | Regime: {:>14} | Multiplier: {:.2} | Portfolio Vega: ${:.0} | Max DD projected: {:.2}%{}",
            self.date,
            self.regime,
            self.multiplier,
            self.portfolio_vega,
            self.projected_max_dd_pct,
            skip,
        )
    }
}

// ─── Log ─────────────────────────────────────────────────────────────────────

/// Append-only daily audit log.
#[derive(Debug, Default)]
pub struct AuditLog {
    pub entries: Vec<RegimeSizingAuditEntry>,
}

impl AuditLog {
    pub fn new() -> Self {
        Self::default()
    }

    /// Append a new daily entry.
    pub fn record(&mut self, entry: RegimeSizingAuditEntry) {
        self.entries.push(entry);
    }

    /// Number of days where `auto_derisk` was set.
    pub fn derisk_count(&self) -> usize {
        self.entries.iter().filter(|e| e.auto_derisk).count()
    }

    /// Filter entries by inclusive date range ("YYYY-MM-DD").
    pub fn slice<'a>(&'a self, from: &str, to: &str) -> Vec<&'a RegimeSizingAuditEntry> {
        self.entries
            .iter()
            .filter(|e| e.date.as_str() >= from && e.date.as_str() <= to)
            .collect()
    }

    /// Average `multiplier` for the given date range.  Returns `None` if no
    /// entries fall within the range.
    pub fn avg_multiplier(&self, from: &str, to: &str) -> Option<f64> {
        let s = self.slice(from, to);
        if s.is_empty() {
            return None;
        }
        Some(s.iter().map(|e| e.multiplier).sum::<f64>() / s.len() as f64)
    }

    // ── Serialisation ─────────────────────────────────────────────────────────

    /// Export as a CSV string (header row + one data row per entry).
    pub fn to_csv(&self) -> String {
        let mut out = String::from(
            "date,regime,multiplier,portfolio_vega,portfolio_delta,\
             net_contracts,equity,auto_derisk,projected_max_dd_pct,skip_reason\n",
        );
        for e in &self.entries {
            out.push_str(&format!(
                "{},{},{:.4},{:.4},{:.6},{},{:.6},{},{:.4},{}\n",
                e.date, e.regime, e.multiplier, e.portfolio_vega, e.portfolio_delta,
                e.net_contracts, e.equity, e.auto_derisk, e.projected_max_dd_pct,
                e.skip_reason.as_deref().unwrap_or(""),
            ));
        }
        out
    }

    /// Export as a compact JSON array (no extra crate dependencies).
    pub fn to_json(&self) -> String {
        let rows: Vec<String> = self
            .entries
            .iter()
            .map(|e| {
                format!(
                    r#"  {{"date":"{d}","regime":"{r}","multiplier":{m:.4},"portfolio_vega":{pv:.4},"portfolio_delta":{pd:.6},"net_contracts":{nc},"equity":{eq:.6},"auto_derisk":{ad},"projected_max_dd_pct":{dd:.4},"skip_reason":{sk}}}"#,
                    d  = e.date,
                    r  = e.regime,
                    m  = e.multiplier,
                    pv = e.portfolio_vega,
                    pd = e.portfolio_delta,
                    nc = e.net_contracts,
                    eq = e.equity,
                    ad = e.auto_derisk,
                    dd = e.projected_max_dd_pct,
                    sk = match &e.skip_reason {
                        Some(s) => format!("\"{}\"", s),
                        None    => "null".to_string(),
                    },
                )
            })
            .collect();
        format!("[\n{}\n]", rows.join(",\n"))
    }
}
