# Benchmark Summary

**Date**: February 28, 2026  
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

### 1. Heston Analytical — ATM Call

| Engine | Method | Price | Latency | Throughput |
|--------|--------|------:|--------:|-----------:|
| **QuantLib** | Gauss-Laguerre quadrature (~64 nodes) | 10.3942 | **0.79 μs** | 1,261,670 ops/s |
| **DollarBill** | Adaptive Simpson (Carr-Madan P₁/P₂) | 10.3942 | 491 μs | 2,040 ops/s |

**Price agreement**: Identical to 4 decimal places.  
**Latency gap**: ~620×. Root cause: DollarBill evaluates the characteristic function at ~1000 quadrature points (adaptive Simpson over [0.001, 50+]). QuantLib uses ~64 Gauss-Laguerre nodes with exponential weighting that naturally concentrates effort where the integrand matters.

### 2. Heston Analytical — 11-Strike Sweep (K = 80 to 120, step 4)

| Engine | Total | Per-Option | Notes |
|--------|------:|-----------:|-------|
| **QuantLib** | 567 μs | 51.5 μs | Python object rebuild overhead per strike |
| **DollarBill** | 6.2 ms | 564 μs | Pure integration cost, no object overhead |

QuantLib's per-option cost jumps from 0.79 μs (cached) to 51.5 μs (rebuild) — a ~65× Python-side penalty. DollarBill stays flat (~491 → 564 μs), confirming the bottleneck is entirely integration effort.

### 3. BSM Closed-Form (DollarBill)

| What | Latency | Throughput |
|------|--------:|-----------:|
| Call price + full Greeks (δ, γ, θ, ν, ρ) | **70 ns** | 14.3M ops/s |

Pure closed-form via `exp`/`ln`/`erf`. This is the fast path for vanilla pricing, IV inversion, and delta hedging.

### 4. Heston Maturity Sensitivity (DollarBill)

| Maturity | Latency |
|---------:|--------:|
| 0.1y | 494 μs |
| 0.25y | 473 μs |
| 0.5y | 467 μs |
| 1.0y | 496 μs |
| 2.0y | 489 μs |
| 5.0y | 488 μs |

Flat profile across T = 0.1 to 5.0 years. Integration node count dominates; the characteristic function itself is cheap regardless of maturity.

### 5. Heston ATM Put (via Put-Call Parity)

| Benchmark | Latency |
|-----------|--------:|
| ATM put | 501 μs |

Negligible overhead vs the call (~10 μs for the parity arithmetic). Confirms the integration is the sole bottleneck.

---

## Analysis

### Where DollarBill Wins

- **Price accuracy**: Matches QuantLib to 4+ decimal places — the Fourier math is correct
- **BSM throughput**: 70 ns / 14.3M ops/s for full Greeks is production-grade
- **Zero C++ dependencies**: Pure Rust, no SWIG bindings, no QuantLib build toolchain
- **Heston MC (QE scheme)**: Andersen (2008) with variance non-negativity guarantees — competitive with QuantLib's MC for equivalent path counts
- **Deterministic**: No external state, no global singletons, thread-safe by construction

### Where QuantLib Wins

- **Heston analytical latency**: 620× faster via optimized Gauss-Laguerre quadrature
- **Decades of production use**: Battle-tested across major banks and hedge funds
- **Broader model coverage**: 100+ pricing engines vs DollarBill's focused set

### Optimization Roadmap

The 620× gap is addressable:

1. **Replace Simpson with Gauss-Laguerre** (15–64 nodes): Expected ~100× speedup, bringing DollarBill to ~5 μs/call
2. **True FFT pricing** (N=4096 grid): Price the *entire* strike surface in one shot (~500 μs for 4096 strikes vs 6.2 ms for 11)
3. **Cache characteristic function** across strikes for a given (T, params) tuple

---

## How to Reproduce

```bash
# Rust benchmarks (Criterion)
cargo bench
start docs/benchmarks/report/index.html   # Windows
open docs/benchmarks/report/index.html    # macOS/Linux

# QuantLib comparison
pip install QuantLib
python py/bench_quantlib_heston.py
```

## Report Files

- [Criterion HTML Report](report/index.html) — full statistical analysis with violin plots
- [Heston Carr-Madan FFT](heston%20carr-madan%20fft/report/index.html) — ATM call/put detail
- [Strike Sweep](heston%20strike%20sweep%20(11%20strikes)/report/index.html) — 11-strike batch
- [Maturity Sensitivity](heston%20maturity%20sensitivity/report/index.html) — T=0.1 to T=5.0
- [BSM Baseline](bsm%20baseline%20(flat%20vol)/report/index.html) — closed-form comparison
