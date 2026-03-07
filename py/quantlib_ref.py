"""Compute Heston reference prices using QuantLib Python."""
import QuantLib as ql

today = ql.Date(1, 1, 2025)
ql.Settings.instance().evaluationDate = today
maturityDate = ql.Date(1, 1, 2026)

spot_handle = ql.QuoteHandle(ql.SimpleQuote(100.0))
rate_ts = ql.YieldTermStructureHandle(
    ql.FlatForward(today, ql.QuoteHandle(ql.SimpleQuote(0.05)), ql.Actual365Fixed()))
div_ts = ql.YieldTermStructureHandle(
    ql.FlatForward(today, ql.QuoteHandle(ql.SimpleQuote(0.0)), ql.Actual365Fixed()))

# ── Classic Heston ──
v0, kappa, theta, sigma, rho = 0.04, 2.0, 0.04, 0.3, -0.7
proc = ql.HestonProcess(rate_ts, div_ts, spot_handle, v0, kappa, theta, sigma, rho)
model = ql.HestonModel(proc)
engine = ql.AnalyticHestonEngine(model, 128)

print("=" * 70)
print("QuantLib Heston Reference Prices (AnalyticHestonEngine, 128-pt GL)")
print("S=100, T=1y, r=5%, v0=0.04, kappa=2, theta=0.04, sigma=0.3, rho=-0.7")
print("=" * 70)

for K in [80, 90, 100, 110, 120]:
    payoff = ql.PlainVanillaPayoff(ql.Option.Call, float(K))
    exercise = ql.EuropeanExercise(maturityDate)
    option = ql.VanillaOption(payoff, exercise)
    option.setPricingEngine(engine)
    print(f"  K={K:>3}  Call = {option.NPV():.8f}")

# ── High vol-of-vol ──
print()
print("High Vol-of-Vol: v0=0.09, kappa=1.5, theta=0.09, sigma=1.0, rho=-0.9, T=0.5y")
proc2 = ql.HestonProcess(rate_ts, div_ts, spot_handle, 0.09, 1.5, 0.09, 1.0, -0.9)
model2 = ql.HestonModel(proc2)
engine2 = ql.AnalyticHestonEngine(model2, 128)
matDate2 = today + ql.Period(6, ql.Months)
exercise2 = ql.EuropeanExercise(matDate2)
payoff2 = ql.PlainVanillaPayoff(ql.Option.Call, 100.0)
option2 = ql.VanillaOption(payoff2, exercise2)
option2.setPricingEngine(engine2)
print(f"  K=100  Call = {option2.NPV():.8f}")

# ── Near-BS ──
print()
print("Near-BS: v0=0.01, kappa=5, theta=0.01, sigma=0.01, rho=0.0, T=1y")
proc3 = ql.HestonProcess(rate_ts, div_ts, spot_handle, 0.01, 5.0, 0.01, 0.01, 0.0)
model3 = ql.HestonModel(proc3)
engine3 = ql.AnalyticHestonEngine(model3, 128)
exercise3 = ql.EuropeanExercise(maturityDate)
payoff3 = ql.PlainVanillaPayoff(ql.Option.Call, 100.0)
option3 = ql.VanillaOption(payoff3, exercise3)
option3.setPricingEngine(engine3)
print(f"  K=100  Call = {option3.NPV():.8f}")

print("=" * 70)
