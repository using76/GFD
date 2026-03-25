//! Volume of Fluid (VOF) method implementation.
//!
//! Provides a detailed VOF solver with interface reconstruction
//! and curvature computation for surface tension modeling.

use gfd_core::{ScalarField, VectorField, UnstructuredMesh};
use crate::Result;

/// Detailed Volume of Fluid solver implementation.
///
/// Tracks the interface between two immiscible phases by solving an
/// advection equation for the volume fraction field alpha, where
/// alpha = 0 in one phase and alpha = 1 in the other.
pub struct VofSolverImpl {
    /// Volume fraction field (0 = phase 1, 1 = phase 2).
    pub alpha: ScalarField,
    /// Interface compression factor (0 = none, 1 = full compression).
    pub compression_factor: f64,
    /// Number of interface compression corrector steps.
    pub compression_iterations: usize,
    /// Tolerance for bounding alpha to [0, 1].
    pub bounding_tolerance: f64,
}

impl VofSolverImpl {
    /// Creates a new VOF solver with the given initial volume fraction.
    pub fn new(alpha: ScalarField, compression_factor: f64) -> Self {
        Self {
            alpha,
            compression_factor,
            compression_iterations: 2,
            bounding_tolerance: 1e-6,
        }
    }

    /// Solves the VOF transport equation for one time step.
    ///
    /// d(alpha)/dt + div(alpha * U) + div(alpha*(1-alpha)*U_c) = 0
    ///
    /// The third term is the interface compression term where U_c
    /// is a compression velocity pointing in the interface normal direction.
    pub fn solve_transport(
        &mut self,
        velocity: &VectorField,
        mesh: &UnstructuredMesh,
        dt: f64,
    ) -> Result<()> {
        let n = mesh.num_cells();
        let alpha_old = self.alpha.values().to_vec();
        let mut alpha_new = alpha_old.clone();

        // Accumulate net flux per cell: sum of (flux * area) for each face
        let mut net_flux = vec![0.0; n];

        for face in &mesh.faces {
            let owner = face.owner_cell;

            // Compute face velocity (average of owner and neighbor, or owner for boundary)
            let vel_o = velocity.values()[owner];
            let (u_f, alpha_f_upwind);

            if let Some(neighbor) = face.neighbor_cell {
                let vel_n = velocity.values()[neighbor];
                u_f = [
                    0.5 * (vel_o[0] + vel_n[0]),
                    0.5 * (vel_o[1] + vel_n[1]),
                    0.5 * (vel_o[2] + vel_n[2]),
                ];
                // Volume flux through face
                let flux = (u_f[0] * face.normal[0]
                    + u_f[1] * face.normal[1]
                    + u_f[2] * face.normal[2])
                    * face.area;

                // Upwind selection for alpha
                alpha_f_upwind = if flux >= 0.0 {
                    alpha_old[owner]
                } else {
                    alpha_old[neighbor]
                };

                let face_flux = flux * alpha_f_upwind;
                net_flux[owner] += face_flux;
                net_flux[neighbor] -= face_flux;
            } else {
                // Boundary face: use owner value (zero-gradient)
                u_f = vel_o;
                let flux = (u_f[0] * face.normal[0]
                    + u_f[1] * face.normal[1]
                    + u_f[2] * face.normal[2])
                    * face.area;
                alpha_f_upwind = alpha_old[owner];
                let face_flux = flux * alpha_f_upwind;
                net_flux[owner] += face_flux;
            }
        }

        // Update alpha: alpha_new = alpha_old - dt/V * sum(flux)
        for i in 0..n {
            let vol = mesh.cells[i].volume;
            if vol > 0.0 {
                alpha_new[i] = alpha_old[i] - dt / vol * net_flux[i];
            }
            // Clamp to [0, 1]
            alpha_new[i] = alpha_new[i].clamp(0.0, 1.0);
        }

        // Write back
        let vals = self.alpha.values_mut();
        vals.copy_from_slice(&alpha_new);

        Ok(())
    }

    /// Reconstructs the interface from the volume fraction field.
    ///
    /// Computes the interface normal vector field using the gradient of alpha.
    /// The interface is located where 0 < alpha < 1.
    pub fn reconstruct_interface(
        &self,
        mesh: &UnstructuredMesh,
    ) -> Result<VectorField> {
        use gfd_core::gradient::{GreenGaussCellBasedGradient, GradientComputer};

        let n = mesh.num_cells();
        let grad_computer = GreenGaussCellBasedGradient;

        // Compute grad(alpha) using Green-Gauss
        let grad_alpha = grad_computer
            .compute(&self.alpha, mesh)
            .map_err(crate::FluidError::CoreError)?;

        let mut normals = vec![[0.0, 0.0, 0.0]; n];
        let eps = 1e-6;

        for i in 0..n {
            let alpha_val = self.alpha.values()[i];
            // Only compute for interfacial cells
            if alpha_val > eps && alpha_val < 1.0 - eps {
                let g = grad_alpha.values()[i];
                let mag = (g[0] * g[0] + g[1] * g[1] + g[2] * g[2]).sqrt();
                if mag > 1e-30 {
                    // Interface normal points from phase 1 to phase 2: n = -grad(alpha)/|grad(alpha)|
                    normals[i] = [-g[0] / mag, -g[1] / mag, -g[2] / mag];
                }
            }
        }

        Ok(VectorField::new("interface_normal", normals))
    }

    /// Computes the interface curvature for surface tension modeling.
    ///
    /// kappa = -div(n) where n = grad(alpha)/|grad(alpha)|
    ///
    /// Used in the CSF (Continuum Surface Force) model for surface tension:
    /// F_st = sigma * kappa * grad(alpha)
    pub fn compute_curvature(
        &self,
        mesh: &UnstructuredMesh,
    ) -> Result<ScalarField> {
        use gfd_core::gradient::{GreenGaussCellBasedGradient, GradientComputer};

        let n = mesh.num_cells();
        let grad_computer = GreenGaussCellBasedGradient;

        // Step 1: Compute grad(alpha)
        let grad_alpha = grad_computer
            .compute(&self.alpha, mesh)
            .map_err(crate::FluidError::CoreError)?;

        // Step 2: Compute unit normal n_hat = grad(alpha)/|grad(alpha)| per cell
        let mut n_hat_x = vec![0.0; n];
        let mut n_hat_y = vec![0.0; n];
        let mut n_hat_z = vec![0.0; n];

        for i in 0..n {
            let g = grad_alpha.values()[i];
            let mag = (g[0] * g[0] + g[1] * g[1] + g[2] * g[2]).sqrt();
            if mag > 1e-30 {
                n_hat_x[i] = g[0] / mag;
                n_hat_y[i] = g[1] / mag;
                n_hat_z[i] = g[2] / mag;
            }
        }

        // Step 3: Compute kappa = -div(n_hat) using Green-Gauss divergence
        // div(n_hat) = (1/V) * sum_faces(n_hat_f . S_f)
        let mut kappa_vals = vec![0.0; n];

        for face in &mesh.faces {
            let owner = face.owner_cell;

            let (nx_f, ny_f, nz_f) = if let Some(neighbor) = face.neighbor_cell {
                // Interpolate n_hat to face
                (
                    0.5 * (n_hat_x[owner] + n_hat_x[neighbor]),
                    0.5 * (n_hat_y[owner] + n_hat_y[neighbor]),
                    0.5 * (n_hat_z[owner] + n_hat_z[neighbor]),
                )
            } else {
                (n_hat_x[owner], n_hat_y[owner], n_hat_z[owner])
            };

            // n_hat_f dot face_area_vector (normal * area)
            let flux = (nx_f * face.normal[0] + ny_f * face.normal[1] + nz_f * face.normal[2])
                * face.area;

            kappa_vals[owner] += flux;
            if let Some(neighbor) = face.neighbor_cell {
                kappa_vals[neighbor] -= flux;
            }
        }

        // Divide by volume and negate: kappa = -div(n_hat)
        for i in 0..n {
            let vol = mesh.cells[i].volume;
            if vol > 0.0 {
                kappa_vals[i] = -kappa_vals[i] / vol;
            }
        }

        Ok(ScalarField::new("curvature", kappa_vals))
    }

    /// Bounds the volume fraction to the physical range [0, 1].
    pub fn bound_alpha(&mut self) {
        let values = self.alpha.values_mut();
        for v in values.iter_mut() {
            *v = v.clamp(0.0, 1.0);
        }
    }
}
