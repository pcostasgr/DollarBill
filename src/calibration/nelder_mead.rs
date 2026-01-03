// Simple Nelder-Mead optimization algorithm
// Pure Rust implementation - no external dependencies

/// Nelder-Mead algorithm parameters
#[derive(Debug, Clone)]
pub struct NelderMeadConfig {
    pub max_iterations: usize,
    pub tolerance: f64,
    pub alpha: f64,  // Reflection coefficient (default: 1.0)
    pub gamma: f64,  // Expansion coefficient (default: 2.0)
    pub rho: f64,    // Contraction coefficient (default: 0.5)
    pub sigma: f64,  // Shrink coefficient (default: 0.5)
}

impl Default for NelderMeadConfig {
    fn default() -> Self {
        Self {
            max_iterations: 500,
            tolerance: 1e-6,
            alpha: 1.0,
            gamma: 2.0,
            rho: 0.5,
            sigma: 0.5,
        }
    }
}

/// Result of Nelder-Mead optimization
#[derive(Debug, Clone)]
pub struct OptimizationResult {
    pub best_params: Vec<f64>,
    pub best_value: f64,
    pub iterations: usize,
    pub converged: bool,
}

/// Nelder-Mead simplex optimizer
pub struct NelderMead {
    config: NelderMeadConfig,
}

impl NelderMead {
    pub fn new(config: NelderMeadConfig) -> Self {
        Self { config }
    }
    
    /// Minimize the objective function starting from initial_params
    pub fn minimize<F>(
        &self,
        objective: F,
        initial_params: Vec<f64>,
    ) -> OptimizationResult
    where
        F: Fn(&[f64]) -> f64,
    {
        let n = initial_params.len();
        
        // Create initial simplex (n+1 vertices)
        let mut simplex = self.create_initial_simplex(&initial_params);
        
        // Evaluate objective at each vertex
        let mut values: Vec<f64> = simplex.iter().map(|p| objective(p)).collect();
        
        let mut iteration = 0;
        let mut converged = false;
        
        while iteration < self.config.max_iterations {
            // Sort simplex by objective value (best to worst)
            let mut indices: Vec<usize> = (0..simplex.len()).collect();
            indices.sort_by(|&a, &b| values[a].partial_cmp(&values[b]).unwrap());
            
            let best_idx = indices[0];
            let worst_idx = indices[n];
            let second_worst_idx = indices[n - 1];
            
            // Check convergence
            let range = values[worst_idx] - values[best_idx];
            if range < self.config.tolerance {
                converged = true;
                break;
            }
            
            // Calculate centroid (excluding worst point)
            let centroid = self.calculate_centroid(&simplex, &indices[..n]);
            
            // Reflection
            let reflected = self.reflect(&simplex[worst_idx], &centroid, self.config.alpha);
            let f_reflected = objective(&reflected);
            
            if f_reflected < values[second_worst_idx] && f_reflected >= values[best_idx] {
                // Accept reflection
                simplex[worst_idx] = reflected;
                values[worst_idx] = f_reflected;
            } else if f_reflected < values[best_idx] {
                // Expansion
                let expanded = self.reflect(&simplex[worst_idx], &centroid, self.config.gamma);
                let f_expanded = objective(&expanded);
                
                if f_expanded < f_reflected {
                    simplex[worst_idx] = expanded;
                    values[worst_idx] = f_expanded;
                } else {
                    simplex[worst_idx] = reflected;
                    values[worst_idx] = f_reflected;
                }
            } else {
                // Contraction
                let contracted = self.contract(&simplex[worst_idx], &centroid, self.config.rho);
                let f_contracted = objective(&contracted);
                
                if f_contracted < values[worst_idx] {
                    simplex[worst_idx] = contracted;
                    values[worst_idx] = f_contracted;
                } else {
                    // Shrink - need to clone best point to avoid borrow issues
                    let best_point = simplex[best_idx].clone();
                    self.shrink(&mut simplex, &best_point, self.config.sigma);
                    for i in 0..simplex.len() {
                        values[i] = objective(&simplex[i]);
                    }
                }
            }
            
            iteration += 1;
        }
        
        // Find best point
        let best_idx = values
            .iter()
            .enumerate()
            .min_by(|(_, a), (_, b)| a.partial_cmp(b).unwrap())
            .map(|(i, _)| i)
            .unwrap();
        
        OptimizationResult {
            best_params: simplex[best_idx].clone(),
            best_value: values[best_idx],
            iterations: iteration,
            converged,
        }
    }
    
    /// Create initial simplex with n+1 vertices
    fn create_initial_simplex(&self, initial: &[f64]) -> Vec<Vec<f64>> {
        let n = initial.len();
        let mut simplex = Vec::with_capacity(n + 1);
        
        // First vertex is the initial point
        simplex.push(initial.to_vec());
        
        // Create n additional vertices by perturbing each dimension
        for i in 0..n {
            let mut vertex = initial.to_vec();
            let step = if initial[i].abs() > 1e-10 {
                initial[i] * 0.05  // 5% of current value
            } else {
                0.00025  // Small absolute step for near-zero values
            };
            vertex[i] += step;
            simplex.push(vertex);
        }
        
        simplex
    }
    
    /// Calculate centroid of given points
    fn calculate_centroid(&self, simplex: &[Vec<f64>], indices: &[usize]) -> Vec<f64> {
        let n = simplex[0].len();
        let mut centroid = vec![0.0; n];
        
        for &idx in indices {
            for i in 0..n {
                centroid[i] += simplex[idx][i];
            }
        }
        
        for i in 0..n {
            centroid[i] /= indices.len() as f64;
        }
        
        centroid
    }
    
    /// Reflect point through centroid
    fn reflect(&self, point: &[f64], centroid: &[f64], coeff: f64) -> Vec<f64> {
        point.iter()
            .zip(centroid)
            .map(|(p, c)| c + coeff * (c - p))
            .collect()
    }
    
    /// Contract point towards centroid
    fn contract(&self, point: &[f64], centroid: &[f64], coeff: f64) -> Vec<f64> {
        point.iter()
            .zip(centroid)
            .map(|(p, c)| c + coeff * (p - c))
            .collect()
    }
    
    /// Shrink all points towards best point
    fn shrink(&self, simplex: &mut [Vec<f64>], best: &[f64], coeff: f64) {
        for vertex in simplex.iter_mut() {
            for i in 0..vertex.len() {
                vertex[i] = best[i] + coeff * (vertex[i] - best[i]);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_rosenbrock() {
        // Minimize Rosenbrock function: f(x,y) = (1-x)^2 + 100(y-x^2)^2
        // Global minimum at (1, 1) with f(1,1) = 0
        let rosenbrock = |params: &[f64]| {
            let x = params[0];
            let y = params[1];
            (1.0 - x).powi(2) + 100.0 * (y - x.powi(2)).powi(2)
        };
        
        let optimizer = NelderMead::new(NelderMeadConfig::default());
        let result = optimizer.minimize(rosenbrock, vec![0.0, 0.0]);
        
        // Check if close to minimum
        assert!((result.best_params[0] - 1.0).abs() < 0.01);
        assert!((result.best_params[1] - 1.0).abs() < 0.01);
        assert!(result.best_value < 0.001);
    }
    
    #[test]
    fn test_sphere() {
        // Minimize sphere function: f(x) = sum(x_i^2)
        // Global minimum at origin with f(0) = 0
        let sphere = |params: &[f64]| {
            params.iter().map(|x| x * x).sum()
        };
        
        let optimizer = NelderMead::new(NelderMeadConfig::default());
        let result = optimizer.minimize(sphere, vec![5.0, -3.0, 2.0]);
        
        for &param in &result.best_params {
            assert!(param.abs() < 0.01);
        }
        assert!(result.best_value < 0.001);
    }
}
