"""Compare Heston CF values at specific u-points and compute prices via 
trapezoidal P1/P2 integration, Gatheral formula, and QuantLib."""
import numpy as np
import cmath
import QuantLib as ql

# Parameters
S, K, T, r = 100.0, 100.0, 1.0, 0.05
v0, kappa, theta, sigma_v, rho = 0.04, 2.0, 0.04, 0.3, -0.7


def heston_cf(z, T, r, v0, kappa, theta, sigma, rho):
    """Heston CF for log-return X = ln(S_T/S_0), Lord-Kahl Formulation 2."""
    i = 1j
    beta = kappa - rho * sigma * i * z
    d_sq = beta**2 + sigma**2 * (z**2 + i * z)
    d = cmath.sqrt(d_sq)
    
    # Ensure Re(d) >= 0 (principal square root does this)
    
    g = (beta - d) / (beta + d) if abs(beta + d) > 1e-14 else 0.0
    
    exp_d_tau = cmath.exp(-d * T)
    
    one_minus_g = 1.0 - g
    one_minus_g_exp = 1.0 - g * exp_d_tau
    
    log_ratio = cmath.log(one_minus_g_exp / one_minus_g)
    
    C = (kappa * theta / sigma**2) * ((beta - d) * T - 2.0 * log_ratio)
    D = ((beta - d) / sigma**2) * (1.0 - exp_d_tau) / one_minus_g_exp
    
    exponent = i * z * r * T + C + D * v0
    return cmath.exp(exponent)


# ── CF at specific points ──
print("=" * 70)
print("Heston CF values at specific u-points")
print("=" * 70)
print(f"  {'u':>6}  {'Re(phi)':>14}  {'Im(phi)':>14}  {'|phi|':>14}")
for u_val in [0.0, 0.1, 0.5, 1.0, 2.0, 5.0, 10.0, 20.0]:
    z = complex(u_val, 0)
    cf = heston_cf(z, T, r, v0, kappa, theta, sigma_v, rho)
    print(f"  {u_val:6.1f}  {cf.real:14.10f}  {cf.imag:14.10f}  {abs(cf):14.10f}")

# Verify phi(0) = 1, phi(-i) = e^(rT)
cf0 = heston_cf(0.0, T, r, v0, kappa, theta, sigma_v, rho)
cfni = heston_cf(-1j, T, r, v0, kappa, theta, sigma_v, rho)
print(f"\n  phi(0)  = {cf0.real:.10f} + {cf0.imag:.10f}i  (should be 1)")
print(f"  phi(-i) = {cfni.real:.10f} + {cfni.imag:.10f}i  (should be {np.exp(r*T):.10f})")

# ── Compute P1, P2 via trapezoidal rule ──
N_trap = 10000
u_max = 200.0
du = u_max / N_trap

integral_p1 = 0.0
integral_p2 = 0.0
k = np.log(K / S)

for j in range(1, N_trap + 1):
    u = j * du
    z = complex(u, 0)
    
    # P2 integrand: Re[e^{-iuk} phi(u) / (iu)]
    cf2 = heston_cf(z, T, r, v0, kappa, theta, sigma_v, rho)
    integrand2 = (cmath.exp(-1j * u * k) * cf2 / (1j * u)).real
    integral_p2 += integrand2 * du
    
    # P1 integrand: Re[e^{-iuk} phi(u-i) / (iu * phi(-i))]
    cf1 = heston_cf(z - 1j, T, r, v0, kappa, theta, sigma_v, rho)
    integrand1 = (cmath.exp(-1j * u * k) * cf1 / (1j * u * np.exp(r * T))).real
    integral_p1 += integrand1 * du

P1 = 0.5 + integral_p1 / np.pi
P2 = 0.5 + integral_p2 / np.pi
call_p1p2 = S * P1 - K * np.exp(-r * T) * P2

# P1/P2 WITHOUT the phi(-i) correction
integral_p1_nocorr = 0.0
for j in range(1, N_trap + 1):
    u = j * du
    z = complex(u, 0)
    cf1 = heston_cf(z - 1j, T, r, v0, kappa, theta, sigma_v, rho)
    integrand1 = (cmath.exp(-1j * u * k) * cf1 / (1j * u)).real
    integral_p1_nocorr += integrand1 * du
P1_nocorr = 0.5 + integral_p1_nocorr / np.pi
call_nocorr = S * P1_nocorr - K * np.exp(-r * T) * P2

# ── Gatheral single integral ──
integral_gath = 0.0
log_moneyness = np.log(S / K)
for j in range(1, N_trap + 1):
    v = j * du
    z = complex(v, -0.5)
    cf = heston_cf(z, T, r, v0, kappa, theta, sigma_v, rho)
    phase = cmath.exp(1j * v * log_moneyness)
    integrand = (phase * cf).real / (v**2 + 0.25)
    integral_gath += integrand * du

call_gath = S - np.sqrt(S * K) * np.exp(-r * T / 2.0) / np.pi * integral_gath

print()
print("=" * 70)
print("Python P1/P2 Pricing (trapezoidal, 10000 steps, u_max=200)")
print(f"  P1 (with 1/phi(-i)):    {P1:.10f}")
print(f"  P2:                     {P2:.10f}")
print(f"  Call (P1/P2 correct):   {call_p1p2:.8f}")
print(f"  P1 (NO correction):     {P1_nocorr:.10f}")
print(f"  Call (P1/P2 no-corr):   {call_nocorr:.8f}")
print(f"  Call (Gatheral):        {call_gath:.8f}")
print("=" * 70)

# ── QuantLib reference ──
today = ql.Date(1, 1, 2025)
ql.Settings.instance().evaluationDate = today
maturityDate = ql.Date(1, 1, 2026)
spot_h = ql.QuoteHandle(ql.SimpleQuote(S))
rate_h = ql.YieldTermStructureHandle(ql.FlatForward(today, ql.QuoteHandle(ql.SimpleQuote(r)), ql.Actual365Fixed()))
div_h = ql.YieldTermStructureHandle(ql.FlatForward(today, ql.QuoteHandle(ql.SimpleQuote(0.0)), ql.Actual365Fixed()))
proc = ql.HestonProcess(rate_h, div_h, spot_h, v0, kappa, theta, sigma_v, rho)
mod = ql.HestonModel(proc)
eng = ql.AnalyticHestonEngine(mod, 128)
payoff = ql.PlainVanillaPayoff(ql.Option.Call, K)
exercise = ql.EuropeanExercise(maturityDate)
opt = ql.VanillaOption(payoff, exercise)
opt.setPricingEngine(eng)
print(f"  QuantLib (128-GL):      {opt.NPV():.8f}")
print("=" * 70)
