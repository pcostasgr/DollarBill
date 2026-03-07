//! Pure Rust Gauss-Laguerre quadrature implementation.
//!
//! Computes nodes and weights for n-point Gauss-Laguerre quadrature,
//! which approximates integrals of the form:
//!
//!   ∫₀^∞ e^{-x} f(x) dx ≈ Σᵢ wᵢ · f(xᵢ)
//!
//! Also provides exponential-modified weights w̃ᵢ = wᵢ·exp(xᵢ) for
//! general semi-infinite integrals without the e^{-x} weight:
//!
//!   ∫₀^∞ g(x) dx ≈ Σᵢ w̃ᵢ · g(xᵢ)
//!
//! **No external dependencies** — uses Newton's method to find roots of
//! Laguerre polynomials and computes weights from the analytical formula:
//!
//!   wᵢ = xᵢ / (n · L_{n-1}(xᵢ))²

const MAX_NEWTON_ITERATIONS: usize = 200;
const NEWTON_TOLERANCE: f64 = 1e-15;

/// Precomputed Gauss-Laguerre quadrature rule.
///
/// # Supported node counts
///
/// Between 2 and 128 nodes. Common choices for Heston model pricing:
/// - **32 nodes**: fast, good accuracy for typical parameters
/// - **48 nodes**: balanced speed/accuracy
/// - **64 nodes**: high accuracy, recommended for calibration
///
/// # Example
///
/// ```
/// use dollarbill::models::gauss_laguerre::GaussLaguerreRule;
///
/// let rule = GaussLaguerreRule::new(32);
///
/// // ∫₀^∞ e^{-x} dx = 1
/// let result = rule.integrate_weighted(|_x| 1.0);
/// assert!((result - 1.0).abs() < 1e-12);
///
/// // ∫₀^∞ x² · e^{-x} dx = Γ(3) = 2
/// let result = rule.integrate_weighted(|x| x * x);
/// assert!((result - 2.0).abs() < 1e-10);
/// ```
#[derive(Debug, Clone)]
pub struct GaussLaguerreRule {
    /// Quadrature nodes (roots of Laguerre polynomial Lₙ).
    pub nodes: Vec<f64>,
    /// Standard Gauss-Laguerre weights (for ∫₀^∞ e^{-x} f(x) dx).
    pub weights: Vec<f64>,
    /// Exponential-modified weights wᵢ·exp(xᵢ) (for ∫₀^∞ f(x) dx).
    pub exp_weights: Vec<f64>,
    /// Number of quadrature nodes.
    pub n: usize,
}

impl GaussLaguerreRule {
    /// Compute an n-point Gauss-Laguerre quadrature rule.
    ///
    /// Uses Newton's method to find roots of the Laguerre polynomial Lₙ(x)
    /// with initial approximations from Stroud & Secrest, then computes
    /// weights via the analytical formula wᵢ = xᵢ / (n · L_{n-1}(xᵢ))².
    ///
    /// # Arguments
    /// * `n` - Number of quadrature nodes (2..=128)
    ///
    /// # Panics
    /// Panics if `n < 2` or `n > 128`.
    pub fn new(n: usize) -> Self {
        assert!(n >= 2, "Gauss-Laguerre requires at least 2 nodes");
        assert!(n <= 128, "Maximum supported node count is 128");

        let mut nodes = vec![0.0_f64; n];
        let mut weights = vec![0.0_f64; n];
        let mut exp_weights = vec![0.0_f64; n];

        for i in 0..n {
            // Initial approximation for the i-th root of Lₙ(x)
            // Asymptotic approximations from Stroud & Secrest / Numerical Recipes
            let z0 = if i == 0 {
                3.0 / (1.0 + 2.4 * n as f64)
            } else if i == 1 {
                nodes[0] + 15.0 / (1.0 + 2.5 * n as f64)
            } else {
                // Extrapolation from the previous two roots
                let ratio = (1.0 + 2.55 * (i as f64 - 1.0)) / (1.9 * (i as f64 - 1.0));
                nodes[i - 1] + ratio * (nodes[i - 1] - nodes[i - 2])
            };

            let mut z = z0;

            // Newton's method: refine root of Lₙ(x) = 0
            for _ in 0..MAX_NEWTON_ITERATIONS {
                let (p_n, p_nm1) = eval_laguerre(n, z);

                // Derivative: Lₙ'(x) = n · (Lₙ(x) - L_{n-1}(x)) / x
                let dp = if z.abs() > 1e-30 {
                    n as f64 * (p_n - p_nm1) / z
                } else {
                    // Lₙ'(0) = -n  (from the series representation)
                    -(n as f64)
                };

                if dp.abs() < 1e-30 {
                    break;
                }

                let delta = p_n / dp;
                z -= delta;

                // All Laguerre roots are strictly positive
                if z < 1e-30 {
                    z = z0 * 0.5;
                }

                if delta.abs() < NEWTON_TOLERANCE * z.abs().max(1.0) {
                    break;
                }
            }

            nodes[i] = z;

            // Weight: wᵢ = xᵢ / (n · L_{n-1}(xᵢ))²
            let (_, p_nm1) = eval_laguerre(n, z);
            let denom = (n as f64 * p_nm1).powi(2);

            weights[i] = if denom.abs() > 1e-300 {
                z / denom
            } else {
                0.0
            };

            // Exponential weight: w̃ᵢ = wᵢ · exp(xᵢ)
            // For large xᵢ exp(xᵢ) can be huge, but wᵢ is correspondingly
            // tiny so the product stays representable in f64.
            exp_weights[i] = if z < 700.0 {
                weights[i] * z.exp()
            } else {
                // Fall back to log-space to avoid overflow
                let log_w = z.ln() - 2.0 * ((n as f64) * p_nm1.abs()).ln();
                (log_w + z).exp()
            };
        }

        GaussLaguerreRule {
            nodes,
            weights,
            exp_weights,
            n,
        }
    }

    /// Integrate ∫₀^∞ f(x) dx using exponential-modified weights.
    ///
    /// This is the method to use for general semi-infinite integrals
    /// (e.g. Heston characteristic function Fourier inversion).
    ///
    /// Approximation: ∫₀^∞ f(x) dx ≈ Σᵢ w̃ᵢ · f(xᵢ)
    pub fn integrate<F: Fn(f64) -> f64>(&self, f: F) -> f64 {
        self.nodes
            .iter()
            .zip(self.exp_weights.iter())
            .map(|(&x, &w)| {
                let val = f(x);
                if val.is_finite() {
                    w * val
                } else {
                    0.0
                }
            })
            .sum()
    }

    /// Integrate ∫₀^∞ e^{-x} f(x) dx using standard weights.
    ///
    /// Use this when the integrand already contains the e^{-x} decay.
    pub fn integrate_weighted<F: Fn(f64) -> f64>(&self, f: F) -> f64 {
        self.nodes
            .iter()
            .zip(self.weights.iter())
            .map(|(&x, &w)| {
                let val = f(x);
                if val.is_finite() {
                    w * val
                } else {
                    0.0
                }
            })
            .sum()
    }

    /// Return the number of nodes.
    pub fn node_count(&self) -> usize {
        self.n
    }
}

// ───────────────────────────────────────────────────────────────────────────
// Laguerre polynomial helpers
// ───────────────────────────────────────────────────────────────────────────

/// Evaluate Laguerre polynomial Lₙ(x) **and** L_{n-1}(x) via the
/// three-term recurrence:
///
///   (k+1) · L_{k+1}(x) = (2k+1 − x) · L_k(x) − k · L_{k-1}(x)
///
/// Returns `(L_n(x), L_{n-1}(x))`.
fn eval_laguerre(n: usize, x: f64) -> (f64, f64) {
    if n == 0 {
        return (1.0, 0.0);
    }

    let mut p_prev = 1.0; // L₀(x) = 1
    let mut p_curr = 1.0 - x; // L₁(x) = 1 − x

    if n == 1 {
        return (p_curr, p_prev);
    }

    for k in 1..n {
        let kf = k as f64;
        let p_next = ((2.0 * kf + 1.0 - x) * p_curr - kf * p_prev) / (kf + 1.0);
        p_prev = p_curr;
        p_curr = p_next;
    }

    (p_curr, p_prev)
}

// ───────────────────────────────────────────────────────────────────────────
// Tests
// ───────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // ── Node / weight sanity ──────────────────────────────────────────

    #[test]
    fn test_nodes_positive_and_sorted() {
        for &n in &[2, 4, 8, 16, 32, 48, 64] {
            let rule = GaussLaguerreRule::new(n);
            assert_eq!(rule.nodes.len(), n);

            for i in 0..n {
                assert!(
                    rule.nodes[i] > 0.0,
                    "n={n}: node[{i}] should be positive, got {}",
                    rule.nodes[i]
                );
                if i > 0 {
                    assert!(
                        rule.nodes[i] > rule.nodes[i - 1],
                        "n={n}: nodes should be strictly increasing"
                    );
                }
            }
        }
    }

    #[test]
    fn test_weights_positive() {
        for &n in &[4, 16, 32, 64] {
            let rule = GaussLaguerreRule::new(n);
            for i in 0..n {
                assert!(rule.weights[i] > 0.0, "n={n}: weight[{i}] should be positive");
                assert!(
                    rule.exp_weights[i] > 0.0,
                    "n={n}: exp_weight[{i}] should be positive"
                );
            }
        }
    }

    #[test]
    fn test_weights_sum_to_one() {
        // ∫₀^∞ e^{-x} · 1 dx = 1, so Σ wᵢ = 1 exactly for all n ≥ 1
        for &n in &[4, 8, 16, 32, 64] {
            let rule = GaussLaguerreRule::new(n);
            let sum: f64 = rule.weights.iter().sum();
            assert!(
                (sum - 1.0).abs() < 1e-10,
                "n={n}: weights should sum to 1.0, got {sum}"
            );
        }
    }

    // ── Exact integration tests (polynomials) ─────────────────────────

    #[test]
    fn test_integrate_constant() {
        // ∫₀^∞ e^{-x} · 1 dx = 1
        let rule = GaussLaguerreRule::new(32);
        let result = rule.integrate_weighted(|_| 1.0);
        assert!(
            (result - 1.0).abs() < 1e-12,
            "∫ e^(-x) dx = 1, got {result}"
        );
    }

    #[test]
    fn test_integrate_x() {
        // ∫₀^∞ x · e^{-x} dx = Γ(2) = 1
        let rule = GaussLaguerreRule::new(32);
        let result = rule.integrate_weighted(|x| x);
        assert!(
            (result - 1.0).abs() < 1e-12,
            "∫ x·e^(-x) dx = 1, got {result}"
        );
    }

    #[test]
    fn test_integrate_x_squared() {
        // ∫₀^∞ x² · e^{-x} dx = Γ(3) = 2
        let rule = GaussLaguerreRule::new(32);
        let result = rule.integrate_weighted(|x| x * x);
        assert!(
            (result - 2.0).abs() < 1e-10,
            "∫ x²·e^(-x) dx = 2, got {result}"
        );
    }

    #[test]
    fn test_integrate_x5() {
        // ∫₀^∞ x⁵ · e^{-x} dx = Γ(6) = 120
        let rule = GaussLaguerreRule::new(32);
        let result = rule.integrate_weighted(|x| x.powi(5));
        assert!(
            (result - 120.0).abs() < 1e-6,
            "∫ x⁵·e^(-x) dx = 120, got {result}"
        );
    }

    // ── General (exponential-weight) integration ──────────────────────

    #[test]
    fn test_integrate_exp_decay() {
        // ∫₀^∞ e^{-x} dx = 1  (via exp_weights)
        let rule = GaussLaguerreRule::new(32);
        let result = rule.integrate(|x| (-x).exp());
        assert!(
            (result - 1.0).abs() < 1e-6,
            "∫ e^(-x) dx via exp_weights = 1, got {result}"
        );
    }

    #[test]
    fn test_integrate_exp_2x() {
        // ∫₀^∞ e^{-2x} dx = 0.5
        let rule = GaussLaguerreRule::new(64);
        let result = rule.integrate(|x| (-2.0 * x).exp());
        assert!(
            (result - 0.5).abs() < 1e-4,
            "∫ e^(-2x) dx = 0.5, got {result}"
        );
    }

    // ── Accuracy: 32 vs 64 nodes ─────────────────────────────────────

    #[test]
    fn test_64_at_least_as_accurate_as_32() {
        let rule32 = GaussLaguerreRule::new(32);
        let rule64 = GaussLaguerreRule::new(64);

        // ∫₀^∞ x⁵ · e^{-x} dx = 120
        let err32 = (rule32.integrate_weighted(|x| x.powi(5)) - 120.0).abs();
        let err64 = (rule64.integrate_weighted(|x| x.powi(5)) - 120.0).abs();

        assert!(
            err64 <= err32 + 1e-10,
            "64 nodes should be at least as accurate as 32"
        );
    }

    // ── Laguerre polynomial evaluation ────────────────────────────────

    #[test]
    fn test_laguerre_l0() {
        let (val, _) = eval_laguerre(0, 3.0);
        assert!((val - 1.0).abs() < 1e-15);
    }

    #[test]
    fn test_laguerre_l1() {
        // L₁(x) = 1 - x
        let (val, prev) = eval_laguerre(1, 3.0);
        assert!((val - (-2.0)).abs() < 1e-15);
        assert!((prev - 1.0).abs() < 1e-15);
    }

    #[test]
    fn test_laguerre_l2() {
        // L₂(x) = (x² - 4x + 2) / 2
        let x = 3.0;
        let expected = (x * x - 4.0 * x + 2.0) / 2.0;
        let (val, _) = eval_laguerre(2, x);
        assert!(
            (val - expected).abs() < 1e-14,
            "L₂({x}) = {expected}, got {val}"
        );
    }

    #[test]
    fn test_laguerre_at_zero() {
        // Lₙ(0) = 1 for all n
        for n in 0..10 {
            let (val, _) = eval_laguerre(n, 0.0);
            assert!(
                (val - 1.0).abs() < 1e-14,
                "L_{n}(0) should be 1, got {val}"
            );
        }
    }
}
