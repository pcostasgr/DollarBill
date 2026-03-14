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

Requirements
------------
  pip install QuantLib scipy
  # QuantLib is also available as: pip install QuantLib-Python
"""

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


# ─── Entry point ──────────────────────────────────────────────────────────────

def main() -> int:
    parser = argparse.ArgumentParser(description="DollarBill pricing validator")
    parser.add_argument("--rust", action="store_true",
                        help="Run 'cargo test -- --nocapture' and include Rust computed prices")
    parser.add_argument("--filter", default="",
                        help="Cargo test name filter (default: empty = run all test_quantlib tests)")
    args = parser.parse_args()

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

    print()
    print(bold("=" * 100))
    print(bold("  Validation complete."))
    if not args.rust:
        print(f"  Tip: re-run with {bold('--rust')} to include Rust computed prices in the comparison.")
    print(bold("=" * 100))
    print()
    return 0


if __name__ == "__main__":
    sys.exit(main())
