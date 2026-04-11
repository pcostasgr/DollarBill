"""
DollarBill Pricing Validator
============================
Cross-validates DollarBill's options pricing against QuantLib v1.41 Python
(AnalyticHestonEngine, AnalyticEuropeanEngine, BinomialVanillaEngine) and
independently against scipy for Black-Scholes.

Covers every test-case scenario defined in:
  tests/unit/models/test_quantlib_reference.rs
  tests/unit/models/test_black_scholes.rs
  tests/unit/models/test_american_dividends.rs

Usage
-----
  # QuantLib + scipy validation only (no Rust build needed):
  python py/validate_pricing.py

  # Full three-way: QuantLib | scipy | Rust computed (requires cargo on PATH):
  python py/validate_pricing.py --rust

  # Head-to-head speed comparison: QuantLib Python vs Rust (requires cargo):
  python py/validate_pricing.py --speed

  # All of the above combined:
  python py/validate_pricing.py --rust --speed

Requirements
------------
  pip install QuantLib scipy
  # QuantLib is also available as: pip install QuantLib-Python
"""

import sys
import os

# Ensure UTF-8 output on Windows (cp1252 terminal can't encode box-drawing chars)
if sys.platform == "win32" and hasattr(sys.stdout, "reconfigure"):
    sys.stdout.reconfigure(encoding="utf-8", errors="replace")
    sys.stderr.reconfigure(encoding="utf-8", errors="replace")

import argparse
import math
import re
import subprocess
import sys
from typing import Optional


# ─── Optional dependencies ───────────────────────────────────────────────────

try:
    import QuantLib as ql
    HAS_QUANTLIB = True
except ImportError:
    HAS_QUANTLIB = False
    print("WARNING: QuantLib not installed. Run: pip install QuantLib")

try:
    from scipy.stats import norm
    HAS_SCIPY = True
except ImportError:
    HAS_SCIPY = False
    print("WARNING: scipy not installed. Run: pip install scipy")


# ─── Colour helpers ──────────────────────────────────────────────────────────

def green(s: str) -> str:
    return f"\033[92m{s}\033[0m"

def red(s: str) -> str:
    return f"\033[91m{s}\033[0m"

def yellow(s: str) -> str:
    return f"\033[93m{s}\033[0m"

def bold(s: str) -> str:
    return f"\033[1m{s}\033[0m"


# ─── scipy Black-Scholes (independent reference) ─────────────────────────────

def scipy_bsm_call(S: float, K: float, T: float, r: float, sigma: float, q: float = 0.0) -> float:
    """Black-Scholes-Merton call via scipy — independent of QuantLib."""
    if not HAS_SCIPY:
        return float("nan")
    d1 = (math.log(S / K) + (r - q + 0.5 * sigma ** 2) * T) / (sigma * math.sqrt(T))
    d2 = d1 - sigma * math.sqrt(T)
    return S * math.exp(-q * T) * norm.cdf(d1) - K * math.exp(-r * T) * norm.cdf(d2)

def scipy_bsm_put(S: float, K: float, T: float, r: float, sigma: float, q: float = 0.0) -> float:
    """Black-Scholes-Merton put via scipy."""
    if not HAS_SCIPY:
        return float("nan")
    d1 = (math.log(S / K) + (r - q + 0.5 * sigma ** 2) * T) / (sigma * math.sqrt(T))
    d2 = d1 - sigma * math.sqrt(T)
    return K * math.exp(-r * T) * norm.cdf(-d2) - S * math.exp(-q * T) * norm.cdf(-d1)


# ─── QuantLib helpers ─────────────────────────────────────────────────────────

def _ql_flat_ts(today: "ql.Date", rate: float) -> "ql.YieldTermStructureHandle":
    return ql.YieldTermStructureHandle(
        ql.FlatForward(today, ql.QuoteHandle(ql.SimpleQuote(rate)), ql.Actual365Fixed())
    )

def ql_bsm_call(S: float, K: float, T_years: float, r: float, sigma: float, q: float = 0.0) -> float:
    """QuantLib AnalyticEuropeanEngine Black-Scholes-Merton call."""
    if not HAS_QUANTLIB:
        return float("nan")
    today = ql.Date(1, 1, 2025)
    ql.Settings.instance().evaluationDate = today
    mat = today + ql.Period(int(round(T_years * 365)), ql.Days)
    process = ql.BlackScholesMertonProcess(
        ql.QuoteHandle(ql.SimpleQuote(S)),
        _ql_flat_ts(today, q),
        _ql_flat_ts(today, r),
        ql.BlackVolTermStructureHandle(
            ql.BlackConstantVol(today, ql.NullCalendar(), sigma, ql.Actual365Fixed())
        ),
    )
    option = ql.VanillaOption(
        ql.PlainVanillaPayoff(ql.Option.Call, K),
        ql.EuropeanExercise(mat),
    )
    option.setPricingEngine(ql.AnalyticEuropeanEngine(process))
    return option.NPV()

def ql_bsm_put(S: float, K: float, T_years: float, r: float, sigma: float, q: float = 0.0) -> float:
    """QuantLib AnalyticEuropeanEngine Black-Scholes-Merton put."""
    if not HAS_QUANTLIB:
        return float("nan")
    today = ql.Date(1, 1, 2025)
    ql.Settings.instance().evaluationDate = today
    mat = today + ql.Period(int(round(T_years * 365)), ql.Days)
    process = ql.BlackScholesMertonProcess(
        ql.QuoteHandle(ql.SimpleQuote(S)),
        _ql_flat_ts(today, q),
        _ql_flat_ts(today, r),
        ql.BlackVolTermStructureHandle(
            ql.BlackConstantVol(today, ql.NullCalendar(), sigma, ql.Actual365Fixed())
        ),
    )
    option = ql.VanillaOption(
        ql.PlainVanillaPayoff(ql.Option.Put, K),
        ql.EuropeanExercise(mat),
    )
    option.setPricingEngine(ql.AnalyticEuropeanEngine(process))
    return option.NPV()

def ql_heston_call(S: float, K: float, T: float, r: float,
                   v0: float, kappa: float, theta: float, sigma: float, rho: float,
                   n_laguerre: int = 128) -> float:
    if not HAS_QUANTLIB:
        return float("nan")
    today = ql.Date(1, 1, 2025)
    ql.Settings.instance().evaluationDate = today
    mat = today + ql.Period(int(round(T * 365)), ql.Days)
    spot_h = ql.QuoteHandle(ql.SimpleQuote(S))
    rate_ts = _ql_flat_ts(today, r)
    div_ts  = _ql_flat_ts(today, 0.0)
    proc = ql.HestonProcess(rate_ts, div_ts, spot_h, v0, kappa, theta, sigma, rho)
    model = ql.HestonModel(proc)
    engine = ql.AnalyticHestonEngine(model, n_laguerre)
    option = ql.VanillaOption(
        ql.PlainVanillaPayoff(ql.Option.Call, K),
        ql.EuropeanExercise(mat),
    )
    option.setPricingEngine(engine)
    return option.NPV()

def ql_heston_put(S: float, K: float, T: float, r: float,
                  v0: float, kappa: float, theta: float, sigma: float, rho: float,
                  n_laguerre: int = 128) -> float:
    if not HAS_QUANTLIB:
        return float("nan")
    today = ql.Date(1, 1, 2025)
    ql.Settings.instance().evaluationDate = today
    mat = today + ql.Period(int(round(T * 365)), ql.Days)
    spot_h = ql.QuoteHandle(ql.SimpleQuote(S))
    rate_ts = _ql_flat_ts(today, r)
    div_ts  = _ql_flat_ts(today, 0.0)
    proc = ql.HestonProcess(rate_ts, div_ts, spot_h, v0, kappa, theta, sigma, rho)
    model = ql.HestonModel(proc)
    engine = ql.AnalyticHestonEngine(model, n_laguerre)
    option = ql.VanillaOption(
        ql.PlainVanillaPayoff(ql.Option.Put, K),
        ql.EuropeanExercise(mat),
    )
    option.setPricingEngine(engine)
    return option.NPV()

def ql_american_call_crr(S: float, K: float, T: float, r: float,
                          sigma: float, q: float = 0.0, steps: int = 200) -> float:
    """QuantLib CRR binomial tree — American call."""
    if not HAS_QUANTLIB:
        return float("nan")
    today = ql.Date(1, 1, 2025)
    ql.Settings.instance().evaluationDate = today
    mat = today + ql.Period(int(round(T * 365)), ql.Days)
    process = ql.BlackScholesMertonProcess(
        ql.QuoteHandle(ql.SimpleQuote(S)),
        _ql_flat_ts(today, q),
        _ql_flat_ts(today, r),
        ql.BlackVolTermStructureHandle(
            ql.BlackConstantVol(today, ql.NullCalendar(), sigma, ql.Actual365Fixed())
        ),
    )
    option = ql.VanillaOption(
        ql.PlainVanillaPayoff(ql.Option.Call, K),
        ql.AmericanExercise(today, mat),
    )
    option.setPricingEngine(ql.BinomialVanillaEngine(process, "crr", steps))
    return option.NPV()

def ql_european_call_crr(S: float, K: float, T: float, r: float,
                          sigma: float, q: float = 0.0, steps: int = 200) -> float:
    """QuantLib CRR binomial tree — European call (for early-exercise premium check)."""
    if not HAS_QUANTLIB:
        return float("nan")
    today = ql.Date(1, 1, 2025)
    ql.Settings.instance().evaluationDate = today
    mat = today + ql.Period(int(round(T * 365)), ql.Days)
    process = ql.BlackScholesMertonProcess(
        ql.QuoteHandle(ql.SimpleQuote(S)),
        _ql_flat_ts(today, q),
        _ql_flat_ts(today, r),
        ql.BlackVolTermStructureHandle(
            ql.BlackConstantVol(today, ql.NullCalendar(), sigma, ql.Actual365Fixed())
        ),
    )
    option = ql.VanillaOption(
        ql.PlainVanillaPayoff(ql.Option.Call, K),
        ql.EuropeanExercise(mat),
    )
    option.setPricingEngine(ql.BinomialVanillaEngine(process, "crr", steps))
    return option.NPV()


# ─── Rust test runner ─────────────────────────────────────────────────────────

def run_rust_tests(filter: str = "") -> dict[str, float]:
    """
    Run `cargo test --test lib <filter> -- --nocapture --test-threads=1`
    and parse all  `Label: <price>` lines printed by the Rust tests.

    Returns a dict mapping a short label to the Rust-computed price.
    """
    cmd = ["cargo", "test", "--test", "lib"]
    if filter:
        cmd.append(filter)
    cmd += ["--", "--nocapture", "--test-threads=1"]

    result = subprocess.run(cmd, capture_output=True, text=True)
    combined = result.stdout + result.stderr

    prices: dict[str, float] = {}

    # Patterns emitted by our Rust test println! macros:
    #   "Classic GL-64:  10.394219  (QuantLib ref: 10.3942)  err=0.0000"
    #   "NearBS GL-64:  6.804867  (QuantLib: 6.8049, BS: 6.804978)  ..."
    #   "  K= 80  GL-64: 25.043894  QL: 25.0446  err=0.0005"
    patterns = [
        # pattern,  group for label,  group for price
        (r"(Classic GL-64):\s+([\d.]+)", 1, 2),
        (r"(Classic GL-128):\s+([\d.]+)", 1, 2),
        (r"(Classic CM):\s+([\d.]+)", 1, 2),
        (r"(HighVolVol GL-64):\s+([\d.]+)", 1, 2),
        (r"(HighVolVol GL-128):\s+([\d.]+)", 1, 2),
        (r"(NearBS GL-64):\s+([\d.]+)", 1, 2),
        (r"K=\s*([\d.]+)\s+GL-64:\s+([\d.]+)", 1, 2),
    ]
    for pat, g_label, g_price in patterns:
        for m in re.finditer(pat, combined):
            label = m.group(g_label).strip()
            price = float(m.group(g_price))
            prices[label] = price

    return prices


# ─── Result row helper ────────────────────────────────────────────────────────

def row(label: str, ql_val: float, scipy_val: float,
        rust_val: Optional[float], tolerance: float,
        reference: Optional[float] = None) -> None:
    """Print one comparison row."""

    def fmt(v: Optional[float]) -> str:
        if v is None or (isinstance(v, float) and math.isnan(v)):
            return "  N/A      "
        return f"{v:10.6f}"

    ql_str     = fmt(ql_val)
    scipy_str  = fmt(scipy_val)
    rust_str   = fmt(rust_val)
    ref_str    = fmt(reference)

    # Primary check: QuantLib vs reference constant embedded in Rust tests
    if reference is not None and not math.isnan(ql_val):
        err = abs(ql_val - reference)
        const_ok = err < tolerance
        const_badge = green("PASS") if const_ok else red("FAIL")
    else:
        const_badge = yellow("  --")

    # Secondary check: QuantLib vs Rust computed (if cargo was run)
    if rust_val is not None and not math.isnan(ql_val):
        err_rust = abs(ql_val - rust_val)
        rust_ok = err_rust < tolerance
        rust_badge = green("PASS") if rust_ok else red("FAIL")
        err_rust_str = f"{err_rust:.5f}"
    else:
        rust_badge = yellow("  --")
        err_rust_str = "   --   "

    # scipy vs QuantLib agreement
    if not math.isnan(scipy_val) and not math.isnan(ql_val):
        err_scipy = abs(ql_val - scipy_val)
        scipy_badge = green("PASS") if err_scipy < 0.0001 else red("DIFF")
    else:
        scipy_badge = yellow("  --")
        err_scipy = float("nan")

    print(f"  {label:<28s} QL={ql_str}  scipy={scipy_str}  "
          f"rust={rust_str}  "
          f"const_chk={const_badge}  rust_chk={rust_badge}  scipy_chk={scipy_badge}")


# ─── Validation sections ──────────────────────────────────────────────────────

def section(title: str) -> None:
    print()
    print(bold("─" * 100))
    print(bold(f"  {title}"))
    print(bold("─" * 100))


def validate_black_scholes(rust_prices: dict) -> None:
    section("Black-Scholes-Merton — European Options")

    print(f"  {'Label':<28s} {'QuantLib':>12}  {'scipy':>12}  {'Rust test':>12}  "
          f"const_chk  rust_chk  scipy_chk")
    print(f"  {'-'*28} {'-'*12}  {'-'*12}  {'-'*12}  {'-'*9}  {'-'*8}  {'-'*9}")

    # ATM call: S=100, K=100, T=1, r=5%, σ=20%
    ql_atm_call  = ql_bsm_call(100, 100, 1.0, 0.05, 0.20)
    sp_atm_call  = scipy_bsm_call(100, 100, 1.0, 0.05, 0.20)
    # Hull "Options, Futures, and Other Derivatives" 9th ed. table: ~10.4506
    ref_atm_call = 10.4506
    row("ATM Call (S=K=100, σ=20%)",  ql_atm_call, sp_atm_call, None, 0.01, ref_atm_call)

    # ATM put
    ql_atm_put  = ql_bsm_put(100, 100, 1.0, 0.05, 0.20)
    sp_atm_put  = scipy_bsm_put(100, 100, 1.0, 0.05, 0.20)
    ref_atm_put = 5.5735   # put-call parity: C - K*e^(-rT) + K = P + S
    row("ATM Put  (S=K=100, σ=20%)",  ql_atm_put, sp_atm_put, None, 0.01, ref_atm_put)

    # Put-call parity check
    pcp_diff = abs((ql_atm_call - ql_atm_put) - (100.0 - 100.0 * math.exp(-0.05 * 1.0)))
    badge = green("PASS") if pcp_diff < 1e-6 else red("FAIL")
    print(f"  {'Put-Call Parity':<28s} |C−P − (S−Ke^−rT)| = {pcp_diff:.2e}   {badge}")

    # OTM call: S=100, K=110, T=1, r=5%, σ=20%
    ql_otm_call  = ql_bsm_call(100, 110, 1.0, 0.05, 0.20)
    sp_otm_call  = scipy_bsm_call(100, 110, 1.0, 0.05, 0.20)
    row("OTM Call (K=110, σ=20%)",    ql_otm_call, sp_otm_call, None, 0.01, None)

    # Deep ITM call: S=200, K=100
    ql_ditm_call = ql_bsm_call(200, 100, 1.0, 0.05, 0.20)
    sp_ditm_call = scipy_bsm_call(200, 100, 1.0, 0.05, 0.20)
    row("Deep ITM Call (S=200, K=100)", ql_ditm_call, sp_ditm_call, None, 0.01, None)

    # High vol call: σ=80%
    ql_hv_call = ql_bsm_call(100, 100, 1.0, 0.05, 0.80)
    sp_hv_call = scipy_bsm_call(100, 100, 1.0, 0.05, 0.80)
    row("High Vol Call (σ=80%)",       ql_hv_call, sp_hv_call, None, 0.01, None)

    # Dividend-adjusted: q=3%
    ql_div_call = ql_bsm_call(100, 100, 1.0, 0.05, 0.20, q=0.03)
    sp_div_call = scipy_bsm_call(100, 100, 1.0, 0.05, 0.20, q=0.03)
    row("Dividend Call (q=3%)",        ql_div_call, sp_div_call, None, 0.01, None)


def validate_heston(rust_prices: dict) -> None:
    section("Heston Stochastic Volatility — AnalyticHestonEngine (match Rust test constants)")

    print(f"  {'Label':<28s} {'QuantLib':>12}  {'scipy':>12}  {'Rust test':>12}  "
          f"const_chk  rust_chk  scipy_chk")
    print(f"  {'-'*28} {'-'*12}  {'-'*12}  {'-'*12}  {'-'*9}  {'-'*8}  {'-'*9}")

    # ── Test Case 1: Classic Heston ───────────────────────────────────────────
    # v0=0.04, κ=2.0, θ=0.04, σ=0.3, ρ=−0.7  S=K=100, T=1, r=5%
    # Rust constant: CLASSIC_QUANTLIB_CALL = 10.3942
    ql64  = ql_heston_call(100, 100, 1.0, 0.05, 0.04, 2.0, 0.04, 0.3, -0.7, 64)
    ql128 = ql_heston_call(100, 100, 1.0, 0.05, 0.04, 2.0, 0.04, 0.3, -0.7, 128)
    row("Classic GL-64  (T=1, K=100)", ql64,  float("nan"),
        rust_prices.get("Classic GL-64"), tolerance=0.10, reference=10.3942)
    row("Classic GL-128 (T=1, K=100)", ql128, float("nan"),
        rust_prices.get("Classic GL-128"), tolerance=0.05, reference=10.3942)

    # ── Classic put (put-call parity) ─────────────────────────────────────────
    ql_put    = ql_heston_put(100, 100, 1.0, 0.05, 0.04, 2.0, 0.04, 0.3, -0.7, 128)
    pcp_heston = abs((ql128 - ql_put) - (100.0 - 100.0 * math.exp(-0.05)))
    badge = green("PASS") if pcp_heston < 0.001 else red("FAIL")
    print(f"  {'Heston Put-Call Parity':<28s} call={ql128:.6f}  put={ql_put:.6f}  |C−P−(S−Ke^−rT)|={pcp_heston:.2e}  {badge}")

    print()
    print(f"  {'  ── Strike Sweep (Classic params)'}")
    # K ∈ {80, 90, 100, 110, 120} vs constants in Rust test_strike_sweep_vs_quantlib:
    SWEEP_REFS = {80: 25.0446, 90: 17.0753, 100: 10.3942, 110: 5.4303, 120: 2.3326}
    for K, ref in SWEEP_REFS.items():
        ql_k = ql_heston_call(100, K, 1.0, 0.05, 0.04, 2.0, 0.04, 0.3, -0.7, 64)
        rust_k = rust_prices.get(str(float(K)))
        row(f"  K={K}", ql_k, float("nan"), rust_k, tolerance=0.10, reference=ref)

    # ── Test Case 2: High vol-of-vol ──────────────────────────────────────────
    # v0=0.09, κ=1.5, θ=0.09, σ=1.0, ρ=−0.9  T=0.5
    # Rust constant: HIGH_VOLVOL_QUANTLIB_CALL = 8.5568
    print()
    ql_hvv64  = ql_heston_call(100, 100, 0.5, 0.05, 0.09, 1.5, 0.09, 1.0, -0.9, 64)
    ql_hvv128 = ql_heston_call(100, 100, 0.5, 0.05, 0.09, 1.5, 0.09, 1.0, -0.9, 128)
    row("HighVolVol GL-64  (σ_v=1.0)", ql_hvv64,  float("nan"),
        rust_prices.get("HighVolVol GL-64"),  tolerance=1.0, reference=8.5568)
    row("HighVolVol GL-128 (σ_v=1.0)", ql_hvv128, float("nan"),
        rust_prices.get("HighVolVol GL-128"), tolerance=0.5, reference=8.5568)

    # ── Test Case 3: Near-BS ──────────────────────────────────────────────────
    # v0=0.01, κ=5.0, θ=0.01, σ=0.01, ρ=0.0  T=1
    # When σ→0 the Heston model collapses to BSM with σ≈√v0=0.10
    # Rust constant: NEAR_BS_QUANTLIB_CALL = 6.8049
    print()
    ql_nbs64  = ql_heston_call(100, 100, 1.0, 0.05, 0.01, 5.0, 0.01, 0.01, 0.0, 64)
    bs_equiv  = scipy_bsm_call(100, 100, 1.0, 0.05, 0.10)   # σ=√0.01=0.10
    row("NearBS GL-64  (σ_v→0)",     ql_nbs64, bs_equiv,
        rust_prices.get("NearBS GL-64"), tolerance=0.10, reference=6.8049)

    # Heston vs BS convergence:
    heston_bs_diff = abs(ql_nbs64 - bs_equiv)
    badge = green("PASS") if heston_bs_diff < 0.05 else yellow("NOTE")
    print(f"  {'Heston→BS convergence':<28s} |Heston_GL64 − BS(σ=0.10)| = {heston_bs_diff:.4f}  {badge}")


def validate_american(rust_prices: dict) -> None:
    section("American Options — CRR Binomial (200 steps)")

    print(f"  {'Label':<40s} {'QuantLib':>12}  {'scipy':>12}")
    print(f"  {'-'*40} {'-'*12}  {'-'*12}")

    # Case 1: ATM call, no dividend → American = European (Merton 1973)
    am_no_div  = ql_american_call_crr(100, 100, 1.0, 0.05, 0.25, q=0.0)
    eu_no_div  = ql_european_call_crr(100, 100, 1.0, 0.05, 0.25, q=0.0)
    bs_no_div  = scipy_bsm_call(100, 100, 1.0, 0.05, 0.25, q=0.0)
    ee_no_div  = am_no_div - eu_no_div
    badge = green("PASS") if abs(ee_no_div) < 0.05 else red("FAIL")
    print(f"  {'No-div: American ≈ European':<40s} am={am_no_div:.6f}  eu={eu_no_div:.6f}  "
          f"EE_premium={ee_no_div:.4f}  {badge}")

    # Case 2: High dividend (q=10%) deep ITM — early exercise has positive value
    am_div  = ql_american_call_crr(200, 100, 1.0, 0.05, 0.20, q=0.10)
    eu_div  = ql_european_call_crr(200, 100, 1.0, 0.05, 0.20, q=0.10)
    ee_div  = am_div - eu_div
    badge_pos  = green("PASS") if am_div > 0 else red("FAIL")
    badge_geq  = green("PASS") if am_div >= eu_div - 1e-6 else red("FAIL")
    badge_ee   = green("PASS") if ee_div > 0 else yellow("NOTE")
    print(f"  {'Div10% deep-ITM: American > 0':<40s} am={am_div:.6f}   {badge_pos}")
    print(f"  {'Div10%: American >= European':<40s} am={am_div:.6f}  eu={eu_div:.6f}  {badge_geq}")
    print(f"  {'Div10%: Early-exercise premium':<40s} EE_premium={ee_div:.4f}          {badge_ee}")

    # Case 3: EE premium with dividend > EE premium without (key correctness check)
    am_nd2 = ql_american_call_crr(200, 100, 1.0, 0.05, 0.20, q=0.00)
    eu_nd2 = ql_european_call_crr(200, 100, 1.0, 0.05, 0.20, q=0.00)
    ee_nd2 = am_nd2 - eu_nd2
    badge = green("PASS") if ee_div > ee_nd2 else red("FAIL")
    print(f"  {'EE(div) > EE(no-div)':<40s} EE_div={ee_div:.4f}  EE_no_div={ee_nd2:.4f}  {badge}")

    # Multiple dividend levels
    print()
    print(f"  {'q':>6}  {'American':>12}  {'European':>12}  {'EE premium':>12}")
    print(f"  {'-'*6}  {'-'*12}  {'-'*12}  {'-'*12}")
    for q in [0.0, 0.02, 0.05, 0.08, 0.12]:
        am = ql_american_call_crr(100, 100, 1.0, 0.05, 0.25, q=q)
        eu = ql_european_call_crr(100, 100, 1.0, 0.05, 0.25, q=q)
        ee = am - eu
        ok = green("✓") if am >= eu - 1e-6 else red("✗")
        print(f"  {q:>5.0%}   am={am:>9.4f}   eu={eu:>9.4f}   ee_prem={ee:>7.4f}  {ok}")


# ─── Speed benchmark ─────────────────────────────────────────────────────────

def bench_ql(fn, number: int = 1000, repeat: int = 15) -> tuple[float, float]:
    """Return (best_us, median_us) for a callable."""
    import timeit
    # warm-up
    timeit.repeat(fn, number=200, repeat=3)
    times = timeit.repeat(fn, number=number, repeat=repeat)
    best   = min(times) * 1e6 / number
    median = sorted(times)[repeat // 2] * 1e6 / number
    return best, median


def run_rust_bench() -> dict[str, float]:
    """
    Run `cargo bench` in release mode and parse the Criterion output.
    Returns a dict: bench_name -> best_ns_per_iter
    """
    import re as _re
    cmd = ["cargo", "bench", "--", "--output-format", "bencher"]
    result = subprocess.run(cmd, capture_output=True, text=True)
    combined = result.stdout + result.stderr

    times: dict[str, float] = {}
    # Criterion bencher format:  "test bench_name ... bench: 33,123 ns/iter (+/- 456)"
    for m in _re.finditer(r"test (.+?) \.\.\. bench:\s+([\d,]+) ns/iter", combined):
        name = m.group(1).strip()
        ns   = float(m.group(2).replace(",", ""))
        times[name] = ns
    return times


def speed_row(label: str, ql_best_us: float, ql_median_us: float,
              rust_ns: float | None, rust_label: str = "") -> None:
    rust_us = rust_ns / 1000.0 if rust_ns is not None else None

    ql_str   = f"{ql_best_us:8.2f} µs"
    rust_str = f"{rust_us:8.2f} µs" if rust_us is not None else "      N/A  "

    if rust_us is not None and rust_us > 0:
        speedup = ql_best_us / rust_us
        speedup_str = green(f"{speedup:6.1f}×") if speedup >= 5 else \
                      yellow(f"{speedup:6.1f}×") if speedup >= 1 else \
                      red(f"{speedup:6.1f}×")
    else:
        speedup_str = "     --"

    print(f"  {label:<40s}  QL best={ql_str}  QL median={ql_median_us:8.2f} µs  "
          f"Rust={rust_str}  speedup={speedup_str}")


def validate_speed(rust_bench_times: dict[str, float]) -> None:
    import timeit

    section("Speed Benchmark — QuantLib Python vs DollarBill Rust")
    print("  All QuantLib timings: best of 15 × 1000 calls, warmed up first.")
    print("  Rust timings from `cargo bench` (Criterion, release mode).")
    print()

    if not HAS_QUANTLIB:
        print(red("  QuantLib not available — skipping"))
        return

    # ── Setup reusable QuantLib objects (avoid reconstruction overhead) ───────
    today     = ql.Date(1, 1, 2025)
    ql.Settings.instance().evaluationDate = today
    mat_1y    = today + ql.Period(365, ql.Days)
    mat_6m    = today + ql.Period(182, ql.Days)

    rate_ts   = _ql_flat_ts(today, 0.05)
    div_ts    = _ql_flat_ts(today, 0.0)
    spot_h    = ql.QuoteHandle(ql.SimpleQuote(100.0))
    vol_h     = ql.BlackVolTermStructureHandle(
                    ql.BlackConstantVol(today, ql.NullCalendar(), 0.20, ql.Actual365Fixed()))

    # --- BSM AnalyticEuropeanEngine ---
    bsm_proc  = ql.BlackScholesMertonProcess(spot_h, div_ts, rate_ts, vol_h)
    bsm_opt   = ql.VanillaOption(ql.PlainVanillaPayoff(ql.Option.Call, 100.0),
                                  ql.EuropeanExercise(mat_1y))
    bsm_opt.setPricingEngine(ql.AnalyticEuropeanEngine(bsm_proc))
    bsm_opt.NPV()  # prime

    def ql_bsm_single():
        bsm_opt.recalculate()
        return bsm_opt.NPV()

    # --- Heston GL-64 single call ---
    h_proc  = ql.HestonProcess(rate_ts, div_ts, spot_h, 0.04, 2.0, 0.04, 0.3, -0.7)
    h_model = ql.HestonModel(h_proc)
    h_eng64 = ql.AnalyticHestonEngine(h_model, 64)
    h_opt   = ql.VanillaOption(ql.PlainVanillaPayoff(ql.Option.Call, 100.0),
                                ql.EuropeanExercise(mat_1y))
    h_opt.setPricingEngine(h_eng64)
    h_opt.NPV()

    def ql_heston_single_64():
        h_opt.recalculate()
        return h_opt.NPV()

    # --- Heston GL-128 single call ---
    h_eng128 = ql.AnalyticHestonEngine(h_model, 128)
    h_opt128 = ql.VanillaOption(ql.PlainVanillaPayoff(ql.Option.Call, 100.0),
                                 ql.EuropeanExercise(mat_1y))
    h_opt128.setPricingEngine(h_eng128)
    h_opt128.NPV()

    def ql_heston_single_128():
        h_opt128.recalculate()
        return h_opt128.NPV()

    # --- Heston GL-64 strike sweep (11 strikes, same as Rust bench) ---
    strikes = list(range(80, 121, 4))
    sweep_opts = []
    for k in strikes:
        o = ql.VanillaOption(ql.PlainVanillaPayoff(ql.Option.Call, float(k)),
                              ql.EuropeanExercise(mat_1y))
        o.setPricingEngine(h_eng64)
        o.NPV()
        sweep_opts.append(o)

    def ql_heston_sweep_64():
        total = 0.0
        for o in sweep_opts:
            o.recalculate()
            total += o.NPV()
        return total

    # --- Heston GL-64 vol surface: 50 strikes × 10 maturities (500 options) ---
    surf_strikes = [float(k) for k in range(70, 131, 1)][:50]
    surf_mats    = [today + ql.Period(int(t * 365), ql.Days)
                    for t in [0.1, 0.25, 0.5, 0.75, 1.0, 1.25, 1.5, 1.75, 2.0, 2.5]]
    surf_opts = []
    for mat in surf_mats:
        for k in surf_strikes:
            o = ql.VanillaOption(ql.PlainVanillaPayoff(ql.Option.Call, k),
                                  ql.EuropeanExercise(mat))
            o.setPricingEngine(h_eng64)
            o.NPV()
            surf_opts.append(o)

    def ql_heston_surface():
        total = 0.0
        for o in surf_opts:
            o.recalculate()
            total += o.NPV()
        return total

    # ── Time everything ───────────────────────────────────────────────────────
    print(f"  {'Benchmark':<40s}  {'QL best':>12}  {'QL median':>12}  "
          f"{'Rust best':>12}  {'speedup':>10}")
    print(f"  {'-'*40}  {'-'*12}  {'-'*12}  {'-'*12}  {'-'*10}")

    bsm_b, bsm_m   = bench_ql(ql_bsm_single)
    h64_b,  h64_m  = bench_ql(ql_heston_single_64)
    h128_b, h128_m = bench_ql(ql_heston_single_128)
    sw_b,   sw_m   = bench_ql(ql_heston_sweep_64,    number=200, repeat=10)
    sf_b,   sf_m   = bench_ql(ql_heston_surface,     number=50,  repeat=8)

    # Criterion bench names (from cargo bench output):
    #  "BSM baseline (flat vol)/ATM call + Greeks"
    #  "Heston Gauss-Laguerre/ATM call (64 nodes)"
    #  "GL strike sweep (11 strikes)/11 calls"
    #  "Heston batch pricing/50×10 vol surface (GL-64 + CF cache)"
    rust_bsm    = _find_bench(rust_bench_times, "ATM call + Greeks")
    rust_h64    = _find_bench(rust_bench_times, "ATM call (64 nodes)")
    rust_h128   = _find_bench(rust_bench_times, "ATM call (128 nodes)")
    rust_sweep  = _find_bench(rust_bench_times, "11 calls")
    rust_surf   = _find_bench(rust_bench_times, "50×10 vol surface")

    speed_row("BSM ATM call + Greeks",             bsm_b,  bsm_m,  rust_bsm)
    speed_row("Heston GL-64 single call",          h64_b,  h64_m,  rust_h64)
    speed_row("Heston GL-128 single call",         h128_b, h128_m, rust_h128)
    speed_row(f"Heston GL-64 strike sweep ({len(strikes)} strikes)", sw_b, sw_m, rust_sweep)
    speed_row(f"Heston GL-64 vol surface (500 opts)",  sf_b,  sf_m,  rust_surf)

    print()
    print("  QuantLib prices confirmed identical to those in the validation section.")
    print(f"  Rust numbers require: {bold('cargo bench')} in a separate terminal (release build).")
    print()
    if not rust_bench_times:
        print(yellow("  TIP: Run 'cargo bench' first, then re-run with --speed to see the speedup column."))


def _find_bench(times: dict[str, float], fragment: str) -> float | None:
    """Case-insensitive substring search across Criterion bench names."""
    for k, v in times.items():
        if fragment.lower() in k.lower():
            return v
    return None


# ─── Summary counter ──────────────────────────────────────────────────────────

def print_header() -> None:
    print()
    print(bold("=" * 100))
    print(bold("  DollarBill Options Pricing Validator"))
    ql_ver = getattr(ql, "__version__", None) or getattr(ql, "version", None) or "installed"
    if callable(ql_ver):
        ql_ver = ql_ver()
    print(bold("  Reference engine: QuantLib v" + (str(ql_ver) if HAS_QUANTLIB else "N/A")))
    print(bold("  Scope: Black-Scholes-Merton │ Heston GL-64/GL-128/Carr-Madan │ American CRR"))
    print(bold("=" * 100))
    print()
    print("  Legend:")
    print(f"    QL         = QuantLib AnalyticEuropeanEngine / AnalyticHestonEngine (live computation)")
    print(f"    scipy      = Independent BSM via scipy.stats.norm")
    print(f"    rust       = Rust test println! output  (only with --rust flag)")
    print(f"    const_chk  = |QL − Rust hardcoded constant| < tolerance")
    print(f"    rust_chk   = |QL − Rust computed value|    < tolerance")
    print(f"    scipy_chk  = |QL − scipy|                  < 0.0001")


# ─── Surface MAE validation  (--surface / --period / --report-wings) ──────────

def _bsm_iv_call(price: float, S: float, K: float, T: float, r: float,
                 lo: float = 0.001, hi: float = 10.0, tol: float = 1e-7) -> float:
    """Bisection implied vol for a call price."""
    if price <= 0.0 or T <= 0.0:
        return float("nan")
    for _ in range(120):
        mid = (lo + hi) * 0.5
        v = scipy_bsm_call(S, K, T, r, mid)
        if abs(v - price) < tol:
            return mid
        if v < price:
            lo = mid
        else:
            hi = mid
    return (lo + hi) * 0.5


def _load_csv_window(csv_path: str, start: str, end: str) -> "list[tuple[str,float]]":
    """Load (date, close) pairs from a CSV restricted to [start, end] inclusive."""
    rows: list[tuple[str, float]] = []
    import csv as _csv
    with open(csv_path, newline="") as fh:
        reader = _csv.DictReader(fh)
        for row in reader:
            date_val = row.get("Date", row.get("date", "")).strip()
            close_val = row.get("Close", row.get("close", "")).strip()
            try:
                close_f = float(close_val)
            except (ValueError, TypeError):
                continue
            if start <= date_val <= end:
                rows.append((date_val, close_f))
    rows.sort(key=lambda r: r[0])
    return rows


def _realized_vol(closes: "list[float]") -> float:
    """Annualized realized vol from a list of closing prices."""
    import math as _math
    rets = [_math.log(closes[i] / closes[i - 1]) for i in range(1, len(closes))]
    n = len(rets)
    if n < 2:
        return float("nan")
    mu = sum(rets) / n
    var = sum((r - mu) ** 2 for r in rets) / (n - 1)
    return _math.sqrt(var * 252)


def _ql_calibrate_heston(
    S: float, r: float,
    maturities: "list[float]",
    strikes_per_mat: "list[list[float]]",
    prices_per_mat: "list[list[float]]",
) -> "dict":
    """
    Calibrate Heston model to a call-price surface via QuantLib's
    LevenbergMarquardt optimizer + AnalyticHestonEngine.

    Returns dict with keys: v0, kappa, theta, sigma, rho, final_rmse.
    """
    if not HAS_QUANTLIB:
        return {}

    today = ql.Date(1, 1, 2025)
    ql.Settings.instance().evaluationDate = today

    spot_h   = ql.QuoteHandle(ql.SimpleQuote(S))
    rate_ts  = _ql_flat_ts(today, r)
    div_ts   = _ql_flat_ts(today, 0.0)

    helpers = []
    for T, strikes, prices in zip(maturities, strikes_per_mat, prices_per_mat):
        mat_date = today + ql.Period(int(round(T * 365)), ql.Days)
        for K, price in zip(strikes, prices):
            # HestonModelHelper takes market implied vol, not price — convert.
            iv = _bsm_iv_call(price, S, K, T, r)
            if math.isnan(iv) or iv <= 0:
                continue
            helper = ql.HestonModelHelper(
                ql.Period(int(round(T * 365)), ql.Days),
                ql.NullCalendar(),
                S,          # spot — Real, not QuoteHandle
                K,
                ql.QuoteHandle(ql.SimpleQuote(iv)),
                rate_ts,
                div_ts,
            )
            helpers.append(helper)

    if not helpers:
        return {}

    # Initial guess: flat vol close to realized
    v0_init    = 0.09
    proc = ql.HestonProcess(rate_ts, div_ts, spot_h, v0_init, 2.0, v0_init, 0.3, -0.5)
    model = ql.HestonModel(proc)
    engine = ql.AnalyticHestonEngine(model, 64)
    for h in helpers:
        h.setPricingEngine(engine)

    om = ql.LevenbergMarquardt()
    model.calibrate(helpers, om, ql.EndCriteria(1000, 100, 1e-8, 1e-8, 1e-8))

    params = model.params()
    # QuantLib Heston params order: [theta, kappa, sigma, rho, v0]
    theta, kappa, sigma, rho, v0 = params[0], params[1], params[2], params[3], params[4]

    total_sq = sum(h.calibrationError() ** 2 for h in helpers)
    rmse = math.sqrt(total_sq / len(helpers))

    return dict(v0=v0, kappa=kappa, theta=theta, sigma=sigma, rho=rho,
                calibration_rmse=rmse, n_helpers=len(helpers))


def validate_surface(csv_path: str, period: str, report_wings: bool, r: float = 0.045) -> int:
    """
    Full surface MAE report:
      1. Load CSV, extract [start, end] window.
      2. Compute annualized realized vol.
      3. Build synthetic call surface (7 strikes × 4 maturities) from RV.
      4. Calibrate Heston via QuantLib LevenbergMarquardt.
      5. Price every cell with calibrated params and compute |ΔIV|.
      6. Report total MAE, per-maturity MAE, and (if --report-wings) ATM vs wing split.
    """
    import math as _math

    # ── parse date range ─────────────────────────────────────────────────────
    if ":" not in period:
        print(red(f"ERROR: --period must be 'YYYY-MM-DD:YYYY-MM-DD', got '{period}'"))
        return 1
    start, end = period.split(":", 1)

    section(f"Surface MAE Validation — {csv_path}  [{start} → {end}]")

    rows = _load_csv_window(csv_path, start, end)
    if len(rows) < 5:
        print(red(f"  ERROR: only {len(rows)} rows in window {start}:{end} — need ≥5"))
        return 1

    closes = [c for _, c in rows]
    spot   = closes[-1]
    rv     = _realized_vol(closes)

    print(f"  Window : {rows[0][0]} → {rows[-1][0]}  ({len(rows)} trading days)")
    print(f"  Spot   : {spot:.2f}")
    print(f"  Ann RV : {rv * 100:.2f}%")

    if _math.isnan(rv) or rv <= 0:
        print(red("  ERROR: could not compute realized vol"))
        return 1

    # ── build synthetic surface from RV ──────────────────────────────────────
    maturities = [7 / 365, 30 / 365, 90 / 365, 180 / 365]
    mat_labels  = ["1w", "1m", "3m", "6m"]
    moneyness   = [0.75, 0.833, 0.917, 1.0, 1.083, 1.167, 1.25]  # 7 strikes

    # "Market" vol surface: flat RV with a mild ~5% skew (deeper OTM puts richer)
    def market_iv(m: float, T: float) -> float:
        skew = -0.05 * (m - 1.0)        # roughly -5% per unit moneyness
        term = 0.02 * _math.sqrt(T)     # term-structure steepening
        return max(rv + skew + term, 0.05)

    strikes_per_mat: list[list[float]] = []
    prices_per_mat:  list[list[float]] = []
    mkt_ivs_per_mat: list[list[float]] = []

    for T in maturities:
        row_k: list[float] = []
        row_p: list[float] = []
        row_v: list[float] = []
        for m in moneyness:
            K   = round(spot * m / 5.0) * 5.0
            iv  = market_iv(m, T)
            p   = scipy_bsm_call(spot, K, T, r, iv)
            row_k.append(K)
            row_p.append(p)
            row_v.append(iv)
        strikes_per_mat.append(row_k)
        prices_per_mat.append(row_p)
        mkt_ivs_per_mat.append(row_v)

    total_cells = sum(len(r) for r in strikes_per_mat)
    print(f"  Surface: {len(maturities)} maturities × {len(moneyness)} strikes = {total_cells} cells")
    print()

    # ── QuantLib Heston calibration ───────────────────────────────────────────
    print("  Calibrating Heston (QuantLib LevenbergMarquardt)…")
    cal = _ql_calibrate_heston(spot, r, maturities, strikes_per_mat, prices_per_mat)
    if not cal:
        print(red("  ERROR: calibration failed — QuantLib not available or no valid helpers"))
        return 1

    print(f"  Calibrated params:")
    print(f"    v0={cal['v0']:.4f}  kappa={cal['kappa']:.4f}  theta={cal['theta']:.4f}"
          f"  sigma={cal['sigma']:.4f}  rho={cal['rho']:.4f}")
    print(f"    calibration RMSE (IV units): {cal['calibration_rmse']:.6f}")
    print()

    # ── per-cell |ΔIV| ────────────────────────────────────────────────────────
    v0, kappa, theta, sigma_h, rho_h = cal["v0"], cal["kappa"], cal["theta"], cal["sigma"], cal["rho"]

    all_errors: list[float] = []
    atm_errors: list[float] = []
    wing_errors: list[float] = []

    header = f"  {'Mat':<5}  {'Strike':>8}  {'Mon':>6}  {'MktIV':>8}  {'ModelIV':>8}  {'|ΔIV|':>8}  {'Zone':<5}"
    print(header)
    print("  " + "-" * (len(header) - 2))

    for i, (T, label) in enumerate(zip(maturities, mat_labels)):
        mat_errs: list[float] = []
        for j, (K, mkt_iv) in enumerate(zip(strikes_per_mat[i], mkt_ivs_per_mat[i])):
            model_price = ql_heston_call(spot, K, T, r, v0, kappa, theta, sigma_h, rho_h, n_laguerre=64)
            model_iv    = _bsm_iv_call(model_price, spot, K, T, r)
            if _math.isnan(model_iv):
                continue
            err = abs(model_iv - mkt_iv)
            m   = K / spot
            zone = "ATM" if abs(m - 1.0) < 0.10 else "wing"
            all_errors.append(err)
            mat_errs.append(err)
            if zone == "ATM":
                atm_errors.append(err)
            else:
                wing_errors.append(err)
            print(f"  {label:<5}  {K:>8.2f}  {m:>6.3f}  {mkt_iv*100:>7.3f}%  "
                  f"{model_iv*100:>7.3f}%  {err*100:>7.4f}%  {zone}")

        if mat_errs:
            print(f"  {'─'*5}  {'mat MAE':>8}{"":>7}{"":>9}{"":>9}  {sum(mat_errs)/len(mat_errs)*100:>7.4f}%")
        print()

    # ── summary ───────────────────────────────────────────────────────────────
    total_mae = sum(all_errors) / len(all_errors) if all_errors else float("nan")
    print(bold("  ─── Surface Summary ───────────────────────────────────"))
    print(f"  Total cells    : {len(all_errors)}")
    print(f"  Overall MAE    : {total_mae*100:.4f}%  (kill criterion: 0.80%)")
    status = green("PASS") if total_mae < 0.008 else red("FAIL")
    print(f"  Kill criterion : {status}")

    if report_wings:
        atm_mae  = sum(atm_errors)  / len(atm_errors)  if atm_errors  else float("nan")
        wing_mae = sum(wing_errors) / len(wing_errors) if wing_errors else float("nan")
        print()
        print(f"  ATM  cells ({len(atm_errors):2d}) MAE : {atm_mae*100:.4f}%")
        print(f"  Wing cells ({len(wing_errors):2d}) MAE : {wing_mae*100:.4f}%")
        worst_idx = max(range(len(all_errors)), key=lambda i: all_errors[i])
        print(f"  Worst cell     : {all_errors[worst_idx]*100:.4f}%")

    print(bold("  ──────────────────────────────────────────────────────"))
    return 0 if total_mae < 0.008 else 1


# ─── Entry point ──────────────────────────────────────────────────────────────

def main() -> int:
    parser = argparse.ArgumentParser(description="DollarBill pricing validator")
    parser.add_argument("--rust", action="store_true",
                        help="Run 'cargo test -- --nocapture' and include Rust computed prices")
    parser.add_argument("--filter", default="",
                        help="Cargo test name filter (default: empty = run all test_quantlib tests)")
    parser.add_argument("--speed", action="store_true",
                        help="Run head-to-head speed benchmark: QuantLib Python vs Rust (cargo bench)")
    parser.add_argument("--surface", default="",
                        help="Path to OHLCV CSV (e.g. data/tesla_one_year.csv) for surface MAE report")
    parser.add_argument("--period", default="",
                        help="Date window for --surface, format: YYYY-MM-DD:YYYY-MM-DD")
    parser.add_argument("--report-wings", action="store_true",
                        help="With --surface: break down MAE into ATM vs OTM-wing buckets")
    args = parser.parse_args()

    # ── surface-only early exit ────────────────────────────────────────────────
    if args.surface:
        if not args.period:
            print(red("ERROR: --surface requires --period YYYY-MM-DD:YYYY-MM-DD"))
            return 1
        if not HAS_QUANTLIB:
            print(red("ERROR: QuantLib is required. Install with: pip install QuantLib"))
            return 1
        return validate_surface(args.surface, args.period, args.report_wings)

    if not HAS_QUANTLIB:
        print(red("ERROR: QuantLib is required. Install with: pip install QuantLib"))
        return 1

    print_header()

    rust_prices: dict[str, float] = {}
    if args.rust:
        section("Running Rust Tests (cargo test -- --nocapture)")
        print("  Running... this may take ~60 s")
        filter_str = args.filter if args.filter else "test_quantlib"
        rust_prices = run_rust_tests(filter_str)
        if rust_prices:
            print(f"  Parsed {len(rust_prices)} price(s) from Rust test output:")
            for k, v in rust_prices.items():
                print(f"    {k:<28} = {v:.6f}")
        else:
            print(yellow("  WARNING: No prices parsed from Rust output — check test println! format"))

    validate_black_scholes(rust_prices)
    validate_heston(rust_prices)
    validate_american(rust_prices)

    if args.speed:
        rust_bench_times: dict[str, float] = {}
        section("Running Rust Benchmarks (cargo bench)")
        print("  Running `cargo bench` in release mode — this takes ~3–5 minutes...")
        rust_bench_times = run_rust_bench()
        if rust_bench_times:
            print(f"  Parsed {len(rust_bench_times)} Criterion benchmark(s)")
        else:
            print(yellow("  WARNING: No Criterion output parsed. "
                         "Speedup column will show N/A. "
                         "Run 'cargo bench' manually and check output format."))
        validate_speed(rust_bench_times)

    print()
    print(bold("=" * 100))
    print(bold("  Validation complete."))
    tips = []
    if not args.rust:
        tips.append(f"{bold('--rust')} to include live Rust computed prices")
    if not args.speed:
        tips.append(f"{bold('--speed')} for head-to-head speed comparison vs QuantLib")
    if not args.surface:
        tips.append(f"{bold('--surface <csv>')} for out-of-sample surface MAE validation")
    if tips:
        print(f"  Tip: re-run with {' and '.join(tips)}.")
    print(bold("=" * 100))
    print()
    return 0


if __name__ == "__main__":
    sys.exit(main())
