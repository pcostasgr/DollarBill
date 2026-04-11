// CMA-ES (Covariance Matrix Adaptation Evolution Strategy)
// Hansen & Ostermeier 2001 — pure Rust, no external dependencies.
//
// Suitable for noisy, non-convex optimisation of 2–20 dimensional problems.
// Replaces the custom Nelder-Mead simplex for Heston calibration.

/// Configuration for the CMA-ES solver.
#[derive(Debug, Clone)]
pub struct CmaesConfig {
    /// Population size λ.  0 → auto: 4 + ⌊3 ln n⌋
    pub lambda: usize,
    /// Maximum number of function evaluations (not generations).
    pub max_fevals: usize,
    /// Stop when the function-value range across the population is below this.
    pub ftol: f64,
    /// Stop when all parameter ranges are below this.
    pub xtol: f64,
    /// Initial step size σ₀ (coordinate-independent scaling).
    pub sigma0: f64,
}

impl Default for CmaesConfig {
    fn default() -> Self {
        Self {
            lambda: 0,
            max_fevals: 10_000,
            ftol: 1e-8,
            xtol: 1e-8,
            sigma0: 0.3,
        }
    }
}

/// Result returned by [`Cmaes::minimize`].
#[derive(Debug, Clone)]
pub struct CmaesResult {
    pub best_params: Vec<f64>,
    pub best_value: f64,
    pub fevals: usize,
    pub converged: bool,
}

/// CMA-ES optimiser.
pub struct Cmaes {
    pub config: CmaesConfig,
}

impl Cmaes {
    pub fn new(config: CmaesConfig) -> Self {
        Self { config }
    }

    /// Minimise `objective(params) -> f64` starting from `x0`.
    ///
    /// Constraints are enforced by the caller via penalty returns (1e10).
    pub fn minimize<F>(&self, objective: F, x0: Vec<f64>) -> CmaesResult
    where
        F: Fn(&[f64]) -> f64,
    {
        let n = x0.len();
        assert!(n >= 1, "CMA-ES requires at least 1 dimension");

        // ── Strategy parameters (Hansen's defaults) ───────────────────────
        let lambda = if self.config.lambda == 0 {
            (4.0 + (3.0 * (n as f64).ln()).floor()) as usize
        } else {
            self.config.lambda
        };
        let mu = lambda / 2;

        // Recombination weights (log-sum-of-halves scheme)
        let weights_raw: Vec<f64> = (0..mu)
            .map(|i| ((mu + 1) as f64).ln() - ((i + 1) as f64).ln())
            .collect();
        let sum_w: f64 = weights_raw.iter().sum();
        let weights: Vec<f64> = weights_raw.iter().map(|w| w / sum_w).collect();
        let mueff: f64 = 1.0 / weights.iter().map(|w| w * w).sum::<f64>();

        // Step-size control
        let cs = (mueff + 2.0) / (n as f64 + mueff + 5.0);
        let ds = 1.0 + cs + 2.0 * ((mueff - 1.0) / (n as f64 + 1.0)).sqrt().max(0.0);
        let enn = (n as f64).sqrt() * (1.0 - 1.0 / (4.0 * n as f64) + 1.0 / (21.0 * n as f64 * n as f64));

        // Covariance matrix adaptation
        let cc = (4.0 + mueff / n as f64) / (n as f64 + 4.0 + 2.0 * mueff / n as f64);
        let c1 = 2.0 / ((n as f64 + 1.3).powi(2) + mueff);
        let cmu = {
            let a = (2.0 * (mueff - 2.0 + 1.0 / mueff)) / ((n as f64 + 2.0).powi(2) + mueff);
            a.min(1.0 - c1)
        };

        // ── State ─────────────────────────────────────────────────────────
        let mut mean: Vec<f64> = x0.clone();
        let mut sigma = self.config.sigma0;

        // Evolution paths
        let mut ps = vec![0.0_f64; n];
        let mut pc = vec![0.0_f64; n];

        // Covariance matrix (stored as flat row-major n×n)
        let mut cov: Vec<f64> = vec![0.0; n * n];
        for i in 0..n {
            cov[i * n + i] = 1.0;
        }

        let mut best_params = x0.clone();
        let mut best_value = objective(&best_params);
        let mut fevals = 1;
        let mut converged = false;

        'outer: loop {
            // ── Sample λ offspring ────────────────────────────────────────
            let chol = cholesky(&cov, n);

            let mut offspring: Vec<Vec<f64>> = Vec::with_capacity(lambda);
            let mut zs: Vec<Vec<f64>> = Vec::with_capacity(lambda);
            for _ in 0..lambda {
                let z: Vec<f64> = (0..n).map(|_| randn()).collect();
                let y = mat_vec_mul(&chol, &z, n);
                let x: Vec<f64> = mean.iter().zip(y.iter()).map(|(m, yi)| m + sigma * yi).collect();
                zs.push(z);
                offspring.push(x);
            }

            // ── Evaluate & rank ───────────────────────────────────────────
            let fvals: Vec<f64> = offspring.iter().map(|x| objective(x)).collect();
            fevals += lambda;

            let mut order: Vec<usize> = (0..lambda).collect();
            order.sort_by(|&a, &b| fvals[a].partial_cmp(&fvals[b]).unwrap_or(std::cmp::Ordering::Equal));

            if fvals[order[0]] < best_value {
                best_value = fvals[order[0]];
                best_params = offspring[order[0]].clone();
            }

            // ── Update mean ───────────────────────────────────────────────
            let old_mean = mean.clone();
            mean = vec![0.0; n];
            for (wi, &idx) in weights.iter().zip(order.iter().take(mu)) {
                for d in 0..n {
                    mean[d] += wi * offspring[idx][d];
                }
            }

            // ── Update step-size path ps ──────────────────────────────────
            // ps ← (1-cs)·ps + √(cs(2-cs)·mueff) · C^{-½}·(mean_new - mean_old)/σ
            let invsqrt_c = inv_sqrt_cov(&cov, n);
            let dm: Vec<f64> = mean.iter().zip(old_mean.iter()).map(|(a, b)| (a - b) / sigma).collect();
            let cdm = mat_vec_mul(&invsqrt_c, &dm, n);

            let coeff_s = (cs * (2.0 - cs) * mueff).sqrt();
            for d in 0..n {
                ps[d] = (1.0 - cs) * ps[d] + coeff_s * cdm[d];
            }

            let hs_norm: f64 = ps.iter().map(|v| v * v).sum::<f64>().sqrt();
            let gen = (fevals as f64 / lambda as f64).floor() as usize + 1;
            let hs = hs_norm / ((1.0 - (1.0 - cs).powi(2 * gen as i32)).sqrt()) / enn
                < 1.4 + 2.0 / (n as f64 + 1.0);

            // ── Update rank-one path pc ───────────────────────────────────
            let hs_val = if hs { 1.0 } else { 0.0 };
            let coeff_c = (cc * (2.0 - cc) * mueff).sqrt();
            for d in 0..n {
                pc[d] = (1.0 - cc) * pc[d] + hs_val * coeff_c * dm[d];
            }

            // ── Update covariance matrix ──────────────────────────────────
            // C ← (1−c1−cmu)·C + c1·(pc·pcᵀ + δhs·C) + cmu·Σ wᵢ yᵢyᵢᵀ
            let delta_hs = (1.0 - hs_val) * cc * (2.0 - cc);
            for i in 0..n {
                for j in 0..n {
                    let rank1 = pc[i] * pc[j] + delta_hs * cov[i * n + j];
                    let mut rank_mu = 0.0_f64;
                    for (wi, &idx) in weights.iter().zip(order.iter().take(mu)) {
                        let yi = (offspring[idx][i] - old_mean[i]) / sigma;
                        let yj = (offspring[idx][j] - old_mean[j]) / sigma;
                        rank_mu += wi * yi * yj;
                    }
                    cov[i * n + j] = (1.0 - c1 - cmu) * cov[i * n + j]
                        + c1 * rank1
                        + cmu * rank_mu;
                }
            }

            // ── Update step size σ ────────────────────────────────────────
            sigma *= ((cs / ds) * (hs_norm / enn - 1.0)).exp();

            // ── Convergence checks ────────────────────────────────────────
            let fmin = fvals[order[0]];
            let fmax = fvals[order[lambda - 1]];
            if (fmax - fmin).abs() < self.config.ftol {
                converged = true;
                break 'outer;
            }

            let xrange: f64 = (0..n)
                .map(|d| {
                    let lo = offspring.iter().map(|x| x[d]).fold(f64::INFINITY, f64::min);
                    let hi = offspring.iter().map(|x| x[d]).fold(f64::NEG_INFINITY, f64::max);
                    hi - lo
                })
                .fold(0.0_f64, f64::max);
            if xrange < self.config.xtol {
                converged = true;
                break 'outer;
            }

            if fevals >= self.config.max_fevals {
                break 'outer;
            }
        }

        CmaesResult { best_params, best_value, fevals, converged }
    }
}

// ── Linear algebra helpers ────────────────────────────────────────────────────

/// Cholesky decomposition L such that A = LLᵀ (lower triangular, row-major n×n).
/// Falls back to eigenvalue clamping if A is near-singular.
fn cholesky(a: &[f64], n: usize) -> Vec<f64> {
    let mut l = vec![0.0_f64; n * n];
    for i in 0..n {
        for j in 0..=i {
            let mut s: f64 = a[i * n + j];
            for k in 0..j {
                s -= l[i * n + k] * l[j * n + k];
            }
            if i == j {
                l[i * n + j] = s.max(1e-14).sqrt();
            } else {
                let diag = l[j * n + j];
                l[i * n + j] = if diag.abs() > 1e-14 { s / diag } else { 0.0 };
            }
        }
    }
    l
}

/// Approximate C^{-½} via eigendecomposition (Jacobi method – exact for n≤10).
/// For our 5-D Heston case this is fast and exact enough.
fn inv_sqrt_cov(cov: &[f64], n: usize) -> Vec<f64> {
    // Use the Jacobi method to get eigenvalues/vectors of the symmetric cov matrix
    let mut a = cov.to_vec(); // working copy
    let mut v = {                // identity
        let mut id = vec![0.0_f64; n * n];
        for i in 0..n { id[i * n + i] = 1.0; }
        id
    };

    // Jacobi sweeps
    for _ in 0..50 {
        let mut converged = true;
        for p in 0..n {
            for q in (p + 1)..n {
                let apq = a[p * n + q];
                if apq.abs() < 1e-12 { continue; }
                converged = false;
                let app = a[p * n + p];
                let aqq = a[q * n + q];
                let theta = 0.5 * (aqq - app) / apq;
                let t = if theta >= 0.0 {
                    1.0 / (theta + (1.0 + theta * theta).sqrt())
                } else {
                    -1.0 / (-theta + (1.0 + theta * theta).sqrt())
                };
                let c = 1.0 / (1.0 + t * t).sqrt();
                let s = t * c;
                // Update a
                let (app2, aqq2, apq2) = (app - t * apq, aqq + t * apq, 0.0);
                a[p * n + p] = app2;
                a[q * n + q] = aqq2;
                a[p * n + q] = apq2;
                a[q * n + p] = apq2;
                for r in 0..n {
                    if r == p || r == q { continue; }
                    let arp = a[r * n + p];
                    let arq = a[r * n + q];
                    a[r * n + p] = c * arp - s * arq;
                    a[p * n + r] = a[r * n + p];
                    a[r * n + q] = s * arp + c * arq;
                    a[q * n + r] = a[r * n + q];
                }
                // Update eigenvectors v
                for r in 0..n {
                    let vrp = v[r * n + p];
                    let vrq = v[r * n + q];
                    v[r * n + p] = c * vrp - s * vrq;
                    v[r * n + q] = s * vrp + c * vrq;
                }
            }
        }
        if converged { break; }
    }

    // Eigenvalues are now diagonal of a; build C^{-½} = V diag(λ^{-½}) Vᵀ
    let inv_sqrt_diag: Vec<f64> = (0..n).map(|i| {
        let lam = a[i * n + i].max(1e-10); // clamp negatives
        lam.sqrt().recip()
    }).collect();

    // result = V * diag * Vᵀ
    let mut result = vec![0.0_f64; n * n];
    for i in 0..n {
        for j in 0..n {
            let mut s = 0.0_f64;
            for k in 0..n {
                s += v[i * n + k] * inv_sqrt_diag[k] * v[j * n + k];
            }
            result[i * n + j] = s;
        }
    }
    result
}

/// Matrix-vector product: out = A·v  (A is n×n row-major, v is n)
fn mat_vec_mul(a: &[f64], v: &[f64], n: usize) -> Vec<f64> {
    let mut out = vec![0.0_f64; n];
    for i in 0..n {
        for j in 0..n {
            out[i] += a[i * n + j] * v[j];
        }
    }
    out
}

// ── Minimal PRNG (xoshiro256** seeded from system time) ───────────────────────
// Avoids the rand crate dependency while still producing high-quality uniform
// and Gaussian samples.

use std::cell::Cell;
use std::time::{SystemTime, UNIX_EPOCH};

thread_local! {
    static RNG: Cell<[u64; 4]> = Cell::new({
        let seed = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos() as u64)
            .unwrap_or(0x853c49e6748fea9b);
        // splitmix64 to derive 4 independent words
        let s = |mut x: u64| -> u64 {
            x = x.wrapping_add(0x9e3779b97f4a7c15);
            x = (x ^ (x >> 30)).wrapping_mul(0xbf58476d1ce4e5b9);
            x = (x ^ (x >> 27)).wrapping_mul(0x94d049bb133111eb);
            x ^ (x >> 31)
        };
        [s(seed), s(seed.wrapping_add(1)), s(seed.wrapping_add(2)), s(seed.wrapping_add(3))]
    });
}

/// xoshiro256** step — returns a u64
fn next_u64() -> u64 {
    RNG.with(|cell| {
        let mut s = cell.get();
        let result = (s[1].wrapping_mul(5)).rotate_left(7).wrapping_mul(9);
        let t = s[1] << 17;
        s[2] ^= s[0]; s[3] ^= s[1]; s[1] ^= s[2]; s[0] ^= s[3];
        s[2] ^= t;
        s[3] = s[3].rotate_left(45);
        cell.set(s);
        result
    })
}

/// Uniform sample in (0, 1)
fn rand01() -> f64 {
    // Upper 53 bits → f64 in [0, 1)
    (next_u64() >> 11) as f64 * (1.0 / (1u64 << 53) as f64)
}

/// Standard normal via Box-Muller transform
fn randn() -> f64 {
    loop {
        let u = rand01();
        let v = rand01();
        if u > 0.0 {
            return (-2.0 * u.ln()).sqrt() * (2.0 * std::f64::consts::PI * v).cos();
        }
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn minimises_rosenbrock_2d() {
        // Rosenbrock: minimum at (1,1) with f=0
        let f = |x: &[f64]| {
            let a = 1.0 - x[0];
            let b = x[1] - x[0].powi(2);
            a.powi(2) + 100.0 * b.powi(2)
        };
        let cfg = CmaesConfig { max_fevals: 5_000, sigma0: 0.5, ftol: 1e-10, xtol: 1e-10, ..Default::default() };
        let res = Cmaes::new(cfg).minimize(f, vec![-1.0, 1.5]);
        assert!(res.best_value < 1e-6, "Rosenbrock residual too high: {:.2e}", res.best_value);
        assert!((res.best_params[0] - 1.0).abs() < 1e-3, "x0 = {:.4}", res.best_params[0]);
        assert!((res.best_params[1] - 1.0).abs() < 1e-3, "x1 = {:.4}", res.best_params[1]);
    }

    #[test]
    fn minimises_sphere_5d() {
        let f = |x: &[f64]| x.iter().map(|v| v.powi(2)).sum::<f64>();
        let cfg = CmaesConfig { max_fevals: 5_000, sigma0: 1.0, ftol: 1e-10, xtol: 1e-10, ..Default::default() };
        let res = Cmaes::new(cfg).minimize(f, vec![3.0, -2.0, 1.5, -0.5, 4.0]);
        assert!(res.best_value < 1e-8, "sphere residual: {:.2e}", res.best_value);
    }
}
