//! Cholesky factorization direct solver for SPD matrices.
//!
//! Performs LL^T decomposition using dense storage converted from CSR.
//! Suitable for small-to-medium SPD systems.

use gfd_core::{SparseMatrix, SolverStats};
use crate::{LinalgError, Result};
use crate::traits::LinearSolverTrait;

/// Cholesky factorization solver for SPD matrices.
///
/// Converts the sparse matrix to dense, performs LL^T factorization,
/// then solves via forward/backward substitution. Best for small systems
/// where fill-in makes sparse factorization impractical to implement simply.
#[derive(Debug, Clone)]
pub struct Cholesky;

impl Cholesky {
    /// Creates a new Cholesky solver.
    pub fn new() -> Self {
        Self
    }
}

impl Default for Cholesky {
    fn default() -> Self {
        Self::new()
    }
}

impl LinearSolverTrait for Cholesky {
    fn solve(&mut self, a: &SparseMatrix, b: &[f64], x: &mut [f64]) -> Result<SolverStats> {
        let n = a.nrows;
        if a.nrows != a.ncols {
            return Err(LinalgError::DimensionMismatch(format!(
                "Cholesky requires square matrix, got {}x{}",
                a.nrows, a.ncols
            )));
        }
        if b.len() != n || x.len() != n {
            return Err(LinalgError::DimensionMismatch(format!(
                "Matrix is {}x{}, b has {} elements, x has {} elements",
                a.nrows, a.ncols, b.len(), x.len()
            )));
        }

        if n == 0 {
            return Ok(SolverStats {
                iterations: 0,
                final_residual: 0.0,
                converged: true,
            });
        }

        // Convert CSR to dense column-major storage for L factor.
        // We only need the lower triangle of A.
        // L is stored row-major: l[i * n + j] = L(i, j) for j <= i.
        let mut l = vec![0.0; n * n];

        // Extract dense matrix from CSR (lower triangle, symmetrize).
        let mut dense = vec![0.0; n * n];
        for i in 0..n {
            for idx in a.row_ptr[i]..a.row_ptr[i + 1] {
                let j = a.col_idx[idx];
                dense[i * n + j] = a.values[idx];
            }
        }

        // LL^T factorization (row-oriented Cholesky).
        for i in 0..n {
            for j in 0..=i {
                let mut sum = dense[i * n + j];

                for k in 0..j {
                    sum -= l[i * n + k] * l[j * n + k];
                }

                if i == j {
                    if sum <= 0.0 {
                        return Err(LinalgError::SingularMatrix(format!(
                            "Cholesky: matrix is not positive definite (negative pivot {} at row {})",
                            sum, i
                        )));
                    }
                    l[i * n + j] = sum.sqrt();
                } else {
                    let l_jj = l[j * n + j];
                    if l_jj.abs() < 1e-300 {
                        return Err(LinalgError::SingularMatrix(format!(
                            "Cholesky: zero diagonal at row {}",
                            j
                        )));
                    }
                    l[i * n + j] = sum / l_jj;
                }
            }
        }

        // Forward substitution: L * y = b
        let mut y = vec![0.0; n];
        for i in 0..n {
            let mut sum = b[i];
            for j in 0..i {
                sum -= l[i * n + j] * y[j];
            }
            let l_ii = l[i * n + i];
            if l_ii.abs() < 1e-300 {
                return Err(LinalgError::SingularMatrix(format!(
                    "Cholesky: zero L diagonal at row {}",
                    i
                )));
            }
            y[i] = sum / l_ii;
        }

        // Backward substitution: L^T * x = y
        for i in (0..n).rev() {
            let mut sum = y[i];
            for j in (i + 1)..n {
                sum -= l[j * n + i] * x[j]; // L^T(i,j) = L(j,i)
            }
            let l_ii = l[i * n + i];
            x[i] = sum / l_ii;
        }

        // Compute residual for stats.
        let mut residual = 0.0;
        let mut b_norm_sq = 0.0;
        let mut ax_val;
        for i in 0..n {
            ax_val = 0.0;
            for idx in a.row_ptr[i]..a.row_ptr[i + 1] {
                ax_val += a.values[idx] * x[a.col_idx[idx]];
            }
            let diff = b[i] - ax_val;
            residual += diff * diff;
            b_norm_sq += b[i] * b[i];
        }

        let final_residual = if b_norm_sq > 0.0 {
            (residual / b_norm_sq).sqrt()
        } else {
            residual.sqrt()
        };

        Ok(SolverStats {
            iterations: 0, // Direct solver: no iterations.
            final_residual,
            converged: true,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cholesky_solves_spd_system() {
        // 3x3 SPD: [4 -1 0; -1 4 -1; 0 -1 4]
        let row_ptr = vec![0, 2, 5, 7];
        let col_idx = vec![0, 1, 0, 1, 2, 1, 2];
        let values = vec![4.0, -1.0, -1.0, 4.0, -1.0, -1.0, 4.0];
        let a = SparseMatrix::new(3, 3, row_ptr, col_idx, values).unwrap();
        let b = vec![3.0, 2.0, 3.0];
        let mut x = vec![0.0; 3];

        let mut solver = Cholesky::new();
        let stats = solver.solve(&a, &b, &mut x).unwrap();
        assert!(stats.converged);

        let mut ax = vec![0.0; 3];
        a.spmv(&x, &mut ax).unwrap();
        for i in 0..3 {
            assert!(
                (ax[i] - b[i]).abs() < 1e-10,
                "Mismatch at {}: {} vs {}",
                i,
                ax[i],
                b[i]
            );
        }
    }

    #[test]
    fn cholesky_diagonal_matrix() {
        let row_ptr = vec![0, 1, 2, 3];
        let col_idx = vec![0, 1, 2];
        let values = vec![4.0, 9.0, 16.0];
        let a = SparseMatrix::new(3, 3, row_ptr, col_idx, values).unwrap();
        let b = vec![8.0, 27.0, 64.0];
        let mut x = vec![0.0; 3];

        let mut solver = Cholesky::new();
        let stats = solver.solve(&a, &b, &mut x).unwrap();
        assert!(stats.converged);
        assert!((x[0] - 2.0).abs() < 1e-12);
        assert!((x[1] - 3.0).abs() < 1e-12);
        assert!((x[2] - 4.0).abs() < 1e-12);
    }

    #[test]
    fn cholesky_rejects_non_spd() {
        // Negative diagonal => not positive definite.
        let row_ptr = vec![0, 1, 2];
        let col_idx = vec![0, 1];
        let values = vec![-4.0, 1.0];
        let a = SparseMatrix::new(2, 2, row_ptr, col_idx, values).unwrap();
        let b = vec![1.0, 1.0];
        let mut x = vec![0.0; 2];

        let mut solver = Cholesky::new();
        assert!(solver.solve(&a, &b, &mut x).is_err());
    }

    #[test]
    fn cholesky_identity() {
        let row_ptr = vec![0, 1, 2, 3, 4];
        let col_idx = vec![0, 1, 2, 3];
        let values = vec![1.0, 1.0, 1.0, 1.0];
        let a = SparseMatrix::new(4, 4, row_ptr, col_idx, values).unwrap();
        let b = vec![1.0, 2.0, 3.0, 4.0];
        let mut x = vec![0.0; 4];

        let mut solver = Cholesky::new();
        solver.solve(&a, &b, &mut x).unwrap();
        for i in 0..4 {
            assert!((x[i] - b[i]).abs() < 1e-12);
        }
    }
}
