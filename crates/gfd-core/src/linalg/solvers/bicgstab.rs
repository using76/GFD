//! Bi-Conjugate Gradient Stabilized (BiCGSTAB) solver for non-symmetric systems.

use crate::linalg::{LinearSystem, SolverConfig, SolverStats};
use crate::Result;
use super::LinearSolver;

/// BiCGSTAB iterative solver.
///
/// Suitable for general non-symmetric systems. Often preferred over GMRES
/// for its fixed memory requirements per iteration.
#[derive(Debug, Clone)]
pub struct BiCgStab {
    /// Solver configuration.
    pub config: SolverConfig,
}

impl BiCgStab {
    /// Creates a new BiCGSTAB solver with the given configuration.
    pub fn new(config: SolverConfig) -> Self {
        Self { config }
    }
}

impl Default for BiCgStab {
    fn default() -> Self {
        Self {
            config: SolverConfig::default(),
        }
    }
}

impl LinearSolver for BiCgStab {
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

        let r_hat = r.clone(); // r_hat_0 = r_0
        let b_norm: f64 = system.b.iter().map(|bi| bi * bi).sum::<f64>().sqrt().max(1e-30);

        let mut rho = 1.0_f64;
        let mut alpha = 1.0_f64;
        let mut omega = 1.0_f64;
        let mut v = vec![0.0_f64; n];
        let mut p = vec![0.0_f64; n];
        let mut s = vec![0.0_f64; n];
        let mut t = vec![0.0_f64; n];

        let mut iterations = 0;
        let mut final_residual = r.iter().map(|ri| ri * ri).sum::<f64>().sqrt() / b_norm;

        if final_residual < tol {
            return Ok(SolverStats { iterations: 0, final_residual, converged: true });
        }

        for k in 0..max_iter {
            iterations = k + 1;

            let rho_new: f64 = r_hat.iter().zip(r.iter()).map(|(rh, ri)| rh * ri).sum();
            if rho_new.abs() < 1e-30 { break; }

            let beta = (rho_new / rho.max(1e-30)) * (alpha / omega.max(1e-30));
            rho = rho_new;

            for i in 0..n {
                p[i] = r[i] + beta * (p[i] - omega * v[i]);
            }

            // v = A * p
            system.a.spmv(&p, &mut v)?;

            let r_hat_v: f64 = r_hat.iter().zip(v.iter()).map(|(rh, vi)| rh * vi).sum();
            if r_hat_v.abs() < 1e-30 { break; }
            alpha = rho / r_hat_v;

            for i in 0..n {
                s[i] = r[i] - alpha * v[i];
            }

            // Check if s is small enough
            let s_norm = s.iter().map(|si| si * si).sum::<f64>().sqrt() / b_norm;
            if s_norm < tol {
                for i in 0..n {
                    system.x[i] += alpha * p[i];
                }
                return Ok(SolverStats { iterations, final_residual: s_norm, converged: true });
            }

            // t = A * s
            system.a.spmv(&s, &mut t)?;

            let t_s: f64 = t.iter().zip(s.iter()).map(|(ti, si)| ti * si).sum();
            let t_t: f64 = t.iter().map(|ti| ti * ti).sum();
            omega = if t_t.abs() > 1e-30 { t_s / t_t } else { 1.0 };

            for i in 0..n {
                system.x[i] += alpha * p[i] + omega * s[i];
                r[i] = s[i] - omega * t[i];
            }

            final_residual = r.iter().map(|ri| ri * ri).sum::<f64>().sqrt() / b_norm;

            if final_residual < tol {
                return Ok(SolverStats { iterations, final_residual, converged: true });
            }
        }

        Ok(SolverStats { iterations, final_residual, converged: final_residual < tol })
    }

    fn name(&self) -> &str {
        "BiCGSTAB"
    }
}
