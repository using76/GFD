//! Bi-Conjugate Gradient Stabilized (BiCGSTAB) solver for non-symmetric systems.

use gfd_core::{SparseMatrix, SolverStats};
use crate::{LinalgError, Result};
use crate::traits::LinearSolverTrait;

/// BiCGSTAB solver.
///
/// Solves A * x = b for general (non-symmetric) sparse matrices.
/// Reuses workspace vectors across calls for the same system size.
#[derive(Debug, Clone)]
pub struct BiCGSTAB {
    /// Convergence tolerance for the relative residual norm ||r||/||b||.
    pub tol: f64,
    /// Maximum number of iterations.
    pub max_iter: usize,
    /// Cached workspace vectors (reused across solve calls).
    ws_n: usize,
    ws_r: Vec<f64>,
    ws_ax: Vec<f64>,
    ws_r_hat: Vec<f64>,
    ws_v: Vec<f64>,
    ws_p: Vec<f64>,
    ws_s: Vec<f64>,
    ws_t: Vec<f64>,
}

impl BiCGSTAB {
    /// Creates a new BiCGSTAB solver.
    pub fn new(tol: f64, max_iter: usize) -> Self {
        Self {
            tol, max_iter,
            ws_n: 0, ws_r: Vec::new(), ws_ax: Vec::new(), ws_r_hat: Vec::new(),
            ws_v: Vec::new(), ws_p: Vec::new(), ws_s: Vec::new(), ws_t: Vec::new(),
        }
    }

    /// Ensure workspace is allocated for the given size.
    fn ensure_workspace(&mut self, n: usize) {
        if self.ws_n == n { return; }
        self.ws_r = vec![0.0; n];
        self.ws_ax = vec![0.0; n];
        self.ws_r_hat = vec![0.0; n];
        self.ws_v = vec![0.0; n];
        self.ws_p = vec![0.0; n];
        self.ws_s = vec![0.0; n];
        self.ws_t = vec![0.0; n];
        self.ws_n = n;
    }
}

impl Default for BiCGSTAB {
    fn default() -> Self {
        Self::new(1e-6, 1000)
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

        // Reuse workspace vectors (avoid per-call allocation)
        self.ensure_workspace(n);
        let r = &mut self.ws_r;
        let ax = &mut self.ws_ax;
        let r_hat = &mut self.ws_r_hat;
        let v = &mut self.ws_v;
        let p = &mut self.ws_p;
        let s = &mut self.ws_s;
        let t = &mut self.ws_t;

        // r = b - A * x
        a.spmv(x, ax)?;
        for i in 0..n {
            r[i] = b[i] - ax[i];
        }

        // r_hat = r
        r_hat.copy_from_slice(r);

        let mut rho = 1.0_f64;
        let mut alpha = 1.0_f64;
        let mut omega = 1.0_f64;

        // Reset iteration vectors
        v.iter_mut().for_each(|x| *x = 0.0);
        p.iter_mut().for_each(|x| *x = 0.0);

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
                unsafe {
                    *p.get_unchecked_mut(i) = *r.get_unchecked(i) + beta * (*p.get_unchecked(i) - omega * *v.get_unchecked(i));
                }
            }

            // v = A * p
            a.spmv(&p, v)?;

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
                unsafe {
                    *s.get_unchecked_mut(i) = *r.get_unchecked(i) - alpha * *v.get_unchecked(i);
                }
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
            a.spmv(&s, t)?;

            // Fused: compute (t^T s), (t^T t) in one pass
            let mut ts = 0.0;
            let mut tt = 0.0;
            for i in 0..n {
                unsafe {
                    let ti = *t.get_unchecked(i);
                    ts += ti * *s.get_unchecked(i);
                    tt += ti * ti;
                }
            }
            if tt.abs() < 1e-300 {
                return Err(LinalgError::SingularMatrix(
                    "BiCGSTAB breakdown: t^T t is zero".to_string(),
                ));
            }
            omega = ts / tt;

            // Fused: x += alpha*p + omega*s, r = s - omega*t
            for i in 0..n {
                unsafe {
                    *x.get_unchecked_mut(i) += alpha * *p.get_unchecked(i) + omega * *s.get_unchecked(i);
                    *r.get_unchecked_mut(i) = *s.get_unchecked(i) - omega * *t.get_unchecked(i);
                }
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
