//! Generalized Minimal Residual (GMRES) solver for non-symmetric systems.

use crate::linalg::{LinearSystem, SolverConfig, SolverStats};
use crate::Result;
use super::LinearSolver;

/// GMRES iterative solver with restarts.
///
/// Suitable for general non-symmetric systems.
#[derive(Debug, Clone)]
pub struct Gmres {
    /// Solver configuration.
    pub config: SolverConfig,
    /// Restart parameter (Krylov subspace dimension before restart).
    pub restart: usize,
}

impl Gmres {
    /// Creates a new GMRES solver with the given configuration and restart parameter.
    pub fn new(config: SolverConfig, restart: usize) -> Self {
        Self { config, restart }
    }
}

impl Default for Gmres {
    fn default() -> Self {
        Self {
            config: SolverConfig::default(),
            restart: 30,
        }
    }
}

impl LinearSolver for Gmres {
    fn solve(&mut self, system: &mut LinearSystem) -> Result<SolverStats> {
        let n = system.size();
        let tol = self.config.tolerance;
        let max_iter = self.config.max_iterations;
        let m = self.restart.min(n).max(1); // Krylov subspace size

        let b_norm: f64 = system.b.iter().map(|bi| bi * bi).sum::<f64>().sqrt().max(1e-30);
        let mut iterations = 0;
        let mut final_residual = 0.0_f64;

        let mut tmp = vec![0.0_f64; n];

        for _restart in 0..(max_iter / m + 1) {
            // r = b - A*x
            system.a.spmv(&system.x, &mut tmp)?;
            let mut r = vec![0.0_f64; n];
            for i in 0..n {
                r[i] = system.b[i] - tmp[i];
            }

            let r_norm: f64 = r.iter().map(|ri| ri * ri).sum::<f64>().sqrt();
            final_residual = r_norm / b_norm;
            if final_residual < tol {
                return Ok(SolverStats { iterations, final_residual, converged: true });
            }

            // Arnoldi + Givens rotation GMRES
            let mut v_basis: Vec<Vec<f64>> = Vec::new();
            let mut h = vec![vec![0.0_f64; m]; m + 1]; // Hessenberg matrix
            let mut cs = vec![0.0_f64; m]; // Givens cosines
            let mut sn = vec![0.0_f64; m]; // Givens sines
            let mut g = vec![0.0_f64; m + 1]; // Right-hand side
            g[0] = r_norm;

            // v_0 = r / ||r||
            let mut v0 = r.clone();
            if r_norm > 1e-30 {
                for vi in v0.iter_mut() { *vi /= r_norm; }
            }
            v_basis.push(v0);

            let mut j_final = 0;
            for j in 0..m {
                iterations += 1;
                j_final = j;

                // w = A * v_j
                let mut w = vec![0.0_f64; n];
                system.a.spmv(&v_basis[j], &mut w)?;

                // Modified Gram-Schmidt
                for i in 0..=j {
                    h[i][j] = w.iter().zip(v_basis[i].iter()).map(|(a, b)| a * b).sum();
                    for k in 0..n {
                        w[k] -= h[i][j] * v_basis[i][k];
                    }
                }
                h[j + 1][j] = w.iter().map(|wi| wi * wi).sum::<f64>().sqrt();

                if h[j + 1][j] > 1e-30 {
                    for wi in w.iter_mut() { *wi /= h[j + 1][j]; }
                }
                v_basis.push(w);

                // Apply previous Givens rotations
                for i in 0..j {
                    let temp = cs[i] * h[i][j] + sn[i] * h[i + 1][j];
                    h[i + 1][j] = -sn[i] * h[i][j] + cs[i] * h[i + 1][j];
                    h[i][j] = temp;
                }

                // Compute new Givens rotation
                let denom = (h[j][j].powi(2) + h[j + 1][j].powi(2)).sqrt();
                if denom > 1e-30 {
                    cs[j] = h[j][j] / denom;
                    sn[j] = h[j + 1][j] / denom;
                } else {
                    cs[j] = 1.0;
                    sn[j] = 0.0;
                }

                h[j][j] = cs[j] * h[j][j] + sn[j] * h[j + 1][j];
                h[j + 1][j] = 0.0;

                let temp = -sn[j] * g[j];
                g[j] = cs[j] * g[j];
                g[j + 1] = temp;

                final_residual = g[j + 1].abs() / b_norm;
                if final_residual < tol {
                    j_final = j;
                    break;
                }
            }

            // Back substitution
            let km = j_final + 1;
            let mut y = vec![0.0_f64; km];
            for i in (0..km).rev() {
                if h[i][i].abs() < 1e-30 { continue; }
                let mut s = g[i];
                for j in (i + 1)..km {
                    s -= h[i][j] * y[j];
                }
                y[i] = s / h[i][i];
            }

            // x = x + V * y
            for i in 0..km {
                for k in 0..n {
                    system.x[k] += y[i] * v_basis[i][k];
                }
            }

            if final_residual < tol || iterations >= max_iter {
                break;
            }
        }

        Ok(SolverStats { iterations, final_residual, converged: final_residual < tol })
    }

    fn name(&self) -> &str {
        "GMRES"
    }
}
