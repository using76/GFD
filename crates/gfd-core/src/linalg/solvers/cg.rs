//! Conjugate Gradient solver for symmetric positive-definite systems.

use crate::linalg::{LinearSystem, SolverConfig, SolverStats};
use crate::Result;
use super::LinearSolver;

/// Conjugate Gradient iterative solver.
///
/// Only suitable for symmetric positive-definite matrices.
#[derive(Debug, Clone)]
pub struct ConjugateGradient {
    /// Solver configuration.
    pub config: SolverConfig,
}

impl ConjugateGradient {
    /// Creates a new Conjugate Gradient solver with the given configuration.
    pub fn new(config: SolverConfig) -> Self {
        Self { config }
    }
}

impl Default for ConjugateGradient {
    fn default() -> Self {
        Self {
            config: SolverConfig::default(),
        }
    }
}

impl LinearSolver for ConjugateGradient {
    fn solve(&mut self, system: &mut LinearSystem) -> Result<SolverStats> {
        let n = system.size();
        let tol = self.config.tolerance;
        let max_iter = self.config.max_iterations;

        // r = b - A*x
        let mut r = vec![0.0_f64; n];
        let mut ax = vec![0.0_f64; n];
        system.a.spmv(&system.x, &mut ax)?;
        for i in 0..n {
            r[i] = system.b[i] - ax[i];
        }

        let mut p = r.clone();
        let mut rs_old: f64 = r.iter().map(|ri| ri * ri).sum();
        let b_norm: f64 = system.b.iter().map(|bi| bi * bi).sum::<f64>().sqrt().max(1e-30);

        let mut iterations = 0;
        let mut final_residual = rs_old.sqrt() / b_norm;

        if final_residual < tol {
            return Ok(SolverStats {
                iterations: 0,
                final_residual,
                converged: true,
            });
        }

        let mut ap = vec![0.0_f64; n];

        for k in 0..max_iter {
            iterations = k + 1;

            // ap = A * p
            system.a.spmv(&p, &mut ap)?;

            // alpha = rs_old / (p . ap)
            let p_ap: f64 = p.iter().zip(ap.iter()).map(|(pi, api)| pi * api).sum();
            if p_ap.abs() < 1e-30 {
                break;
            }
            let alpha = rs_old / p_ap;

            // x = x + alpha * p
            // r = r - alpha * ap
            for i in 0..n {
                system.x[i] += alpha * p[i];
                r[i] -= alpha * ap[i];
            }

            let rs_new: f64 = r.iter().map(|ri| ri * ri).sum();
            final_residual = rs_new.sqrt() / b_norm;

            if final_residual < tol {
                return Ok(SolverStats {
                    iterations,
                    final_residual,
                    converged: true,
                });
            }

            let beta = rs_new / rs_old.max(1e-30);
            for i in 0..n {
                p[i] = r[i] + beta * p[i];
            }
            rs_old = rs_new;
        }

        Ok(SolverStats {
            iterations,
            final_residual,
            converged: final_residual < tol,
        })
    }

    fn name(&self) -> &str {
        "ConjugateGradient"
    }
}
