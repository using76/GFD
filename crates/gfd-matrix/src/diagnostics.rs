//! Matrix diagnostics: diagonal dominance checking, zero pivot detection.

use gfd_core::SparseMatrix;
use serde::{Deserialize, Serialize};

/// Result of a diagonal dominance check.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiagnosticReport {
    /// Number of rows in the matrix.
    pub nrows: usize,
    /// Diagonal dominance ratio for each row: |a_ii| / sum_j!=i |a_ij|.
    /// A ratio >= 1.0 means the row is diagonally dominant.
    pub row_ratios: Vec<f64>,
    /// Number of rows that are strictly diagonally dominant (ratio > 1.0).
    pub num_dominant: usize,
    /// Number of rows that are weakly diagonally dominant (ratio == 1.0).
    pub num_weakly_dominant: usize,
    /// Number of rows that are NOT diagonally dominant (ratio < 1.0).
    pub num_not_dominant: usize,
    /// Whether ALL rows are at least weakly diagonally dominant.
    pub is_diagonally_dominant: bool,
}

/// Check the diagonal dominance of a sparse matrix.
///
/// For each row i, computes the ratio:
///   ratio_i = |a_ii| / sum_{j != i} |a_ij|
///
/// A matrix is diagonally dominant if ratio_i >= 1.0 for all rows.
///
/// # Arguments
/// * `matrix` - The sparse matrix to check.
///
/// # Returns
/// A `DiagnosticReport` with per-row ratios and summary statistics.
pub fn check_diagonal_dominance(matrix: &SparseMatrix) -> DiagnosticReport {
    let n = matrix.nrows;
    let mut row_ratios = Vec::with_capacity(n);
    let mut num_dominant = 0;
    let mut num_weakly_dominant = 0;
    let mut num_not_dominant = 0;

    for i in 0..n {
        let start = matrix.row_ptr[i];
        let end = matrix.row_ptr[i + 1];

        let mut diag_val = 0.0_f64;
        let mut off_diag_sum = 0.0_f64;

        for idx in start..end {
            let col = matrix.col_idx[idx];
            let val = matrix.values[idx].abs();
            if col == i {
                diag_val = val;
            } else {
                off_diag_sum += val;
            }
        }

        let ratio = if off_diag_sum > 0.0 {
            diag_val / off_diag_sum
        } else if diag_val > 0.0 {
            f64::INFINITY
        } else {
            0.0
        };

        if ratio > 1.0 {
            num_dominant += 1;
        } else if (ratio - 1.0).abs() < 1e-14 {
            num_weakly_dominant += 1;
        } else {
            num_not_dominant += 1;
        }

        row_ratios.push(ratio);
    }

    let is_diagonally_dominant = num_not_dominant == 0;

    DiagnosticReport {
        nrows: n,
        row_ratios,
        num_dominant,
        num_weakly_dominant,
        num_not_dominant,
        is_diagonally_dominant,
    }
}

/// Find rows with zero (or near-zero) diagonal entries (pivots).
///
/// Returns a vector of row indices where |a_ii| < `tolerance`.
///
/// # Arguments
/// * `matrix` - The sparse matrix to check.
///
/// # Returns
/// Vector of row indices with zero or near-zero pivots.
pub fn find_zero_pivots(matrix: &SparseMatrix) -> Vec<usize> {
    let tolerance = 1e-15;
    let diagonal = matrix.diagonal();
    diagonal
        .iter()
        .enumerate()
        .filter_map(|(i, &d)| if d.abs() < tolerance { Some(i) } else { None })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_dominant_matrix() -> SparseMatrix {
        // Tridiagonal: [4 -1 0; -1 4 -1; 0 -1 4]
        let row_ptr = vec![0, 2, 5, 7];
        let col_idx = vec![0, 1, 0, 1, 2, 1, 2];
        let values = vec![4.0, -1.0, -1.0, 4.0, -1.0, -1.0, 4.0];
        SparseMatrix::new(3, 3, row_ptr, col_idx, values).unwrap()
    }

    #[test]
    fn diagonal_dominance_check() {
        let m = make_dominant_matrix();
        let report = check_diagonal_dominance(&m);
        assert!(report.is_diagonally_dominant);
        assert_eq!(report.num_not_dominant, 0);
        // Row 0: |4|/|-1| = 4.0
        assert!((report.row_ratios[0] - 4.0).abs() < 1e-12);
        // Row 1: |4|/(|-1|+|-1|) = 2.0
        assert!((report.row_ratios[1] - 2.0).abs() < 1e-12);
    }

    #[test]
    fn no_zero_pivots_in_dominant() {
        let m = make_dominant_matrix();
        let zeros = find_zero_pivots(&m);
        assert!(zeros.is_empty());
    }

    #[test]
    fn detect_zero_pivot() {
        // Matrix with zero diagonal at row 1.
        let row_ptr = vec![0, 1, 3, 4];
        let col_idx = vec![0, 0, 2, 2];
        let values = vec![1.0, -1.0, -1.0, 1.0];
        let m = SparseMatrix::new(3, 3, row_ptr, col_idx, values).unwrap();
        let zeros = find_zero_pivots(&m);
        assert_eq!(zeros, vec![1]);
    }
}
