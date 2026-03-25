//! Linear algebra types and solvers.

pub mod solvers;
pub mod preconditioners;

use serde::{Deserialize, Serialize};

use crate::{CoreError, Result};

/// A sparse matrix in Compressed Sparse Row (CSR) format.
#[derive(Debug, Clone)]
pub struct SparseMatrix {
    /// Row pointer array: row i has entries in col_idx[row_ptr[i]..row_ptr[i+1]].
    pub row_ptr: Vec<usize>,
    /// Column indices of non-zero entries.
    pub col_idx: Vec<usize>,
    /// Values of non-zero entries.
    pub values: Vec<f64>,
    /// Number of rows.
    pub nrows: usize,
    /// Number of columns.
    pub ncols: usize,
}

impl SparseMatrix {
    /// Creates a new sparse matrix from CSR components.
    pub fn new(
        nrows: usize,
        ncols: usize,
        row_ptr: Vec<usize>,
        col_idx: Vec<usize>,
        values: Vec<f64>,
    ) -> Result<Self> {
        if row_ptr.len() != nrows + 1 {
            return Err(CoreError::SparseMatrixError(format!(
                "row_ptr length {} does not match nrows + 1 = {}",
                row_ptr.len(),
                nrows + 1
            )));
        }
        if col_idx.len() != values.len() {
            return Err(CoreError::SparseMatrixError(format!(
                "col_idx length {} does not match values length {}",
                col_idx.len(),
                values.len()
            )));
        }
        Ok(Self {
            row_ptr,
            col_idx,
            values,
            nrows,
            ncols,
        })
    }

    /// Creates a zero matrix with no non-zero entries.
    pub fn zeros(nrows: usize, ncols: usize) -> Self {
        Self {
            row_ptr: vec![0; nrows + 1],
            col_idx: Vec::new(),
            values: Vec::new(),
            nrows,
            ncols,
        }
    }

    /// Returns the number of non-zero entries.
    pub fn nnz(&self) -> usize {
        self.values.len()
    }

    /// Performs sparse matrix-vector multiplication: y = A * x.
    pub fn spmv(&self, x: &[f64], y: &mut [f64]) -> Result<()> {
        if x.len() != self.ncols {
            return Err(CoreError::DimensionMismatch {
                expected: self.ncols,
                got: x.len(),
            });
        }
        if y.len() != self.nrows {
            return Err(CoreError::DimensionMismatch {
                expected: self.nrows,
                got: y.len(),
            });
        }
        // Use unchecked inner loop for performance (bounds already validated).
        let vals = &self.values;
        let cols = &self.col_idx;
        let rp = &self.row_ptr;
        for i in 0..self.nrows {
            let rs = rp[i];
            let re = rp[i + 1];
            let mut sum = 0.0;
            for idx in rs..re {
                // SAFETY: row_ptr and col_idx are validated at construction;
                // x.len() == ncols and col_idx[idx] < ncols are invariants.
                unsafe {
                    sum += *vals.get_unchecked(idx) * *x.get_unchecked(*cols.get_unchecked(idx));
                }
            }
            y[i] = sum;
        }
        Ok(())
    }

    /// Returns the diagonal entries of the matrix.
    pub fn diagonal(&self) -> Vec<f64> {
        let mut diag = vec![0.0; self.nrows.min(self.ncols)];
        for i in 0..self.nrows {
            for idx in self.row_ptr[i]..self.row_ptr[i + 1] {
                if self.col_idx[idx] == i {
                    diag[i] = self.values[idx];
                    break;
                }
            }
        }
        diag
    }
}

/// A linear system Ax = b.
#[derive(Debug, Clone)]
pub struct LinearSystem {
    /// Coefficient matrix A.
    pub a: SparseMatrix,
    /// Right-hand side vector b.
    pub b: Vec<f64>,
    /// Solution vector x.
    pub x: Vec<f64>,
}

impl LinearSystem {
    /// Creates a new linear system with a zero initial guess.
    pub fn new(a: SparseMatrix, b: Vec<f64>) -> Self {
        let n = a.nrows;
        Self {
            a,
            b,
            x: vec![0.0; n],
        }
    }

    /// Creates a new linear system with a given initial guess.
    pub fn with_initial_guess(a: SparseMatrix, b: Vec<f64>, x0: Vec<f64>) -> Self {
        Self { a, b, x: x0 }
    }

    /// Returns the dimension of the system.
    pub fn size(&self) -> usize {
        self.a.nrows
    }
}

/// Statistics from a linear solver run.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct SolverStats {
    /// Number of iterations performed.
    pub iterations: usize,
    /// Final residual norm.
    pub final_residual: f64,
    /// Whether the solver converged within tolerance.
    pub converged: bool,
}

/// Configuration for iterative linear solvers.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SolverConfig {
    /// Convergence tolerance for the relative residual norm.
    pub tolerance: f64,
    /// Maximum number of iterations allowed.
    pub max_iterations: usize,
    /// Whether to print convergence information.
    pub verbose: bool,
}

impl Default for SolverConfig {
    fn default() -> Self {
        Self {
            tolerance: 1e-6,
            max_iterations: 1000,
            verbose: false,
        }
    }
}
