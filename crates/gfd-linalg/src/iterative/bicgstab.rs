//! Bi-Conjugate Gradient Stabilized (BiCGSTAB) solver for non-symmetric systems.

use gfd_core::{SparseMatrix, SolverStats};
use crate::{LinalgError, Result};
use crate::traits::LinearSolverTrait;

/// BiCGSTAB solver.
///
/// Solves A * x = b for general (non-symmetric) sparse matrices.
#[derive(Debug, Clone)]
pub struct BiCGSTAB {
    /// Convergence tolerance for the relative residual norm ||r||/||b||.
    pub tol: f64,
    /// Maximum number of iterations.
    pub max_iter: usize,
}

impl BiCGSTAB {
    /// Creates a new BiCGSTAB solver.
    pub fn new(tol: f64, max_iter: usize) -> Self {
        Self { tol, max_iter }
    }
}

impl Default for BiCGSTAB {
    fn default() -> Self {
        Self {
            tol: 1e-6,
            max_iter: 1000,
        }
    }
}

/// Dot product of two vectors (4-way unrolled for SIMD auto-vectorization).
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

/// L2 norm.
fn norm2(v: &[f64]) -> f64 {
    dot(v, v).sqrt()
}

impl LinearSolverTrait for BiCGSTAB {
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
            for xi in x.iter_mut() {
                *xi = 0.0;
            }
            return Ok(SolverStats {
                iterations: 0,
                final_residual: 0.0,
                converged: true,
            });
        }

        // r = b - A * x
        let mut r = vec![0.0; n];
        let mut ax = vec![0.0; n];
        a.spmv(x, &mut ax)?;
        for i in 0..n {
            r[i] = b[i] - ax[i];
        }

        // r_hat = r (arbitrary choice, but r_hat^T r != 0 required)
        let r_hat = r.clone();

        let mut rho = 1.0_f64;
        let mut alpha = 1.0_f64;
        let mut omega = 1.0_f64;

        let mut v = vec![0.0; n];
        let mut p = vec![0.0; n];
        let mut s = vec![0.0; n];
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
                return Err(LinalgError::SingularMatrix(
                    "BiCGSTAB breakdown: rho is zero".to_string(),
                ));
            }

            let beta = (rho_new / rho) * (alpha / omega);

            // p = r + beta * (p - omega * v)
            for i in 0..n {
                p[i] = r[i] + beta * (p[i] - omega * v[i]);
            }

            // v = A * p
            a.spmv(&p, &mut v)?;

            // alpha = rho_new / (r_hat^T * v)
            let r_hat_dot_v = dot(&r_hat, &v);
            if r_hat_dot_v.abs() < 1e-300 {
                return Err(LinalgError::SingularMatrix(
                    "BiCGSTAB breakdown: r_hat^T v is zero".to_string(),
                ));
            }
            alpha = rho_new / r_hat_dot_v;

            // s = r - alpha * v
            for i in 0..n {
                s[i] = r[i] - alpha * v[i];
            }

            // Check if s is small enough (early termination).
            let s_norm = norm2(&s);
            if s_norm / b_norm < self.tol {
                // x = x + alpha * p
                for i in 0..n {
                    x[i] += alpha * p[i];
                }
                return Ok(SolverStats {
                    iterations: iter + 1,
                    final_residual: s_norm / b_norm,
                    converged: true,
                });
            }

            // t = A * s
            a.spmv(&s, &mut t)?;

            // omega = (t^T s) / (t^T t)
            let t_dot_t = dot(&t, &t);
            if t_dot_t.abs() < 1e-300 {
                return Err(LinalgError::SingularMatrix(
                    "BiCGSTAB breakdown: t^T t is zero".to_string(),
                ));
            }
            omega = dot(&t, &s) / t_dot_t;

            // x = x + alpha * p + omega * s
            for i in 0..n {
                x[i] += alpha * p[i] + omega * s[i];
            }

            // r = s - omega * t
            for i in 0..n {
                r[i] = s[i] - omega * t[i];
            }

            if omega.abs() < 1e-300 {
                return Err(LinalgError::SingularMatrix(
                    "BiCGSTAB breakdown: omega is zero".to_string(),
                ));
            }

            rho = rho_new;
        }

        let final_residual = norm2(&r) / b_norm;
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

    fn make_test_system() -> (SparseMatrix, Vec<f64>) {
        // 3x3: [4 -1 0; -1 4 -1; 0 -1 4]
        let row_ptr = vec![0, 2, 5, 7];
        let col_idx = vec![0, 1, 0, 1, 2, 1, 2];
        let values = vec![4.0, -1.0, -1.0, 4.0, -1.0, -1.0, 4.0];
        let a = SparseMatrix::new(3, 3, row_ptr, col_idx, values).unwrap();
        let b = vec![3.0, 2.0, 3.0];
        (a, b)
    }

    #[test]
    fn bicgstab_solves_system() {
        let (a, b) = make_test_system();
        let mut x = vec![0.0; 3];
        let mut solver = BiCGSTAB::new(1e-10, 100);
        let stats = solver.solve(&a, &b, &mut x).unwrap();
        assert!(stats.converged);

        // Verify: A*x should equal b.
        let mut ax = vec![0.0; 3];
        a.spmv(&x, &mut ax).unwrap();
        for i in 0..3 {
            assert!(
                (ax[i] - b[i]).abs() < 1e-8,
                "Mismatch at {}: {} vs {}",
                i, ax[i], b[i]
            );
        }
    }

    #[test]
    fn bicgstab_zero_rhs() {
        let (a, _) = make_test_system();
        let b = vec![0.0; 3];
        let mut x = vec![1.0, 2.0, 3.0];
        let mut solver = BiCGSTAB::new(1e-10, 100);
        let stats = solver.solve(&a, &b, &mut x).unwrap();
        assert!(stats.converged);
        for &xi in &x {
            assert!(xi.abs() < 1e-12);
        }
    }

    #[test]
    fn bicgstab_nonsymmetric() {
        // Non-symmetric 3x3 matrix.
        let row_ptr = vec![0, 2, 5, 7];
        let col_idx = vec![0, 1, 0, 1, 2, 1, 2];
        let values = vec![4.0, -1.0, -2.0, 4.0, -1.0, -1.0, 3.0];
        let a = SparseMatrix::new(3, 3, row_ptr, col_idx, values).unwrap();
        let b = vec![3.0, 1.0, 2.0];
        let mut x = vec![0.0; 3];

        let mut solver = BiCGSTAB::new(1e-10, 100);
        let stats = solver.solve(&a, &b, &mut x).unwrap();
        assert!(stats.converged);

        let mut ax = vec![0.0; 3];
        a.spmv(&x, &mut ax).unwrap();
        for i in 0..3 {
            assert!((ax[i] - b[i]).abs() < 1e-8);
        }
    }
}
