//! Face-flux computation kernel.
//!
//! ## CUDA kernel (reference)
//!
//! ```c
//! // flux.cu — compute face flux from velocity and face geometry
//! //
//! // Each thread handles one face.
//! //
//! // F_f = dot(u_f, S_f)
//! //     = u_f.x * Sf_x + u_f.y * Sf_y + u_f.z * Sf_z
//! //
//! // where u_f is the interpolated face velocity and S_f = area * normal.
//!
//! extern "C" __global__
//! void compute_face_flux(
//!     const double* __restrict__ vel_x,   // cell velocity x
//!     const double* __restrict__ vel_y,   // cell velocity y
//!     const double* __restrict__ vel_z,   // cell velocity z
//!     const double* __restrict__ face_nx, // face normal x
//!     const double* __restrict__ face_ny, // face normal y
//!     const double* __restrict__ face_nz, // face normal z
//!     const double* __restrict__ face_area,
//!     const int*    __restrict__ owner,
//!     const int*    __restrict__ neighbor,
//!     double*       __restrict__ flux,
//!     int num_faces)
//! {
//!     int f = blockIdx.x * blockDim.x + threadIdx.x;
//!     if (f >= num_faces) return;
//!
//!     int o = owner[f];
//!     int n = neighbor[f];
//!
//!     // Simple linear interpolation (weight = 0.5 for now).
//!     double uf_x = 0.5 * (vel_x[o] + vel_x[n]);
//!     double uf_y = 0.5 * (vel_y[o] + vel_y[n]);
//!     double uf_z = 0.5 * (vel_z[o] + vel_z[n]);
//!
//!     double Sf_x = face_area[f] * face_nx[f];
//!     double Sf_y = face_area[f] * face_ny[f];
//!     double Sf_z = face_area[f] * face_nz[f];
//!
//!     flux[f] = uf_x * Sf_x + uf_y * Sf_y + uf_z * Sf_z;
//! }
//! ```

use crate::memory::GpuVector;
use crate::transfer::MeshGpuData;
use crate::Result;

/// Compute the face mass flux `F_f = dot(u_f, S_f)` for every face.
///
/// # Arguments
/// * `vel_x`, `vel_y`, `vel_z` — cell-centred velocity components.
/// * `mesh` — mesh data on the GPU.
/// * `flux` — output face fluxes (must be pre-allocated with `num_faces` entries).
///
/// When the `cuda` feature is active this launches the CUDA kernel above;
/// otherwise it performs the computation on the CPU.
pub fn compute_face_flux_gpu(
    vel_x: &GpuVector,
    vel_y: &GpuVector,
    vel_z: &GpuVector,
    mesh: &MeshGpuData,
    flux: &mut GpuVector,
) -> Result<()> {
    #[cfg(feature = "cuda")]
    {
        // TODO: load PTX, launch kernel
        // For now fall through to CPU.
    }

    // CPU fallback
    let vx = vel_x.cpu_data();
    let vy = vel_y.cpu_data();
    let vz = vel_z.cpu_data();
    let owner = mesh.face_owner.cpu_data();
    let neighbor = mesh.face_neighbor.cpu_data();
    let nx = mesh.face_normal_x.cpu_data();
    let ny = mesh.face_normal_y.cpu_data();
    let nz = mesh.face_normal_z.cpu_data();
    let area = mesh.face_area.cpu_data();
    let out = flux.cpu_data_mut();

    for f in 0..mesh.num_faces {
        let o = owner[f] as usize;
        let n = neighbor[f] as usize;

        // Linear interpolation (weight 0.5).
        let uf_x = 0.5 * (vx[o] + vx[n]);
        let uf_y = 0.5 * (vy[o] + vy[n]);
        let uf_z = 0.5 * (vz[o] + vz[n]);

        let sf_x = area[f] * nx[f];
        let sf_y = area[f] * ny[f];
        let sf_z = area[f] * nz[f];

        out[f] = uf_x * sf_x + uf_y * sf_y + uf_z * sf_z;
    }

    Ok(())
}
