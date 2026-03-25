//! GPU memory management — vectors and scalars.

#[cfg(feature = "cuda")]
use cudarc::driver::CudaSlice;

use crate::device::GpuDeviceHandle;
use crate::{GpuError, Result};

// ---------------------------------------------------------------------------
// GpuVector
// ---------------------------------------------------------------------------

/// A vector that lives either on the GPU or on the CPU (fallback).
pub struct GpuVector {
    #[cfg(feature = "cuda")]
    pub(crate) cuda_buf: Option<CudaSlice<f64>>,
    /// CPU-side storage (always present — used as the sole storage when CUDA
    /// is disabled, and as a staging buffer when CUDA is enabled).
    pub(crate) cpu_buf: Vec<f64>,
    len: usize,
}

impl std::fmt::Debug for GpuVector {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GpuVector")
            .field("len", &self.len)
            .finish()
    }
}

impl Clone for GpuVector {
    fn clone(&self) -> Self {
        // Clone only the CPU buffer — GPU buffers are not cheaply cloneable.
        Self {
            #[cfg(feature = "cuda")]
            cuda_buf: None,
            cpu_buf: self.cpu_buf.clone(),
            len: self.len,
        }
    }
}

impl GpuVector {
    // -- constructors -------------------------------------------------------

    /// Upload a host slice to the device (or store it CPU-side as fallback).
    pub fn from_cpu(data: &[f64], device: &GpuDeviceHandle) -> Result<Self> {
        let len = data.len();

        #[cfg(feature = "cuda")]
        {
            if let Some(ref dev) = device.inner {
                let buf = dev
                    .htod_sync_copy(data)
                    .map_err(|e| GpuError::TransferFailed(e.to_string()))?;
                return Ok(Self {
                    cuda_buf: Some(buf),
                    cpu_buf: data.to_vec(),
                    len,
                });
            }
        }

        // CPU fallback
        let _ = device; // suppress unused warning without cuda
        Ok(Self {
            #[cfg(feature = "cuda")]
            cuda_buf: None,
            cpu_buf: data.to_vec(),
            len,
        })
    }

    /// Create a zero-filled vector of length `n`.
    pub fn zeros(n: usize, device: &GpuDeviceHandle) -> Result<Self> {
        let data = vec![0.0f64; n];
        Self::from_cpu(&data, device)
    }

    // -- accessors ----------------------------------------------------------

    /// Number of elements.
    pub fn len(&self) -> usize {
        self.len
    }

    /// Whether the vector is empty.
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    // -- transfers ----------------------------------------------------------

    /// Copy device data back to the provided host slice.
    pub fn to_cpu(&self, out: &mut [f64]) -> Result<()> {
        if out.len() < self.len {
            return Err(GpuError::DimensionMismatch {
                expected: self.len,
                got: out.len(),
            });
        }

        #[cfg(feature = "cuda")]
        {
            if let Some(ref buf) = self.cuda_buf {
                // cudarc dtoh_sync_copy returns a Vec<f64>
                let host = buf
                    .device()
                    .dtoh_sync_copy(buf)
                    .map_err(|e| GpuError::TransferFailed(e.to_string()))?;
                out[..self.len].copy_from_slice(&host);
                return Ok(());
            }
        }

        // CPU fallback
        out[..self.len].copy_from_slice(&self.cpu_buf);
        Ok(())
    }

    // -- mutable CPU access (for fallback paths) ----------------------------

    /// Returns a mutable reference to the CPU buffer.
    ///
    /// **Only valid when no CUDA buffer is active** (i.e., in CPU-fallback
    /// mode). When CUDA is active the CPU buffer may be stale.
    pub(crate) fn cpu_data_mut(&mut self) -> &mut [f64] {
        &mut self.cpu_buf
    }

    /// Returns a reference to the CPU buffer.
    pub(crate) fn cpu_data(&self) -> &[f64] {
        &self.cpu_buf
    }

    /// Synchronise the CPU buffer from the GPU (no-op when on CPU).
    pub fn sync_to_host(&mut self) -> Result<()> {
        #[cfg(feature = "cuda")]
        {
            if let Some(ref buf) = self.cuda_buf {
                let host = buf
                    .device()
                    .dtoh_sync_copy(buf)
                    .map_err(|e| GpuError::TransferFailed(e.to_string()))?;
                self.cpu_buf = host;
                return Ok(());
            }
        }
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// GpuScalar — a single f64 on the device
// ---------------------------------------------------------------------------

/// A single f64 stored on the GPU (used for reduction results like dot
/// products). In CPU-fallback mode this is just a plain `f64`.
#[derive(Debug, Clone)]
pub struct GpuScalar {
    value: f64,
}

impl GpuScalar {
    /// Create from a host value.
    pub fn new(value: f64) -> Self {
        Self { value }
    }

    /// Read the value back to the host.
    pub fn to_host(&self) -> f64 {
        self.value
    }
}
