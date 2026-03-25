//! Generalized Minimal Residual (GMRES) solver with restarts.

use gfd_core::{SparseMatrix, SolverStats};
use crate::{LinalgError, Result};
use crate::traits::LinearSolverTrait;

/// GMRES(m) solver with restart parameter m.
///
/// Solves A * x = b for general sparse matrices using the Arnoldi process
/// and Givens rotations.
#[derive(Debug, Clone)]
pub struct GMRES {
    /// Convergence tolerance for the relative residual norm.
    pub tol: f64,
    /// Maximum number of total iterations (outer * inner).
    pub max_iter: usize,
    /// Restart parameter (Krylov subspace dimension before restart).
    pub restart: usize,
}

impl GMRES {
    /// Creates a new GMRES solver.
    pub fn new(tol: f64, max_iter: usize, restart: usize) -> Self {
        Self {
            tol,
            max_iter,
            restart,
        }
    }
}

impl Default for GMRES {
    fn default() -> Self {
        Self {
            tol: 1e-6,
            max_iter: 1000,
            restart: 30,
        }
    }
}

fn dot(a: &[f64], b: &[f64]) -> f64 {
    a.iter().zip(b.iter()).map(|(x, y)| x * y).sum()
}

fn norm2(v: &[f64]) -> f64 {
    dot(v, v).sqrt()
}

/// Apply a Givens rotation to (h_i, h_j), returning the rotated pair.
fn apply_givens(cs: f64, sn: f64, h_i: f64, h_j: f64) -> (f64, f64) {
    let new_i = cs * h_i + sn * h_j;
    let new_j = -sn * h_i + cs * h_j;
    (new_i, new_j)
}

/// Generate Givens rotation coefficients to zero out h_j.
fn generate_givens(h_i: f64, h_j: f64) -> (f64, f64) {
    if h_j.abs() < 1e-300 {
        (1.0, 0.0)
    } else if h_i.abs() < 1e-300 {
        (0.0, h_j.signum())
    } else {
        let t = (h_i * h_i + h_j * h_j).sqrt();
        (h_i / t, h_j / t)
    }
}

impl LinearSolverTrait for GMRES {
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

        let m = self.restart;
        let mut total_iter = 0;

        loop {
            // Compute r = b - A * x.
            let mut r = vec![0.0; n];
            let mut ax = vec![0.0; n];
            a.spmv(x, &mut ax)?;
            for i in 0..n {
                r[i] = b[i] - ax[i];
            }

            let r_norm = norm2(&r);
            if r_norm / b_norm < self.tol {
                return Ok(SolverStats {
                    iterations: total_iter,
                    final_residual: r_norm / b_norm,
                    converged: true,
                });
            }

            if total_iter >= self.max_iter {
                return Ok(SolverStats {
                    iterations: total_iter,
                    final_residual: r_norm / b_norm,
                    converged: false,
                });
            }

            // V: orthonormal basis vectors (m+1 vectors of length n).
            let mut v_basis: Vec<Vec<f64>> = Vec::with_capacity(m + 1);
            // First basis vector: v_0 = r / ||r||.
            let mut v0 = r;
            for vi in v0.iter_mut() {
                *vi /= r_norm;
            }
            v_basis.push(v0);

            // Upper Hessenberg matrix H (stored column-major, (m+1) x m).
            let mut h = vec![vec![0.0; m + 1]; m]; // h[j][i] = H(i,j)

            // Givens rotation parameters.
            let mut cs = vec![0.0; m];
            let mut sn = vec![0.0; m];

            // g = ||r|| * e_1.
            let mut g = vec![0.0; m + 1];
            g[0] = r_norm;

            let mut j = 0;
            while j < m && total_iter < self.max_iter {
                // w = A * v_j.
                let mut w = vec![0.0; n];
                a.spmv(&v_basis[j], &mut w)?;

                // Arnoldi: orthogonalize w against v_0..v_j.
                for i in 0..=j {
                    h[j][i] = dot(&w, &v_basis[i]);
                    for k in 0..n {
                        w[k] -= h[j][i] * v_basis[i][k];
                    }
                }
                h[j][j + 1] = norm2(&w);

                if h[j][j + 1].abs() > 1e-300 {
                    let inv = 1.0 / h[j][j + 1];
                    for wi in w.iter_mut() {
                        *wi *= inv;
                    }
                    v_basis.push(w);
                } else {
                    // Lucky breakdown: w is in the span of V.
                    v_basis.push(vec![0.0; n]);
                }

                // Apply previous Givens rotations to the new column of H.
                for i in 0..j {
                    let (new_i, new_j) = apply_givens(cs[i], sn[i], h[j][i], h[j][i + 1]);
                    h[j][i] = new_i;
                    h[j][i + 1] = new_j;
                }

                // Generate new Givens rotation to zero out H(j+1, j).
                let (c, s) = generate_givens(h[j][j], h[j][j + 1]);
                cs[j] = c;
                sn[j] = s;
                let (new_jj, new_jj1) = apply_givens(cs[j], sn[j], h[j][j], h[j][j + 1]);
                h[j][j] = new_jj;
                h[j][j + 1] = new_jj1;

                // Apply to g.
                let (new_gj, new_gj1) = apply_givens(cs[j], sn[j], g[j], g[j + 1]);
                g[j] = new_gj;
                g[j + 1] = new_gj1;

                total_iter += 1;
                j += 1;

                // Check convergence.
                if g[j].abs() / b_norm < self.tol {
                    break;
                }
            }

            // Back-substitution: solve H * y = g (upper triangular part).
            let k = j;
            let mut y = vec![0.0; k];
            for i in (0..k).rev() {
                y[i] = g[i];
                for l in (i + 1)..k {
                    y[i] -= h[l][i] * y[l];
                }
                if h[i][i].abs() < 1e-300 {
                    return Err(LinalgError::SingularMatrix(
                        "GMRES: zero diagonal in Hessenberg matrix".to_string(),
                    ));
                }
                y[i] /= h[i][i];
            }

            // Update solution: x = x + V * y.
            for i in 0..k {
                for l in 0..n {
                    x[l] += y[i] * v_basis[i][l];
                }
            }

            // Check if we should stop or restart.
            if total_iter >= self.max_iter {
                break;
            }
            if j < m {
                // Converged during inner loop.
                break;
            }
            // Otherwise, restart.
        }

        // Final residual check.
        let mut r = vec![0.0; n];
        let mut ax = vec![0.0; n];
        a.spmv(x, &mut ax)?;
        for i in 0..n {
            r[i] = b[i] - ax[i];
        }
        let final_residual = norm2(&r) / b_norm;

        Ok(SolverStats {
            iterations: total_iter,
            final_residual,
            converged: final_residual < self.tol,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gmres_solves_spd_system() {
        let row_ptr = vec![0, 2, 5, 7];
        let col_idx = vec![0, 1, 0, 1, 2, 1, 2];
        let values = vec![4.0, -1.0, -1.0, 4.0, -1.0, -1.0, 4.0];
        let a = SparseMatrix::new(3, 3, row_ptr, col_idx, values).unwrap();
        let b = vec![3.0, 2.0, 3.0];
        let mut x = vec![0.0; 3];

        let mut solver = GMRES::new(1e-10, 100, 10);
        let stats = solver.solve(&a, &b, &mut x).unwrap();
        assert!(stats.converged);

        let mut ax = vec![0.0; 3];
        a.spmv(&x, &mut ax).unwrap();
        for i in 0..3 {
            assert!((ax[i] - b[i]).abs() < 1e-8);
        }
    }

    #[test]
    fn gmres_nonsymmetric() {
        let row_ptr = vec![0, 2, 5, 7];
        let col_idx = vec![0, 1, 0, 1, 2, 1, 2];
        let values = vec![4.0, -1.0, -2.0, 4.0, -1.0, -1.0, 3.0];
        let a = SparseMatrix::new(3, 3, row_ptr, col_idx, values).unwrap();
        let b = vec![3.0, 1.0, 2.0];
        let mut x = vec![0.0; 3];

        let mut solver = GMRES::new(1e-10, 100, 10);
        let stats = solver.solve(&a, &b, &mut x).unwrap();
        assert!(stats.converged);

        let mut ax = vec![0.0; 3];
        a.spmv(&x, &mut ax).unwrap();
        for i in 0..3 {
            assert!((ax[i] - b[i]).abs() < 1e-8);
        }
    }
}
