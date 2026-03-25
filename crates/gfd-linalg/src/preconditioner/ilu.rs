//! ILU(0) — Incomplete LU factorization with zero fill-in.
//!
//! Computes L and U factors that preserve the sparsity pattern of A.
//! For each row k:
//!   For each j < k where a(k,j) != 0:
//!     a(k,j) = a(k,j) / a(j,j)
//!     For each m > j where a(j,m) != 0 AND a(k,m) != 0:
//!       a(k,m) -= a(k,j) * a(j,m)

use gfd_core::SparseMatrix;
use crate::{LinalgError, Result};
use crate::traits::PreconditionerTrait;

/// ILU(0) preconditioner.
///
/// Stores the factored LU in a single modified CSR matrix where
/// L is in the lower triangle (with implicit unit diagonal) and
/// U is in the upper triangle (including diagonal).
#[derive(Debug, Clone)]
pub struct ILU0 {
    /// Combined L+U values in the same sparsity pattern as A.
    lu_values: Vec<f64>,
    /// CSR row pointers (same as A).
    row_ptr: Vec<usize>,
    /// CSR column indices (same as A).
    col_idx: Vec<usize>,
    /// Diagonal entry positions in the values array (for fast access).
    diag_ptr: Vec<usize>,
    /// Matrix dimension.
    n: usize,
}

impl ILU0 {
    pub fn new() -> Self {
        Self {
            lu_values: Vec::new(),
            row_ptr: Vec::new(),
            col_idx: Vec::new(),
            diag_ptr: Vec::new(),
            n: 0,
        }
    }
}

impl Default for ILU0 {
    fn default() -> Self {
        Self::new()
    }
}

impl PreconditionerTrait for ILU0 {
    fn setup(&mut self, a: &SparseMatrix) -> Result<()> {
        self.n = a.nrows;
        self.row_ptr = a.row_ptr.clone();
        self.col_idx = a.col_idx.clone();
        self.lu_values = a.values.clone();

        // Find diagonal pointers
        self.diag_ptr = vec![0; self.n];
        for i in 0..self.n {
            let mut found = false;
            for idx in self.row_ptr[i]..self.row_ptr[i + 1] {
                if self.col_idx[idx] == i {
                    self.diag_ptr[i] = idx;
                    found = true;
                    break;
                }
            }
            if !found {
                return Err(LinalgError::SingularMatrix(
                    format!("No diagonal entry in row {}", i),
                ));
            }
        }

        // Build a column-lookup for fast "does position (row, col) exist?" queries
        // For each row, store a map col -> index in values array
        let mut col_to_idx: Vec<std::collections::HashMap<usize, usize>> =
            Vec::with_capacity(self.n);
        for i in 0..self.n {
            let mut map = std::collections::HashMap::new();
            for idx in self.row_ptr[i]..self.row_ptr[i + 1] {
                map.insert(self.col_idx[idx], idx);
            }
            col_to_idx.push(map);
        }

        // ILU(0) factorization (in-place on lu_values)
        for k in 1..self.n {
            // For each column j < k in row k (lower triangle)
            let row_start = self.row_ptr[k];
            let row_end = self.row_ptr[k + 1];

            for idx_kj in row_start..row_end {
                let j = self.col_idx[idx_kj];
                if j >= k {
                    break; // past the diagonal, done with L part of this row
                }

                // a(k,j) = a(k,j) / a(j,j)
                let diag_j = self.lu_values[self.diag_ptr[j]];
                if diag_j.abs() < 1e-30 {
                    continue; // skip near-zero diagonal
                }
                self.lu_values[idx_kj] /= diag_j;
                let l_kj = self.lu_values[idx_kj];

                // For each column m > j in row j (upper triangle of row j)
                for idx_jm in self.row_ptr[j]..self.row_ptr[j + 1] {
                    let m = self.col_idx[idx_jm];
                    if m <= j {
                        continue; // only upper triangle
                    }

                    // Check if position (k, m) exists in the sparsity pattern
                    if let Some(&idx_km) = col_to_idx[k].get(&m) {
                        // a(k,m) -= l(k,j) * u(j,m)
                        self.lu_values[idx_km] -= l_kj * self.lu_values[idx_jm];
                    }
                    // If (k,m) doesn't exist → zero fill-in, skip (ILU(0))
                }
            }
        }

        Ok(())
    }

    fn apply(&self, r: &[f64], z: &mut [f64]) -> Result<()> {
        if r.len() != self.n || z.len() != self.n {
            return Err(LinalgError::DimensionMismatch(format!(
                "ILU apply: expected {}, got r={} z={}",
                self.n, r.len(), z.len()
            )));
        }

        // Solve L * y = r (forward substitution)
        // L has unit diagonal, lower entries stored in lu_values below diagonal
        let mut y = r.to_vec();
        for i in 0..self.n {
            for idx in self.row_ptr[i]..self.diag_ptr[i] {
                let j = self.col_idx[idx];
                y[i] -= self.lu_values[idx] * y[j];
            }
            // Unit diagonal for L: y[i] = y[i] / 1.0 (no-op)
        }

        // Solve U * z = y (backward substitution)
        // U has non-unit diagonal, upper entries stored at and above diagonal
        for i in (0..self.n).rev() {
            z[i] = y[i];
            for idx in (self.diag_ptr[i] + 1)..self.row_ptr[i + 1] {
                let j = self.col_idx[idx];
                z[i] -= self.lu_values[idx] * z[j];
            }
            let diag = self.lu_values[self.diag_ptr[i]];
            if diag.abs() < 1e-30 {
                z[i] = 0.0;
            } else {
                z[i] /= diag;
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::traits::PreconditionerTrait;

    fn make_test_matrix() -> SparseMatrix {
        // 3x3 SPD matrix:
        // [ 4 -1  0]
        // [-1  4 -1]
        // [ 0 -1  4]
        SparseMatrix::new(
            3, 3,
            vec![0, 2, 5, 7],
            vec![0, 1, 0, 1, 2, 1, 2],
            vec![4.0, -1.0, -1.0, 4.0, -1.0, -1.0, 4.0],
        )
        .unwrap()
    }

    #[test]
    fn test_ilu0_setup() {
        let a = make_test_matrix();
        let mut ilu = ILU0::new();
        ilu.setup(&a).unwrap();
        assert_eq!(ilu.n, 3);
        assert_eq!(ilu.diag_ptr.len(), 3);
    }

    #[test]
    fn test_ilu0_apply_identity_residual() {
        // For a diagonal matrix, ILU(0) = exact LU = D
        let a = SparseMatrix::new(
            3, 3,
            vec![0, 1, 2, 3],
            vec![0, 1, 2],
            vec![2.0, 3.0, 4.0],
        )
        .unwrap();

        let mut ilu = ILU0::new();
        ilu.setup(&a).unwrap();

        let r = vec![2.0, 6.0, 12.0];
        let mut z = vec![0.0; 3];
        ilu.apply(&r, &mut z).unwrap();

        // z should be A^{-1} * r = [1.0, 2.0, 3.0]
        assert!((z[0] - 1.0).abs() < 1e-10);
        assert!((z[1] - 2.0).abs() < 1e-10);
        assert!((z[2] - 3.0).abs() < 1e-10);
    }

    #[test]
    fn test_ilu0_tridiagonal() {
        let a = make_test_matrix();
        let mut ilu = ILU0::new();
        ilu.setup(&a).unwrap();

        // Apply to r = [1, 0, 0]
        let r = vec![1.0, 0.0, 0.0];
        let mut z = vec![0.0; 3];
        ilu.apply(&r, &mut z).unwrap();

        // Verify L*U*z ≈ r
        let mut lu_z = vec![0.0; 3];
        a.spmv(&z, &mut lu_z).unwrap();
        // For tridiagonal, ILU(0) = exact LU, so residual should be ~0
        for i in 0..3 {
            assert!(
                (lu_z[i] - r[i]).abs() < 1e-10,
                "row {}: {} != {}",
                i,
                lu_z[i],
                r[i]
            );
        }
    }
}
