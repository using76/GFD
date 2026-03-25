//! Conjugate Gradient (CG) solver for symmetric positive-definite systems.

use gfd_core::{SparseMatrix, SolverStats};
use crate::{LinalgError, Result};
use crate::traits::LinearSolverTrait;

/// Conjugate Gradient solver.
///
/// Solves A * x = b where A is symmetric positive-definite.
/// Reuses workspace vectors across solve calls for the same system size.
#[derive(Debug, Clone)]
pub struct CG {
    /// Convergence tolerance for the relative residual norm ||r||/||b||.
    pub tol: f64,
    /// Maximum number of iterations.
    pub max_iter: usize,
    ws_n: usize,
    ws_r: Vec<f64>,
    ws_ax: Vec<f64>,
    ws_p: Vec<f64>,
    ws_ap: Vec<f64>,
}

impl CG {
    /// Creates a new CG solver with the given tolerance and maximum iterations.
    pub fn new(tol: f64, max_iter: usize) -> Self {
        Self { tol, max_iter, ws_n: 0, ws_r: Vec::new(), ws_ax: Vec::new(), ws_p: Vec::new(), ws_ap: Vec::new() }
    }

    fn ensure_workspace(&mut self, n: usize) {
        if self.ws_n == n { return; }
        self.ws_r = vec![0.0; n];
        self.ws_ax = vec![0.0; n];
        self.ws_p = vec![0.0; n];
        self.ws_ap = vec![0.0; n];
        self.ws_n = n;
    }
}

impl Default for CG {
    fn default() -> Self {
        Self::new(1e-6, 1000)
    }
}

/// Compute the dot product of two vectors (4-way unrolled).
#[inline(always)]
fn dot(a: &[f64], b: &[f64]) -> f64 {
    let n = a.len();
    let chunks = n / 4;
    let mut s0 = 0.0;
    let mut s1 = 0.0;
    let mut s2 = 0.0;
    let mut s3 = 0.0;
    for i in 0..chunks {
        let base = i * 4;
        unsafe {
            s0 += *a.get_unchecked(base) * *b.get_unchecked(base);
            s1 += *a.get_unchecked(base + 1) * *b.get_unchecked(base + 1);
            s2 += *a.get_unchecked(base + 2) * *b.get_unchecked(base + 2);
            s3 += *a.get_unchecked(base + 3) * *b.get_unchecked(base + 3);
        }
    }
    for i in (chunks * 4)..n {
        unsafe { s0 += *a.get_unchecked(i) * *b.get_unchecked(i); }
    }
    s0 + s1 + s2 + s3
}

/// Compute the L2 norm of a vector.
fn norm2(v: &[f64]) -> f64 {
    dot(v, v).sqrt()
}

impl LinearSolverTrait for CG {
    fn solve(&mut self, a: &SparseMatrix, b: &[f64], x: &mut [f64]) -> Result<SolverStats> {
        let n = a.nrows;
        if b.len() != n || x.len() != n {
            return Err(LinalgError::DimensionMismatch(format!(
                "Matrix is {}x{}, b has {} elements, x has {} elements",
                a.nrows, a.ncols, b.len(), x.len()
            )));
        }

        let b_norm = norm2(b);
        if b_norm == 0.0 {
            // b = 0 => x = 0 is the solution.
            for xi in x.iter_mut() {
                *xi = 0.0;
            }
            return Ok(SolverStats {
                iterations: 0,
                final_residual: 0.0,
                converged: true,
            });
        }

        self.ensure_workspace(n);
        let r = &mut self.ws_r;
        let ax = &mut self.ws_ax;
        let p = &mut self.ws_p;
        let ap = &mut self.ws_ap;

        // r = b - A * x, p = r, compute rTr (fused)
        a.spmv(x, ax)?;
        let mut r_dot_r = 0.0;
        for i in 0..n {
            let ri = b[i] - ax[i];
            r[i] = ri;
            p[i] = ri;
            r_dot_r += ri * ri;
        }

        for iter in 0..self.max_iter {
            // Check convergence.
            let res_norm = r_dot_r.sqrt();
            if res_norm / b_norm < self.tol {
                return Ok(SolverStats {
                    iterations: iter,
                    final_residual: res_norm / b_norm,
                    converged: true,
                });
            }

            // ap = A * p
            a.spmv(&p, ap)?;

            // alpha = rTr / (p^T * A * p)
            let p_dot_ap = dot(&p, &ap);
            if p_dot_ap.abs() < 1e-300 {
                return Err(LinalgError::SingularMatrix(
                    "CG breakdown: p^T A p is zero".to_string(),
                ));
            }
            let alpha = r_dot_r / p_dot_ap;

            // Fused update: x += alpha*p, r -= alpha*Ap, compute rTr_new
            let mut r_dot_r_new = 0.0;
            for i in 0..n {
                unsafe {
                    *x.get_unchecked_mut(i) += alpha * *p.get_unchecked(i);
                    *r.get_unchecked_mut(i) -= alpha * *ap.get_unchecked(i);
                    r_dot_r_new += *r.get_unchecked(i) * *r.get_unchecked(i);
                }
            }

            let beta = r_dot_r_new / r_dot_r;

            // p = r + beta * p
            for i in 0..n {
                unsafe {
                    *p.get_unchecked_mut(i) = *r.get_unchecked(i) + beta * *p.get_unchecked(i);
                }
            }

            r_dot_r = r_dot_r_new;
        }

        let final_residual = r_dot_r.sqrt() / b_norm;
        Ok(SolverStats {
            iterations: self.max_iter,
            final_residual,
            converged: false,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Build a simple SPD tridiagonal matrix: [2 -1 0; -1 2 -1; 0 -1 2]
    fn make_spd_system() -> (SparseMatrix, Vec<f64>) {
        let row_ptr = vec![0, 2, 5, 7];
        let col_idx = vec![0, 1, 0, 1, 2, 1, 2];
        let values = vec![2.0, -1.0, -1.0, 2.0, -1.0, -1.0, 2.0];
        let a = SparseMatrix::new(3, 3, row_ptr, col_idx, values).unwrap();
        let b = vec![1.0, 0.0, 1.0];
        (a, b)
    }

    #[test]
    fn cg_solves_spd_system() {
        let (a, b) = make_spd_system();
        let mut x = vec![0.0; 3];
        let mut solver = CG::new(1e-10, 100);
        let stats = solver.solve(&a, &b, &mut x).unwrap();
        assert!(stats.converged);

        // Verify: A*x should equal b.
        let mut ax = vec![0.0; 3];
        a.spmv(&x, &mut ax).unwrap();
        for i in 0..3 {
            assert!((ax[i] - b[i]).abs() < 1e-8, "Mismatch at index {}: {} vs {}", i, ax[i], b[i]);
        }
    }

    #[test]
    fn cg_zero_rhs() {
        let (a, _) = make_spd_system();
        let b = vec![0.0; 3];
        let mut x = vec![1.0, 2.0, 3.0];
        let mut solver = CG::new(1e-10, 100);
        let stats = solver.solve(&a, &b, &mut x).unwrap();
        assert!(stats.converged);
        for &xi in &x {
            assert!(xi.abs() < 1e-12);
        }
    }

    #[test]
    fn cg_larger_system() {
        // 5x5 SPD tridiagonal.
        let n = 5;
        let mut row_ptr = vec![0usize];
        let mut col_idx = Vec::new();
        let mut values = Vec::new();

        for i in 0..n {
            if i > 0 {
                col_idx.push(i - 1);
                values.push(-1.0);
            }
            col_idx.push(i);
            values.push(3.0);
            if i < n - 1 {
                col_idx.push(i + 1);
                values.push(-1.0);
            }
            row_ptr.push(col_idx.len());
        }

        let a = SparseMatrix::new(n, n, row_ptr, col_idx, values).unwrap();
        let b = vec![1.0; n];
        let mut x = vec![0.0; n];

        let mut solver = CG::new(1e-10, 200);
        let stats = solver.solve(&a, &b, &mut x).unwrap();
        assert!(stats.converged);

        let mut ax = vec![0.0; n];
        a.spmv(&x, &mut ax).unwrap();
        for i in 0..n {
            assert!((ax[i] - b[i]).abs() < 1e-8);
        }
    }
}
