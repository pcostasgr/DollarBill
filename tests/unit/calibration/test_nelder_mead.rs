// Optimizer tests — now covering CMA-ES (replaces the old custom Nelder-Mead)

use crate::helpers::EPSILON;
use dollarbill::calibration::cmaes::{Cmaes, CmaesConfig};

/// Convenience builder: tight tolerances, moderate budget
fn tight_cfg() -> CmaesConfig {
    CmaesConfig { max_fevals: 10_000, sigma0: 1.0, ftol: 1e-8, xtol: 1e-8, ..Default::default() }
}

#[test]
fn test_optimize_sphere() {
    // Sphere function: f(x,y) = x² + y²  — global minimum at (0, 0)
    let result = Cmaes::new(tight_cfg()).minimize(
        |x: &[f64]| x.iter().map(|v| v * v).sum::<f64>(),
        vec![10.0, 10.0],
    );
    assert!(result.best_value < 0.01, "Should find minimum near 0: {:.4e}", result.best_value);
    assert!((result.best_params[0]).abs() < 0.1, "x should be near 0");
    assert!((result.best_params[1]).abs() < 0.1, "y should be near 0");
}

#[test]
fn test_optimize_rosenbrock() {
    // Rosenbrock: f(x,y) = (1-x)² + 100(y-x²)²  — global minimum at (1,1)
    let cfg = CmaesConfig { max_fevals: 20_000, sigma0: 0.5, ftol: 1e-8, xtol: 1e-8, ..Default::default() };
    let result = Cmaes::new(cfg).minimize(
        |x: &[f64]| (1.0 - x[0]).powi(2) + 100.0 * (x[1] - x[0] * x[0]).powi(2),
        vec![0.0, 0.0],
    );
    assert!(result.best_value < 1e-4, "Should find near-zero residual: {:.4e}", result.best_value);
    assert!((result.best_params[0] - 1.0).abs() < 0.05, "x should be near 1");
    assert!((result.best_params[1] - 1.0).abs() < 0.05, "y should be near 1");
}

#[test]
fn test_optimize_beale() {
    // Beale function — minimum at (3, 0.5)
    let cfg = CmaesConfig { max_fevals: 20_000, sigma0: 1.5, ftol: 1e-8, xtol: 1e-8, ..Default::default() };
    let result = Cmaes::new(cfg).minimize(
        |x: &[f64]| {
            let (a, b) = (x[0], x[1]);
            (1.5 - a + a*b).powi(2)
                + (2.25 - a + a*b*b).powi(2)
                + (2.625 - a + a*b*b*b).powi(2)
        },
        vec![1.0, 1.0],
    );
    assert!(result.best_value < 0.01, "Beale residual too high: {:.4e}", result.best_value);
}

#[test]
fn test_budget_respected() {
    // CMA-ES should not exceed max_fevals significantly
    let cfg = CmaesConfig { max_fevals: 200, sigma0: 1.0, ftol: 1e-20, xtol: 1e-20, ..Default::default() };
    let result = Cmaes::new(cfg).minimize(
        |x: &[f64]| (1.0 - x[0]).powi(2) + 100.0 * (x[1] - x[0] * x[0]).powi(2),
        vec![0.0, 0.0],
    );
    // should not converge (too few evals) and must not overshoot the budget too much
    assert!(!result.converged, "Should not converge with 200 fevals on Rosenbrock");
    assert!(result.fevals <= 300, "fevals {} way over budget", result.fevals);
}

#[test]
fn test_convergence_to_ftol() {
    // Loose ftol should allow early stop with sphere
    let cfg = CmaesConfig { max_fevals: 100_000, sigma0: 2.0, ftol: 1.0, xtol: 1e-20, ..Default::default() };
    let result = Cmaes::new(cfg).minimize(
        |x: &[f64]| x.iter().map(|v| v * v).sum::<f64>(),
        vec![5.0, 5.0],
    );
    assert!(result.converged, "Loose ftol should allow early convergence");
    assert!(result.fevals < 5_000, "Should stop early: {} fevals", result.fevals);
}

#[test]
fn test_1d_optimization() {
    // Single-parameter case: f(x) = (x - 3)²  — minimum at x = 3
    let result = Cmaes::new(tight_cfg()).minimize(
        |x: &[f64]| (x[0] - 3.0).powi(2),
        vec![0.0],
    );
    assert!((result.best_params[0] - 3.0).abs() < 0.1, "Should find x ≈ 3 (got {:.4})", result.best_params[0]);
}

#[test]
fn test_4d_sphere() {
    let result = Cmaes::new(tight_cfg()).minimize(
        |x: &[f64]| x.iter().map(|v| v * v).sum::<f64>(),
        vec![5.0, -3.0, 2.0, -4.0],
    );
    assert!(result.best_value < 0.01, "4-D sphere residual: {:.4e}", result.best_value);
    for &p in &result.best_params {
        assert!(p.abs() < 0.2, "Parameter {} not near 0", p);
    }
}

#[test]
fn test_already_near_minimum() {
    // Starting very close to minimum — should converge rapidly
    let cfg = CmaesConfig { max_fevals: 2_000, sigma0: 0.01, ftol: 1e-10, xtol: 1e-10, ..Default::default() };
    let result = Cmaes::new(cfg).minimize(
        |x: &[f64]| x.iter().map(|v| v * v).sum::<f64>(),
        vec![0.0001, 0.0001],
    );
    assert!(result.best_value < 1e-4, "Should quickly reach near-zero: {:.4e}", result.best_value);
}

#[test]
fn test_asymmetric_function() {
    // f(x,y) = (x-2)² + (y+3)²  — minimum at (2, -3)
    let result = Cmaes::new(tight_cfg()).minimize(
        |x: &[f64]| (x[0] - 2.0).powi(2) + (x[1] + 3.0).powi(2),
        vec![0.0, 0.0],
    );
    assert!((result.best_params[0] - 2.0).abs() < 0.1, "x should be near 2");
    assert!((result.best_params[1] + 3.0).abs() < 0.1, "y should be near −3");
}

#[test]
fn test_constrained_via_penalty() {
    // x ∈ [2, 4], y ∈ [2, 4]: minimise distance to (3,3) with hard penalty outside bounds
    let result = Cmaes::new(tight_cfg()).minimize(
        |x: &[f64]| {
            if x[0] < 2.0 || x[0] > 4.0 || x[1] < 2.0 || x[1] > 4.0 { return 1e10; }
            (x[0] - 3.0).powi(2) + (x[1] - 3.0).powi(2)
        },
        vec![2.5, 2.5],
    );
    assert!((result.best_params[0] - 3.0).abs() < 0.2, "x should be near 3");
    assert!((result.best_params[1] - 3.0).abs() < 0.2, "y should be near 3");
}

#[test]
fn test_optimize_sphere() {
    // Sphere function: f(x,y) = x² + y²
    // Global minimum at (0, 0) with value 0
    let config = NelderMeadConfig::default();
    let optimizer = NelderMead::new(config);
    
    let sphere = |params: &[f64]| -> f64 {
        params.iter().map(|x| x * x).sum()
    };
    
    let initial = vec![10.0, 10.0];
    let result = optimizer.minimize(sphere, initial);
    
    assert!(result.converged, "Optimizer should converge");
    assert!(result.best_value < 0.01, "Should find minimum near 0");
    assert!((result.best_params[0]).abs() < 0.1, "x should be near 0");
    assert!((result.best_params[1]).abs() < 0.1, "y should be near 0");
}

#[test]
fn test_optimize_rosenbrock() {
    // Rosenbrock function: f(x,y) = (1-x)² + 100(y-x²)²
    // Global minimum at (1, 1) with value 0
    let mut config = NelderMeadConfig::default();
    config.max_iterations = 1000; // Rosenbrock is harder
    config.tolerance = 1e-4;
    
    let optimizer = NelderMead::new(config);
    
    let rosenbrock = |params: &[f64]| -> f64 {
        let x = params[0];
        let y = params[1];
        (1.0 - x).powi(2) + 100.0 * (y - x * x).powi(2)
    };
    
    let initial = vec![0.0, 0.0];
    let result = optimizer.minimize(rosenbrock, initial);
    
    assert!(result.best_value < 1.0, "Should find a good minimum");
    // Rosenbrock is hard, so we allow some error
    assert!((result.best_params[0] - 1.0).abs() < 0.5, "x should be near 1");
    assert!((result.best_params[1] - 1.0).abs() < 0.5, "y should be near 1");
}

#[test]
fn test_optimize_beale() {
    // Beale function: f(x,y) = (1.5 - x + xy)² + (2.25 - x + xy²)² + (2.625 - x + xy³)²
    // Global minimum at (3, 0.5) with value 0
    let mut config = NelderMeadConfig::default();
    config.max_iterations = 800;
    
    let optimizer = NelderMead::new(config);
    
    let beale = |params: &[f64]| -> f64 {
        let x = params[0];
        let y = params[1];
        (1.5 - x + x * y).powi(2) +
        (2.25 - x + x * y * y).powi(2) +
        (2.625 - x + x * y * y * y).powi(2)
    };
    
    let initial = vec![1.0, 1.0];
    let result = optimizer.minimize(beale, initial);
    
    assert!(result.best_value < 1.0, "Should find a good minimum");
}

#[test]
fn test_convergence_criteria() {
    // Test that optimizer stops when tolerance is reached
    let mut config = NelderMeadConfig::default();
    config.tolerance = 0.1; // Loose tolerance
    config.max_iterations = 10000;
    
    let optimizer = NelderMead::new(config);
    
    let sphere = |params: &[f64]| -> f64 {
        params.iter().map(|x| x * x).sum()
    };
    
    let initial = vec![5.0, 5.0];
    let result = optimizer.minimize(sphere, initial);
    
    assert!(result.converged, "Should converge with loose tolerance");
    assert!(result.iterations < 500, "Should converge quickly with loose tolerance");
}

#[test]
fn test_max_iterations() {
    // Test that optimizer respects iteration limit
    let mut config = NelderMeadConfig::default();
    config.max_iterations = 10; // Very limited
    config.tolerance = 1e-10; // Very tight (won't reach)
    
    let optimizer = NelderMead::new(config);
    
    let rosenbrock = |params: &[f64]| -> f64 {
        let x = params[0];
        let y = params[1];
        (1.0 - x).powi(2) + 100.0 * (y - x * x).powi(2)
    };
    
    let initial = vec![0.0, 0.0];
    let result = optimizer.minimize(rosenbrock, initial);
    
    assert_eq!(result.iterations, 10, "Should stop at max iterations");
    assert!(!result.converged, "Should not have converged in 10 iterations");
}

#[test]
fn test_1d_optimization() {
    // Test with single parameter
    let config = NelderMeadConfig::default();
    let optimizer = NelderMead::new(config);
    
    let quadratic = |params: &[f64]| -> f64 {
        let x = params[0];
        (x - 3.0).powi(2) // Minimum at x = 3
    };
    
    let initial = vec![0.0];
    let result = optimizer.minimize(quadratic, initial);
    
    assert!(result.converged, "Should converge");
    assert!((result.best_params[0] - 3.0).abs() < 0.1, "Should find minimum at x=3");
}

#[test]
fn test_multiple_dimensions() {
    // Test with more parameters (4D)
    let config = NelderMeadConfig::default();
    let optimizer = NelderMead::new(config);
    
    let sphere_4d = |params: &[f64]| -> f64 {
        params.iter().map(|x| x * x).sum()
    };
    
    let initial = vec![5.0, -3.0, 2.0, -4.0];
    let result = optimizer.minimize(sphere_4d, initial);
    
    assert!(result.converged, "Should converge in 4D");
    for &param in &result.best_params {
        assert!(param.abs() < 0.5, "All parameters should be near 0");
    }
}

#[test]
fn test_custom_coefficients() {
    // Test with non-default coefficients
    let mut config = NelderMeadConfig::default();
    config.alpha = 1.5;  // More aggressive reflection
    config.gamma = 2.5;  // More aggressive expansion
    config.rho = 0.3;    // More aggressive contraction
    
    let optimizer = NelderMead::new(config);
    
    let sphere = |params: &[f64]| -> f64 {
        params.iter().map(|x| x * x).sum()
    };
    
    let initial = vec![10.0, 10.0];
    let result = optimizer.minimize(sphere, initial);
    
    assert!(result.converged, "Should converge with custom coefficients");
    assert!(result.best_value < 0.1, "Should still find minimum");
}

#[test]
fn test_already_at_minimum() {
    // Test when starting very close to minimum
    let config = NelderMeadConfig::default();
    let optimizer = NelderMead::new(config);
    
    let sphere = |params: &[f64]| -> f64 {
        params.iter().map(|x| x * x).sum()
    };
    
    let initial = vec![0.0001, 0.0001];
    let result = optimizer.minimize(sphere, initial);
    
    assert!(result.converged, "Should converge immediately");
    assert!(result.iterations < 20, "Should converge very quickly");
}

#[test]
fn test_asymmetric_function() {
    // Test with non-symmetric objective
    let config = NelderMeadConfig::default();
    let optimizer = NelderMead::new(config);
    
    let asymmetric = |params: &[f64]| -> f64 {
        let x = params[0];
        let y = params[1];
        (x - 2.0).powi(2) + (y + 3.0).powi(2)  // Minimum at (2, -3)
    };
    
    let initial = vec![0.0, 0.0];
    let result = optimizer.minimize(asymmetric, initial);
    
    assert!(result.converged, "Should converge");
    assert!((result.best_params[0] - 2.0).abs() < 0.2, "x should be near 2");
    assert!((result.best_params[1] + 3.0).abs() < 0.2, "y should be near -3");
}

#[test]
fn test_optimizer_finds_local_minimum() {
    // Even if not global minimum, should find a local minimum
    let config = NelderMeadConfig::default();
    let optimizer = NelderMead::new(config);
    
    let multi_modal = |params: &[f64]| -> f64 {
        let x = params[0];
        x.sin() + 0.1 * x * x  // Multiple local minima
    };
    
    let initial = vec![1.0];
    let result = optimizer.minimize(multi_modal, initial);
    
    // Should find some local minimum
    assert!(result.best_value.is_finite(), "Should find a finite minimum");
}
