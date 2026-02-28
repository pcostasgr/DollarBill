// Criterion benchmarks for DollarBill pricing engines
//
// Run:   cargo bench
// HTML:  target/criterion/report/index.html

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use std::time::Duration;

use dollarbill::models::bs_mod;
use dollarbill::models::heston::HestonParams;
use dollarbill::models::heston_analytical::{heston_call_carr_madan, heston_put_carr_madan};

// ── Fixed test parameters (classic Heston literature example) ───────────────
const SPOT: f64 = 100.0;
const STRIKE: f64 = 100.0;
const MATURITY: f64 = 1.0;
const RATE: f64 = 0.05;

const V0: f64 = 0.04;
const KAPPA: f64 = 2.0;
const THETA: f64 = 0.04;
const SIGMA_V: f64 = 0.3;
const RHO: f64 = -0.7;

fn heston_params() -> HestonParams {
    HestonParams {
        s0: SPOT,
        v0: V0,
        kappa: KAPPA,
        theta: THETA,
        sigma: SIGMA_V,
        rho: RHO,
        r: RATE,
        t: MATURITY,
    }
}

// ── Carr-Madan FFT: single ATM call ────────────────────────────────────────
fn bench_carr_madan_single(c: &mut Criterion) {
    let mut group = c.benchmark_group("Heston Carr-Madan FFT");
    group.sample_size(200);
    group.measurement_time(Duration::from_secs(10));

    let params = heston_params();

    group.bench_function("ATM call", |b| {
        b.iter(|| {
            black_box(heston_call_carr_madan(SPOT, STRIKE, MATURITY, RATE, &params))
        })
    });

    group.bench_function("ATM put (via parity)", |b| {
        b.iter(|| {
            black_box(heston_put_carr_madan(SPOT, STRIKE, MATURITY, RATE, &params))
        })
    });

    group.finish();
}

// ── Carr-Madan: sweep across strikes ────────────────────────────────────────
fn bench_carr_madan_strike_sweep(c: &mut Criterion) {
    let mut group = c.benchmark_group("Heston strike sweep (11 strikes)");
    group.sample_size(100);
    group.measurement_time(Duration::from_secs(10));

    let params = heston_params();
    let strikes: Vec<f64> = (80..=120).step_by(4).map(|k| k as f64).collect();

    group.bench_function("11 calls", |b| {
        b.iter(|| {
            for &k in &strikes {
                black_box(heston_call_carr_madan(SPOT, k, MATURITY, RATE, &params));
            }
        })
    });

    group.finish();
}

// ── Carr-Madan: varying maturity ────────────────────────────────────────────
fn bench_carr_madan_maturity(c: &mut Criterion) {
    let mut group = c.benchmark_group("Heston maturity sensitivity");
    group.sample_size(100);
    group.measurement_time(Duration::from_secs(8));

    let params = heston_params();

    for &t in &[0.1, 0.25, 0.5, 1.0, 2.0, 5.0] {
        group.bench_with_input(BenchmarkId::new("call", format!("T={t}")), &t, |b, &t| {
            b.iter(|| black_box(heston_call_carr_madan(SPOT, STRIKE, t, RATE, &params)))
        });
    }

    group.finish();
}

// ── BSM baseline for comparison ─────────────────────────────────────────────
fn bench_bsm_baseline(c: &mut Criterion) {
    let mut group = c.benchmark_group("BSM baseline (flat vol)");
    group.sample_size(200);
    group.measurement_time(Duration::from_secs(5));

    let vol = V0.sqrt(); // match Heston's initial vol

    group.bench_function("ATM call + Greeks", |b| {
        b.iter(|| {
            black_box(bs_mod::black_scholes_merton_call(
                SPOT, STRIKE, MATURITY, RATE, vol, 0.0,
            ))
        })
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_carr_madan_single,
    bench_carr_madan_strike_sweep,
    bench_carr_madan_maturity,
    bench_bsm_baseline,
);
criterion_main!(benches);
