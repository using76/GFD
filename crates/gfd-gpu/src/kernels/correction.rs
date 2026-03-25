//! Velocity and pressure correction kernels.
//!
//! These are stubs that will be replaced with CUDA kernels once the PTX
//! compilation pipeline is in place.

use crate::memory::GpuVector;
use crate::Result;

/// Correct cell velocities after the pressure-correction step.
///
/// ```text
/// u* = u - (dt / rho) * grad(p')
/// ```
///
/// # Arguments
/// * `vel_x`, `vel_y`, `vel_z` — cell velocity components (modified in-place).
/// * `grad_p_x`, `grad_p_y`, `grad_p_z` — pressure-correction gradient.
/// * `dt` — time-step size.
/// * `rho` — density (uniform for now).
pub fn correct_velocity_gpu(
    vel_x: &mut GpuVector,
    vel_y: &mut GpuVector,
    vel_z: &mut GpuVector,
    grad_p_x: &GpuVector,
    grad_p_y: &GpuVector,
    grad_p_z: &GpuVector,
    dt: f64,
    rho: f64,
) -> Result<()> {
    #[cfg(feature = "cuda")]
    {
        // TODO: CUDA kernel for velocity correction
        // Fall through to CPU.
    }

    // CPU fallback
    let factor = dt / rho;
    let vx = vel_x.cpu_data_mut();
    let gpx = grad_p_x.cpu_data();
    for i in 0..vx.len() {
        vx[i] -= factor * gpx[i];
    }

    let vy = vel_y.cpu_data_mut();
    let gpy = grad_p_y.cpu_data();
    for i in 0..vy.len() {
        vy[i] -= factor * gpy[i];
    }

    let vz = vel_z.cpu_data_mut();
    let gpz = grad_p_z.cpu_data();
    for i in 0..vz.len() {
        vz[i] -= factor * gpz[i];
    }

    Ok(())
}

/// Correct the pressure field.
///
/// ```text
/// p = p + alpha_p * p'
/// ```
///
/// # Arguments
/// * `pressure` — current pressure field (modified in-place).
/// * `pressure_correction` — pressure correction p'.
/// * `alpha_p` — under-relaxation factor (typically 0.3).
pub fn correct_pressure_gpu(
    pressure: &mut GpuVector,
    pressure_correction: &GpuVector,
    alpha_p: f64,
) -> Result<()> {
    #[cfg(feature = "cuda")]
    {
        // TODO: CUDA kernel for pressure correction
        // Fall through to CPU.
    }

    // CPU fallback
    let p = pressure.cpu_data_mut();
    let pc = pressure_correction.cpu_data();
    for i in 0..p.len() {
        p[i] += alpha_p * pc[i];
    }

    Ok(())
}
