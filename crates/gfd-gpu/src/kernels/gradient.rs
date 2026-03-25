//! Green-Gauss gradient kernel.
//!
//! ## CUDA kernel (reference)
//!
//! ```c
//! // gradient.cu — Green-Gauss cell gradient of a scalar field
//! //
//! // For each face, the contribution to the owning and neighbouring cell
//! // gradients is:
//! //
//! //   grad(phi)_C += phi_f * S_f / V_C
//! //
//! // Because multiple faces contribute to the same cell we use atomicAdd.
//!
//! extern "C" __global__
//! void green_gauss_gradient(
//!     const double* __restrict__ phi,       // cell-centred scalar
//!     const double* __restrict__ face_nx,
//!     const double* __restrict__ face_ny,
//!     const double* __restrict__ face_nz,
//!     const double* __restrict__ face_area,
//!     const int*    __restrict__ owner,
//!     const int*    __restrict__ neighbor,
//!     const double* __restrict__ cell_vol,
//!     double*       __restrict__ grad_x,
//!     double*       __restrict__ grad_y,
//!     double*       __restrict__ grad_z,
//!     int num_faces)
//! {
//!     int f = blockIdx.x * blockDim.x + threadIdx.x;
//!     if (f >= num_faces) return;
//!
//!     int o = owner[f];
//!     int n = neighbor[f];
//!     double phi_f = 0.5 * (phi[o] + phi[n]);
//!
//!     double Sf_x = face_area[f] * face_nx[f];
//!     double Sf_y = face_area[f] * face_ny[f];
//!     double Sf_z = face_area[f] * face_nz[f];
//!
//!     atomicAdd(&grad_x[o],  phi_f * Sf_x / cell_vol[o]);
//!     atomicAdd(&grad_y[o],  phi_f * Sf_y / cell_vol[o]);
//!     atomicAdd(&grad_z[o],  phi_f * Sf_z / cell_vol[o]);
//!
//!     atomicAdd(&grad_x[n], -phi_f * Sf_x / cell_vol[n]);
//!     atomicAdd(&grad_y[n], -phi_f * Sf_y / cell_vol[n]);
//!     atomicAdd(&grad_z[n], -phi_f * Sf_z / cell_vol[n]);
//! }
//! ```

use crate::memory::GpuVector;
use crate::transfer::MeshGpuData;
use crate::Result;

/// Compute the Green-Gauss cell gradient of a scalar field.
///
/// # Arguments
/// * `phi` — cell-centred scalar field.
/// * `mesh` — mesh data on the GPU.
/// * `grad_x`, `grad_y`, `grad_z` — output gradient components (per cell).
///
/// Gradient arrays must be pre-allocated with `num_cells` entries and **zeroed**
/// before calling this function (contributions are accumulated).
pub fn green_gauss_gradient_gpu(
    phi: &GpuVector,
    mesh: &MeshGpuData,
    grad_x: &mut GpuVector,
    grad_y: &mut GpuVector,
    grad_z: &mut GpuVector,
) -> Result<()> {
    #[cfg(feature = "cuda")]
    {
        // TODO: load PTX, launch kernel
        // Fall through to CPU for now.
    }

    // CPU fallback
    let phi_data = phi.cpu_data();
    let owner = mesh.face_owner.cpu_data();
    let neighbor = mesh.face_neighbor.cpu_data();
    let nx = mesh.face_normal_x.cpu_data();
    let ny = mesh.face_normal_y.cpu_data();
    let nz = mesh.face_normal_z.cpu_data();
    let area = mesh.face_area.cpu_data();
    let vol = mesh.cell_volume.cpu_data();
    let gx = grad_x.cpu_data_mut();
    let gy = grad_y.cpu_data_mut();
    let gz = grad_z.cpu_data_mut();

    for f in 0..mesh.num_faces {
        let o = owner[f] as usize;
        let n_idx = neighbor[f] as usize;

        let phi_f = 0.5 * (phi_data[o] + phi_data[n_idx]);
        let sf_x = area[f] * nx[f];
        let sf_y = area[f] * ny[f];
        let sf_z = area[f] * nz[f];

        gx[o] += phi_f * sf_x / vol[o];
        gy[o] += phi_f * sf_y / vol[o];
        gz[o] += phi_f * sf_z / vol[o];

        gx[n_idx] -= phi_f * sf_x / vol[n_idx];
        gy[n_idx] -= phi_f * sf_y / vol[n_idx];
        gz[n_idx] -= phi_f * sf_z / vol[n_idx];
    }

    Ok(())
}
