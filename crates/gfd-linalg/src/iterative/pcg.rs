//! Preconditioned Conjugate Gradient (PCG) solver.

use gfd_core::{SparseMatrix, SolverStats};
use crate::{LinalgError, Result};
use crate::traits::{LinearSolverTrait, PreconditionerTrait};

fn dot(a: &[f64], b: &[f64]) -> f64 {
    a.iter().zip(b.iter()).map(|(x, y)| x * y).sum()
}

/// PCG solver with a pluggable preconditioner.
pub struct PCG {
    pub tol: f64,
    pub max_iter: usize,
    preconditioner: Box<dyn PreconditionerTrait>,
    setup_done: bool,
}

impl PCG {
    pub fn new(tol: f64, max_iter: usize, preconditioner: Box<dyn PreconditionerTrait>) -> Self {
        Self {
            tol,
            max_iter,
            preconditioner,
            setup_done: false,
        }
    }
}

impl LinearSolverTrait for PCG {
    fn solve(&mut self, a: &SparseMatrix, b: &[f64], x: &mut [f64]) -> Result<SolverStats> {
        let n = a.nrows;
        if b.len() != n || x.len() != n {
            return Err(LinalgError::DimensionMismatch(format!(
                "PCG: matrix {}x{}, b {}, x {}", a.nrows, a.ncols, b.len(), x.len()
            )));
        }

        let b_norm: f64 = dot(b, b).sqrt();
        if b_norm == 0.0 {
            x.iter_mut().for_each(|xi| *xi = 0.0);
            return Ok(SolverStats { iterations: 0, final_residual: 0.0, converged: true });
        }

        // Setup preconditioner (once per matrix)
        if !self.setup_done {
            self.preconditioner.setup(a)?;
            self.setup_done = true;
        }

        // r = b - A*x
        let mut r = vec![0.0; n];
        let mut ax = vec![0.0; n];
        a.spmv(x, &mut ax)?;
        for i in 0..n { r[i] = b[i] - ax[i]; }

        // z = M^{-1} r
        let mut z = vec![0.0; n];
        self.preconditioner.apply(&r, &mut z)?;

        // p = z
        let mut p = z.clone();
        let mut rz = dot(&r, &z);

        let mut ap = vec![0.0; n];

        for iter in 0..self.max_iter {
            let res_norm = dot(&r, &r).sqrt();
            if res_norm / b_norm < self.tol {
                return Ok(SolverStats {
                    iterations: iter,
                    final_residual: res_norm / b_norm,
                    converged: true,
                });
            }

            // ap = A*p
            a.spmv(&p, &mut ap)?;

            let pap = dot(&p, &ap);
            if pap.abs() < 1e-300 {
                return Err(LinalgError::SingularMatrix("PCG: p^T A p = 0".into()));
            }
            let alpha = rz / pap;

            // x += alpha * p
            for i in 0..n { x[i] += alpha * p[i]; }
            // r -= alpha * Ap
            for i in 0..n { r[i] -= alpha * ap[i]; }

            // z = M^{-1} r
            self.preconditioner.apply(&r, &mut z)?;

            let rz_new = dot(&r, &z);
            let beta = rz_new / rz;

            // p = z + beta * p
            for i in 0..n { p[i] = z[i] + beta * p[i]; }

            rz = rz_new;
        }

        let final_residual = dot(&r, &r).sqrt() / b_norm;
        Ok(SolverStats { iterations: self.max_iter, final_residual, converged: false })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::preconditioner::jacobi::Jacobi;

    #[test]
    fn pcg_with_jacobi() {
        let a = SparseMatrix::new(
            3, 3,
            vec![0, 2, 5, 7],
            vec![0, 1, 0, 1, 2, 1, 2],
            vec![4.0, -1.0, -1.0, 4.0, -1.0, -1.0, 4.0],
        ).unwrap();
        let b = vec![3.0, 2.0, 3.0];
        let mut x = vec![0.0; 3];

        let precond = Box::new(Jacobi::new());
        let mut solver = PCG::new(1e-10, 100, precond);
        let stats = solver.solve(&a, &b, &mut x).unwrap();
        assert!(stats.converged);

        let mut ax = vec![0.0; 3];
        a.spmv(&x, &mut ax).unwrap();
        for i in 0..3 {
            assert!((ax[i] - b[i]).abs() < 1e-8);
        }
    }

    #[test]
    fn pcg_with_ilu0() {
        use crate::preconditioner::ilu::ILU0;

        let a = SparseMatrix::new(
            3, 3,
            vec![0, 2, 5, 7],
            vec![0, 1, 0, 1, 2, 1, 2],
            vec![4.0, -1.0, -1.0, 4.0, -1.0, -1.0, 4.0],
        ).unwrap();
        let b = vec![3.0, 2.0, 3.0];
        let mut x = vec![0.0; 3];

        let precond = Box::new(ILU0::new());
        let mut solver = PCG::new(1e-10, 100, precond);
        let stats = solver.solve(&a, &b, &mut x).unwrap();
        assert!(stats.converged);
        // ILU(0) on tridiagonal = exact, so should converge in 1 iteration
        assert!(stats.iterations <= 2, "Expected <=2 iters, got {}", stats.iterations);
    }
}
