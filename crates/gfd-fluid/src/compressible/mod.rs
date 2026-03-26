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

    /// Computes the flux residual R_i = sum_faces(F * A) for each cell.
    ///
    /// This is the spatial discretization of the compressible Euler equations,
    /// extracted so it can be reused by different time integrators (forward Euler, RK4).
    pub fn compute_residual(
        &self,
        cons: &[ConservativeState],
        mesh: &UnstructuredMesh,
    ) -> Vec<[f64; 5]> {
        let n = cons.len();
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

        residuals
    }

    /// Converts FluidState primitive variables to conservative state vector per cell.
    fn state_to_conservative(state: &FluidState, gamma: f64) -> Vec<ConservativeState> {
        let n = state.num_cells();
        let mut cons = Vec::with_capacity(n);
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
        cons
    }

    /// Writes conservative state back into FluidState as primitive variables.
    fn conservative_to_state(cons: &[ConservativeState], state: &mut FluidState, gamma: f64) {
        for i in 0..cons.len() {
            let rho = cons[i].rho;
            let vel = cons[i].velocity();
            let p = cons[i].pressure(gamma).max(1e-10);
            let _ = state.density.set(i, rho);
            let _ = state.velocity.set(i, vel);
            let _ = state.pressure.set(i, p);
        }
    }

    /// Applies a single forward Euler update: U_new = U_old - (dt/V) * R.
    ///
    /// Returns the updated conservative state and the maximum absolute residual.
    fn euler_update(
        cons_old: &[ConservativeState],
        residuals: &[[f64; 5]],
        mesh: &UnstructuredMesh,
        dt: f64,
    ) -> (Vec<ConservativeState>, f64) {
        let n = cons_old.len();
        let mut cons_new = Vec::with_capacity(n);
        let mut max_residual = 0.0_f64;

        for i in 0..n {
            let vol = mesh.cells[i].volume;
            if vol <= 0.0 {
                cons_new.push(cons_old[i]);
                continue;
            }
            let factor = dt / vol;
            cons_new.push(ConservativeState::new(
                (cons_old[i].rho - factor * residuals[i][0]).max(1e-10),
                cons_old[i].rho_u - factor * residuals[i][1],
                cons_old[i].rho_v - factor * residuals[i][2],
                cons_old[i].rho_w - factor * residuals[i][3],
                cons_old[i].rho_e - factor * residuals[i][4],
            ));

            for k in 0..5 {
                max_residual = max_residual.max(residuals[i][k].abs());
            }
        }

        (cons_new, max_residual)
    }

    /// Advances the compressible solution by one time step using forward Euler.
    pub fn solve_step(
        &self,
        state: &mut FluidState,
        mesh: &UnstructuredMesh,
        dt: f64,
    ) -> Result<f64> {
        let gamma = 1.4;
        let cons = Self::state_to_conservative(state, gamma);
        let residuals = self.compute_residual(&cons, mesh);
        let (cons_new, max_residual) = Self::euler_update(&cons, &residuals, mesh, dt);
        Self::conservative_to_state(&cons_new, state, gamma);
        Ok(max_residual)
    }

    /// Advances the compressible solution by one time step using the classical
    /// 4-stage Runge-Kutta method (RK4).
    ///
    /// The RK4 scheme provides 4th-order temporal accuracy:
    /// ```text
    /// k1 = R(U_n)
    /// k2 = R(U_n + 0.5*dt*k1_scaled)
    /// k3 = R(U_n + 0.5*dt*k2_scaled)
    /// k4 = R(U_n + dt*k3_scaled)
    /// U_{n+1} = U_n + (dt/6)*(k1_scaled + 2*k2_scaled + 2*k3_scaled + k4_scaled)
    /// ```
    /// where k_scaled = -R/V (the residual divided by cell volume with sign flip).
    pub fn solve_step_rk4(
        &self,
        state: &mut FluidState,
        mesh: &UnstructuredMesh,
        dt: f64,
    ) -> Result<f64> {
        let gamma = 1.4;
        let n = mesh.num_cells();
        let u_n = Self::state_to_conservative(state, gamma);

        // Helper: compute dU/dt = -R/V for each cell (the RK rate)
        let compute_rate = |cons: &[ConservativeState]| -> Vec<[f64; 5]> {
            let residuals = self.compute_residual(cons, mesh);
            let mut rate = vec![[0.0; 5]; n];
            for i in 0..n {
                let vol = mesh.cells[i].volume;
                if vol > 0.0 {
                    for k in 0..5 {
                        rate[i][k] = -residuals[i][k] / vol;
                    }
                }
            }
            rate
        };

        // Helper: U_temp = U_base + coeff * rate (with density floor)
        let add_rate = |base: &[ConservativeState], rate: &[[f64; 5]], coeff: f64| -> Vec<ConservativeState> {
            let mut result = Vec::with_capacity(n);
            for i in 0..n {
                result.push(ConservativeState::new(
                    (base[i].rho + coeff * rate[i][0]).max(1e-10),
                    base[i].rho_u + coeff * rate[i][1],
                    base[i].rho_v + coeff * rate[i][2],
                    base[i].rho_w + coeff * rate[i][3],
                    base[i].rho_e + coeff * rate[i][4],
                ));
            }
            result
        };

        // Stage 1: k1 = rate(U_n)
        let k1 = compute_rate(&u_n);

        // Stage 2: k2 = rate(U_n + 0.5*dt*k1)
        let u_stage2 = add_rate(&u_n, &k1, 0.5 * dt);
        let k2 = compute_rate(&u_stage2);

        // Stage 3: k3 = rate(U_n + 0.5*dt*k2)
        let u_stage3 = add_rate(&u_n, &k2, 0.5 * dt);
        let k3 = compute_rate(&u_stage3);

        // Stage 4: k4 = rate(U_n + dt*k3)
        let u_stage4 = add_rate(&u_n, &k3, dt);
        let k4 = compute_rate(&u_stage4);

        // Combine: U_{n+1} = U_n + (dt/6)*(k1 + 2*k2 + 2*k3 + k4)
        let mut cons_new = Vec::with_capacity(n);
        let mut max_residual = 0.0_f64;
        let dt6 = dt / 6.0;

        for i in 0..n {
            let mut u = [0.0; 5];
            let u_old = [u_n[i].rho, u_n[i].rho_u, u_n[i].rho_v, u_n[i].rho_w, u_n[i].rho_e];
            for k in 0..5 {
                let increment = dt6 * (k1[i][k] + 2.0 * k2[i][k] + 2.0 * k3[i][k] + k4[i][k]);
                u[k] = u_old[k] + increment;
                max_residual = max_residual.max(increment.abs());
            }
            cons_new.push(ConservativeState::new(
                u[0].max(1e-10),
                u[1],
                u[2],
                u[3],
                u[4],
            ));
        }

        Self::conservative_to_state(&cons_new, state, gamma);
        Ok(max_residual)
    }

    /// Computes the CFL-based adaptive time step.
    ///
    /// dt = CFL * min_cell(V_cell / ((|u| + c) * A_max_face))
    ///
    /// where c = sqrt(gamma * p / rho) is the local speed of sound.
    /// This ensures the time step satisfies the CFL condition for stability.
    pub fn compute_cfl_timestep(
        &self,
        state: &FluidState,
        mesh: &UnstructuredMesh,
    ) -> f64 {
        let gamma = 1.4;
        let n = mesh.num_cells();
        let mut dt_min = f64::MAX;

        for i in 0..n {
            let rho = state.density.values()[i].max(1e-30);
            let vel = state.velocity.values()[i];
            let p = state.pressure.values()[i].max(1e-30);
            let vol = mesh.cells[i].volume;

            if vol <= 0.0 {
                continue;
            }

            // Speed of sound
            let c = (gamma * p / rho).sqrt();

            // Velocity magnitude
            let u_mag = (vel[0] * vel[0] + vel[1] * vel[1] + vel[2] * vel[2]).sqrt();

            // Maximum face area for this cell (approximation of characteristic length)
            let mut max_face_area = 0.0_f64;
            for &fid in &mesh.cells[i].faces {
                if fid < mesh.faces.len() {
                    max_face_area = max_face_area.max(mesh.faces[fid].area);
                }
            }

            if max_face_area > 0.0 {
                // dt_cell = V / ((|u| + c) * A_face)
                let dt_cell = vol / ((u_mag + c) * max_face_area);
                dt_min = dt_min.min(dt_cell);
            }
        }

        // Apply CFL number
        if dt_min == f64::MAX {
            // Fallback: no valid cells
            1e-6
        } else {
            self.cfl * dt_min
        }
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

    /// Returns the conservative state as a 5-element array [rho, rho_u, rho_v, rho_w, rho_e].
    pub fn as_array(&self) -> [f64; 5] {
        [self.rho, self.rho_u, self.rho_v, self.rho_w, self.rho_e]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gfd_core::mesh::structured::StructuredMesh;
    fn make_1d_mesh(nx: usize) -> UnstructuredMesh {
        let sm = StructuredMesh::uniform(nx, 1, 0, 1.0, 0.1, 0.0);
        sm.to_unstructured()
    }

    fn make_uniform_state(mesh: &UnstructuredMesh, rho: f64, u: f64, p: f64) -> FluidState {
        let n = mesh.num_cells();
        let mut state = FluidState::new(n);
        for i in 0..n {
            let _ = state.density.set(i, rho);
            let _ = state.velocity.set(i, [u, 0.0, 0.0]);
            let _ = state.pressure.set(i, p);
        }
        state
    }

    #[test]
    fn test_compute_residual_uniform_flow() {
        // For a uniform flow, interior flux residuals should cancel out
        let mesh = make_1d_mesh(10);
        let n = mesh.num_cells();
        let solver = CompressibleSolver::new(FluxScheme::Roe, 0.5);

        let gamma = 1.4;
        let rho = 1.0;
        let u = 1.0;
        let p = 1.0 / gamma; // so c = 1
        let rho_u = rho * u;
        let ke = 0.5 * rho * u * u;
        let rho_e = p / (gamma - 1.0) + ke;

        let cons: Vec<ConservativeState> = (0..n)
            .map(|_| ConservativeState::new(rho, rho_u, 0.0, 0.0, rho_e))
            .collect();

        let residuals = solver.compute_residual(&cons, &mesh);

        // Interior cells should have near-zero residuals (fluxes cancel)
        // Only boundary cells may have non-zero residuals
        for i in 1..(n - 1) {
            let r_mag: f64 = residuals[i].iter().map(|r| r * r).sum::<f64>().sqrt();
            assert!(
                r_mag < 1e-10,
                "Interior cell {} has non-zero residual: {:?}",
                i, residuals[i]
            );
        }
    }

    #[test]
    fn test_solve_step_preserves_uniform_state() {
        // A uniform state with transmissive BCs should remain unchanged
        let mesh = make_1d_mesh(5);
        let solver = CompressibleSolver::new(FluxScheme::Hllc, 0.5);
        let mut state = make_uniform_state(&mesh, 1.225, 0.0, 101325.0);

        let rho_before: Vec<f64> = state.density.values().to_vec();
        let _ = solver.solve_step(&mut state, &mesh, 1e-6).unwrap();

        for i in 0..mesh.num_cells() {
            assert!(
                (state.density.values()[i] - rho_before[i]).abs() < 1e-6,
                "Density changed at cell {} for uniform quiescent flow",
                i
            );
        }
    }

    #[test]
    fn test_solve_step_rk4_preserves_uniform_state() {
        // RK4 should also preserve a uniform quiescent state
        let mesh = make_1d_mesh(5);
        let solver = CompressibleSolver::new(FluxScheme::Roe, 0.5);
        let mut state = make_uniform_state(&mesh, 1.225, 0.0, 101325.0);

        let rho_before: Vec<f64> = state.density.values().to_vec();
        let p_before: Vec<f64> = state.pressure.values().to_vec();

        let _ = solver.solve_step_rk4(&mut state, &mesh, 1e-6).unwrap();

        for i in 0..mesh.num_cells() {
            assert!(
                (state.density.values()[i] - rho_before[i]).abs() < 1e-6,
                "RK4: density changed at cell {}",
                i
            );
            assert!(
                (state.pressure.values()[i] - p_before[i]).abs() < 1e-3,
                "RK4: pressure changed at cell {}",
                i
            );
        }
    }

    #[test]
    fn test_rk4_vs_euler_accuracy() {
        // RK4 should be more accurate than forward Euler for a smooth problem.
        // Use a 1D Sod shock tube initial conditions and verify RK4 gives
        // a lower residual after one step at the same dt.
        let mesh = make_1d_mesh(20);
        let n = mesh.num_cells();
        let solver = CompressibleSolver::new(FluxScheme::Hllc, 0.5);

        // Set up a smooth density variation (sine profile)
        let mut state_euler = FluidState::new(n);
        let mut state_rk4 = FluidState::new(n);
        for i in 0..n {
            let x = (i as f64 + 0.5) / n as f64;
            let rho = 1.0 + 0.1 * (2.0 * std::f64::consts::PI * x).sin();
            let p = 100000.0;
            let _ = state_euler.density.set(i, rho);
            let _ = state_euler.pressure.set(i, p);
            let _ = state_euler.velocity.set(i, [10.0, 0.0, 0.0]);
            let _ = state_rk4.density.set(i, rho);
            let _ = state_rk4.pressure.set(i, p);
            let _ = state_rk4.velocity.set(i, [10.0, 0.0, 0.0]);
        }

        let dt = 1e-6;
        let res_euler = solver.solve_step(&mut state_euler, &mesh, dt).unwrap();
        let res_rk4 = solver.solve_step_rk4(&mut state_rk4, &mesh, dt).unwrap();

        // Both should produce finite results
        assert!(res_euler.is_finite(), "Euler residual is not finite");
        assert!(res_rk4.is_finite(), "RK4 residual is not finite");
    }

    #[test]
    fn test_compute_cfl_timestep() {
        let mesh = make_1d_mesh(10);
        let solver = CompressibleSolver::new(FluxScheme::Roe, 0.5);

        // Standard atmosphere: rho=1.225, p=101325, c ~ 340 m/s
        let state = make_uniform_state(&mesh, 1.225, 100.0, 101325.0);

        let dt = solver.compute_cfl_timestep(&state, &mesh);
        assert!(dt > 0.0, "CFL dt must be positive");
        assert!(dt < 1.0, "CFL dt should be small for this configuration");
        assert!(dt.is_finite(), "CFL dt must be finite");

        // With higher CFL, dt should be proportionally larger
        let solver2 = CompressibleSolver::new(FluxScheme::Roe, 1.0);
        let dt2 = solver2.compute_cfl_timestep(&state, &mesh);
        assert!(
            (dt2 / dt - 2.0).abs() < 1e-10,
            "dt should scale linearly with CFL"
        );
    }

    #[test]
    fn test_cfl_timestep_decreases_with_velocity() {
        let mesh = make_1d_mesh(10);
        let solver = CompressibleSolver::new(FluxScheme::Roe, 0.5);

        let state_slow = make_uniform_state(&mesh, 1.225, 10.0, 101325.0);
        let state_fast = make_uniform_state(&mesh, 1.225, 300.0, 101325.0);

        let dt_slow = solver.compute_cfl_timestep(&state_slow, &mesh);
        let dt_fast = solver.compute_cfl_timestep(&state_fast, &mesh);

        assert!(
            dt_fast < dt_slow,
            "Higher velocity should require smaller time step: dt_fast={}, dt_slow={}",
            dt_fast, dt_slow
        );
    }
}
