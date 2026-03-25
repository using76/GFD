//! GPU sparse-matrix types (CSR on device).

use crate::device::GpuDeviceHandle;
use crate::memory::GpuVector;
use crate::{GpuError, Result};
use gfd_core::linalg::SparseMatrix;

// ---------------------------------------------------------------------------
// GpuSparseMatrix
// ---------------------------------------------------------------------------

/// A CSR sparse matrix stored on the GPU (or on the CPU as fallback).
pub struct GpuSparseMatrix {
    /// Row pointer array (as f64 for uniform storage; cast back to usize
    /// when needed on the CPU fallback path).
    pub(crate) row_ptr: GpuVector,
    /// Column indices (stored as f64).
    pub(crate) col_idx: GpuVector,
    /// Non-zero values.
    pub(crate) values: GpuVector,
    nrows: usize,
    ncols: usize,
    nnz: usize,
}

impl std::fmt::Debug for GpuSparseMatrix {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GpuSparseMatrix")
            .field("nrows", &self.nrows)
            .field("ncols", &self.ncols)
            .field("nnz", &self.nnz)
            .finish()
    }
}

impl GpuSparseMatrix {
    /// Upload a CPU `SparseMatrix` to the device.
    pub fn from_cpu(matrix: &SparseMatrix, device: &GpuDeviceHandle) -> Result<Self> {
        // Convert integer arrays to f64 for uniform GPU storage.
        let row_ptr_f64: Vec<f64> = matrix.row_ptr.iter().map(|&v| v as f64).collect();
        let col_idx_f64: Vec<f64> = matrix.col_idx.iter().map(|&v| v as f64).collect();

        let row_ptr = GpuVector::from_cpu(&row_ptr_f64, device)?;
        let col_idx = GpuVector::from_cpu(&col_idx_f64, device)?;
        let values = GpuVector::from_cpu(&matrix.values, device)?;

        Ok(Self {
            row_ptr,
            col_idx,
            values,
            nrows: matrix.nrows,
            ncols: matrix.ncols,
            nnz: matrix.nnz(),
        })
    }

    /// Number of rows.
    pub fn nrows(&self) -> usize {
        self.nrows
    }

    /// Number of columns.
    pub fn ncols(&self) -> usize {
        self.ncols
    }

    /// Number of non-zero entries.
    pub fn nnz(&self) -> usize {
        self.nnz
    }

    /// Sparse matrix-vector multiply: `y = alpha * A * x + beta * y`.
    ///
    /// When the `cuda` feature is enabled this will use cuSPARSE (not yet
    /// implemented — currently falls back to CPU). Without `cuda` it uses
    /// the CPU `SparseMatrix::spmv` path.
    pub fn spmv(
        &self,
        x: &GpuVector,
        y: &mut GpuVector,
        alpha: f64,
        beta: f64,
    ) -> Result<()> {
        if x.len() != self.ncols {
            return Err(GpuError::DimensionMismatch {
                expected: self.ncols,
                got: x.len(),
            });
        }
        if y.len() != self.nrows {
            return Err(GpuError::DimensionMismatch {
                expected: self.nrows,
                got: y.len(),
            });
        }

        // TODO(cuda): cuSPARSE SpMV path.
        // For now, fall through to CPU in all cases.

        // Reconstruct CSR on the host side.
        let row_ptr: Vec<usize> = self
            .row_ptr
            .cpu_data()
            .iter()
            .map(|&v| v as usize)
            .collect();
        let col_idx: Vec<usize> = self
            .col_idx
            .cpu_data()
            .iter()
            .map(|&v| v as usize)
            .collect();
        let vals = self.values.cpu_data();
        let x_host = x.cpu_data();
        let y_host = y.cpu_data_mut();

        // y = alpha * A*x + beta * y
        for i in 0..self.nrows {
            let mut ax_i = 0.0;
            for idx in row_ptr[i]..row_ptr[i + 1] {
                ax_i += vals[idx] * x_host[col_idx[idx]];
            }
            y_host[i] = alpha * ax_i + beta * y_host[i];
        }

        Ok(())
    }
}
