// Criterion benchmarks for DollarBill pricing engines
//
// Run:   cargo bench
// HTML:  target/criterion/report/index.html

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use std::time::Duration;

use dollarbill::models::bs_mod;
use dollarbill::models::heston::HestonParams;
use dollarbill::models::heston_analytical::{
    heston_call_carr_madan, heston_put_carr_madan,
    heston_call_gauss_laguerre, heston_put_gauss_laguerre,
    heston_call_price, heston_put_price, IntegrationMethod,
    HestonCfCache,
};
use dollarbill::models::gauss_laguerre::GaussLaguerreRule;

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
    bench_gauss_laguerre_single,
    bench_gauss_laguerre_node_sweep,
    bench_gauss_laguerre_strike_sweep,
    bench_gauss_laguerre_precomputed,
    bench_unified_dispatch,
    bench_batch_pricing,
);
criterion_main!(benches);

// ═══════════════════════════════════════════════════════════════════════════════
// Gauss-Laguerre benchmarks
// ═══════════════════════════════════════════════════════════════════════════════

// ── GL: single ATM call ─────────────────────────────────────────────────────
fn bench_gauss_laguerre_single(c: &mut Criterion) {
    let mut group = c.benchmark_group("Heston Gauss-Laguerre");
    group.sample_size(200);
    group.measurement_time(Duration::from_secs(10));

    let params = heston_params();
    let rule64 = GaussLaguerreRule::new(64);

    group.bench_function("ATM call (64 nodes)", |b| {
        b.iter(|| {
            black_box(heston_call_gauss_laguerre(
                SPOT, STRIKE, MATURITY, RATE, &params, &rule64,
            ))
        })
    });

    group.bench_function("ATM put (64 nodes, via parity)", |b| {
        b.iter(|| {
            black_box(heston_put_gauss_laguerre(
                SPOT, STRIKE, MATURITY, RATE, &params, &rule64,
            ))
        })
    });

    group.finish();
}

// ── GL: sweep across node counts (32, 48, 64, 96, 128) ─────────────────────
fn bench_gauss_laguerre_node_sweep(c: &mut Criterion) {
    let mut group = c.benchmark_group("GL node-count sweep");
    group.sample_size(200);
    group.measurement_time(Duration::from_secs(8));

    let params = heston_params();

    for &n in &[32_usize, 48, 64, 96, 128] {
        let rule = GaussLaguerreRule::new(n);
        group.bench_with_input(
            BenchmarkId::new("ATM call", format!("n={n}")),
            &rule,
            |b, rule| {
                b.iter(|| {
                    black_box(heston_call_gauss_laguerre(
                        SPOT, STRIKE, MATURITY, RATE, &params, rule,
                    ))
                })
            },
        );
    }

    group.finish();
}

// ── GL: strike sweep (same 11 strikes) ──────────────────────────────────────
fn bench_gauss_laguerre_strike_sweep(c: &mut Criterion) {
    let mut group = c.benchmark_group("GL strike sweep (11 strikes)");
    group.sample_size(100);
    group.measurement_time(Duration::from_secs(10));

    let params = heston_params();
    let strikes: Vec<f64> = (80..=120).step_by(4).map(|k| k as f64).collect();
    let rule64 = GaussLaguerreRule::new(64);

    group.bench_function("11 calls (64 nodes)", |b| {
        b.iter(|| {
            for &k in &strikes {
                black_box(heston_call_gauss_laguerre(
                    SPOT, k, MATURITY, RATE, &params, &rule64,
                ));
            }
        })
    });

    group.finish();
}

// ── GL with pre-computed rule vs new-rule-each-call ─────────────────────────
fn bench_gauss_laguerre_precomputed(c: &mut Criterion) {
    let mut group = c.benchmark_group("GL precomputed vs on-the-fly");
    group.sample_size(200);
    group.measurement_time(Duration::from_secs(8));

    let params = heston_params();
    let rule64 = GaussLaguerreRule::new(64);

    group.bench_function("pre-computed rule", |b| {
        b.iter(|| {
            black_box(heston_call_gauss_laguerre(
                SPOT, STRIKE, MATURITY, RATE, &params, &rule64,
            ))
        })
    });

    group.bench_function("new rule each call", |b| {
        b.iter(|| {
            let rule = GaussLaguerreRule::new(64);
            black_box(heston_call_gauss_laguerre(
                SPOT, STRIKE, MATURITY, RATE, &params, &rule,
            ))
        })
    });

    group.finish();
}

// ── Unified dispatch: compare CM vs GL via heston_call_price ────────────────
fn bench_unified_dispatch(c: &mut Criterion) {
    let mut group = c.benchmark_group("Unified dispatch comparison");
    group.sample_size(200);
    group.measurement_time(Duration::from_secs(10));

    let params = heston_params();

    group.bench_function("heston_call_price (CarrMadan)", |b| {
        let method = IntegrationMethod::CarrMadan;
        b.iter(|| {
            black_box(heston_call_price(
                SPOT, STRIKE, MATURITY, RATE, &params, &method,
            ))
        })
    });

    for &n in &[32_usize, 64] {
        group.bench_function(
            &format!("heston_call_price (GL-{n})"),
            |b| {
                let method = IntegrationMethod::GaussLaguerre { nodes: n };
                b.iter(|| {
                    black_box(heston_call_price(
                        SPOT, STRIKE, MATURITY, RATE, &params, &method,
                    ))
                })
            },
        );
    }

    group.finish();
}

// ═══════════════════════════════════════════════════════════════════════════════
// Batch pricing benchmark: 50 strikes × N maturities, with CF cache
// ═══════════════════════════════════════════════════════════════════════════════

fn bench_batch_pricing(c: &mut Criterion) {
    let mut group = c.benchmark_group("Batch pricing (50 strikes × N maturities)");
    group.sample_size(100);
    group.measurement_time(Duration::from_secs(10));

    let params = heston_params();
    let rule64 = GaussLaguerreRule::new(64);

    // 50 strikes: 60% to 140% of spot in equal steps
    let n_strikes = 50_usize;
    let strikes: Vec<f64> = (0..n_strikes)
        .map(|i| SPOT * (0.60 + 0.80 * (i as f64) / (n_strikes - 1) as f64))
        .collect();

    let maturities_5  = vec![0.25, 0.50, 1.0, 2.0, 5.0];
    let maturities_10 = vec![0.08, 0.17, 0.25, 0.50, 0.75, 1.0, 1.5, 2.0, 3.0, 5.0];

    // ── Naïve: call heston_call_gauss_laguerre individually ─────────────
    for (label, mats) in &[("5 mat", &maturities_5), ("10 mat", &maturities_10)] {
        let total = n_strikes * mats.len();
        group.bench_function(
            &format!("naïve GL-64 ({total} opts, {label})"),
            |b| {
                b.iter(|| {
                    let mut sum = 0.0_f64;
                    for &tau in mats.iter() {
                        for &k in &strikes {
                            sum += heston_call_gauss_laguerre(
                                SPOT, k, tau, RATE, &params, &rule64,
                            );
                        }
                    }
                    black_box(sum)
                })
            },
        );
    }

    // ── Cached: build HestonCfCache per maturity, batch price strikes ───
    for (label, mats) in &[("5 mat", &maturities_5), ("10 mat", &maturities_10)] {
        let total = n_strikes * mats.len();
        group.bench_function(
            &format!("cached GL-64 ({total} opts, {label})"),
            |b| {
                b.iter(|| {
                    let mut sum = 0.0_f64;
                    for &tau in mats.iter() {
                        let cache = HestonCfCache::new(
                            SPOT, tau, RATE, &params, &rule64,
                        );
                        let prices = cache.price_calls(&strikes);
                        for p in &prices {
                            sum += p;
                        }
                    }
                    black_box(sum)
                })
            },
        );
    }

    // ── Cached, cache-only overhead (just building caches, no pricing) ──
    group.bench_function("cache build only (10 maturities)", |b| {
        b.iter(|| {
            for &tau in &maturities_10 {
                black_box(HestonCfCache::new(
                    SPOT, tau, RATE, &params, &rule64,
                ));
            }
        })
    });

    group.finish();
}
