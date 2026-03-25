//! GPU device detection and selection.

#[cfg(feature = "cuda")]
use cudarc::driver::CudaDevice;
#[cfg(feature = "cuda")]
use std::sync::Arc;

#[cfg(feature = "cuda")]
use crate::GpuError;
use crate::Result;

// ---------------------------------------------------------------------------
// Device metadata
// ---------------------------------------------------------------------------

/// Static metadata for a detected GPU device.
#[derive(Debug, Clone)]
pub struct GpuDevice {
    /// Device ordinal (0, 1, ...).
    pub id: usize,
    /// Human-readable device name.
    pub name: String,
    /// Compute capability (major, minor).
    pub compute_capability: (usize, usize),
    /// Total device memory in bytes.
    pub memory_bytes: usize,
    /// Number of streaming multiprocessors.
    pub sm_count: usize,
}

// ---------------------------------------------------------------------------
// Device handle — wraps an active CUDA context
// ---------------------------------------------------------------------------

/// A handle to an active GPU device.
///
/// When compiled without `cuda` the inner field is `None` and all operations
/// that take a `GpuDeviceHandle` fall back to CPU paths.
#[derive(Clone)]
pub struct GpuDeviceHandle {
    #[cfg(feature = "cuda")]
    pub(crate) inner: Option<Arc<CudaDevice>>,
    #[cfg(not(feature = "cuda"))]
    pub(crate) _phantom: (),
}

impl std::fmt::Debug for GpuDeviceHandle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GpuDeviceHandle").finish()
    }
}

impl GpuDeviceHandle {
    /// Creates a CPU-only (no-op) device handle.
    pub fn cpu_fallback() -> Self {
        #[cfg(feature = "cuda")]
        {
            Self { inner: None }
        }
        #[cfg(not(feature = "cuda"))]
        {
            Self { _phantom: () }
        }
    }

    /// Returns `true` when backed by a real CUDA device.
    pub fn has_cuda(&self) -> bool {
        #[cfg(feature = "cuda")]
        {
            self.inner.is_some()
        }
        #[cfg(not(feature = "cuda"))]
        {
            false
        }
    }
}

// ---------------------------------------------------------------------------
// Detection / selection
// ---------------------------------------------------------------------------

/// Enumerates all visible CUDA devices.
///
/// Returns an empty `Vec` when the `cuda` feature is disabled or no devices
/// are found.
#[cfg(feature = "cuda")]
pub fn detect_devices() -> Vec<GpuDevice> {
    let count = match cudarc::driver::result::device::get_count() {
        Ok(n) => n,
        Err(_) => return Vec::new(),
    };
    let mut devices = Vec::with_capacity(count as usize);
    for ordinal in 0..count {
        // We create a temporary context just to query properties.
        if let Ok(dev) = CudaDevice::new(ordinal as usize) {
            // cudarc doesn't expose all properties directly — fill what we can.
            devices.push(GpuDevice {
                id: ordinal as usize,
                name: format!("CUDA device {}", ordinal),
                compute_capability: (0, 0),
                memory_bytes: 0,
                sm_count: 0,
            });
            drop(dev);
        }
    }
    devices
}

#[cfg(not(feature = "cuda"))]
pub fn detect_devices() -> Vec<GpuDevice> {
    Vec::new()
}

/// Creates a device handle for the given ordinal.
///
/// Without the `cuda` feature this always returns a CPU-fallback handle
/// regardless of the `id` parameter.
#[cfg(feature = "cuda")]
pub fn select_device(id: usize) -> Result<GpuDeviceHandle> {
    let dev = CudaDevice::new(id).map_err(|e| GpuError::DeviceNotFound(id))?;
    Ok(GpuDeviceHandle {
        inner: Some(dev),
    })
}

#[cfg(not(feature = "cuda"))]
pub fn select_device(_id: usize) -> Result<GpuDeviceHandle> {
    tracing::info!("CUDA not available — returning CPU-fallback device handle");
    Ok(GpuDeviceHandle::cpu_fallback())
}
