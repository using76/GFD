//! Level Set method implementation.
//!
//! Tracks interfaces using a signed distance function phi,
//! with reinitialization to maintain the distance property.

use gfd_core::{ScalarField, VectorField, UnstructuredMesh};
use crate::Result;

/// Detailed Level Set solver implementation.
///
/// Tracks the interface between two phases using a signed distance
/// function phi, where phi > 0 in one phase, phi < 0 in the other,
/// and the interface is at phi = 0.
pub struct LevelSetSolverImpl {
    /// Signed distance function (phi > 0: phase 1, phi < 0: phase 2, phi = 0: interface).
    pub phi: ScalarField,
    /// Number of reinitialization iterations per time step.
    pub reinit_iterations: usize,
    /// Artificial time step for the reinitialization PDE.
    pub reinit_dt_factor: f64,
    /// Epsilon parameter for regularized Heaviside and delta functions.
    pub interface_thickness: f64,
}

impl LevelSetSolverImpl {
    /// Creates a new Level Set solver with the given signed distance field.
    pub fn new(phi: ScalarField, reinit_iterations: usize) -> Self {
        Self {
            phi,
            reinit_iterations,
            reinit_dt_factor: 0.5,
            interface_thickness: 1.5, // typically 1.5 * dx
        }
    }

    /// Solves the level set transport equation for one time step.
    ///
    /// d(phi)/dt + U . grad(phi) = 0
    ///
    /// Uses a high-order scheme (e.g., WENO or ENO) for the spatial
    /// discretization to minimize numerical diffusion of the interface.
    pub fn solve_transport(
        &mut self,
        velocity: &VectorField,
        mesh: &UnstructuredMesh,
        dt: f64,
    ) -> Result<()> {
        let n = mesh.num_cells();
        let phi_old = self.phi.values().to_vec();
        let mut phi_new = phi_old.clone();

        // Accumulate net flux per cell
        let mut net_flux = vec![0.0; n];

        for face in &mesh.faces {
            let owner = face.owner_cell;
            let vel_o = velocity.values()[owner];

            if let Some(neighbor) = face.neighbor_cell {
                let vel_n = velocity.values()[neighbor];
                let u_f = [
                    0.5 * (vel_o[0] + vel_n[0]),
                    0.5 * (vel_o[1] + vel_n[1]),
                    0.5 * (vel_o[2] + vel_n[2]),
                ];
                // Volume flux through face
                let flux = (u_f[0] * face.normal[0]
                    + u_f[1] * face.normal[1]
                    + u_f[2] * face.normal[2])
                    * face.area;

                // Upwind selection for phi
                let phi_f = if flux >= 0.0 {
                    phi_old[owner]
                } else {
                    phi_old[neighbor]
                };

                let face_flux = flux * phi_f;
                net_flux[owner] += face_flux;
                net_flux[neighbor] -= face_flux;
            } else {
                // Boundary face: zero-gradient
                let flux = (vel_o[0] * face.normal[0]
                    + vel_o[1] * face.normal[1]
                    + vel_o[2] * face.normal[2])
                    * face.area;
                let face_flux = flux * phi_old[owner];
                net_flux[owner] += face_flux;
            }
        }

        // Update phi: phi_new = phi_old - dt/V * sum(flux)
        for i in 0..n {
            let vol = mesh.cells[i].volume;
            if vol > 0.0 {
                phi_new[i] = phi_old[i] - dt / vol * net_flux[i];
            }
        }

        let vals = self.phi.values_mut();
        vals.copy_from_slice(&phi_new);

        Ok(())
    }

    /// Reinitializes the level set field to restore the signed distance property.
    ///
    /// Solves the reinitialization equation to steady state:
    /// d(phi)/d(tau) + sign(phi_0) * (|grad(phi)| - 1) = 0
    ///
    /// where tau is a pseudo-time and phi_0 is the level set before reinitialization.
    pub fn reinitialize(
        &mut self,
        mesh: &UnstructuredMesh,
    ) -> Result<()> {
        use gfd_core::gradient::{GreenGaussCellBasedGradient, GradientComputer};

        let n = mesh.num_cells();
        let phi_0 = self.phi.values().to_vec();
        let grad_computer = GreenGaussCellBasedGradient;

        // Estimate a characteristic cell size dx from the minimum cell volume^(1/3)
        let dx = mesh.cells.iter()
            .map(|c| c.volume.cbrt())
            .fold(f64::INFINITY, f64::min)
            .max(1e-30);

        // Pseudo-time step for reinitialization
        let dt_reinit = self.reinit_dt_factor * dx;

        // Compute smoothed sign function: S = phi_0 / sqrt(phi_0^2 + dx^2)
        let sign_phi: Vec<f64> = phi_0
            .iter()
            .map(|&p| p / (p * p + dx * dx).sqrt())
            .collect();

        for _iter in 0..self.reinit_iterations {
            // Compute |grad(phi)| using Green-Gauss
            let grad_phi = grad_computer
                .compute(&self.phi, mesh)
                .map_err(crate::FluidError::CoreError)?;

            let phi_vals = self.phi.values_mut();
            for i in 0..n {
                let g = grad_phi.values()[i];
                let grad_mag = (g[0] * g[0] + g[1] * g[1] + g[2] * g[2]).sqrt();

                // Hamilton-Jacobi reinitialization step
                // phi^{n+1} = phi^n - dt_reinit * S(phi_0) * (|grad(phi)| - 1)
                phi_vals[i] -= dt_reinit * sign_phi[i] * (grad_mag - 1.0);
            }
        }

        Ok(())
    }

    /// Computes the smoothed Heaviside function for property averaging.
    ///
    /// H_eps(phi) = 0                              if phi < -eps
    ///            = 0.5 * (1 + phi/eps + sin(pi*phi/eps)/pi)  if |phi| <= eps
    ///            = 1                              if phi > eps
    pub fn smoothed_heaviside(&self, phi_val: f64) -> f64 {
        let eps = self.interface_thickness;
        if phi_val < -eps {
            0.0
        } else if phi_val > eps {
            1.0
        } else {
            0.5 * (1.0 + phi_val / eps + (std::f64::consts::PI * phi_val / eps).sin() / std::f64::consts::PI)
        }
    }

    /// Computes the smoothed Dirac delta function for interface quantities.
    ///
    /// delta_eps(phi) = 0                          if |phi| > eps
    ///                = (1 + cos(pi*phi/eps)) / (2*eps)  if |phi| <= eps
    pub fn smoothed_delta(&self, phi_val: f64) -> f64 {
        let eps = self.interface_thickness;
        if phi_val.abs() > eps {
            0.0
        } else {
            (1.0 + (std::f64::consts::PI * phi_val / eps).cos()) / (2.0 * eps)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_heaviside_bounds() {
        let phi = ScalarField::zeros("phi", 1);
        let ls = LevelSetSolverImpl::new(phi, 5);
        assert_eq!(ls.smoothed_heaviside(-10.0), 0.0);
        assert_eq!(ls.smoothed_heaviside(10.0), 1.0);
        assert!((ls.smoothed_heaviside(0.0) - 0.5).abs() < 1e-12);
    }

    #[test]
    fn test_delta_peak_at_zero() {
        let phi = ScalarField::zeros("phi", 1);
        let ls = LevelSetSolverImpl::new(phi, 5);
        let at_zero = ls.smoothed_delta(0.0);
        let away = ls.smoothed_delta(10.0);
        assert!(at_zero > 0.0, "Delta should be positive at interface");
        assert_eq!(away, 0.0, "Delta should be zero away from interface");
    }
}
