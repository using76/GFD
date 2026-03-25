//! Conjugate Gradient solver running entirely on the GPU (or CPU fallback).

use crate::memory::GpuVector;
use crate::sparse::GpuSparseMatrix;
use crate::{GpuError, Result};
use gfd_core::linalg::SolverStats;

use super::GpuLinearSolver;

// ---------------------------------------------------------------------------
// BLAS-1 helpers (CPU fallback; will be replaced with cuBLAS when cuda is on)
// ---------------------------------------------------------------------------

/// Inner product of two `GpuVector`s.
fn gpu_dot(a: &GpuVector, b: &GpuVector) -> Result<f64> {
    if a.len() != b.len() {
        return Err(GpuError::DimensionMismatch {
            expected: a.len(),
            got: b.len(),
        });
    }
    // TODO(cuda): cuBLAS ddot
    let ha = a.cpu_data();
    let hb = b.cpu_data();
    Ok(ha.iter().zip(hb.iter()).map(|(x, y)| x * y).sum())
}

/// `y += alpha * x`
fn gpu_axpy(alpha: f64, x: &GpuVector, y: &mut GpuVector) -> Result<()> {
    if x.len() != y.len() {
        return Err(GpuError::DimensionMismatch {
            expected: x.len(),
            got: y.len(),
        });
    }
    // TODO(cuda): cuBLAS daxpy
    let hx = x.cpu_data();
    let hy = y.cpu_data_mut();
    for i in 0..hx.len() {
        hy[i] += alpha * hx[i];
    }
    Ok(())
}

/// Element-wise copy `dst = src`.
fn gpu_copy(src: &GpuVector, dst: &mut GpuVector) -> Result<()> {
    if src.len() != dst.len() {
        return Err(GpuError::DimensionMismatch {
            expected: src.len(),
            got: dst.len(),
        });
    }
    // TODO(cuda): cuBLAS dcopy
    let hs = src.cpu_data();
    let hd = dst.cpu_data_mut();
    hd.copy_from_slice(hs);
    Ok(())
}

/// `x *= alpha`
fn gpu_scale(alpha: f64, x: &mut GpuVector) -> Result<()> {
    // TODO(cuda): cuBLAS dscal
    let hx = x.cpu_data_mut();
    for v in hx.iter_mut() {
        *v *= alpha;
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// GPU CG solver
// ---------------------------------------------------------------------------

/// Conjugate Gradient solver.
///
/// Implements the standard CG algorithm with all vector operations dispatched
/// through GPU helpers (which fall back to CPU when the `cuda` feature is
/// disabled).
pub struct GpuCG {
    /// Convergence tolerance (relative residual norm).
    pub tol: f64,
    /// Maximum number of iterations.
    pub max_iter: usize,
}

impl GpuCG {
    /// Create a new GPU CG solver with the given tolerance and iteration limit.
    pub fn new(tol: f64, max_iter: usize) -> Self {
        Self { tol, max_iter }
    }
}

impl Default for GpuCG {
    fn default() -> Self {
        Self {
            tol: 1e-6,
            max_iter: 1000,
        }
    }
}

impl GpuLinearSolver for GpuCG {
    /// Solve `A x = b` using the Conjugate Gradient method.
    ///
    /// The algorithm keeps **all** working vectors in `GpuVector` form so that
    /// when the CUDA backend is wired up no extra host/device copies are
    /// needed.
    fn solve(
        &mut self,
        a: &GpuSparseMatrix,
        b: &GpuVector,
        x: &mut GpuVector,
    ) -> Result<SolverStats> {
        let n = b.len();
        if a.nrows() != n || a.ncols() != n || x.len() != n {
            return Err(GpuError::DimensionMismatch {
                expected: n,
                got: x.len(),
            });
        }

        let device = &crate::device::GpuDeviceHandle::cpu_fallback();

        // r = b - A*x
        let mut r = GpuVector::zeros(n, device)?;
        // r = A*x
        a.spmv(x, &mut r, 1.0, 0.0)?;
        // r = b - r  =>  r = -1*r + b  =>  first negate r, then add b
        gpu_scale(-1.0, &mut r)?;
        gpu_axpy(1.0, b, &mut r)?;

        // p = r
        let mut p = GpuVector::zeros(n, device)?;
        gpu_copy(&r, &mut p)?;

        let mut rr = gpu_dot(&r, &r)?;
        let rr0 = rr;

        if rr0 < 1e-30 {
            return Ok(SolverStats {
                iterations: 0,
                final_residual: rr0.sqrt(),
                converged: true,
            });
        }

        let mut ap = GpuVector::zeros(n, device)?;
        let mut iterations = 0;

        for k in 0..self.max_iter {
            iterations = k + 1;

            // ap = A * p
            a.spmv(&p, &mut ap, 1.0, 0.0)?;

            let pap = gpu_dot(&p, &ap)?;
            if pap.abs() < 1e-30 {
                return Err(GpuError::SolverFailed(
                    "CG breakdown: p^T A p ~ 0".to_string(),
                ));
            }
            let alpha = rr / pap;

            // x += alpha * p
            gpu_axpy(alpha, &p, x)?;

            // r -= alpha * ap
            gpu_axpy(-alpha, &ap, &mut r)?;

            let rr_new = gpu_dot(&r, &r)?;

            // Check convergence (relative residual).
            if rr_new / rr0 < self.tol * self.tol {
                return Ok(SolverStats {
                    iterations,
                    final_residual: rr_new.sqrt(),
                    converged: true,
                });
            }

            let beta = rr_new / rr;
            rr = rr_new;

            // p = r + beta * p
            gpu_scale(beta, &mut p)?;
            gpu_axpy(1.0, &r, &mut p)?;
        }

        Ok(SolverStats {
            iterations,
            final_residual: rr.sqrt(),
            converged: false,
        })
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::device::GpuDeviceHandle;
    use gfd_core::linalg::SparseMatrix;

    /// Build a simple 3x3 SPD matrix and solve Ax = b with CG.
    #[test]
    fn test_gpu_cg_cpu_fallback() {
        // A = [[4, 1, 0],
        //      [1, 3, 1],
        //      [0, 1, 4]]
        let a = SparseMatrix::new(
            3,
            3,
            vec![0, 2, 5, 7],
            vec![0, 1, 0, 1, 2, 1, 2],
            vec![4.0, 1.0, 1.0, 3.0, 1.0, 1.0, 4.0],
        )
        .unwrap();

        let b_host = [1.0, 2.0, 3.0];

        let device = GpuDeviceHandle::cpu_fallback();
        let ga = crate::sparse::GpuSparseMatrix::from_cpu(&a, &device).unwrap();
        let gb = GpuVector::from_cpu(&b_host, &device).unwrap();
        let mut gx = GpuVector::zeros(3, &device).unwrap();

        let mut solver = GpuCG::new(1e-10, 100);
        let stats = solver.solve(&ga, &gb, &mut gx).unwrap();

        assert!(stats.converged, "CG did not converge: {:?}", stats);

        // Verify Ax ≈ b
        let mut result = vec![0.0; 3];
        gx.to_cpu(&mut result).unwrap();

        let mut check = vec![0.0; 3];
        a.spmv(&result, &mut check).unwrap();

        for i in 0..3 {
            assert!(
                (check[i] - b_host[i]).abs() < 1e-8,
                "Mismatch at {}: {} vs {}",
                i,
                check[i],
                b_host[i]
            );
        }
    }

    /// CG on an identity matrix — should converge in 1 iteration.
    #[test]
    fn test_gpu_cg_identity() {
        let n = 5;
        let row_ptr: Vec<usize> = (0..=n).collect();
        let col_idx: Vec<usize> = (0..n).collect();
        let values = vec![1.0; n];
        let a = SparseMatrix::new(n, n, row_ptr, col_idx, values).unwrap();

        let b_host: Vec<f64> = (1..=n).map(|i| i as f64).collect();

        let device = GpuDeviceHandle::cpu_fallback();
        let ga = crate::sparse::GpuSparseMatrix::from_cpu(&a, &device).unwrap();
        let gb = GpuVector::from_cpu(&b_host, &device).unwrap();
        let mut gx = GpuVector::zeros(n, &device).unwrap();

        let mut solver = GpuCG::new(1e-12, 100);
        let stats = solver.solve(&ga, &gb, &mut gx).unwrap();

        assert!(stats.converged);
        assert!(stats.iterations <= 2, "Identity should converge fast");

        let mut result = vec![0.0; n];
        gx.to_cpu(&mut result).unwrap();
        for i in 0..n {
            assert!(
                (result[i] - b_host[i]).abs() < 1e-10,
                "Mismatch at {}: {} vs {}",
                i,
                result[i],
                b_host[i]
            );
        }
    }
}
