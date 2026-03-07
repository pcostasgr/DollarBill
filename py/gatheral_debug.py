"""Debug the Gatheral formula to find the correct form."""
import numpy as np
import cmath

S, K, T, r = 100.0, 100.0, 1.0, 0.05
v0, kappa, theta, sigma_v, rho = 0.04, 2.0, 0.04, 0.3, -0.7


def heston_cf(z, T, r, v0, kappa, theta, sigma, rho):
    i = 1j
    beta = kappa - rho * sigma * i * z
    d_sq = beta**2 + sigma**2 * (z**2 + i * z)
    d = cmath.sqrt(d_sq)
    g = (beta - d) / (beta + d) if abs(beta + d) > 1e-14 else 0.0
    exp_d_tau = cmath.exp(-d * T)
    one_minus_g = 1.0 - g
    one_minus_g_exp = 1.0 - g * exp_d_tau
    log_ratio = cmath.log(one_minus_g_exp / one_minus_g)
    C = (kappa * theta / sigma**2) * ((beta - d) * T - 2.0 * log_ratio)
    D = ((beta - d) / sigma**2) * (1.0 - exp_d_tau) / one_minus_g_exp
    return cmath.exp(i * z * r * T + C + D * v0)


# ── Various Gatheral-type formulas ──
N = 10000
u_max = 200.0
du = u_max / N
log_m = np.log(S / K)  # ln(S/K)
sqrt_sk = np.sqrt(S * K)
disc_half = np.exp(-r * T / 2.0)

# Form 1: C = S - sqrt(SK)*exp(-rT/2)/pi * integral 
integral1 = 0.0
for j in range(1, N + 1):
    v = j * du
    z = complex(v, -0.5)
    cf = heston_cf(z, T, r, v0, kappa, theta, sigma_v, rho)
    phase = cmath.exp(1j * v * log_m)
    integral1 += (phase * cf).real / (v**2 + 0.25) * du
call1 = S - sqrt_sk * disc_half / np.pi * integral1

# Form 2: Lewis 2001 formula
# C = S*e^(-qT) - (K*e^(-rT))/pi * int_0^inf Re[e^{-i(u-i/2)k} phi(u-i/2) / (u^2+1/4)] du
# where k = ln(K/S)
k = np.log(K / S)
integral2 = 0.0
for j in range(1, N + 1):
    u = j * du
    z = complex(u, -0.5)
    cf = heston_cf(z, T, r, v0, kappa, theta, sigma_v, rho)
    # e^{-i(u-i/2)k} = e^{-iuk} * e^{-k/2}
    phase2 = cmath.exp(-1j * (u - 0.5j) * k)
    integral2 += (phase2 * cf).real / (u**2 + 0.25) * du
call2 = S - K * np.exp(-r * T) / np.pi * integral2

# Form 3: Correct Gatheral - note the formula uses F = S*exp(rT)
# Actually from Gatheral eq (2.9):
# C(x,v,tau) = S*P1 - K*exp(-r*tau)*P2  (standard form)
# But the single-integral version is:
# C = (1/2)(S - K*exp(-rT)) + (1/pi)*int_0^inf Re[...] du
# No, that's not right either.

# Let me try the CORRECT Lewis-2000 formula:
# C = S - (S*K)^(1/2) * exp(-rT/2) / pi * int_0^inf Re[phi_tilde(u-i/2)*exp(iu*lnSK)] / (u^2+1/4) du
# where phi_tilde is CF of x = ln(S_T) (NOT log-return)

# phi_tilde(z) = phi_return(z) * exp(iz*ln(S))
# So phi_tilde(u-i/2) = phi(u-i/2) * exp(i(u-i/2)*ln(S))
#                      = phi(u-i/2) * exp(iu*ln(S) + ln(S)/2)
#                      = phi(u-i/2) * S^(1/2) * exp(iu*ln(S))

# Substituting back:
# C = S - sqrt(SK) * e^(-rT/2) / pi * int Re[phi(u-i/2) * sqrt(S) * exp(iu*lnS) * exp(iu*ln(S/K))] / (u^2+1/4) du
# Hmm this gets complicated. Let me try different approach.

# The Lewis (2000) formula (from "Option Valuation under Stochastic Volatility"):
# For CF defined over ln(S_T):
# phi_log_ST(z) = exp(iz*lnS + iz*r*T + C + D*v0) = exp(iz*lnS) * phi_return(z)
# 
# C = S - sqrt(SK)/(2*pi) * exp(-rT/2) * int_{-inf}^{inf} phi_log_ST(z-i/2) / (z^2+1/4) * exp(-iz*lnK) dz
# = S - sqrt(SK)/(2*pi) * exp(-rT/2) * int_{-inf}^{inf} exp(i(z-i/2)*lnS) * phi_return(z-i/2) / (z^2+1/4) * exp(-iz*lnK) dz
# = S - sqrt(SK)/(2*pi) * exp(-rT/2) * int exp(iz*lnS + lnS/2) * phi(z-i/2) * exp(-iz*lnK) / (z^2+1/4) dz
# = S - sqrt(SK)/(2*pi) * exp(-rT/2) * sqrt(S) * int exp(iz*ln(S/K)) * phi(z-i/2) / (z^2+1/4) dz

# For half-line (symmetric integrand):
# = S - sqrt(SK) * exp(-rT/2) * sqrt(S) / pi * int_0^inf Re[exp(iv*ln(S/K)) * phi(v-i/2)] / (v^2+1/4) dv
# = S - S * sqrt(K/S)^... 

# Actually this is getting confusing. Let me just verify using the known-correct P1P2.
# The issue might be that the Gatheral formula as commonly stated is for CF of ln(S_T), 
# not for the CF of the log-return. Our heston_cf is for the log-return.

# For the CORRECT implementation with our log-return CF:
# Lewis (2001) Theorem 3.1:
# C = S - K*exp(-rT)*inv_FT
# inv_FT = (1/2pi) int_{ik+R} exp(-iz*ln(K/S)) * phi(-z) / (iz(iz-1)) dz * S*exp(rT)
# This is getting complicated.

# Let me just verify with the trapezoidal P1P2 which we know is correct.
N = 10000
du = u_max / N
k = np.log(K / S)

integral_p1 = 0.0
integral_p2 = 0.0
for j in range(1, N + 1):
    u = j * du
    z = complex(u, 0)
    cf2 = heston_cf(z, T, r, v0, kappa, theta, sigma_v, rho)
    integrand2 = (cmath.exp(-1j * u * k) * cf2 / (1j * u)).real
    integral_p2 += integrand2 * du
    cf1 = heston_cf(z - 1j, T, r, v0, kappa, theta, sigma_v, rho)
    integrand1 = (cmath.exp(-1j * u * k) * cf1 / (1j * u * np.exp(r * T))).real
    integral_p1 += integrand1 * du

P1 = 0.5 + integral_p1 / np.pi
P2 = 0.5 + integral_p2 / np.pi
call_correct = S * P1 - K * np.exp(-r * T) * P2

print(f"Correct P1/P2:    {call_correct:.8f}")
print(f"Gatheral form 1:  {call1:.8f}")
print(f"Lewis form 2:     {call2:.8f}")
print(f"Note: Gatheral/Lewis forms use CF of log-return, which may need adjustment")
