# Benchmark Summary

**Date**: March 7, 2026  
**Machine**: Windows, Rust 2021 edition (release profile)  
**Harness**: [Criterion.rs](https://github.com/bheisler/criterion.rs) v0.5 (200 samples, 10s measurement)  
**QuantLib**: v1.41 via Python bindings (`AnalyticHestonEngine`)

## Parameters

All Heston benchmarks use the same classic literature example:

| Parameter | Value | Description |
|-----------|------:|-------------|
| S | 100.0 | Spot price |
| K | 100.0 | Strike (ATM) |
| T | 1.0 | Time to maturity (years) |
| r | 0.05 | Risk-free rate |
| q | 0.0 | Dividend yield |
| v₀ | 0.04 | Initial variance (σ = 20%) |
| κ | 2.0 | Mean reversion speed |
| θ | 0.04 | Long-term variance |
| σ | 0.3 | Vol-of-vol |
| ρ | −0.7 | Spot-vol correlation |

Feller condition satisfied: 2κθ/σ² = 1.78 > 1.

---

## Results

### 1. Heston Analytical — ATM Call (all engines)

| Engine | Method | Price | Latency | Throughput |
|--------|--------|------:|--------:|-----------:|
| **DollarBill** | GL-32 (precomputed rule) | 10.3942 | **15 µs** | 66,700 ops/s |
| **DollarBill** | GL-64 (precomputed rule) | 10.3942 | **33 µs** | 30,300 ops/s |
| **QuantLib** | Gauss-Laguerre (~64 nodes) | 10.3942 | 39.25 µs | 25,480 ops/s |
| **DollarBill** | GL-128 (precomputed rule) | 10.3942 | 69 µs | 14,500 ops/s |
| **DollarBill** | Carr-Madan (adaptive Simpson) | 10.4506* | 474 µs | 2,040 ops/s |

\* Carr-Madan uses the legacy `characteristic_function()`; GL uses the corrected Lord-Kahl Formulation 2 CF.

**Price agreement**: DollarBill GL-64 matches QuantLib to **6 significant figures** (10.394219 vs 10.394218).

**GL-32 is now faster than QuantLib** (15 µs vs 39 µs) while being fully converged — the optimization roadmap from the previous benchmark report has been **completed**.

### 2. Heston Analytical — 11-Strike Sweep (K = 80 to 120, step 4)

| Engine | Method | Total | Per-Option |
|--------|--------|------:|-----------:|
| **DollarBill** | GL-64 (precomputed) | **398 µs** | **36 µs** |
| **QuantLib** | AnalyticHestonEngine | 531 µs | 48 µs |
| **DollarBill** | Carr-Madan (adaptive Simpson) | 5.43 ms | 494 µs |

DollarBill GL-64 sweep is **13.6× faster** than Carr-Madan and **1.3× faster** than QuantLib for the 11-strike batch.

### 2b. Batch Pricing with CF Cache — 50 Strikes × N Maturities (GL-64) 🆕

`HestonCfCache` evaluates the characteristic function **once** per GL node per maturity, then reuses the cached CF/iz values across all strikes. Only the cheap phase factor `exp(−i·z·ln(K/S))` is computed per strike.

| Approach | Options | Total | Per-Option | Speedup |
|----------|--------:|------:|-----------:|--------:|
| Naïve GL-64 | 250 (50K × 5T) | 5.85 ms | 23.4 µs | — |
| **Cached GL-64** | **250 (50K × 5T)** | **0.58 ms** | **2.3 µs** | **10.0×** |
| Naïve GL-64 | 500 (50K × 10T) | 12.2 ms | 24.3 µs | — |
| **Cached GL-64** | **500 (50K × 10T)** | **1.16 ms** | **2.3 µs** | **10.5×** |
| Cache build only | 10 maturities | 0.21 ms | 20.6 µs/mat | — |

For a full vol surface (50 strikes × 10 maturities = 500 options), total wall time is **1.16 ms** — amortized cost of **2.3 µs per option**.

### 3. GL Node-Count Sweep (DollarBill)

| Nodes | Latency | Price | Error vs GL-128 |
|------:|--------:|------:|-----------------:|
| 8 | — | 10.394168 | 5.0e-5 |
| 16 | — | 10.394218 | 1.0e-6 |
| 32 | 15 µs | 10.394219 | 0.0 |
| 48 | 22 µs | 10.394219 | 0.0 |
| 64 | 33 µs | 10.394219 | 0.0 |
| 96 | 51 µs | 10.394219 | 0.0 |
| 128 | 69 µs | 10.394219 | 0.0 |

GL converges by 16 nodes (error < 1e-6). Even GL-8 is within 50 µ$ of the converged price. **32 nodes is the sweet spot** for production use: 15 µs with full convergence.

### 4. GL Precomputed vs On-the-Fly Rule

| Setup | Latency |
|-------|--------:|
| Pre-computed `GaussLaguerreRule` | 37 µs |
| New rule each call | 307 µs |

Rule construction costs ~270 µs (Newton root-finding + eigenvalue decomposition). **Always cache the `GaussLaguerreRule` when pricing multiple options** with the same node count.

### 5. BSM Closed-Form (DollarBill)

| What | Latency | Throughput |
|------|--------:|-----------:|
| Call price + full Greeks (δ, γ, θ, ν, ρ) | **79 ns** | 12.7M ops/s |

Pure closed-form via `exp`/`ln`/`erf`. This is the fast path for vanilla pricing, IV inversion, and delta hedging.

### 6. Heston Maturity Sensitivity — Carr-Madan (DollarBill)

| Maturity | Latency |
|---------:|--------:|
| 0.1y | 397 µs |
| 0.25y | 439 µs |
| 0.5y | 457 µs |
| 1.0y | 457 µs |
| 2.0y | 492 µs |
| 5.0y | 546 µs |

Flat profile across T = 0.1 to 5.0 years. Integration node count dominates; the characteristic function itself is cheap regardless of maturity.

### 7. Unified Dispatch Overhead

| Method | Latency |
|--------|--------:|
| `heston_call_price(CarrMadan)` | 472 µs |
| `heston_call_price(GL-32)` | 63 µs |
| `heston_call_price(GL-64)` | 328 µs |

The unified `heston_call_price()` dispatch adds negligible overhead vs direct function calls. The GL-64 dispatch number (328 µs) is higher than the precomputed benchmark (33 µs) because it constructs a new `GaussLaguerreRule` each call.

### 8. QuantLib Cross-Validation — Strike Sweep

| Strike | DollarBill GL-64 | QuantLib | Abs Error |
|-------:|-----------------:|---------:|----------:|
| 80 | 25.044557 | 25.0446 | < 0.0001 |
| 90 | 17.075310 | 17.0753 | < 0.0001 |
| 100 | 10.394219 | 10.3942 | < 0.0001 |
| 110 | 5.430339 | 5.4303 | < 0.0001 |
| 120 | 2.332633 | 2.3326 | < 0.0001 |

Put-call parity holds to **machine precision** (< 1e-12) across all strikes.

High vol-of-vol case (σ=1.0, ρ=−0.9): DollarBill GL-64 = 8.593, QuantLib = 8.557 (0.04 abs error — GL converges but this extreme case needs more nodes or a different integration strategy).

---

## Analysis

### Where DollarBill Wins

- **Price accuracy**: Matches QuantLib to **6 significant figures** — verified via QuantLib v1.41 cross-validation
- **GL-32 latency**: 15 µs/call, **faster than QuantLib** (39 µs) while fully converged
- **BSM throughput**: 79 ns / 12.7M ops/s for full Greeks is production-grade
- **Zero C++ dependencies**: Pure Rust, no SWIG bindings, no QuantLib build toolchain
- **11-strike sweep**: GL-64 batch (398 µs) beats QuantLib (531 µs) by 1.3×
- **Heston MC (QE scheme)**: Andersen (2008) with variance non-negativity guarantees
- **Deterministic**: No external state, no global singletons, thread-safe by construction

### Where QuantLib Wins

- **Decades of production use**: Battle-tested across major banks and hedge funds
- **Broader model coverage**: 100+ pricing engines vs DollarBill's focused set
- **High vol-of-vol**: More robust for extreme parameters (σ > 0.8)

### Optimization Roadmap — Status

| Optimization | Status | Impact |
|-------------|--------|--------|
| ~~Replace Simpson with Gauss-Laguerre~~ | ✅ **DONE** | 14.4× speedup (474 → 33 µs) |
| ~~Lord-Kahl CF (stable formulation)~~ | ✅ **DONE** | QuantLib-matched accuracy |
| ~~P₁ normalization fix~~ | ✅ **DONE** | Correct 1/φ(−i) factor |
| ~~CF caching across strikes~~ | ✅ **DONE** | **10× batch speedup** (23 → 2.3 µs/opt) |
| True FFT pricing (N=4096 grid) | 🔲 Planned | Price entire strike surface in one shot |
| SIMD vectorization of GL inner loop | 🔲 Planned | Potential ~2× for single-call |

---

## How to Reproduce

```bash
# Rust benchmarks (Criterion)
cargo bench
cargo bench -- "Gauss-Laguerre"            # GL benchmarks only
cargo bench -- "Carr-Madan"                 # CM benchmarks only
start docs/benchmarks/report/index.html     # Windows — open full HTML report

# QuantLib comparison
pip install QuantLib
python py/bench_quantlib_heston.py          # Timing comparison
python py/quantlib_ref.py                   # Reference prices

# QuantLib cross-validation tests
cargo test --test lib quantlib -- --nocapture
```

## Report Files

- [Criterion HTML Report](report/index.html) — full statistical analysis with violin plots
- [Heston Carr-Madan FFT](heston%20carr-madan%20fft/report/index.html) — ATM call/put detail
- [Strike Sweep](heston%20strike%20sweep%20(11%20strikes)/report/index.html) — 11-strike batch
- [Maturity Sensitivity](heston%20maturity%20sensitivity/report/index.html) — T=0.1 to T=5.0
- [BSM Baseline](bsm%20baseline%20(flat%20vol)/report/index.html) — closed-form comparison
- [GL Node Sweep](gl%20node-count%20sweep/report/index.html) — 32 to 128 node comparison 🆕
- [GL Precomputed vs On-the-fly](gl%20precomputed%20vs%20on-the-fly/report/index.html) — Rule caching impact 🆕
- [Unified Dispatch](unified%20dispatch%20comparison/report/index.html) — CarrMadan vs GL dispatch 🆕
