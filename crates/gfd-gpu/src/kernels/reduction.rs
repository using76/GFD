//! Parallel reduction kernels (max-abs, sum).

use crate::memory::GpuVector;
use crate::Result;

/// Compute the maximum absolute value in a `GpuVector`.
///
/// With the `cuda` feature this will launch a parallel reduction kernel;
/// without it the vector is scanned on the CPU.
pub fn parallel_max_abs(data: &GpuVector) -> Result<f64> {
    #[cfg(feature = "cuda")]
    {
        // TODO: CUDA parallel reduction kernel
        // Fall through to CPU.
    }

    // CPU fallback
    let host = data.cpu_data();
    let max_val = host
        .iter()
        .fold(0.0_f64, |acc, &v| acc.max(v.abs()));
    Ok(max_val)
}

/// Compute the sum of all elements in a `GpuVector`.
///
/// With the `cuda` feature this will launch a parallel reduction kernel;
/// without it the vector is summed on the CPU.
pub fn parallel_sum(data: &GpuVector) -> Result<f64> {
    #[cfg(feature = "cuda")]
    {
        // TODO: CUDA parallel reduction kernel
        // Fall through to CPU.
    }

    // CPU fallback
    let host = data.cpu_data();
    Ok(host.iter().sum())
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::device::GpuDeviceHandle;

    #[test]
    fn test_parallel_max_abs() {
        let device = GpuDeviceHandle::cpu_fallback();
        let v = GpuVector::from_cpu(&[-3.0, 1.0, -7.0, 4.0], &device).unwrap();
        let m = parallel_max_abs(&v).unwrap();
        assert!((m - 7.0).abs() < 1e-15);
    }

    #[test]
    fn test_parallel_sum() {
        let device = GpuDeviceHandle::cpu_fallback();
        let v = GpuVector::from_cpu(&[1.0, 2.0, 3.0, 4.0], &device).unwrap();
        let s = parallel_sum(&v).unwrap();
        assert!((s - 10.0).abs() < 1e-15);
    }
}
