//! GPU acceleration abstractions.
//!
//! Provides device detection and backend selection for GPU-accelerated
//! linear algebra and field operations.  The current implementation is a
//! stub that reports no available GPU; actual backends (CUDA, OpenCL,
//! Vulkan compute) can be enabled via feature flags in the future.

/// Supported GPU compute backends.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum GpuBackend {
    /// No GPU backend (CPU only).
    None,
    /// NVIDIA CUDA.
    Cuda,
    /// OpenCL (cross-vendor).
    OpenCL,
    /// Vulkan compute shaders.
    Vulkan,
}

impl std::fmt::Display for GpuBackend {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            GpuBackend::None => write!(f, "None"),
            GpuBackend::Cuda => write!(f, "CUDA"),
            GpuBackend::OpenCL => write!(f, "OpenCL"),
            GpuBackend::Vulkan => write!(f, "Vulkan"),
        }
    }
}

/// Represents a single GPU device.
#[derive(Debug, Clone)]
pub struct GpuDevice {
    /// Compute backend for this device.
    pub backend: GpuBackend,
    /// Device index (platform-specific).
    pub device_id: usize,
    /// Device memory in bytes.
    pub memory_bytes: u64,
    /// Human-readable device name.
    pub name: String,
}

impl GpuDevice {
    /// Creates a new GPU device descriptor.
    pub fn new(backend: GpuBackend, device_id: usize, memory_bytes: u64, name: String) -> Self {
        Self {
            backend,
            device_id,
            memory_bytes,
            name,
        }
    }

    /// Returns the device memory in megabytes.
    pub fn memory_mb(&self) -> f64 {
        self.memory_bytes as f64 / (1024.0 * 1024.0)
    }
}

/// Detects available GPU devices on the system.
///
/// Currently returns an empty vector (stub).  When a GPU feature flag
/// is enabled, this should enumerate devices via the corresponding API
/// (e.g., `cuDeviceGetCount` for CUDA).
pub fn detect_gpu_devices() -> Vec<GpuDevice> {
    // Stub: no GPU detection implemented yet.
    Vec::new()
}

/// Returns `true` if at least one GPU device is available.
pub fn is_gpu_available() -> bool {
    // Stub: always false until a backend is implemented.
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_no_gpu_available() {
        assert!(!is_gpu_available());
        assert!(detect_gpu_devices().is_empty());
    }

    #[test]
    fn test_gpu_device_memory_conversion() {
        let dev = GpuDevice::new(
            GpuBackend::Cuda,
            0,
            8 * 1024 * 1024 * 1024, // 8 GiB
            "Test GPU".to_string(),
        );
        assert!((dev.memory_mb() - 8192.0).abs() < 1e-6);
    }

    #[test]
    fn test_backend_display() {
        assert_eq!(format!("{}", GpuBackend::Cuda), "CUDA");
        assert_eq!(format!("{}", GpuBackend::None), "None");
    }
}
