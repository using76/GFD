//! LU factorization direct solver with partial pivoting.
//!
//! Converts the sparse matrix to dense, performs PA = LU factorization
//! with row pivoting, then solves via forward/backward substitution.

use gfd_core::{SparseMatrix, SolverStats};
use crate::{LinalgError, Result};
use crate::traits::LinearSolverTrait;

/// LU factorization solver with partial pivoting.
///
/// Converts the sparse matrix to dense, performs LU decomposition with
/// partial (row) pivoting for numerical stability. Suitable for
/// small-to-medium general (non-symmetric) systems.
#[derive(Debug, Clone)]
pub struct LU;

impl LU {
    /// Creates a new LU solver.
    pub fn new() -> Self {
        Self
    }
}

impl Default for LU {
    fn default() -> Self {
        Self::new()
    }
}

impl LinearSolverTrait for LU {
    fn solve(&mut self, a: &SparseMatrix, b: &[f64], x: &mut [f64]) -> Result<SolverStats> {
        let n = a.nrows;
        if a.nrows != a.ncols {
            return Err(LinalgError::DimensionMismatch(format!(
                "LU requires square matrix, got {}x{}",
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

        // Convert CSR to dense row-major: dense[i * n + j] = A(i, j).
        let mut dense = vec![0.0; n * n];
        for i in 0..n {
            for idx in a.row_ptr[i]..a.row_ptr[i + 1] {
                dense[i * n + a.col_idx[idx]] = a.values[idx];
            }
        }

        // Pivot vector: piv[i] = row index after permutation.
        let mut piv: Vec<usize> = (0..n).collect();

        // LU factorization with partial pivoting (Doolittle, in-place on dense).
        for k in 0..n {
            // Find pivot: row with largest |a(i,k)| for i >= k.
            let mut max_val = dense[piv[k] * n + k].abs();
            let mut max_row = k;
            for i in (k + 1)..n {
                let val = dense[piv[i] * n + k].abs();
                if val > max_val {
                    max_val = val;
                    max_row = i;
                }
            }

            if max_val < 1e-300 {
                return Err(LinalgError::SingularMatrix(format!(
                    "LU: singular matrix (zero pivot at column {})",
                    k
                )));
            }

            // Swap pivot indices.
            if max_row != k {
                piv.swap(k, max_row);
            }

            let pivot_row = piv[k];
            let pivot_val = dense[pivot_row * n + k];

            // Eliminate below pivot.
            for i in (k + 1)..n {
                let row_i = piv[i];
                let factor = dense[row_i * n + k] / pivot_val;
                dense[row_i * n + k] = factor; // Store L factor in-place.

                for j in (k + 1)..n {
                    dense[row_i * n + j] -= factor * dense[pivot_row * n + j];
                }
            }
        }

        // Forward substitution: L * y = P * b.
        // L has unit diagonal; L(i,j) for j < i is stored in dense[piv[i]*n + j].
        let mut y = vec![0.0; n];
        for i in 0..n {
            let row_i = piv[i];
            let mut sum = b[row_i];
            for j in 0..i {
                sum -= dense[row_i * n + j] * y[j];
            }
            y[i] = sum; // Unit diagonal, no division needed.
        }

        // Backward substitution: U * x = y.
        // U(i,j) for j >= i is stored in dense[piv[i]*n + j].
        for i in (0..n).rev() {
            let row_i = piv[i];
            let mut sum = y[i];
            for j in (i + 1)..n {
                sum -= dense[row_i * n + j] * x[j];
            }
            let diag = dense[row_i * n + i];
            if diag.abs() < 1e-300 {
                return Err(LinalgError::SingularMatrix(format!(
                    "LU: zero diagonal in U at row {}",
                    i
                )));
            }
            x[i] = sum / diag;
        }

        // Compute residual for stats.
        let mut residual = 0.0;
        let mut b_norm_sq = 0.0;
        for i in 0..n {
            let mut ax_val = 0.0;
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
    fn lu_solves_spd_system() {
        // 3x3 SPD: [4 -1 0; -1 4 -1; 0 -1 4]
        let row_ptr = vec![0, 2, 5, 7];
        let col_idx = vec![0, 1, 0, 1, 2, 1, 2];
        let values = vec![4.0, -1.0, -1.0, 4.0, -1.0, -1.0, 4.0];
        let a = SparseMatrix::new(3, 3, row_ptr, col_idx, values).unwrap();
        let b = vec![3.0, 2.0, 3.0];
        let mut x = vec![0.0; 3];

        let mut solver = LU::new();
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
    fn lu_solves_nonsymmetric() {
        // Non-symmetric 3x3:
        // [4 -1  0]
        // [-2  4 -1]
        // [ 0 -1  3]
        let row_ptr = vec![0, 2, 5, 7];
        let col_idx = vec![0, 1, 0, 1, 2, 1, 2];
        let values = vec![4.0, -1.0, -2.0, 4.0, -1.0, -1.0, 3.0];
        let a = SparseMatrix::new(3, 3, row_ptr, col_idx, values).unwrap();
        let b = vec![3.0, 1.0, 2.0];
        let mut x = vec![0.0; 3];

        let mut solver = LU::new();
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
    fn lu_needs_pivoting() {
        // Matrix where first pivot is zero, requiring pivoting:
        // [0  1]   =>  with pivoting, swap rows => [1 0; 0 1] style
        // [1  0]
        let row_ptr = vec![0, 1, 2];
        let col_idx = vec![1, 0];
        let values = vec![1.0, 1.0];
        let a = SparseMatrix::new(2, 2, row_ptr, col_idx, values).unwrap();
        let b = vec![3.0, 5.0];
        let mut x = vec![0.0; 2];

        let mut solver = LU::new();
        let stats = solver.solve(&a, &b, &mut x).unwrap();
        assert!(stats.converged);
        // x should be [5, 3]
        assert!((x[0] - 5.0).abs() < 1e-10);
        assert!((x[1] - 3.0).abs() < 1e-10);
    }

    #[test]
    fn lu_singular_matrix() {
        // Singular: [1 1; 1 1]
        let row_ptr = vec![0, 2, 4];
        let col_idx = vec![0, 1, 0, 1];
        let values = vec![1.0, 1.0, 1.0, 1.0];
        let a = SparseMatrix::new(2, 2, row_ptr, col_idx, values).unwrap();
        let b = vec![1.0, 1.0];
        let mut x = vec![0.0; 2];

        let mut solver = LU::new();
        assert!(solver.solve(&a, &b, &mut x).is_err());
    }

    #[test]
    fn lu_identity() {
        let row_ptr = vec![0, 1, 2, 3, 4];
        let col_idx = vec![0, 1, 2, 3];
        let values = vec![1.0, 1.0, 1.0, 1.0];
        let a = SparseMatrix::new(4, 4, row_ptr, col_idx, values).unwrap();
        let b = vec![1.0, 2.0, 3.0, 4.0];
        let mut x = vec![0.0; 4];

        let mut solver = LU::new();
        solver.solve(&a, &b, &mut x).unwrap();
        for i in 0..4 {
            assert!((x[i] - b[i]).abs() < 1e-12);
        }
    }
}
