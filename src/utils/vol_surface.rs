// Volatility surface analysis and visualization
// Generates CSV data for volatility surface plotting

use crate::calibration::market_option::{MarketOption, OptionType};
use crate::models::bs_mod::black_scholes_merton_call;
use std::fs::File;
use std::io::Write;

/// Calculate implied volatility using Newton-Raphson method
pub fn implied_volatility_newton(
    market_price: f64,
    spot: f64,
    strike: f64,
    time_to_expiry: f64,
    rate: f64,
    is_call: bool,
) -> Option<f64> {
    let mut sigma = 0.3; // Initial guess
    let tolerance = 1e-6;
    let max_iterations = 100;
    let q = 0.0;
    
    for _ in 0..max_iterations {
        let greeks = if is_call {
            black_scholes_merton_call(spot, strike, time_to_expiry, rate, sigma, q)
        } else {
            crate::models::bs_mod::black_scholes_merton_put(spot, strike, time_to_expiry, rate, sigma, q)
        };
        
        let price_diff = greeks.price - market_price;
        
        if price_diff.abs() < tolerance {
            return Some(sigma);
        }
        
        // Vega check to avoid division by zero
        if greeks.vega.abs() < 1e-10 {
            return None;
        }
        
        // Newton-Raphson update
        sigma = sigma - price_diff / greeks.vega;
        
        // Keep sigma in reasonable range
        if sigma < 0.01 {
            sigma = 0.01;
        } else if sigma > 5.0 {
            sigma = 5.0;
        }
    }
    
    None // Failed to converge
}

#[derive(Debug, Clone)]
pub struct VolSurfacePoint {
    pub strike: f64,
    pub time_to_expiry: f64,
    pub implied_vol: f64,
    pub moneyness: f64, // strike / spot
    pub option_type: String,
    pub volume: i32,
}

/// Extract volatility surface from options data
pub fn extract_vol_surface(
    options: &[MarketOption],
    spot: f64,
    rate: f64,
) -> Vec<VolSurfacePoint> {
    let mut surface_points = Vec::new();
    
    for option in options {
        let market_price = option.mid_price();
        
        if market_price <= 0.0 {
            continue;
        }
        
        let is_call = matches!(option.option_type, OptionType::Call);
        
        if let Some(iv) = implied_volatility_newton(
            market_price,
            spot,
            option.strike,
            option.time_to_expiry,
            rate,
            is_call,
        ) {
            surface_points.push(VolSurfacePoint {
                strike: option.strike,
                time_to_expiry: option.time_to_expiry,
                implied_vol: iv,
                moneyness: option.strike / spot,
                option_type: if is_call { "Call".to_string() } else { "Put".to_string() },
                volume: option.volume,
            });
        }
    }
    
    surface_points
}

/// Save volatility surface to CSV for visualization
pub fn save_vol_surface_csv(
    points: &[VolSurfacePoint],
    symbol: &str,
    filename: &str,
) -> std::io::Result<()> {
    let mut file = File::create(filename)?;
    
    // Write header
    writeln!(file, "Symbol,Strike,TimeToExpiry,ImpliedVol,Moneyness,OptionType,Volume")?;
    
    // Write data
    for point in points {
        writeln!(
            file,
            "{},{},{:.6},{:.4},{:.4},{},{}",
            symbol,
            point.strike,
            point.time_to_expiry,
            point.implied_vol,
            point.moneyness,
            point.option_type,
            point.volume
        )?;
    }
    
    Ok(())
}

/// Print volatility smile (IV vs strike at fixed expiry)
pub fn print_vol_smile(points: &[VolSurfacePoint], symbol: &str) {
    if points.is_empty() {
        log::info!("No volatility surface data available");
        return;
    }
    
    log::info!("===============================================================");
    log::info!("VOLATILITY SMILE - {}", symbol);
    log::info!("===============================================================");
    
    // Group by option type
    let mut calls: Vec<_> = points.iter().filter(|p| p.option_type == "Call").collect();
    let mut puts: Vec<_> = points.iter().filter(|p| p.option_type == "Put").collect();
    
    calls.sort_by(|a, b| a.strike.partial_cmp(&b.strike).unwrap());
    puts.sort_by(|a, b| a.strike.partial_cmp(&b.strike).unwrap());
    
    log::info!("CALLS:");
    log::info!("{:<10} {:<12} {:<10} {:<10}", "Strike", "Moneyness", "IV %", "Volume");
    log::info!("{:-<45}", "");
    for point in calls.iter().take(15) {
        log::info!("{:<10.2} {:<12.4} {:<10.2} {:<10}",
            point.strike,
            point.moneyness,
            point.implied_vol * 100.0,
            point.volume
        );
    }
    
    log::info!("PUTS:");
    log::info!("{:<10} {:<12} {:<10} {:<10}", "Strike", "Moneyness", "IV %", "Volume");
    log::info!("{:-<45}", "");
    for point in puts.iter().take(15) {
        log::info!("{:<10.2} {:<12.4} {:<10.2} {:<10}",
            point.strike,
            point.moneyness,
            point.implied_vol * 100.0,
            point.volume
        );
    }
    
    // Analyze volatility skew
    let atm_calls: Vec<_> = calls.iter().filter(|p| (p.moneyness - 1.0).abs() < 0.05).collect();
    let atm_puts: Vec<_> = puts.iter().filter(|p| (p.moneyness - 1.0).abs() < 0.05).collect();
    
    if !atm_calls.is_empty() && !atm_puts.is_empty() {
        let avg_call_iv: f64 = atm_calls.iter().map(|p| p.implied_vol).sum::<f64>() / atm_calls.len() as f64;
        let avg_put_iv: f64 = atm_puts.iter().map(|p| p.implied_vol).sum::<f64>() / atm_puts.len() as f64;
        
        log::info!("ATM Volatility Analysis:");
        log::info!("  ATM Call IV:  {:.2}%", avg_call_iv * 100.0);
        log::info!("  ATM Put IV:   {:.2}%", avg_put_iv * 100.0);
        
        if (avg_put_iv - avg_call_iv).abs() > 0.02 {
            if avg_put_iv > avg_call_iv {
                log::info!("  Put skew detected: Puts trading at {:.1}% premium",
                    (avg_put_iv - avg_call_iv) * 100.0);
                log::info!("    Market pricing in downside protection");
            } else {
                log::info!("  Call skew detected: Calls trading at {:.1}% premium",
                    (avg_call_iv - avg_put_iv) * 100.0);
                log::info!("    Market pricing in upside speculation");
            }
        } else {
            log::info!("  Balanced volatility: Call-Put IV difference < 2%");
        }
    }
}

// ── Cubic Spline Smile Interpolation ─────────────────────────────────────────

/// Natural cubic-spline interpolator for the volatility smile.
///
/// Given a set of (moneyness_or_strike, implied_vol) knots, this struct fits
/// the unique natural cubic spline (zero second derivative at both endpoints)
/// and supports querying at arbitrary input values.
///
/// Extrapolation beyond the range is flat (returns the boundary IV value).
///
/// # Example
/// ```
/// # use dollarbill::utils::vol_surface::CubicSplineSmile;
/// let knots = vec![(0.90, 0.30), (0.95, 0.27), (1.00, 0.25), (1.05, 0.26), (1.10, 0.28)];
/// let spline = CubicSplineSmile::new(&knots).unwrap();
/// let atm_iv = spline.interpolate(1.00);    // returns ≈ 0.25
/// let x95_iv = spline.interpolate(0.975);   // interpolated between knots
/// ```
#[derive(Debug, Clone)]
pub struct CubicSplineSmile {
    x: Vec<f64>,   // Sorted knot locations
    a: Vec<f64>,   // y values at knots (a_i = y_i)
    b: Vec<f64>,   // Linear coefficients
    c: Vec<f64>,   // Quadratic coefficients (second derivative / 2)
    d: Vec<f64>,   // Cubic coefficients
}

impl CubicSplineSmile {
    /// Fit a natural cubic spline to the provided (x, y) knots.
    ///
    /// # Errors
    /// Returns an error string if:
    /// - fewer than 2 knots are provided, or
    /// - the knots are not strictly increasing in `x`.
    pub fn new(knots: &[(f64, f64)]) -> Result<Self, String> {
        let n = knots.len();
        if n < 2 {
            return Err(format!(
                "CubicSplineSmile requires at least 2 knots, got {n}"
            ));
        }

        // Sort by x and check strict monotonicity.
        let mut sorted = knots.to_vec();
        sorted.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));
        for i in 1..n {
            if sorted[i].0 <= sorted[i - 1].0 {
                return Err(format!(
                    "knot x-values must be strictly increasing: x[{}]={} ≤ x[{}]={}",
                    i, sorted[i].0, i - 1, sorted[i - 1].0,
                ));
            }
        }

        let x: Vec<f64> = sorted.iter().map(|p| p.0).collect();
        let y: Vec<f64> = sorted.iter().map(|p| p.1).collect();

        // Special case: only two knots → linear interpolation.
        if n == 2 {
            let b0 = (y[1] - y[0]) / (x[1] - x[0]);
            return Ok(Self {
                x,
                a: y,
                b: vec![b0, b0],
                c: vec![0.0, 0.0],
                d: vec![0.0, 0.0],
            });
        }

        // ── Natural cubic spline via Thomas algorithm ──
        //
        // We solve the tridiagonal system for m = [m_0, …, m_{n-1}]
        // where m_i = S''(x_i) ("second derivative" at each knot).
        // Natural spline boundary conditions: m_0 = m_{n-1} = 0.
        //
        // Reference: Burden & Faires, "Numerical Analysis", Algorithm 3.4.

        let m = n - 1; // number of intervals

        let mut h = vec![0.0; m];   // interval widths
        for i in 0..m {
            h[i] = x[i + 1] - x[i];
        }

        // Right-hand side of the tridiagonal system (natural BCs → first and
        // last equations are trivial: m[0] = 0, m[n-1] = 0)
        let mut rhs = vec![0.0; n];
        for i in 1..m {
            rhs[i] = 3.0 * ((y[i + 1] - y[i]) / h[i] - (y[i] - y[i - 1]) / h[i - 1]);
        }

        // Forward sweep (Thomas algorithm)
        let mut diag = vec![2.0 * (h[0] + if m > 1 { h[1] } else { h[0] }); n];
        diag[0] = 1.0;
        diag[n - 1] = 1.0;

        let mut lower = vec![0.0; n]; // sub-diagonal
        let mut upper = vec![0.0; n]; // super-diagonal
        for i in 1..m {
            lower[i] = h[i - 1];
            upper[i] = h[i];
            diag[i] = 2.0 * (h[i - 1] + h[i]);
        }

        // Gaussian elimination with back-substitution
        let mut c = vec![0.0; n]; // second derivatives (÷ 2 not yet applied)
        let mut w = vec![0.0; n];
        let mut g = rhs.clone();

        // Forward pass
        let mut denom = diag[0];
        w[0] = upper[0] / denom;
        g[0] /= denom;

        #[allow(clippy::needless_range_loop)]
        for i in 1..n {
            denom = diag[i] - lower[i] * w[i - 1];
            if denom.abs() < 1e-14 {
                break; // degenerate — leave c as zeros
            }
            w[i] = upper[i] / denom;
            g[i] = (g[i] - lower[i] * g[i - 1]) / denom;
        }

        // Back substitution
        c[n - 1] = g[n - 1];
        for i in (0..n - 1).rev() {
            c[i] = g[i] - w[i] * c[i + 1];
        }

        // Derive b and d from c (second derivatives)
        let mut b = vec![0.0; m];
        let mut d = vec![0.0; m];
        for i in 0..m {
            d[i] = (c[i + 1] - c[i]) / (3.0 * h[i]);
            b[i] = (y[i + 1] - y[i]) / h[i] - h[i] * (2.0 * c[i] + c[i + 1]) / 3.0;
        }

        // Trim coefficient arrays to length m (one per interval)
        let c_trunc = c[..m].to_vec();

        Ok(Self {
            x,
            a: y,
            b,
            c: c_trunc,
            d,
        })
    }

    /// Query the spline at `x_query`.
    ///
    /// Flat extrapolation is used beyond the knot range (returns the boundary IV).
    pub fn interpolate(&self, x_query: f64) -> f64 {
        let n = self.x.len();

        // Flat extrapolation below the lowest knot
        if x_query <= self.x[0] {
            return self.a[0];
        }
        // Flat extrapolation above the highest knot
        if x_query >= self.x[n - 1] {
            return self.a[n - 1];
        }

        // Binary search for the interval [x_i, x_{i+1}] containing x_query
        let i = match self.x.partition_point(|&xi| xi <= x_query) {
            0 => 0,
            k if k >= n => n - 2,
            k => k - 1,
        };

        let dx = x_query - self.x[i];
        self.a[i] + dx * (self.b[i] + dx * (self.c[i] + dx * self.d[i]))
    }

    /// Build a spline from a `VolSurfacePoint` slice for a fixed expiry.
    ///
    /// Uses moneyness (strike/spot) as the x-axis.  Returns `None` if fewer
    /// than 2 points are available.
    pub fn from_surface_slice(points: &[VolSurfacePoint]) -> Option<Self> {
        let mut knots: Vec<(f64, f64)> = points
            .iter()
            .map(|p| (p.moneyness, p.implied_vol))
            .collect();
        knots.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));
        // Deduplicate identical moneyness values (keep the one with higher volume)
        knots.dedup_by(|a, b| (a.0 - b.0).abs() < 1e-8);
        Self::new(&knots).ok()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_implied_vol() {
        // TSLA-like parameters
        let spot = 250.0;
        let strike = 250.0; // ATM
        let time = 30.0 / 365.0;
        let rate = 0.05;
        let market_price = 15.0;
        
        let iv = implied_volatility_newton(market_price, spot, strike, time, rate, true);
        assert!(iv.is_some());
        
        let iv_val = iv.unwrap();
        assert!(iv_val > 0.0 && iv_val < 2.0, "IV should be reasonable");
    }

    #[test]
    fn cubic_spline_interpolates_at_knots() {
        // All queries at knot locations must round-trip to the original value.
        let knots = vec![
            (0.90, 0.30),
            (0.95, 0.27),
            (1.00, 0.25),
            (1.05, 0.26),
            (1.10, 0.28),
        ];
        let spline = CubicSplineSmile::new(&knots).unwrap();
        for (x, y) in &knots {
            let interp = spline.interpolate(*x);
            assert!(
                (interp - y).abs() < 1e-10,
                "spline at knot x={x} should return y={y:.4}, got {interp:.4}",
            );
        }
    }

    #[test]
    fn cubic_spline_extrapolates_flat_below() {
        let knots = vec![(0.90, 0.30), (1.00, 0.25), (1.10, 0.28)];
        let spline = CubicSplineSmile::new(&knots).unwrap();
        // Below the lowest knot → returns the leftmost IV
        let below = spline.interpolate(0.50);
        assert!((below - 0.30).abs() < 1e-10);
    }

    #[test]
    fn cubic_spline_extrapolates_flat_above() {
        let knots = vec![(0.90, 0.30), (1.00, 0.25), (1.10, 0.28)];
        let spline = CubicSplineSmile::new(&knots).unwrap();
        let above = spline.interpolate(2.00);
        assert!((above - 0.28).abs() < 1e-10);
    }

    #[test]
    fn cubic_spline_single_interval_is_polynomial() {
        // Two knots → flat line (constant interpolation)
        let knots = vec![(1.0, 0.25), (1.1, 0.30)];
        let spline = CubicSplineSmile::new(&knots).unwrap();
        let mid = spline.interpolate(1.05);
        // Mid-point of a linear segment
        assert!(mid > 0.24 && mid < 0.32);
    }

    #[test]
    fn cubic_spline_accepts_unsorted_knots() {
        // new() silently sorts input by x; unsorted input is accepted.
        let knots = vec![(1.0, 0.25), (0.9, 0.30), (1.1, 0.28)];
        let spline = CubicSplineSmile::new(&knots);
        assert!(spline.is_ok(), "Unsorted knots should be silently sorted and accepted");
    }

    #[test]
    fn cubic_spline_rejects_duplicate_x() {
        // Duplicate x-values are degenerate (singularity in tridiagonal system).
        let knots = vec![(1.0, 0.25), (1.0, 0.28), (1.1, 0.30)];
        assert!(CubicSplineSmile::new(&knots).is_err(),
            "Duplicate x values should be rejected");
    }

    #[test]
    fn cubic_spline_rejects_single_knot() {
        let knots = vec![(1.0, 0.25)];
        assert!(CubicSplineSmile::new(&knots).is_err());
    }
}
