//! Wall treatment adapter for turbulence models.
//!
//! Provides wall function implementations and y+ computation for
//! near-wall treatment in RANS simulations.

use gfd_core::{ScalarField, UnstructuredMesh};
use crate::{FluidState, Result};

/// Adapter for applying wall treatment to turbulence models.
///
/// Handles the near-wall region by computing wall shear stress,
/// y+ values, and applying appropriate wall functions to modify
/// the turbulence boundary conditions.
pub struct WallTreatmentAdapter {
    /// Wall function type: "standard", "scalable", "enhanced", "low_re".
    pub wall_function_type: String,
    /// Von Karman constant (default 0.41).
    pub kappa: f64,
    /// Additive constant in the log-law (default 5.0).
    pub e_constant: f64,
    /// y+ threshold for switching between viscous sublayer and log-law.
    pub y_plus_threshold: f64,
}

impl WallTreatmentAdapter {
    /// Creates a new wall treatment adapter with standard wall functions.
    pub fn new() -> Self {
        Self {
            wall_function_type: "standard".to_string(),
            kappa: 0.41,
            e_constant: 5.0,
            y_plus_threshold: 11.225,
        }
    }

    /// Creates a wall treatment adapter with the specified wall function type.
    pub fn with_type(wall_function_type: impl Into<String>) -> Self {
        Self {
            wall_function_type: wall_function_type.into(),
            kappa: 0.41,
            e_constant: 5.0,
            y_plus_threshold: 11.225,
        }
    }

    /// Applies wall functions to modify boundary conditions at wall faces.
    ///
    /// For the standard log-law wall function:
    /// - If y+ < y+_threshold: viscous sublayer, u+ = y+
    /// - If y+ >= y+_threshold: log-law region, u+ = (1/kappa)*ln(y+) + B
    ///
    /// Modifies the turbulence variable boundary values and wall shear stress.
    pub fn apply_wall_functions(
        &self,
        state: &mut FluidState,
        mesh: &UnstructuredMesh,
        wall_face_indices: &[usize],
    ) -> Result<()> {
        let c_mu = 0.09_f64;

        for &face_id in wall_face_indices {
            let face = &mesh.faces[face_id];
            let owner = face.owner_cell;

            // Wall-normal distance: distance from cell center to face center
            let cc = mesh.cells[owner].center;
            let fc = face.center;
            let y = ((cc[0] - fc[0]).powi(2)
                + (cc[1] - fc[1]).powi(2)
                + (cc[2] - fc[2]).powi(2))
            .sqrt()
            .max(1e-30);

            let rho = state.density.values()[owner];
            let nu = state.viscosity.values()[owner] / rho; // kinematic viscosity

            // Compute wall-parallel velocity magnitude
            let vel = state.velocity.values()[owner];
            let u_n = vel[0] * face.normal[0] + vel[1] * face.normal[1] + vel[2] * face.normal[2];
            let u_par = [
                vel[0] - u_n * face.normal[0],
                vel[1] - u_n * face.normal[1],
                vel[2] - u_n * face.normal[2],
            ];
            let u_mag = (u_par[0] * u_par[0] + u_par[1] * u_par[1] + u_par[2] * u_par[2]).sqrt();

            // Compute u_tau using Newton iteration on u+ = (1/kappa)*ln(E*y+)
            // where u+ = U/u_tau and y+ = y*u_tau/nu
            // Start with initial guess from viscous sublayer: u_tau = sqrt(nu * U / y)
            let mut u_tau = (nu * u_mag / y).sqrt().max(1e-10);

            for _newton_iter in 0..20 {
                let y_plus = u_tau * y / nu;
                let u_plus_computed = if y_plus < self.y_plus_threshold {
                    y_plus // viscous sublayer: u+ = y+
                } else {
                    (1.0 / self.kappa) * (y_plus).ln() + self.e_constant
                };
                let u_plus_target = u_mag / u_tau;

                let residual = u_plus_computed - u_plus_target;
                if residual.abs() < 1e-6 {
                    break;
                }

                // Derivative of residual w.r.t. u_tau
                let du_plus_du_tau = if y_plus < self.y_plus_threshold {
                    y / nu
                } else {
                    1.0 / (self.kappa * u_tau)
                };
                let d_residual = du_plus_du_tau + u_mag / (u_tau * u_tau);

                if d_residual.abs() > 1e-30 {
                    u_tau -= residual / d_residual;
                    u_tau = u_tau.max(1e-10);
                }
            }

            // Set turbulence boundary values for the wall-adjacent cell
            let c_mu_sqrt = c_mu.sqrt();

            // k boundary value: k = u_tau^2 / sqrt(C_mu)
            if let Some(ref mut k_field) = state.turb_kinetic_energy {
                let k_wall = u_tau * u_tau / c_mu_sqrt;
                let _ = k_field.set(owner, k_wall);
            }

            // epsilon boundary value: epsilon = u_tau^3 / (kappa * y)
            if let Some(ref mut eps_field) = state.turb_dissipation {
                let eps_wall = u_tau.powi(3) / (self.kappa * y);
                let _ = eps_field.set(owner, eps_wall);
            }

            // omega boundary value: omega = u_tau / (sqrt(C_mu) * kappa * y)
            if let Some(ref mut omega_field) = state.turb_specific_dissipation {
                let omega_wall = u_tau / (c_mu_sqrt * self.kappa * y);
                let _ = omega_field.set(owner, omega_wall);
            }
        }

        Ok(())
    }

    /// Computes the y+ field for all wall-adjacent cells.
    ///
    /// y+ = y * u_tau / nu
    ///
    /// where y is the wall-normal distance, u_tau = sqrt(tau_w/rho) is the
    /// friction velocity, and nu is the kinematic viscosity.
    pub fn compute_y_plus(
        &self,
        state: &FluidState,
        mesh: &UnstructuredMesh,
    ) -> Result<ScalarField> {
        let n = mesh.num_cells();
        let mut y_plus_field = vec![0.0; n];

        // Identify wall boundary faces from patches containing "wall" in name
        for patch in &mesh.boundary_patches {
            let is_wall = patch.name.to_lowercase().contains("wall");
            if !is_wall {
                continue;
            }

            for &face_id in &patch.face_ids {
                let face = &mesh.faces[face_id];
                let owner = face.owner_cell;

                let cc = mesh.cells[owner].center;
                let fc = face.center;
                let y = ((cc[0] - fc[0]).powi(2)
                    + (cc[1] - fc[1]).powi(2)
                    + (cc[2] - fc[2]).powi(2))
                .sqrt()
                .max(1e-30);

                let rho = state.density.values()[owner];
                let nu = state.viscosity.values()[owner] / rho;

                // Wall-parallel velocity magnitude
                let vel = state.velocity.values()[owner];
                let u_n = vel[0] * face.normal[0]
                    + vel[1] * face.normal[1]
                    + vel[2] * face.normal[2];
                let u_par = [
                    vel[0] - u_n * face.normal[0],
                    vel[1] - u_n * face.normal[1],
                    vel[2] - u_n * face.normal[2],
                ];
                let u_mag = (u_par[0] * u_par[0] + u_par[1] * u_par[1] + u_par[2] * u_par[2])
                    .sqrt();

                // Compute u_tau via Newton iteration
                let mut u_tau = (nu * u_mag / y).sqrt().max(1e-10);

                for _iter in 0..20 {
                    let yp = u_tau * y / nu;
                    let u_plus_computed = if yp < self.y_plus_threshold {
                        yp
                    } else {
                        (1.0 / self.kappa) * yp.ln() + self.e_constant
                    };
                    let u_plus_target = u_mag / u_tau;
                    let residual = u_plus_computed - u_plus_target;
                    if residual.abs() < 1e-6 {
                        break;
                    }
                    let du_plus_du_tau = if yp < self.y_plus_threshold {
                        y / nu
                    } else {
                        1.0 / (self.kappa * u_tau)
                    };
                    let d_residual = du_plus_du_tau + u_mag / (u_tau * u_tau);
                    if d_residual.abs() > 1e-30 {
                        u_tau -= residual / d_residual;
                        u_tau = u_tau.max(1e-10);
                    }
                }

                y_plus_field[owner] = u_tau * y / nu;
            }
        }

        Ok(ScalarField::new("y_plus", y_plus_field))
    }
}

impl Default for WallTreatmentAdapter {
    fn default() -> Self {
        Self::new()
    }
}
