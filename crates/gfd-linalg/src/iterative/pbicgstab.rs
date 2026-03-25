//! Preconditioned BiCGSTAB solver.

use gfd_core::{SparseMatrix, SolverStats};
use crate::{LinalgError, Result};
use crate::traits::{LinearSolverTrait, PreconditionerTrait};

fn dot(a: &[f64], b: &[f64]) -> f64 {
    a.iter().zip(b.iter()).map(|(x, y)| x * y).sum()
}

fn norm2(v: &[f64]) -> f64 {
    dot(v, v).sqrt()
}

/// Preconditioned BiCGSTAB solver.
pub struct PBiCGSTAB {
    pub tol: f64,
    pub max_iter: usize,
    preconditioner: Box<dyn PreconditionerTrait>,
    setup_done: bool,
}

impl PBiCGSTAB {
    pub fn new(tol: f64, max_iter: usize, preconditioner: Box<dyn PreconditionerTrait>) -> Self {
        Self {
            tol,
            max_iter,
            preconditioner,
            setup_done: false,
        }
    }
}

impl LinearSolverTrait for PBiCGSTAB {
    fn solve(&mut self, a: &SparseMatrix, b: &[f64], x: &mut [f64]) -> Result<SolverStats> {
        let n = a.nrows;
        if b.len() != n || x.len() != n {
            return Err(LinalgError::DimensionMismatch(format!(
                "PBiCGSTAB: matrix {}x{}, b {}, x {}", a.nrows, a.ncols, b.len(), x.len()
            )));
        }

        let b_norm = norm2(b);
        if b_norm == 0.0 {
            x.iter_mut().for_each(|xi| *xi = 0.0);
            return Ok(SolverStats { iterations: 0, final_residual: 0.0, converged: true });
        }

        if !self.setup_done {
            self.preconditioner.setup(a)?;
            self.setup_done = true;
        }

        // r = b - A*x
        let mut r = vec![0.0; n];
        let mut ax = vec![0.0; n];
        a.spmv(x, &mut ax)?;
        for i in 0..n { r[i] = b[i] - ax[i]; }

        let r_hat = r.clone();
        let mut rho = 1.0_f64;
        let mut alpha = 1.0_f64;
        let mut omega = 1.0_f64;

        let mut v = vec![0.0; n];
        let mut p = vec![0.0; n];
        let mut p_hat = vec![0.0; n];
        let mut s = vec![0.0; n];
        let mut s_hat = vec![0.0; n];
        let mut t = vec![0.0; n];

        for iter in 0..self.max_iter {
            let res_norm = norm2(&r);
            if res_norm / b_norm < self.tol {
                return Ok(SolverStats {
                    iterations: iter,
                    final_residual: res_norm / b_norm,
                    converged: true,
                });
            }

            let rho_new = dot(&r_hat, &r);
            if rho_new.abs() < 1e-300 {
                return Err(LinalgError::SingularMatrix("PBiCGSTAB: rho=0".into()));
            }

            let beta = (rho_new / rho) * (alpha / omega);

            for i in 0..n {
                p[i] = r[i] + beta * (p[i] - omega * v[i]);
            }

            // p_hat = M^{-1} p
            self.preconditioner.apply(&p, &mut p_hat)?;

            // v = A * p_hat
            a.spmv(&p_hat, &mut v)?;

            let r_hat_v = dot(&r_hat, &v);
            if r_hat_v.abs() < 1e-300 {
                return Err(LinalgError::SingularMatrix("PBiCGSTAB: r_hat^T v=0".into()));
            }
            alpha = rho_new / r_hat_v;

            // s = r - alpha * v
            for i in 0..n {
                s[i] = r[i] - alpha * v[i];
            }

            let s_norm = norm2(&s);
            if s_norm / b_norm < self.tol {
                for i in 0..n { x[i] += alpha * p_hat[i]; }
                return Ok(SolverStats {
                    iterations: iter + 1,
                    final_residual: s_norm / b_norm,
                    converged: true,
                });
            }

            // s_hat = M^{-1} s
            self.preconditioner.apply(&s, &mut s_hat)?;

            // t = A * s_hat
            a.spmv(&s_hat, &mut t)?;

            let tt = dot(&t, &t);
            if tt.abs() < 1e-300 {
                return Err(LinalgError::SingularMatrix("PBiCGSTAB: t^T t=0".into()));
            }
            omega = dot(&t, &s) / tt;

            // x += alpha * p_hat + omega * s_hat
            for i in 0..n {
                x[i] += alpha * p_hat[i] + omega * s_hat[i];
            }

            // r = s - omega * t
            for i in 0..n {
                r[i] = s[i] - omega * t[i];
            }

            if omega.abs() < 1e-300 {
                return Err(LinalgError::SingularMatrix("PBiCGSTAB: omega=0".into()));
            }

            rho = rho_new;
        }

        let final_residual = norm2(&r) / b_norm;
        Ok(SolverStats { iterations: self.max_iter, final_residual, converged: false })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::preconditioner::ilu::ILU0;

    #[test]
    fn pbicgstab_with_ilu0() {
        let a = SparseMatrix::new(
            3, 3,
            vec![0, 2, 5, 7],
            vec![0, 1, 0, 1, 2, 1, 2],
            vec![4.0, -1.0, -2.0, 4.0, -1.0, -1.0, 3.0], // non-symmetric
        ).unwrap();
        let b = vec![3.0, 1.0, 2.0];
        let mut x = vec![0.0; 3];

        let precond = Box::new(ILU0::new());
        let mut solver = PBiCGSTAB::new(1e-10, 100, precond);
        let stats = solver.solve(&a, &b, &mut x).unwrap();
        assert!(stats.converged);

        let mut ax = vec![0.0; 3];
        a.spmv(&x, &mut ax).unwrap();
        for i in 0..3 {
            assert!((ax[i] - b[i]).abs() < 1e-8, "row {}: {} vs {}", i, ax[i], b[i]);
        }
    }
}
