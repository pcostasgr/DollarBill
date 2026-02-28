"""
QuantLib-Python Heston benchmark — same parameters as benches/heston_pricing.rs

Install: pip install QuantLib-Python
Run:     python py/bench_quantlib_heston.py
"""

import QuantLib as ql
import timeit
import sys

# ── Evaluation date ──────────────────────────────────────────────────────────
today = ql.Date(28, 2, 2026)
ql.Settings.instance().evaluationDate = today

# ── Market data (must match Rust constants) ──────────────────────────────────
SPOT = 100.0
STRIKE = 100.0
RATE = 0.05
DIV = 0.0

# Heston params
V0 = 0.04
KAPPA = 2.0
THETA = 0.04
SIGMA = 0.3
RHO = -0.7

# ── Build QuantLib objects ───────────────────────────────────────────────────
spot_handle = ql.QuoteHandle(ql.SimpleQuote(SPOT))
rate_ts = ql.YieldTermStructureHandle(
    ql.FlatForward(today, RATE, ql.Actual365Fixed())
)
div_ts = ql.YieldTermStructureHandle(
    ql.FlatForward(today, DIV, ql.Actual365Fixed())
)

process = ql.HestonProcess(rate_ts, div_ts, spot_handle, V0, KAPPA, THETA, SIGMA, RHO)
model = ql.HestonModel(process)

# AnalyticHestonEngine: Fourier inversion (Lewis / Gatheral formulation)
# tolerance=0.01, maxEvaluations=1000  (matches Carr-Madan integration effort)
engine = ql.AnalyticHestonEngine(model, 0.01, 1000)

payoff = ql.PlainVanillaPayoff(ql.Option.Call, STRIKE)
exercise = ql.EuropeanExercise(today + ql.Period(1, ql.Years))
option = ql.EuropeanOption(payoff, exercise)
option.setPricingEngine(engine)


def price():
    return option.NPV()


# ── Run benchmark ────────────────────────────────────────────────────────────
print("=" * 60)
print("QuantLib Heston AnalyticHestonEngine Benchmark")
print("=" * 60)
print(f"  Spot={SPOT}  K={STRIKE}  T=1y  r={RATE}")
print(f"  v0={V0}  κ={KAPPA}  θ={THETA}  σ={SIGMA}  ρ={RHO}")
print()

npv = price()
print(f"  QuantLib price : {npv:.6f}")

# Warm up JIT / caches
timeit.repeat(price, number=500, repeat=3)

# Actual timing
NUMBER = 1000
REPEAT = 20
times = timeit.repeat(price, number=NUMBER, repeat=REPEAT)
best_us = min(times) * 1e6 / NUMBER
median_us = sorted(times)[REPEAT // 2] * 1e6 / NUMBER

print(f"  Best   : {best_us:8.2f} μs / call  ({1e6/best_us:,.0f} ops/s)")
print(f"  Median : {median_us:8.2f} μs / call  ({1e6/median_us:,.0f} ops/s)")
print()

# ── Strike sweep (same 11 strikes as Rust bench) ────────────────────────────
strikes = list(range(80, 121, 4))


def price_sweep():
    total = 0.0
    for k in strikes:
        payoff_k = ql.PlainVanillaPayoff(ql.Option.Call, float(k))
        opt = ql.EuropeanOption(payoff_k, exercise)
        opt.setPricingEngine(engine)
        total += opt.NPV()
    return total


sweep_times = timeit.repeat(price_sweep, number=100, repeat=10)
sweep_best = min(sweep_times) * 1e6 / 100
print(f"  11-strike sweep: {sweep_best:8.2f} μs / sweep  ({11*1e6/sweep_best:,.0f} single-option ops/s)")
print("=" * 60)
