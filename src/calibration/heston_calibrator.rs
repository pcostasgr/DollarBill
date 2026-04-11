// Heston parameter calibration using CMA-ES (Covariance Matrix Adaptation ES)
// with IV-space objective, soft Feller penalty, multi-restart, and NM fallback.

use crate::models::heston::HestonParams;
use crate::models::heston_analytical::HestonCfCache;
use crate::models::gauss_laguerre::GaussLaguerreRule;
use crate::models::bs_mod::{black_scholes_merton_call, black_scholes_merton_put};
use crate::calibration::market_option::{MarketOption, OptionType};
use crate::calibration::cmaes::{Cmaes, CmaesConfig};

/// Compact calibration parameters (just the 5 we're fitting)
#[derive(Debug, Clone)]
pub struct CalibParams {
    pub kappa: f64,
    pub theta: f64,
    pub sigma: f64,
    pub rho: f64,
    pub v0: f64,
}

impl CalibParams {
    pub fn to_heston(&self, spot: f64, rate: f64, time: f64) -> HestonParams {
        HestonParams {
            s0: spot,
            v0: self.v0,
            kappa: self.kappa,
            theta: self.theta,
            sigma: self.sigma,
            rho: self.rho,
            r: rate,
            t: time,
        }
    }
}

/// Result of Heston calibration
#[derive(Debug, Clone)]
pub struct CalibrationResult {
    pub params: CalibParams,
    pub rmse: f64,
    pub iterations: u64,
    pub success: bool,
    pub initial_error: f64,
    pub final_error: f64,
}

impl CalibrationResult {
    pub fn to_heston(&self, spot: f64, rate: f64, time: f64) -> HestonParams {
        self.params.to_heston(spot, rate, time)
    }
    
    pub fn print_summary(&self) {
        println!("\n{}", "=".repeat(70));
        println!("HESTON CALIBRATION RESULTS");
        println!("{}", "=".repeat(70));
        
        if self.success {
            println!("✓ Calibration succeeded");
        } else {
            println!("⚠ Calibration may not have converged");
        }
        
        println!("\nCalibrated Parameters:");
        println!("  κ (kappa):     {:.4}  (mean reversion speed)", self.params.kappa);
        println!("  θ (theta):     {:.4}  (long-term variance, vol={:.2}%)", 
                 self.params.theta, self.params.theta.sqrt() * 100.0);
        println!("  σ (sigma):     {:.4}  (vol-of-vol)", self.params.sigma);
        println!("  ρ (rho):       {:.4}  (correlation)", self.params.rho);
        println!("  v₀ (v0):       {:.4}  (initial variance, vol={:.2}%)", 
                 self.params.v0, self.params.v0.sqrt() * 100.0);
        
        println!("\nFeller Condition: 2κθ = {:.4} {} σ² = {:.4}",
                 2.0 * self.params.kappa * self.params.theta,
                 if 2.0 * self.params.kappa * self.params.theta > self.params.sigma.powi(2) { ">" } else { "≤" },
                 self.params.sigma.powi(2));
        
        println!("\nFit Quality:");
        println!("  Initial RMSE:  ${:.4}", self.initial_error);
        println!("  Final RMSE:    ${:.4}", self.final_error);
        println!("  Improvement:   {:.1}%", 
                 (self.initial_error - self.final_error) / self.initial_error * 100.0);
        println!("  Iterations:    {}", self.iterations);
        println!("{}", "=".repeat(70));
    }
}

/// Invert BSM price to implied vol via bisection (50 steps ≈ 15 digits).
fn bsm_iv_calib(market_price: f64, s: f64, k: f64, t: f64, r: f64, is_call: bool) -> Option<f64> {
    if market_price <= 0.0 || t <= 0.0 || s <= 0.0 || k <= 0.0 { return None; }
    let discount = (-r * t).exp();
    let intrinsic = if is_call { (s - k * discount).max(0.0) } else { (k * discount - s).max(0.0) };
    if market_price < intrinsic - 1e-6 { return None; }
    let mut lo = 1e-6_f64;
    let mut hi = 10.0_f64;
    for _ in 0..50 {
        let mid = (lo + hi) * 0.5;
        let price = if is_call {
            black_scholes_merton_call(s, k, t, r, mid, 0.0).price
        } else {
            black_scholes_merton_put(s, k, t, r, mid, 0.0).price
        };
        if price < market_price { lo = mid; } else { hi = mid; }
    }
    let iv = (lo + hi) * 0.5;
    if iv.is_finite() { Some(iv) } else { None }
}

/// Box bounds for the 5 Heston parameters [κ, θ, σ, ρ, v₀].
const BOUNDS: [(f64, f64); 5] = [
    (0.01,  20.0),   // kappa
    (1e-6,   4.0),   // theta
    (0.01,   5.0),   // sigma
    (-0.98, -0.01),  // rho (equity: leverage ⇒ negative)
    (1e-6,   4.0),   // v0
];

/// Estimate ATM implied vol from the surface (closest-to-ATM option, shortest mat).
fn estimate_atm_iv(spot: f64, rate: f64, surface: &[MarketOption]) -> f64 {
    // Pick the option closest to ATM among the shortest maturity.
    let min_mat = surface.iter().map(|o| o.time_to_expiry)
        .fold(f64::INFINITY, f64::min);
    let atm_opt = surface.iter()
        .filter(|o| (o.time_to_expiry - min_mat).abs() < 1e-9)
        .min_by(|a, b| {
            (a.strike / spot - 1.0).abs()
                .partial_cmp(&(b.strike / spot - 1.0).abs())
                .unwrap_or(std::cmp::Ordering::Equal)
        });
    if let Some(opt) = atm_opt {
        let is_call = opt.option_type == OptionType::Call;
        bsm_iv_calib(opt.mid_price(), spot, opt.strike, opt.time_to_expiry, rate, is_call)
            .unwrap_or(0.30)
    } else {
        0.30
    }
}

/// IV-space MSE + regime-adaptive ε-insensitive Feller penalty + L2 reg + box penalty.
///
/// `lambda` controls the Feller penalty severity; `atm_iv` scales the penalty
/// to the current vol regime (stronger in crash, lighter in calm markets).
///
/// **Box constraints** use a quadratic penalty (not clamping) so CMA-ES gets
/// gradient information to return to the valid region instead of seeing a flat
/// plateau that confuses covariance matrix adaptation.
fn iv_objective(
    x: &[f64],
    spot: f64,
    rate: f64,
    surface: &[MarketOption],
    maturities: &[f64],
    lambda: f64,
    atm_iv: f64,
    rule: &GaussLaguerreRule,
) -> f64 {
    const BOX_PENALTY_SCALE: f64 = 500.0;

    // Quadratic penalty for out-of-bounds parameters.
    let mut box_penalty = 0.0_f64;
    for (i, &xi) in x.iter().enumerate().take(5) {
        let (lo, hi) = BOUNDS[i];
        if xi < lo { box_penalty += (lo - xi).powi(2) * BOX_PENALTY_SCALE; }
        if xi > hi { box_penalty += (xi - hi).powi(2) * BOX_PENALTY_SCALE; }
    }

    // Clamp for pricing (out-of-bounds candidates still get priced, but penalised).
    let kappa = x[0].clamp(BOUNDS[0].0, BOUNDS[0].1);
    let theta = x[1].clamp(BOUNDS[1].0, BOUNDS[1].1);
    let sigma = x[2].clamp(BOUNDS[2].0, BOUNDS[2].1);
    let rho   = x[3].clamp(BOUNDS[3].0, BOUNDS[3].1);
    let v0    = x[4].clamp(BOUNDS[4].0, BOUNDS[4].1);

    let params = CalibParams { kappa, theta, sigma, rho, v0 };

    // ── IV-space pricing error (batched by maturity via HestonCfCache) ────
    let mut iv_sum_sq = 0.0_f64;
    let mut iv_count  = 0_usize;

    for &mat in maturities {
        let h = params.to_heston(spot, rate, mat);
        let cache = HestonCfCache::new(spot, mat, rate, &h, rule);
        for opt in surface.iter().filter(|o| (o.time_to_expiry - mat).abs() < 1e-9) {
            let is_call = opt.option_type == OptionType::Call;
            let model_price = if is_call {
                cache.price_call(opt.strike)
            } else {
                cache.price_call(opt.strike) - spot + opt.strike * (-rate * mat).exp()
            };
            if let (Some(iv_mkt), Some(iv_mod)) = (
                bsm_iv_calib(opt.mid_price(), spot, opt.strike, mat, rate, is_call),
                bsm_iv_calib(model_price,     spot, opt.strike, mat, rate, is_call),
            ) {
                if iv_mkt > 1e-4 && iv_mod.is_finite() {
                    let diff = iv_mod - iv_mkt;
                    iv_sum_sq += diff * diff;
                    iv_count  += 1;
                }
            }
        }
    }

    let pricing_error = if iv_count > 0 {
        iv_sum_sq / iv_count as f64
    } else {
        return 1e10;
    };

    // ── Regime-adaptive ε-insensitive Feller penalty ──────────────────────
    // Stronger in crash (high ATM IV) where Feller violations are likelier.
    // ε = 0.02: no penalty when feller_ratio > ε; quadratic below.
    let feller_ratio = 2.0 * kappa * theta - sigma * sigma;
    let epsilon_feller = 0.02;
    let regime_scale = (atm_iv / 0.40).powi(2);  // >1 in crash, <1 in calm
    let lambda_eff = lambda * regime_scale;
    let feller_gap = (epsilon_feller - feller_ratio).max(0.0);
    let feller_penalty = lambda_eff * feller_gap * feller_gap;

    // ── L2 regularisation ────────────────────────────────────────────────
    let reg = 1e-4 * (rho * rho + sigma * sigma);

    pricing_error + feller_penalty + reg + box_penalty
}

/// Compute mean |IV_model − IV_market| for a given parameter set.
fn iv_mae(
    params: &CalibParams,
    spot: f64,
    rate: f64,
    surface: &[MarketOption],
    maturities: &[f64],
    rule: &GaussLaguerreRule,
) -> f64 {
    let mut sum   = 0.0_f64;
    let mut count = 0_usize;
    for &mat in maturities {
        let h = params.to_heston(spot, rate, mat);
        let cache = HestonCfCache::new(spot, mat, rate, &h, rule);
        for opt in surface.iter().filter(|o| (o.time_to_expiry - mat).abs() < 1e-9) {
            let is_call = opt.option_type == OptionType::Call;
            let model_price = if is_call {
                cache.price_call(opt.strike)
            } else {
                cache.price_call(opt.strike) - spot + opt.strike * (-rate * mat).exp()
            };
            if let (Some(iv_mkt), Some(iv_mod)) = (
                bsm_iv_calib(opt.mid_price(), spot, opt.strike, mat, rate, is_call),
                bsm_iv_calib(model_price,     spot, opt.strike, mat, rate, is_call),
            ) {
                if iv_mkt > 1e-4 && iv_mod.is_finite() {
                    sum += (iv_mod - iv_mkt).abs();
                    count += 1;
                }
            }
        }
    }
    if count > 0 { sum / count as f64 } else { f64::INFINITY }
}

/// Nelder-Mead polish: 20 iterations of IV-space simplex around a CMA-ES solution.
///
/// Builds a tiny simplex (1 % perturbation) around `x0` and runs NM with the
/// mildest Feller lambda.  Returns the best vertex.
fn nm_polish(
    x0: &[f64],
    spot: f64,
    rate: f64,
    surface: &[MarketOption],
    maturities: &[f64],
    atm_iv: f64,
    rule: &GaussLaguerreRule,
) -> Vec<f64> {
    const DIM: usize = 5;
    const ALPHA: f64 = 1.0;
    const GAMMA: f64 = 2.0;
    const RHO_NM: f64 = 0.5;
    const SIGMA_NM: f64 = 0.5;
    let lambda_polish = 5.0;

    let obj = |x: &[f64]| iv_objective(x, spot, rate, surface, maturities, lambda_polish, atm_iv, rule);

    // Build simplex: x0 + 1% perturbation along each axis
    let mut simplex: Vec<Vec<f64>> = vec![x0.to_vec()];
    for i in 0..DIM {
        let mut v = x0.to_vec();
        v[i] *= 1.01;
        simplex.push(v);
    }
    let mut costs: Vec<f64> = simplex.iter().map(|v| obj(v)).collect();

    for _ in 0..20 {
        let n1 = simplex.len();
        let mut order: Vec<usize> = (0..n1).collect();
        order.sort_unstable_by(|&a, &b| costs[a].partial_cmp(&costs[b]).unwrap_or(std::cmp::Ordering::Equal));
        let best  = order[0];
        let worst = order[n1 - 1];
        let sw    = order[n1 - 2];
        if costs[worst] - costs[best] < 1e-14 { break; }

        let mut c = vec![0.0_f64; DIM];
        for &i in &order[..n1 - 1] {
            for j in 0..DIM { c[j] += simplex[i][j]; }
        }
        for j in 0..DIM { c[j] /= (n1 - 1) as f64; }

        let xr: Vec<f64> = (0..DIM).map(|j| c[j] + ALPHA * (c[j] - simplex[worst][j])).collect();
        let fr = obj(&xr);
        if fr < costs[best] {
            let xe: Vec<f64> = (0..DIM).map(|j| c[j] + GAMMA * (xr[j] - c[j])).collect();
            let fe = obj(&xe);
            if fe < fr { simplex[worst] = xe; costs[worst] = fe; }
            else        { simplex[worst] = xr; costs[worst] = fr; }
        } else if fr < costs[sw] {
            simplex[worst] = xr; costs[worst] = fr;
        } else {
            let xc: Vec<f64> = if fr < costs[worst] {
                (0..DIM).map(|j| c[j] + RHO_NM * (xr[j] - c[j])).collect()
            } else {
                (0..DIM).map(|j| c[j] + RHO_NM * (simplex[worst][j] - c[j])).collect()
            };
            let fc = obj(&xc);
            if fc < costs[worst] {
                simplex[worst] = xc; costs[worst] = fc;
            } else {
                let best_v = simplex[best].clone();
                for i in 1..n1 {
                    let idx = order[i];
                    simplex[idx] = (0..DIM)
                        .map(|j| best_v[j] + SIGMA_NM * (simplex[idx][j] - best_v[j]))
                        .collect();
                    costs[idx] = obj(&simplex[idx]);
                }
            }
        }
    }

    let best_idx = costs.iter().enumerate()
        .min_by(|a, b| a.1.partial_cmp(b.1).unwrap_or(std::cmp::Ordering::Equal))
        .map(|(i, _)| i).unwrap_or(0);
    simplex[best_idx].clone()
}

/// Calibrate Heston parameters using multi-restart CMA-ES + NM polish.
///
/// 1. Estimate ATM IV from the surface → regime-adaptive Feller penalty.
/// 2. **15 restarts** of CMA-ES with 3-way σ₀ rotation (0.4 / 0.15 / 0.25)
///    and ε-insensitive Feller penalty scaled by `(ATM_IV / 0.40)²`.
/// 3. Best admissible (`feller_ratio > −0.02` and lowest MAE) wins.
/// 4. **NM polish** (20 iterations) refines the winning CMA-ES solution.
pub fn calibrate_heston(
    spot: f64,
    rate: f64,
    market_data: Vec<MarketOption>,
    initial_guess: CalibParams,
) -> Result<CalibrationResult, String> {
    if market_data.is_empty() {
        return Err("No market data provided for calibration".to_string());
    }

    println!("Starting Heston calibration (CMA-ES) with {} options...", market_data.len());

    // Pre-compute unique maturities once.
    let mut maturities: Vec<f64> = Vec::new();
    for opt in &market_data {
        if !maturities.iter().any(|&m| (m - opt.time_to_expiry).abs() < 1e-9) {
            maturities.push(opt.time_to_expiry);
        }
    }

    let rule = GaussLaguerreRule::new(32);
    let atm_iv = estimate_atm_iv(spot, rate, &market_data);

    let initial_error = {
        let x = [initial_guess.kappa, initial_guess.theta, initial_guess.sigma,
                  initial_guess.rho, initial_guess.v0];
        iv_objective(&x, spot, rate, &market_data, &maturities, 25.0, atm_iv, &rule)
    };

    // ── Restart seeds: initial_guess + 14 multiplicative perturbations ──
    let seeds: Vec<Vec<f64>> = {
        let ig = vec![initial_guess.kappa, initial_guess.theta, initial_guess.sigma,
                      initial_guess.rho, initial_guess.v0];
        let mut s = vec![ig.clone()];
        // Multiplicative factors for [kappa, theta, sigma, rho_additive, v0].
        // rho uses additive offsets since it spans negative values.
        let perturbations: Vec<[f64; 5]> = vec![
            // ── Diverse starting grid (crash, low-vol, moderate) ──────
            [ 0.5,  0.8,  1.5,  -0.10, 0.8  ],
            [ 2.0,  1.2,  0.6,   0.15, 1.3  ],
            [ 1.5,  0.5,  1.2,  -0.20, 0.6  ],
            [ 0.7,  2.0,  0.8,   0.10, 1.5  ],
            [ 1.3,  0.7,  2.0,  -0.05, 0.9  ],
            [ 0.8,  1.5,  0.4,   0.20, 1.1  ],
            [ 3.0,  0.6,  1.0,  -0.15, 0.5  ],
            [ 0.4,  1.0,  1.8,   0.05, 2.0  ],
            [ 1.0,  0.3,  1.3,  -0.25, 0.7  ],
            [ 1.2,  1.8,  0.7,   0.00, 1.2  ],
            // ── Additional seeds for edge cases ──────────────────────
            [ 0.6,  0.4,  0.5,  -0.05, 1.8  ],
            [ 2.5,  1.3,  1.6,  -0.10, 0.4  ],
            [ 1.0,  1.0,  0.3,   0.25, 1.0  ],
            [ 0.3,  0.9,  2.5,  -0.20, 1.4  ],
        ];
        for p in &perturbations {
            let perturbed = vec![
                (ig[0] * p[0]).clamp(0.05, 18.0),
                (ig[1] * p[1]).clamp(1e-4, 3.5),
                (ig[2] * p[2]).clamp(0.02, 4.5),
                (ig[3] + p[3]).clamp(-0.96, -0.01),
                (ig[4] * p[4]).clamp(1e-4, 3.5),
            ];
            s.push(perturbed);
        }
        s
    };

    // Three sigma0 values rotated across restarts for covariance diversity.
    let sigma0_schedule = [0.4, 0.15, 0.25];

    // ── Multi-restart CMA-ES (≥12 guaranteed, actually 15) ────────────────
    let mut best_params: Option<CalibParams> = None;
    let mut best_mae = f64::INFINITY;
    let mut best_obj = f64::INFINITY;
    let mut best_feller_ratio = f64::NEG_INFINITY;
    let mut total_fevals = 0_usize;

    for (restart_i, seed) in seeds.iter().enumerate() {
        let s0 = sigma0_schedule[restart_i % sigma0_schedule.len()];

        let config = CmaesConfig {
            lambda:     30,
            max_fevals: 15_000,
            sigma0:     s0,
            ftol:       1e-10,
            xtol:       1e-10,
        };

        let mats = &maturities;
        let md   = &market_data;
        let rl   = &rule;
        let aiv  = atm_iv;

        // Phase 1: strong Feller penalty
        let objective_strong = |x: &[f64]| -> f64 {
            iv_objective(x, spot, rate, md, mats, 50.0, aiv, rl)
        };
        let result1 = Cmaes::new(CmaesConfig {
            max_fevals: 8_000,
            ..config.clone()
        }).minimize(objective_strong, seed.clone());

        // Phase 2: mild Feller penalty for refinement
        let objective_mild = |x: &[f64]| -> f64 {
            iv_objective(x, spot, rate, md, mats, 5.0, aiv, rl)
        };
        let result2 = Cmaes::new(CmaesConfig {
            max_fevals: 7_000,
            sigma0: s0 * 0.2,
            ..config.clone()
        }).minimize(objective_mild, result1.best_params);

        total_fevals += result1.fevals + result2.fevals;

        let candidate = CalibParams {
            kappa: result2.best_params[0].clamp(BOUNDS[0].0, BOUNDS[0].1),
            theta: result2.best_params[1].clamp(BOUNDS[1].0, BOUNDS[1].1),
            sigma: result2.best_params[2].clamp(BOUNDS[2].0, BOUNDS[2].1),
            rho:   result2.best_params[3].clamp(BOUNDS[3].0, BOUNDS[3].1),
            v0:    result2.best_params[4].clamp(BOUNDS[4].0, BOUNDS[4].1),
        };

        let mae = iv_mae(&candidate, spot, rate, &market_data, &maturities, &rule);
        let fr = 2.0 * candidate.kappa * candidate.theta - candidate.sigma.powi(2);
        let feller_admissible = fr > -0.02;

        // Selection: among feller-admissible (fr > -0.02) pick lowest MAE.
        // Accept feller-violated only if it has <50% the best MAE.
        let dominated = if let Some(_) = &best_params {
            let prev_admissible = best_feller_ratio > -0.02;
            if feller_admissible && !prev_admissible {
                mae > best_mae * 2.0
            } else if !feller_admissible && prev_admissible {
                mae >= best_mae * 0.5
            } else {
                mae >= best_mae
            }
        } else {
            false
        };

        if !dominated {
            best_params = Some(candidate);
            best_mae = mae;
            best_obj = result2.best_value;
            best_feller_ratio = fr;
        }
    }

    // ── NM polish (20 iterations) on the best CMA-ES solution ─────────────
    let cmaes_best = best_params.unwrap_or(params_from_slice(&seeds[0]));
    let x_cmaes = vec![cmaes_best.kappa, cmaes_best.theta, cmaes_best.sigma,
                       cmaes_best.rho, cmaes_best.v0];
    let x_polished = nm_polish(&x_cmaes, spot, rate, &market_data, &maturities, atm_iv, &rule);

    let polished = CalibParams {
        kappa: x_polished[0].clamp(BOUNDS[0].0, BOUNDS[0].1),
        theta: x_polished[1].clamp(BOUNDS[1].0, BOUNDS[1].1),
        sigma: x_polished[2].clamp(BOUNDS[2].0, BOUNDS[2].1),
        rho:   x_polished[3].clamp(BOUNDS[3].0, BOUNDS[3].1),
        v0:    x_polished[4].clamp(BOUNDS[4].0, BOUNDS[4].1),
    };
    let polished_mae = iv_mae(&polished, spot, rate, &market_data, &maturities, &rule);

    // Keep polished only if it actually improved.
    let (final_params, final_mae) = if polished_mae < best_mae {
        (polished, polished_mae)
    } else {
        (cmaes_best, best_mae)
    };

    let success = final_mae < 0.005; // 0.5% MAE threshold

    Ok(CalibrationResult {
        params:        final_params,
        rmse:          best_obj,
        iterations:    total_fevals as u64,
        success,
        initial_error,
        final_error:   best_obj,
    })
}

/// Helper: build a CalibParams from a raw parameter slice [κ, θ, σ, ρ, v₀].
#[inline]
fn params_from_slice(x: &[f64]) -> CalibParams {
    CalibParams { kappa: x[0], theta: x[1], sigma: x[2], rho: x[3], v0: x[4] }
}

/// Create synthetic market data for testing
pub fn create_mock_market_data(
    spot: f64,
    rate: f64,
    true_params: &CalibParams,
    strikes: &[f64],
    maturities: &[f64],
) -> Vec<MarketOption> {
    let rule = GaussLaguerreRule::new(32);
    let mut market_data = Vec::new();
    
    for &maturity in maturities {
        let heston = true_params.to_heston(spot, rate, maturity);
        let cache = HestonCfCache::new(spot, maturity, rate, &heston, &rule);
        for &strike in strikes {
            let true_price = cache.price_call(strike);
            
            let spread = true_price * 0.03;
            let bid = true_price - spread / 2.0;
            let ask = true_price + spread / 2.0;
            
            market_data.push(MarketOption {
                strike,
                time_to_expiry: maturity,
                bid,
                ask,
                option_type: OptionType::Call,
                volume: 100,
                open_interest: 500,
            });
        }
    }
    
    market_data
}

