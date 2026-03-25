//! Compressible flow solver with various flux schemes.

pub mod roe;
pub mod hllc;
pub mod ausm;

use gfd_core::UnstructuredMesh;
use crate::{FluidState, Result};

/// Compressible flow solver using finite-volume Godunov-type methods.
pub struct CompressibleSolver {
    /// Flux scheme to use.
    pub flux_scheme: FluxScheme,
    /// CFL number for time step control.
    pub cfl: f64,
}

/// Available numerical flux schemes for compressible flow.
#[derive(Debug, Clone, Copy)]
pub enum FluxScheme {
    /// Roe's approximate Riemann solver.
    Roe,
    /// HLLC (Harten-Lax-van Leer-Contact) approximate Riemann solver.
    Hllc,
    /// AUSM+ (Advection Upstream Splitting Method).
    Ausm,
}

impl CompressibleSolver {
    /// Creates a new compressible solver with the given flux scheme.
    pub fn new(flux_scheme: FluxScheme, cfl: f64) -> Self {
        Self { flux_scheme, cfl }
    }

    /// Computes the numerical flux across a face using Roe's method.
    ///
    /// F = 0.5*(F_L + F_R) - 0.5*|A_roe|*(U_R - U_L)
    pub fn roe_flux(
        left: &ConservativeState,
        right: &ConservativeState,
        normal: [f64; 3],
    ) -> [f64; 5] {
        let roe = roe::RoeFlux::new(1.4);
        let flux = roe.compute_flux(left, right, normal);
        flux.as_array()
    }

    /// Computes the numerical flux across a face using HLLC.
    pub fn hllc_flux(
        left: &ConservativeState,
        right: &ConservativeState,
        normal: [f64; 3],
    ) -> [f64; 5] {
        let hllc_solver = hllc::HllcFlux::new(1.4);
        let flux = hllc_solver.compute_flux(left, right, normal);
        flux.as_array()
    }

    /// Computes the numerical flux across a face using AUSM+.
    pub fn ausm_flux(
        left: &ConservativeState,
        right: &ConservativeState,
        normal: [f64; 3],
    ) -> [f64; 5] {
        let ausm_solver = ausm::AusmPlusFlux::new(1.4);
        let flux = ausm_solver.compute_flux(left, right, normal);
        flux.as_array()
    }

    /// Advances the compressible solution by one time step.
    pub fn solve_step(
        &self,
        state: &mut FluidState,
        mesh: &UnstructuredMesh,
        dt: f64,
    ) -> Result<f64> {
        let gamma = 1.4;
        let n = mesh.num_cells();

        // Build conservative state per cell from FluidState
        let mut cons: Vec<ConservativeState> = Vec::with_capacity(n);
        for i in 0..n {
            let rho = state.density.values()[i];
            let vel = state.velocity.values()[i];
            let p = state.pressure.values()[i];
            let rho_u = rho * vel[0];
            let rho_v = rho * vel[1];
            let rho_w = rho * vel[2];
            let ke = 0.5 * rho * (vel[0] * vel[0] + vel[1] * vel[1] + vel[2] * vel[2]);
            let rho_e = p / (gamma - 1.0) + ke;
            cons.push(ConservativeState::new(rho, rho_u, rho_v, rho_w, rho_e));
        }

        // Accumulate flux residuals per cell: R_i = sum_faces(F * A)
        let mut residuals = vec![[0.0; 5]; n];

        for face in &mesh.faces {
            let owner = face.owner_cell;
            let left = &cons[owner];

            let right = if let Some(neighbor) = face.neighbor_cell {
                &cons[neighbor]
            } else {
                // Boundary: use the owner state (transmissive BC)
                left
            };

            let flux_arr = match self.flux_scheme {
                FluxScheme::Roe => Self::roe_flux(left, right, face.normal),
                FluxScheme::Hllc => Self::hllc_flux(left, right, face.normal),
                FluxScheme::Ausm => Self::ausm_flux(left, right, face.normal),
            };

            // Multiply by face area (flux is per unit area)
            for k in 0..5 {
                residuals[owner][k] += flux_arr[k] * face.area;
            }
            if let Some(neighbor) = face.neighbor_cell {
                for k in 0..5 {
                    residuals[neighbor][k] -= flux_arr[k] * face.area;
                }
            }
        }

        // Time integration: forward Euler
        // U^{n+1} = U^n - dt/V * R
        let mut max_residual = 0.0_f64;
        for i in 0..n {
            let vol = mesh.cells[i].volume;
            if vol <= 0.0 {
                continue;
            }
            let factor = dt / vol;
            cons[i] = ConservativeState::new(
                (cons[i].rho - factor * residuals[i][0]).max(1e-10),
                cons[i].rho_u - factor * residuals[i][1],
                cons[i].rho_v - factor * residuals[i][2],
                cons[i].rho_w - factor * residuals[i][3],
                cons[i].rho_e - factor * residuals[i][4],
            );

            for k in 0..5 {
                max_residual = max_residual.max(residuals[i][k].abs());
            }
        }

        // Convert back to primitive variables and update state
        for i in 0..n {
            let rho = cons[i].rho;
            let vel = cons[i].velocity();
            let p = cons[i].pressure(gamma).max(1e-10);
            let _ = state.density.set(i, rho);
            let _ = state.velocity.set(i, vel);
            let _ = state.pressure.set(i, p);
        }

        Ok(max_residual)
    }
}

/// Conservative state vector for compressible flow: [rho, rho*u, rho*v, rho*w, rho*E].
#[derive(Debug, Clone, Copy)]
pub struct ConservativeState {
    /// Density [kg/m^3].
    pub rho: f64,
    /// x-momentum [kg/(m^2*s)].
    pub rho_u: f64,
    /// y-momentum [kg/(m^2*s)].
    pub rho_v: f64,
    /// z-momentum [kg/(m^2*s)].
    pub rho_w: f64,
    /// Total energy per unit volume [J/m^3].
    pub rho_e: f64,
}

impl ConservativeState {
    /// Creates a new conservative state.
    pub fn new(rho: f64, rho_u: f64, rho_v: f64, rho_w: f64, rho_e: f64) -> Self {
        Self {
            rho,
            rho_u,
            rho_v,
            rho_w,
            rho_e,
        }
    }

    /// Computes the velocity components.
    pub fn velocity(&self) -> [f64; 3] {
        [
            self.rho_u / self.rho,
            self.rho_v / self.rho,
            self.rho_w / self.rho,
        ]
    }

    /// Computes the pressure using the ideal gas law.
    ///
    /// p = (gamma - 1) * (rho*E - 0.5*rho*(u^2+v^2+w^2))
    pub fn pressure(&self, gamma: f64) -> f64 {
        let vel = self.velocity();
        let ke = 0.5 * self.rho * (vel[0] * vel[0] + vel[1] * vel[1] + vel[2] * vel[2]);
        (gamma - 1.0) * (self.rho_e - ke)
    }
}
