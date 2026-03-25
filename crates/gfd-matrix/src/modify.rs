//! Post-assembly modification utilities for linear systems.
//!
//! These functions provide direct access to modify the CSR matrix and RHS
//! vector after initial assembly.

use gfd_core::{LinearSystem, SparseMatrix};
use crate::{MatrixError, Result};

/// Modify the diagonal coefficient for a given row.
///
/// Finds the diagonal entry `A[row][row]` in the CSR matrix and adds `delta`
/// to it.
///
/// # Arguments
/// * `matrix` - The sparse matrix to modify.
/// * `row` - The row (and column) index of the diagonal entry.
/// * `delta` - The value to add to the diagonal.
pub fn modify_diagonal(matrix: &mut SparseMatrix, row: usize, delta: f64) -> Result<()> {
    let start = matrix.row_ptr[row];
    let end = matrix.row_ptr[row + 1];

    for idx in start..end {
        if matrix.col_idx[idx] == row {
            matrix.values[idx] += delta;
            return Ok(());
        }
    }

    Err(MatrixError::IndexOutOfBounds {
        row,
        col: row,
        nrows: matrix.nrows,
        ncols: matrix.ncols,
    })
}

/// Modify a specific coefficient `A[row][col]` by adding `delta`.
///
/// # Arguments
/// * `matrix` - The sparse matrix to modify.
/// * `row` - Row index.
/// * `col` - Column index.
/// * `delta` - Value to add to the entry.
pub fn modify_coefficient(
    matrix: &mut SparseMatrix,
    row: usize,
    col: usize,
    delta: f64,
) -> Result<()> {
    let start = matrix.row_ptr[row];
    let end = matrix.row_ptr[row + 1];

    for idx in start..end {
        if matrix.col_idx[idx] == col {
            matrix.values[idx] += delta;
            return Ok(());
        }
    }

    Err(MatrixError::IndexOutOfBounds {
        row,
        col,
        nrows: matrix.nrows,
        ncols: matrix.ncols,
    })
}

/// Add a value to the source (RHS) vector at the given row.
///
/// # Arguments
/// * `system` - The linear system to modify.
/// * `row` - Row index.
/// * `value` - Value to add to `b[row]`.
pub fn add_to_source(system: &mut LinearSystem, row: usize, value: f64) {
    system.b[row] += value;
}

/// Replace an entire equation row with the given coefficients and source.
///
/// This overwrites all entries in the specified row, setting off-diagonal
/// entries to zero for columns not in `coefficients`, and setting the
/// diagonal and specified off-diagonals.
///
/// # Arguments
/// * `system` - The linear system to modify.
/// * `row` - The row to replace.
/// * `coefficients` - Slice of (col, value) pairs for non-zero entries.
/// * `source` - New RHS value for this row.
pub fn insert_equation(
    system: &mut LinearSystem,
    row: usize,
    coefficients: &[(usize, f64)],
    source: f64,
) -> Result<()> {
    let a = &mut system.a;
    let start = a.row_ptr[row];
    let end = a.row_ptr[row + 1];

    // Zero all existing entries in this row.
    for idx in start..end {
        a.values[idx] = 0.0;
    }

    // Set the specified coefficients.
    for &(col, val) in coefficients {
        let mut found = false;
        for idx in start..end {
            if a.col_idx[idx] == col {
                a.values[idx] = val;
                found = true;
                break;
            }
        }
        if !found {
            return Err(MatrixError::IndexOutOfBounds {
                row,
                col,
                nrows: a.nrows,
                ncols: a.ncols,
            });
        }
    }

    // Set RHS.
    system.b[row] = source;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use gfd_core::SparseMatrix;

    fn make_test_matrix() -> SparseMatrix {
        let row_ptr = vec![0, 2, 5, 7];
        let col_idx = vec![0, 1, 0, 1, 2, 1, 2];
        let values = vec![4.0, -1.0, -1.0, 4.0, -1.0, -1.0, 4.0];
        SparseMatrix::new(3, 3, row_ptr, col_idx, values).unwrap()
    }

    #[test]
    fn modify_diagonal_adds() {
        let mut m = make_test_matrix();
        modify_diagonal(&mut m, 1, 2.0).unwrap();
        let diag = m.diagonal();
        assert!((diag[1] - 6.0).abs() < 1e-12);
    }

    #[test]
    fn modify_coefficient_off_diag() {
        let mut m = make_test_matrix();
        modify_coefficient(&mut m, 0, 1, -0.5).unwrap();
        // A[0][1] was -1.0, now -1.5
        let start = m.row_ptr[0];
        let end = m.row_ptr[1];
        for idx in start..end {
            if m.col_idx[idx] == 1 {
                assert!((m.values[idx] - (-1.5)).abs() < 1e-12);
            }
        }
    }

    #[test]
    fn add_to_source_works() {
        let m = make_test_matrix();
        let mut sys = LinearSystem::new(m, vec![10.0, 0.0, 10.0]);
        add_to_source(&mut sys, 1, 5.0);
        assert!((sys.b[1] - 5.0).abs() < 1e-12);
    }
}
