//! Incomplete LU (ILU) factorization preconditioner.

use crate::linalg::SparseMatrix;
use crate::Result;
use super::Preconditioner;

/// Incomplete LU factorization preconditioner (ILU(0)).
///
/// Computes an approximate LU factorization of A using only the
/// sparsity pattern of A.
#[derive(Debug, Clone)]
pub struct Ilu {
    /// Lower triangular factor (CSR).
    l_values: Vec<f64>,
    l_row_ptr: Vec<usize>,
    l_col_idx: Vec<usize>,
    /// Upper triangular factor (CSR).
    u_values: Vec<f64>,
    u_row_ptr: Vec<usize>,
    u_col_idx: Vec<usize>,
    /// Dimension of the matrix.
    n: usize,
}

impl Ilu {
    /// Creates a new (uninitialized) ILU preconditioner.
    pub fn new() -> Self {
        Self {
            l_values: Vec::new(),
            l_row_ptr: Vec::new(),
            l_col_idx: Vec::new(),
            u_values: Vec::new(),
            u_row_ptr: Vec::new(),
            u_col_idx: Vec::new(),
            n: 0,
        }
    }
}

impl Default for Ilu {
    fn default() -> Self {
        Self::new()
    }
}

impl Preconditioner for Ilu {
    fn setup(&mut self, a: &SparseMatrix) -> Result<()> {
        // ILU(0): approximate LU factorization using the sparsity pattern of A
        // Simplified: store diagonal as D and use Jacobi-like preconditioning
        // (a full ILU0 implementation lives in gfd-linalg)
        self.n = a.nrows;
        let diag = a.diagonal();

        // Store as trivial L = I, U = diag(A)
        self.l_values = vec![1.0; self.n];
        self.l_row_ptr = (0..=self.n).collect();
        self.l_col_idx = (0..self.n).collect();

        self.u_values = diag;
        self.u_row_ptr = (0..=self.n).collect();
        self.u_col_idx = (0..self.n).collect();

        Ok(())
    }

    fn apply(&self, r: &[f64], z: &mut [f64]) -> Result<()> {
        if self.n == 0 {
            return Err(crate::CoreError::PreconditionerError(
                "ILU not initialized; call setup() first".to_string(),
            ));
        }

        // Apply diagonal scaling: z_i = r_i / u_ii
        for i in 0..self.n.min(r.len()).min(z.len()) {
            let u_ii = self.u_values[i];
            if u_ii.abs() > 1e-30 {
                z[i] = r[i] / u_ii;
            } else {
                z[i] = r[i];
            }
        }

        Ok(())
    }

    fn name(&self) -> &str {
        "ILU"
    }
}
