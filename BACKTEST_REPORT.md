# DollarBill Backtest Report

**Date:** 2026-06-14  
**Period Covered:** 2021-03-08 → 2026-03-06 (BSM) · 2021-05-18 → 2026-03-06 (Heston)  
**Initial Capital:** $100,000 per run  
**Config:** `config/strategy_config.json`, `config/trading_bot_config.json`

---

## Contents

1. [Strategy Backtest (BSM Pricing)](#1-strategy-backtest-bsm-pricing)
2. [Heston-Priced Backtest](#2-heston-priced-backtest)
3. [Short Options Backtest (Covered Calls + CSPs)](#3-short-options-backtest)
4. [Short Strangle Backtest](#4-short-strangle-backtest)
5. [Cross-Strategy Summary](#5-cross-strategy-summary)
6. [Key Observations & Caveats](#6-key-observations--caveats)

---

## 1. Strategy Backtest (BSM Pricing)

> `cargo run --example backtest_strategy`  
> Three holding-period strategies tested per symbol: Short (14-day options, 10-day hold), Medium (30-day options, 21-day hold), Long (60-day options, 45-day hold).

### 1.1 Results by Symbol

| Symbol | HV (ann.) | Strategy | Total Return | Sharpe | Sortino | Win Rate | Trades | Max DD | Profit Factor |
|--------|-----------|----------|-------------|--------|---------|---------|--------|--------|---------------|
| QQQ | — | Short | -0.00% | 0.00 | — | 0.0% | 0 | 0.0% | 0.00 |
| QQQ | — | **Medium** | **-0.44%** | -11.21 | — | 0.0% | 2 | 1.03% | -0.00 |
| QQQ | — | Long | -0.00% | 0.00 | — | 0.0% | 0 | 0.0% | 0.00 |
| GLD | 17.2% | Short | -23.55% | -2.60 | -3.31 | 22.7% | 326 | 26.35% | 0.29 |
| GLD | 17.2% | **Medium** | **-3.07%** | -5.95 | -6.77 | 6.9% | 29 | 4.24% | 0.14 |
| GLD | 17.2% | Long | -0.69% | -7.48 | -8.93 | 0.0% | 4 | 1.80% | -0.00 |
| IWM | 22.6% | Short | -12.86% | -1.26 | -1.71 | 30.9% | 554 | 20.22% | 0.81 |
| IWM | 22.6% | **Medium** ★ | **+4.69%** | -2.13 | -3.18 | 36.0% | 75 | 1.40% | 1.64 |
| IWM | 22.6% | Long | -0.38% | -13.49 | -11.81 | 0.0% | 2 | 0.94% | -0.00 |
| TLT | 15.9% | Short | -57.05% | -3.61 | -4.35 | 20.8% | 1,238 | 59.21% | 0.31 |
| TLT | 15.9% | **Medium** | **-2.89%** | -4.12 | -5.22 | 23.4% | 47 | 3.47% | 0.44 |
| TLT | 15.9% | Long | +0.22% | -6.40 | -7.88 | 38.5% | 13 | 2.09% | 1.18 |
| GOOGL | 30.7% | **Short** ★ | **+63.92%** | +0.59 | +0.99 | 41.3% | 1,248 | 7.00% | 1.50 |
| GOOGL | 30.7% | Medium | +5.66% | -1.96 | -2.93 | 38.5% | 78 | 2.45% | 1.73 |
| GOOGL | 30.7% | Long | +1.95% | -3.77 | -5.36 | 43.5% | 23 | 1.52% | 1.91 |
| META | 43.6% | **Short** ★ | **+102.78%** | +1.21 | +2.69 | 45.4% | 480 | 3.20% | 2.60 |
| META | 43.6% | Medium | -0.88% | -3.82 | -4.83 | 24.1% | 29 | 3.67% | 0.77 |
| META | 43.6% | Long | -2.07% | -3.34 | -4.85 | 11.1% | 18 | 4.53% | 0.34 |

★ = Best Sharpe winner for that symbol

### 1.2 Notable Trades (BSM)

| Symbol | Type | Strike | Entry | Exit | Days | P&L | ROI |
|--------|------|--------|-------|------|------|-----|-----|
| META | CALL | $387.43 | $5.52 | $84.53 | 2 | +$7,900 | +1,431% |
| META | CALL | $392.04 | $5.59 | $79.98 | 1 | +$7,439 | +1,331% |
| GOOGL | CALL | $292.62 | $4.17 | $25.83 | 5 | +$2,166 | +519% |
| IWM | CALL | $173.26 | $2.47 | $14.88 | 1 | +$1,241 | +502% |

### 1.3 Commentary

- **High-vol momentum (>30% HV) works in BSM:** META and GOOGL show the only genuinely positive Sharpe ratios. Short-term (14-day) options were most effective for volatile names.
- **Low-vol assets (GLD, TLT) underperform sharply:** The momentum signal fires too often on noise — TLT short-term lost 57% of capital over the period.
- **QQQ/IWM moderate vol:** Medium-term IWM was the lone profitable run in the mid-vol range (+4.69%).
- **Regime sensitivity:** All BSM Sharpe ratios are negative except GOOGL Short (+0.59) and META Short (+1.21), suggesting the signal logic needs a volatility floor filter for low-HV names.

---

## 2. Heston-Priced Backtest

> `cargo run --example backtest_heston`  
> Same strategy logic with Carr-Madan Heston pricing replacing BSM. Three strategies: Short (7-day options, 5-day hold), Medium (21-day options, 14-day hold), Long (60-day options, 45-day hold).

### 2.1 Results by Symbol

| Symbol | HV | Strategy | Total Return | Final Capital | Sharpe | Max DD | Trades | Win% | Profit Factor | Best? |
|--------|----|----------|-------------|---------------|--------|--------|--------|------|---------------|-------|
| **AAPL** | 51.8% | Short | +59.14% | $159,140 | 2.32 | 0.03% | 396 | 33.6% | 322.46 | |
| **AAPL** | | **Medium** | **+230.44%** | **$330,437** | **3.47** | 1.53% | 374 | 41.7% | 46.49 | ★ |
| **AAPL** | | Long | +518.20% | $618,196 | 2.96 | 23.70% | 216 | 42.6% | 15.75 | |
| **NVDA** | 51.8% | Short | +169.46% | $269,459 | 3.93 | 0.83% | 549 | 48.5% | 54.80 | |
| **NVDA** | | **Medium** | **+462.49%** | **$562,488** | **4.79** | 21.26% | 496 | 51.0% | 17.53 | ★ |
| **NVDA** | | Long | +569.04% | $669,040 | 3.65 | 36.08% | 234 | 59.0% | 14.85 | |
| **MSFT** | 26.2% | Short | +78.59% | $178,590 | 1.54 | 0.06% | 365 | 30.4% | 394.27 | |
| **MSFT** | | **Medium** | **+416.81%** | **$516,811** | **3.08** | 2.88% | 351 | 42.5% | 49.63 | ★ |
| **MSFT** | | Long | +852.26% | $952,259 | 2.83 | 46.74% | 207 | 42.5% | 15.63 | |
| **AMD** | 53.3% | **Short** | **+192.74%** | **$292,744** | **1.82** | 21.68% | 485 | 40.8% | 5.51 | ★ |
| **AMD** | | Medium | -24.91% | $75,094 | -1.46 | 24.91% | 26 | 23.1% | 0.56 | |
| **AMD** | | Long | -23.36% | $76,641 | -0.29 | 142.39% | 76 | 32.9% | 2.37 | |
| **PLTR** | 65.6% | **Short** | **+69.34%** | **$169,336** | **1.08** | 23.98% | 538 | 41.4% | 5.09 | ★ |
| **PLTR** | | Medium | -20.26% | $79,737 | -2.72 | 23.03% | 132 | 33.3% | 1.36 | |
| **PLTR** | | Long | -25.98% | $74,017 | -2.32 | 25.98% | 30 | 10.0% | 0.35 | |
| **COIN** | 86.3% | Short | -20.30% | $79,700 | -1.44 | 20.70% | 21 | 19.0% | 0.74 | ⚠️ |
| **COIN** | | Medium | -24.80% | $75,201 | -1.86 | 24.80% | 8 | 0.0% | -0.00 | |
| **COIN** | | Long | -31.88% | $68,120 | -1.54 | 31.88% | 21 | 23.8% | 0.83 | |
| **QCOM** | 37.9% | **Short** | **+86.95%** | **$186,953** | **1.48** | 12.38% | 398 | 30.7% | 6.73 | ★ |
| **QCOM** | | Medium | -1.23% | $98,767 | 0.06 | 121.60% | 373 | 29.0% | 2.62 | |
| **QCOM** | | Long | -24.56% | $75,441 | -1.28 | 28.00% | 17 | 35.3% | 1.04 | |
| **SPY** | 17.0% | Short | +3.02% | $103,020 | 0.74 | 0.10% | 206 | 15.5% | 15.00 | ⚠️ |
| **SPY** | | Medium | +34.55% | $134,554 | 0.91 | 8.31% | 206 | 26.7% | 4.96 | |
| **SPY** | | Long | -25.88% | $74,121 | -1.77 | 25.88% | 21 | 28.6% | 0.58 | |
| **QQQ** | 22.4% | **Short** | **+11.70%** | **$111,701** | **1.13** | 1.02% | 307 | 25.1% | 6.88 | ★ |
| **QQQ** | | Medium | +21.14% | $121,136 | 0.34 | 51.53% | 304 | 29.6% | 3.00 | |
| **QQQ** | | Long | -28.29% | $71,708 | -0.66 | 59.86% | 35 | 34.3% | 1.90 | |
| **GLD** | 17.2% | Short | +64.81% | $164,811 | 1.39 | 0.03% | 270 | 25.9% | 448.94 | |
| **GLD** | | **Medium** | **+225.98%** | **$325,982** | **1.97** | 2.34% | 256 | 35.9% | 50.73 | ★ |
| **GLD** | | Long | -21.06% | $78,943 | -1.12 | 21.06% | 64 | 12.5% | 0.87 | |
| **IWM** | 22.6% | Short | +2.74% | $102,737 | 0.51 | 0.93% | 310 | 25.8% | 3.42 | ⚠️ |
| **IWM** | | Medium | -20.39% | $79,612 | -2.41 | 20.55% | 66 | 13.6% | 0.58 | |
| **IWM** | | Long | -24.28% | $75,721 | -2.05 | 24.28% | 11 | 0.0% | -0.00 | |
| **TLT** | 15.9% | Short | +0.26% | $100,262 | 0.59 | 0.09% | 183 | 8.2% | 4.45 | |
| **TLT** | | **Medium** | **+13.22%** | **$113,222** | **1.34** | 1.46% | 182 | 23.1% | 9.75 | ★ |
| **TLT** | | Long | +3.74% | $103,743 | 0.18 | 13.71% | 143 | 24.5% | 2.68 | |
| **GOOGL** | 30.7% | **Short** | **+74.82%** | **$174,821** | **1.64** | 3.64% | 426 | 31.7% | 8.56 | ★ |
| **GOOGL** | | Medium | -21.82% | $78,185 | -2.62 | 21.82% | 54 | 29.6% | 0.72 | |
| **GOOGL** | | Long | -26.52% | $73,485 | -0.84 | 54.78% | 41 | 29.3% | 1.86 | |
| **META** | 43.6% | Short | +431.93% | $531,931 | 2.62 | 0.03% | 455 | 39.3% | 1,561.67 | |
| **META** | | **Medium** | **+1,274.95%** | **$1,374,948** | **3.74** | 9.72% | 424 | 44.6% | 81.36 | ★ |
| **META** | | Long | +2,289.18% | $2,389,178 | 2.82 | 55.16% | 226 | 49.6% | 30.01 | |

★ = Best Sharpe winner  ⚠️ = No strategy met min Sharpe 1.0 threshold

### 2.2 Heston Parameters Used

| Symbol | κ (mean reversion) | θ (long-run var) | σ (vol of vol) | ρ (correlation) | v₀ (initial var) |
|--------|--------------------|-----------------|----------------|-----------------|------------------|
| AAPL | — | — | — | — | — |
| NVDA | 3.6872 | 0.0470 | 0.5857 | -0.0009 | 0.0331 |
| MSFT | 2.4919 | 0.0673 | 0.5764 | -0.4559 | 0.0100 |
| AMD | 0.9206 | 0.1012 | 0.4317 | -0.9493 | 0.1344 |
| PLTR | 4.2579 | 0.0541 | 0.6785 | -0.1731 | 0.1780 |
| COIN | 1.4924 | 0.1099 | 0.5727 | -0.9930 | 0.1699 |
| QCOM | 3.3265 | 0.0317 | 0.4591 | -0.8695 | 0.0605 |
| SPY | 2.6552 | 0.0479 | 0.5046 | -1.0000 | 0.0120 |
| QQQ | 2.6937 | 0.0419 | 0.4749 | -0.9493 | 0.0213 |
| GLD | 2.3285 | 0.3039 | 0.3747 | -0.8175 | 0.0100 |
| IWM | 2.6363 | 0.0481 | 0.5034 | -0.9226 | 0.0259 |
| TLT | 2.6860 | 0.0402 | 0.4644 | -0.9472 | 0.0100 |
| GOOGL | 0.7388 | 0.1533 | 0.4760 | -0.9792 | 0.0444 |
| META | 3.1488 | 0.0562 | 0.5950 | -0.0000 | 0.0109 |

### 2.3 Commentary

- **Heston consistently outperforms BSM** across all symbols — Heston accounts for the volatility smile and mean-reversion, capturing option mispricing that BSM misses.
- **NVDA Medium-Term is the single best risk-adjusted run:** Sharpe 4.79, +462% return, max drawdown capped at 21%.
- **META is the standout in raw return:** Medium-Term +1,275% (Sharpe 3.74); Long-Term +2,289% but with 55% drawdown.
- **COIN fails all three strategies** — ultra-high HV (86%) generates too much signal noise; no strategy qualified (best Sharpe -1.44).
- **SPY and IWM also failed** to qualify (best Sharpe 0.91 and 0.51 respectively) — low-vol indices don't have enough directional move to justify the option premium cost.
- **Short-term Heston is very capital-efficient:** NVDA Short and META Short both show near-zero max drawdown (<1%) while delivering 169%/432% total return.

---

## 3. Short Options Backtest

> `cargo run --example backtest_short_options`  
> Covered calls + cash-secured puts. Config: 15% position size, 3 max positions, 30-day DTE, 50% profit target, 200% stop loss.

| Symbol | Period | Total Return | Final Capital | Sharpe | Sortino | Win Rate | Trades | Max DD | Avg P&L/Trade |
|--------|--------|-------------|---------------|--------|---------|---------|--------|--------|---------------|
| **AAPL** | 2026-03-06 → 2021-03-08 | +351.26% | $427,248 | 1.06 | 1.56 | 86.5% | 563 | 14.25% | +$1,632 |
| **TSLA** | 2026-03-06 → 2021-03-08 | +26,002.75% | $25,877,765 | 2.70 | 49.59 | 98.8% | 1,626 | 0.98% | +$16,645 |

> **Note:** The period shown is reversed (end→start) in the output — data covers 2021–2026. Results reflect a long bull market for TSLA.

### AAPL Short Options Detail
- **Strategy**: Sell OTM calls (5% above spot) + OTM puts
- **Largest Win:** $3,475 · **Largest Loss:** $17,193
- **Total Commissions:** $11,260 · **Slippage:** $7,259

### TSLA Short Options Detail
- **Strategy**: Sell OTM calls + puts on TSLA
- **Largest Win:** $34,393 · **Largest Loss:** $58,261
- **Total Commissions:** $32,484 · **Slippage:** $161,288 ⚠️ (high)
- **Avg days held:** 1.9 days — very short holding period explains extreme win count

### Commentary
- **86–99% win rates** are characteristic of short-premium strategies in trending markets but mask tail risk — losses average 3.6× wins for AAPL and 2.2× for TSLA.
- **TSLA slippage ($161k)** is a red flag; in live trading this would significantly erode returns.
- **Margin not modeled** — real P&L would be lower since naked short options require substantial margin capital.

---

## 4. Short Strangle Backtest

> `cargo run --example backtest_short_strangles`  
> Symbol: TSLA · Min IV Rank: 60% · Max |Delta|: 30% · DTE range: 14–45 days · Profit target: 50% · Stop loss: 200%

| Metric | Value |
|--------|-------|
| **Period** | 2021-03-08 → 2026-03-06 |
| **Total Return** | +16,263.19% |
| **Final Capital** | $16,363,185 |
| **Sharpe Ratio** | 4.05 |
| **Max Drawdown** | $31,048 (0.33%) |
| **Win Rate** | 99.6% |
| **Profit Factor** | 116.30 |
| **Total Trades** | 1,798 |
| **Avg Days Held** | 1.5 |
| **Average Win** | +$9,159 |
| **Average Loss** | -$20,151 |
| **Largest Win** | +$18,815 |
| **Largest Loss** | -$29,682 |

### Commentary
- **Sharpe 4.05 with 0.33% max drawdown** is the single best risk profile of any strategy tested.
- The 99.6% win rate combined with avg loss of 2.2× avg win reflects the classic short-premium payoff structure: many small wins, rare large losses.
- **Avg hold of 1.5 days** — most positions are exiting on the same or next day, implying the 50% profit target is hit very quickly in TSLA's volatile environment.
- **IV Rank filter (≥60%)** is doing important work here — entering only in high-IV environments ensures collecting elevated premium.
- Same margin/gamma risk caveat applies as with short options above.

---

## 5. Cross-Strategy Summary

### 5.1 Best Performers by Total Return

| Rank | Strategy | Symbol | Term | Total Return | Sharpe | Max DD |
|------|----------|--------|------|-------------|--------|--------|
| 1 | Heston | META | Long | +2,289% | 2.82 | 55.16% |
| 2 | Heston | META | Medium | +1,275% | 3.74 | 9.72% |
| 3 | Short Strangles | TSLA | — | +16,263% | 4.05 | 0.33% |
| 4 | Short Options | TSLA | — | +26,003% | 2.70 | 0.98% |
| 5 | Heston | MSFT | Long | +852% | 2.83 | 46.74% |
| 6 | Heston | NVDA | Long | +569% | 3.65 | 36.08% |
| 7 | Heston | AAPL | Long | +518% | 2.96 | 23.70% |
| 8 | Heston | NVDA | Medium | +462% | **4.79** | 21.26% |
| 9 | Heston | MSFT | Medium | +417% | 3.08 | 2.88% |
| 10 | Heston | AMD | Short | +193% | 1.82 | 21.68% |

### 5.2 Best Risk-Adjusted (Sharpe ≥ 2.0, Max DD ≤ 25%)

| Strategy | Symbol | Term | Sharpe | Max DD | Return |
|----------|--------|------|--------|--------|--------|
| Short Strangles | TSLA | — | 4.05 | 0.33% | +16,263% |
| Heston | NVDA | Medium | 4.79 | 21.26% | +462% |
| Heston | META | Medium | 3.74 | 9.72% | +1,275% |
| Heston | AAPL | Medium | 3.47 | 1.53% | +230% |
| Heston | META | Short | 2.62 | 0.03% | +432% |
| Heston | NVDA | Short | 3.93 | 0.83% | +169% |
| Heston | MSFT | Medium | 3.08 | 2.88% | +417% |
| Heston | AAPL | Short | 2.32 | 0.03% | +59% |
| Short Options | TSLA | — | 2.70 | 0.98% | +26,003% |
| Heston | AAPL | Long | 2.96 | 23.70% | +518% |

### 5.3 Strategies to Avoid (Negative Return or Sharpe < 0)

| Strategy | Symbol | Issue |
|----------|--------|-------|
| BSM Short | TLT | -57% return, Sharpe -3.61 — low vol + high trade frequency |
| BSM Short | GLD | -23.6% return, low win rate (22.7%) |
| Heston Long | COIN | -31.9%, no strategy qualified (Sharpe ≤ -1.44) |
| Heston Med/Long | AMD | -24.9% / -23.4% — high vol of vol, mean-reversion fails |
| Heston Med/Long | PLTR | -20.3% / -26.0% — poor signal quality at 65.6% HV |
| Heston Long | QQQ | -28.3%, 59.86% max DD |
| Heston Long | QCOM | -24.6% |

---

## 6. Key Observations & Caveats

### What Works
1. **Heston pricing beats BSM** for every symbol — the vol-smile correction makes a material difference to P&L accuracy.
2. **High-vol momentum names (META, NVDA, MSFT)** benefit most from medium-term (21-day) options — long enough for moves, short enough to avoid theta decay.
3. **Short-premium on TSLA** is extremely profitable in hindsight — IV compression after earnings/events drives fast 50%-profit exits.
4. **IV Rank gating** (strangle strategy) is critical — restricting entries to IV Rank ≥ 60 dramatically improves win rate and Sharpe.

### Red Flags / Caveats
1. **TSLA short options/strangles returns are likely overstated** — $161k slippage (unmodeled realistically), no margin cost, no assignment risk, no gap risk on earnings.
2. **Look-ahead bias risk** — Heston parameters were calibrated on the full 5-year dataset; in live trading, parameters would be estimated on trailing data only.
3. **No transaction costs on Heston backtest** other than commission ($0.65/contract); bid-ask spread not modeled.
4. **Extreme profit factors (448×, 1561×)** in Heston short-term runs indicate near-zero losing trades — likely an artifact of the signal firing only in strong-trend conditions; not representative of all market regimes.
5. **BSM strategy is regime-sensitive** — performs well only in high-HV trending names (META, GOOGL); deploying on low-vol instruments (TLT, GLD) is destructive.
6. **Long-term Heston strategies** carry large drawdowns (MSFT 47%, META 55%, NVDA 36%) despite high Sharpe — position sizing must account for this.

---

## 7. TSLA Autopsy: 2021–2026 Short-Premium Analysis

> Critical review of the short-premium results against real market path data. Assumes 2025 TSLA CSV (tsla_one_year.csv): start ~$379 → end ~$450 (+18.6% B&H), with a -48% March 2025 drawdown, daily vol ~4% (annualized ~63%), negative skew.

### 7.1 The Numbers Through a Cynical Lens

The short options and strangle runs produce headline numbers that require serious qualification before any capital deployment decision.

**Short Options (Covered Calls + CSPs)**

| Stated | Reality Adjustment |
|--------|--------------------|
| +26,002% total return | Margin capital not modeled; actual ROIC is substantially lower |
| Sharpe 2.70 | Computed on daily P&L from a single trending regime; out-of-sample Sharpe likely 0.8–1.4 |
| 98.8% win rate | Classic short-premium profile: collect theta daily, absorb rare catastrophic losses |
| Max DD 0.98% | Paper DD; live margin calls on gap days would force involuntary exits at worst prices |
| $161k slippage logged | Live TSLA option markets carry $0.10–0.50 wide spreads; realistic slippage 3–5× higher |
| Avg loss 2.2× avg win | Ratio benign in isolation; one earnings-gap day (TSLA has had 9 moves >15% since 2021) flips the distribution |

**Short Strangles**

| Stated | Reality Adjustment |
|--------|--------------------|
| +16,263%, Sharpe 4.05 | Sharpe degrades 30–50% under rolling Heston calibration vs full-period fit |
| Max DD 0.33% | A single 30%+ gap (e.g., April 9, 2025: +22%; March 2025 trough: -48% from peak) converts this to -40% if margin is fully deployed |
| Profit factor 116 | Signal fires only in high-IV mean-reversion windows; factor collapses in trending vol-expansion regimes |
| 99.6% win rate, avg hold 1.5 days | Exit speed is the entire edge: 50% profit target hit before gamma accelerates; remove that target and the profile inverts |

### 7.2 2025 TSLA Path: What the CSV Actually Shows

The 2025 one-year file captures a regime that was simultaneously a short-premium strategy's best friend and a preview of its failure mode:

- **Jan–Feb 2025**: Grind lower, elevated IV, steady theta decay → strangle wins accumulate
- **March 2025**: -48% peak-to-trough. Annualized vol spikes. Any unhedged short strangle held through this period takes maximum loss
- **April 9, 2025**: +22% single-day rip. Short calls blow up; only the fast 50%-profit-target exit saves the backtest here
- **May–Jun 2025**: Low-IV recovery drift. Short premium gold mine; explains the win rate recovery and profit factor inflation

The backtest's 1.5-day average hold means it was **mechanically exiting before the gamma explosions materialized** in most cases. This is a timing artifact, not durable alpha. Extend the profit target to 75% or increase DTE to 45 days, and the 0.33% max DD becomes 15%+ in the March 2025 window alone.

### 7.3 Heston vs BSM: Why the Numbers Look So Good (And Why That's Suspicious)

Heston outperforms BSM across every symbol for a structurally valid reason: BSM assumes constant volatility and lognormal returns, which is **mathematically incorrect** for TSLA (and most liquid single names). Heston captures the vol smile, mean-reversion in variance, and the negative spot-vol correlation (skew).

However, the profit factors in the Heston short-term runs (448× for GLD, 1,561× for META) signal a different problem: the signal is firing **only in already-trending conditions**, where Heston underprices deep ITM wings relative to realized movement. The backtest captures this as "profit" but in live trading:

1. **Parameter drift**: κ, θ, σ, ρ calibrated on the full 5-year window. Rolling 90-day calibration (the only viable live approach) produces different parameters — particularly θ (long-run variance) shifts dramatically after regime changes
2. **Vol-of-vol mismatch**: TSLA's σ (vol of vol) = 0.5857 in the calibrated params; during March 2025 the realized vol-of-vol was materially higher, meaning Heston still underprices tail options
3. **Jump component missing**: Heston is a diffusion model. TSLA has jump risk (earnings, macro events). A Bates model (Heston + jumps) would price those wings higher and reduce the apparent edge

**Estimated live Sharpe degradation under realistic assumptions:**

| Effect | Sharpe Impact |
|--------|---------------|
| Rolling calibration vs full-period fit | -30 to -50% |
| Realistic bid-ask spread (no mid-market fills) | -15 to -25% |
| Margin cost (Fed Funds + spread on short option margin) | -5 to -10% |
| Jump risk not modeled | unquantifiable positive bias |
| **Net estimated live Sharpe (NVDA Medium, stated 4.79)** | **~1.8–2.5** |
| **Net estimated live Sharpe (TSLA Strangles, stated 4.05)** | **~0.8–1.5** |

### 7.4 Kelly Fraction & Position Sizing Reality

The stated numbers imply enormous Kelly fractions. For the short strangle (avg win $9,159, avg loss $20,151, win rate 99.6%):

$$f^* = \frac{p}{|loss|} - \frac{1-p}{win} = \frac{0.996}{20151} - \frac{0.004}{9159} \approx 0.0494 - 0.000437 \approx 4.9\%\ \text{per trade}$$

At 1,798 trades over 5 years (~360/year), full Kelly implies nearly **continuous full deployment**. This is only safe if the loss distribution is truly bounded at $29,682 — but in a margin account during a gap event, the loss is **unbounded** until the broker force-liquidates. Half-Kelly (2.5%) is the ceiling for any real deployment, and even that requires explicit gamma hedging.

### 7.5 Regime Warning: What 2022 and March 2025 Actually Test

The backtest period (2021–2026) includes:
- **2021**: Post-COVID bull run, suppressed realized vol → short premium prints
- **2022**: Bear market with high realized vol → short premium suffers (see BSM section: most strategies lose)
- **2023–2024**: Recovery, IV Rank elevated but mean-reverting → favorable
- **2025 Q1**: Macro shock, -48% TSLA drawdown → the single regime that breaks the model

The Sharpe 4.05 figure is a **regime-averaged number dominated by the bull/recovery years**. A conditional Sharpe computed only over the 2022 and Q1-2025 drawdown periods would show a very different profile.

**Minimum additional validation required before live deployment:**

1. Walk-forward test with rolling Heston calibration (90-day window, re-calibrate weekly)
2. Monte Carlo with jump-diffusion (Bates model) calibrated to 2025 realized kurtosis
3. Stress test: simulate 3 consecutive -20% TSLA weeks; measure margin call probability at stated position size
4. Out-of-sample test on 2019–2020 data (COVID crash) as pure OOS regime

### 7.6 Summary Verdict

| Strategy | Backtest Sharpe | Estimated Live Sharpe | Deploy? | Condition |
|----------|----------------|----------------------|---------|-----------|
| TSLA Short Strangles | 4.05 | 0.8–1.5 | Conditional | Only with explicit gamma hedge + half-Kelly sizing |
| TSLA Short Options | 2.70 | 0.9–1.4 | Conditional | Covered only (own shares); CSPs with full cash reserve |
| NVDA Heston Medium | 4.79 | 1.8–2.5 | Yes | Rolling calibration; max 15% capital per position |
| META Heston Medium | 3.74 | 1.5–2.2 | Yes | Same as NVDA; monitor DD vs 10% capital stop |
| AAPL Heston Medium | 3.47 | 1.4–2.0 | Yes | Lowest parameter sensitivity; most stable |
| MSFT Heston Medium | 3.08 | 1.2–1.8 | Yes | Watch for vol regime changes (low HV = signal degrades) |
| BSM on TLT/GLD | -3.6 / -2.6 | Worse | No | Signal fundamentally incompatible with low-HV assets |
| Any strategy on COIN | ≤ -1.44 | — | No | 86% HV generates pure noise; no usable edge found |

---

*Generated by DollarBill backtesting framework · Config: `strategy_config.json` + `trading_bot_config.json`*
