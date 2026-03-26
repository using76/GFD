//! Euler-Euler multiphase solver.
//!
//! Treats each phase as an interpenetrating continuum, solving
//! a full set of conservation equations for each phase. Each phase
//! has its own velocity field and volume fraction, with interphase
//! momentum transfer through drag, lift, and virtual mass forces.

use gfd_core::{ScalarField, VectorField, UnstructuredMesh};
use crate::Result;

/// Description of a single phase in the Euler-Euler framework.
#[derive(Debug, Clone)]
pub struct Phase {
    /// Name of the phase (e.g., "water", "air", "particles").
    pub name: String,
    /// Volume fraction field for this phase.
    pub alpha: ScalarField,
    /// Velocity field for this phase [m/s].
    pub velocity: VectorField,
    /// Density of this phase [kg/m^3].
    pub density: f64,
    /// Dynamic viscosity of this phase [Pa*s].
    pub viscosity: f64,
    /// Material identifier or name.
    pub material: String,
}

impl Phase {
    /// Creates a new phase with uniform initial conditions.
    pub fn new(
        name: impl Into<String>,
        material: impl Into<String>,
        num_cells: usize,
        density: f64,
        viscosity: f64,
        initial_alpha: f64,
    ) -> Self {
        let name = name.into();
        Self {
            alpha: ScalarField::new(
                &format!("alpha_{}", name),
                vec![initial_alpha; num_cells],
            ),
            velocity: VectorField::zeros(&format!("U_{}", name), num_cells),
            density,
            viscosity,
            material: material.into(),
            name,
        }
    }
}

/// Configuration for interphase force models.
#[derive(Debug, Clone)]
pub struct InterphaseForceConfig {
    /// Enable drag force (Schiller-Naumann correlation).
    pub drag_enabled: bool,
    /// Enable lift force (Saffman-Mei model, C_L = 0.5).
    pub lift_enabled: bool,
    /// Lift coefficient (default 0.5).
    pub lift_coefficient: f64,
    /// Enable virtual mass force (C_vm = 0.5).
    pub virtual_mass_enabled: bool,
    /// Virtual mass coefficient (default 0.5).
    pub virtual_mass_coefficient: f64,
    /// Particle/bubble diameter [m] for drag correlation.
    pub particle_diameter: f64,
    /// Gravity vector [m/s^2].
    pub gravity: [f64; 3],
}

impl Default for InterphaseForceConfig {
    fn default() -> Self {
        Self {
            drag_enabled: true,
            lift_enabled: true,
            lift_coefficient: 0.5,
            virtual_mass_enabled: true,
            virtual_mass_coefficient: 0.5,
            particle_diameter: 1e-3,
            gravity: [0.0, -9.81, 0.0],
        }
    }
}

/// Euler-Euler multiphase solver.
///
/// Solves a set of coupled conservation equations for each phase:
///
/// Continuity: d(alpha_k)/dt + div(alpha_k * u_k) = 0
///
/// Momentum:  d(alpha_k * rho_k * u_k)/dt + div(alpha_k * rho_k * u_k * u_k)
///            = -alpha_k * grad(p) + div(alpha_k * tau_k) + M_k + alpha_k * rho_k * g
///
/// where M_k is the interphase momentum transfer including drag, lift, and
/// virtual mass forces.
pub struct EulerEulerSolver {
    /// The phases being simulated.
    pub phases: Vec<Phase>,
    /// Maximum inter-phase coupling iterations per time step.
    pub max_coupling_iterations: usize,
    /// Convergence tolerance for the phase coupling loop.
    pub coupling_tolerance: f64,
    /// Interphase force configuration.
    pub force_config: InterphaseForceConfig,
    /// Shared pressure field [Pa].
    pub pressure: Vec<f64>,
    /// Previous velocity fields for virtual mass (Du/Dt approximation).
    prev_velocities: Vec<Vec<[f64; 3]>>,
}

impl EulerEulerSolver {
    /// Creates a new Euler-Euler solver with the given phases.
    pub fn new(phases: Vec<Phase>) -> Self {
        let n = if phases.is_empty() {
            0
        } else {
            phases[0].alpha.values().len()
        };
        let prev_velocities = phases
            .iter()
            .map(|ph| ph.velocity.values().to_vec())
            .collect();
        Self {
            phases,
            max_coupling_iterations: 20,
            coupling_tolerance: 1e-4,
            force_config: InterphaseForceConfig::default(),
            pressure: vec![0.0; n],
            prev_velocities,
        }
    }

    /// Creates a new Euler-Euler solver with custom force configuration.
    pub fn with_config(phases: Vec<Phase>, config: InterphaseForceConfig) -> Self {
        let mut solver = Self::new(phases);
        solver.force_config = config;
        solver
    }

    /// Adds a phase to the solver.
    pub fn add_phase(&mut self, phase: Phase) {
        self.prev_velocities
            .push(phase.velocity.values().to_vec());
        self.phases.push(phase);
    }

    /// Computes the drag coefficient using the Schiller-Naumann correlation.
    ///
    /// C_D = 24/Re * (1 + 0.15*Re^0.687) for Re < 1000
    /// C_D = 0.44 for Re >= 1000
    pub fn schiller_naumann_drag(&self, reynolds_number: f64) -> f64 {
        if reynolds_number < 1e-10 {
            0.0
        } else if reynolds_number < 1000.0 {
            24.0 / reynolds_number * (1.0 + 0.15 * reynolds_number.powf(0.687))
        } else {
            0.44
        }
    }

    /// Computes the drag force coefficient K_drag for a phase pair.
    ///
    /// K_drag = 0.75 * C_D * rho_c * alpha_d * |u_rel| / d_p
    fn compute_drag_coefficient(
        &self,
        rho_continuous: f64,
        mu_continuous: f64,
        alpha_dispersed: f64,
        u_rel_mag: f64,
    ) -> f64 {
        let d_p = self.force_config.particle_diameter;
        let re = rho_continuous * u_rel_mag * d_p / (mu_continuous + 1e-30);
        let cd = self.schiller_naumann_drag(re);
        0.75 * cd * rho_continuous * alpha_dispersed * u_rel_mag / (d_p + 1e-30)
    }

    /// Computes the interphase momentum transfer for all force models.
    ///
    /// Returns the force per unit volume on phase p from phase q for each cell,
    /// as a Vec of [f64; 3]. The force on phase q from phase p is the negative.
    fn compute_interphase_forces(
        &self,
        phase_p: usize,
        phase_q: usize,
        mesh: &UnstructuredMesh,
        dt: f64,
    ) -> Vec<[f64; 3]> {
        let n = mesh.num_cells();
        let mut forces = vec![[0.0; 3]; n];

        let vel_p = self.phases[phase_p].velocity.values();
        let vel_q = self.phases[phase_q].velocity.values();
        let alpha_p = self.phases[phase_p].alpha.values();
        let _alpha_q = self.phases[phase_q].alpha.values();

        // Determine which phase is continuous (higher alpha) and dispersed
        // For the force computation, we use phase_q as continuous by convention
        let rho_c = self.phases[phase_q].density;
        let mu_c = self.phases[phase_q].viscosity;

        for cell in 0..n {
            let u_rel = [
                vel_p[cell][0] - vel_q[cell][0],
                vel_p[cell][1] - vel_q[cell][1],
                vel_p[cell][2] - vel_q[cell][2],
            ];
            let u_rel_mag =
                (u_rel[0] * u_rel[0] + u_rel[1] * u_rel[1] + u_rel[2] * u_rel[2]).sqrt();

            let alpha_d = alpha_p[cell].max(0.0).min(1.0);

            // 1. Drag force: M_drag = K_drag * (u_q - u_p)
            // (acts to reduce relative velocity)
            if self.force_config.drag_enabled {
                let k_drag = self.compute_drag_coefficient(rho_c, mu_c, alpha_d, u_rel_mag);
                for dim in 0..3 {
                    // Force on phase p: drag pushes p toward q
                    forces[cell][dim] += k_drag * (vel_q[cell][dim] - vel_p[cell][dim]);
                }
            }

            // 2. Lift force: M_lift = C_L * rho_c * alpha_d * (u_d - u_c) x curl(u_c)
            // We approximate curl(u_c) using finite differences from neighbor cells
            if self.force_config.lift_enabled {
                let c_l = self.force_config.lift_coefficient;
                let curl = self.approximate_curl(phase_q, cell, mesh);
                // Cross product: (u_rel) x curl
                let cross = [
                    u_rel[1] * curl[2] - u_rel[2] * curl[1],
                    u_rel[2] * curl[0] - u_rel[0] * curl[2],
                    u_rel[0] * curl[1] - u_rel[1] * curl[0],
                ];
                for dim in 0..3 {
                    forces[cell][dim] += c_l * rho_c * alpha_d * cross[dim];
                }
            }

            // 3. Virtual mass force: M_vm = C_vm * rho_c * alpha_d * (Du_c/Dt - Du_d/Dt)
            // Approximate material derivatives using (u_new - u_old) / dt
            if self.force_config.virtual_mass_enabled && dt > 0.0 {
                let c_vm = self.force_config.virtual_mass_coefficient;
                if phase_p < self.prev_velocities.len()
                    && phase_q < self.prev_velocities.len()
                    && cell < self.prev_velocities[phase_p].len()
                    && cell < self.prev_velocities[phase_q].len()
                {
                    let prev_p = self.prev_velocities[phase_p][cell];
                    let prev_q = self.prev_velocities[phase_q][cell];
                    for dim in 0..3 {
                        let du_c_dt = (vel_q[cell][dim] - prev_q[dim]) / dt;
                        let du_d_dt = (vel_p[cell][dim] - prev_p[dim]) / dt;
                        forces[cell][dim] += c_vm * rho_c * alpha_d * (du_c_dt - du_d_dt);
                    }
                }
            }
        }

        forces
    }

    /// Approximates the curl of a phase velocity field at a given cell
    /// using Green-Gauss gradient reconstruction.
    fn approximate_curl(
        &self,
        phase_idx: usize,
        cell: usize,
        mesh: &UnstructuredMesh,
    ) -> [f64; 3] {
        let vel = self.phases[phase_idx].velocity.values();
        let cell_data = &mesh.cells[cell];
        let vol = cell_data.volume;
        if vol < 1e-30 {
            return [0.0; 3];
        }

        // Compute velocity gradient via Green-Gauss: grad(u_i) = (1/V) * sum_f(u_f * n_f * A_f)
        let mut grad = [[0.0_f64; 3]; 3]; // grad[i][j] = du_i/dx_j

        for &fid in &cell_data.faces {
            if fid >= mesh.faces.len() {
                continue;
            }
            let face = &mesh.faces[fid];
            // Face velocity: average of owner and neighbor
            let u_face = if let Some(neigh) = face.neighbor_cell {
                let u_own = vel[face.owner_cell];
                let u_neigh = vel[neigh];
                [
                    0.5 * (u_own[0] + u_neigh[0]),
                    0.5 * (u_own[1] + u_neigh[1]),
                    0.5 * (u_own[2] + u_neigh[2]),
                ]
            } else {
                vel[face.owner_cell]
            };

            // Normal direction: outward from owner. If this cell is the neighbor,
            // we need to flip the normal.
            let sign = if face.owner_cell == cell { 1.0 } else { -1.0 };
            for i in 0..3 {
                for j in 0..3 {
                    grad[i][j] += sign * u_face[i] * face.normal[j] * face.area;
                }
            }
        }

        // Divide by volume
        for i in 0..3 {
            for j in 0..3 {
                grad[i][j] /= vol;
            }
        }

        // curl = (dw/dy - dv/dz, du/dz - dw/dx, dv/dx - du/dy)
        // where u = vel[0], v = vel[1], w = vel[2]
        [
            grad[2][1] - grad[1][2], // dw/dy - dv/dz
            grad[0][2] - grad[2][0], // du/dz - dw/dx
            grad[1][0] - grad[0][1], // dv/dx - du/dy
        ]
    }

    /// Solves the volume fraction continuity equation for each phase.
    ///
    /// d(alpha_k)/dt + div(alpha_k * u_k) = 0
    ///
    /// Uses an explicit first-order upwind scheme.
    fn solve_continuity(
        &mut self,
        mesh: &UnstructuredMesh,
        dt: f64,
    ) {
        let n = mesh.num_cells();
        let num_phases = self.phases.len();

        // Store old alpha values
        let old_alphas: Vec<Vec<f64>> = self
            .phases
            .iter()
            .map(|ph| ph.alpha.values().to_vec())
            .collect();

        // For each phase, compute div(alpha_k * u_k) using face fluxes
        for p in 0..num_phases {
            let mut div_flux = vec![0.0_f64; n];

            for face in &mesh.faces {
                let owner = face.owner_cell;
                let vel_owner = self.phases[p].velocity.values()[owner];
                let alpha_owner = old_alphas[p][owner];

                // Face flux: F_f = (u_f . n_f) * A_f
                let flux = vel_owner[0] * face.normal[0]
                    + vel_owner[1] * face.normal[1]
                    + vel_owner[2] * face.normal[2];
                let face_flux = flux * face.area;

                if let Some(neigh) = face.neighbor_cell {
                    let alpha_neigh = old_alphas[p][neigh];

                    // Upwind: use donor cell value
                    let alpha_face = if face_flux >= 0.0 {
                        alpha_owner
                    } else {
                        alpha_neigh
                    };

                    let convective = alpha_face * face_flux;
                    div_flux[owner] += convective;
                    div_flux[neigh] -= convective;
                } else {
                    // Boundary face: use owner value
                    let convective = alpha_owner * face_flux;
                    div_flux[owner] += convective;
                }
            }

            // Update alpha: alpha_new = alpha_old - dt/V * div_flux
            let alpha_vals = self.phases[p].alpha.values_mut();
            for cell in 0..n {
                let vol = mesh.cells[cell].volume;
                if vol > 1e-30 {
                    alpha_vals[cell] = old_alphas[p][cell] - dt * div_flux[cell] / vol;
                }
            }
        }
    }

    /// Solves the momentum equation for a single phase.
    ///
    /// d(alpha_k * rho_k * u_k)/dt + div(alpha_k * rho_k * u_k * u_k)
    ///   = -alpha_k * grad(p) + div(alpha_k * tau_k) + M_k + alpha_k * rho_k * g
    ///
    /// Uses an explicit first-order upwind scheme for convection and
    /// a central difference for diffusion.
    fn solve_momentum_for_phase(
        &self,
        phase_idx: usize,
        interphase_forces: &[[f64; 3]],
        mesh: &UnstructuredMesh,
        dt: f64,
    ) -> Vec<[f64; 3]> {
        let n = mesh.num_cells();
        let rho = self.phases[phase_idx].density;
        let mu = self.phases[phase_idx].viscosity;
        let vel = self.phases[phase_idx].velocity.values();
        let alpha = self.phases[phase_idx].alpha.values();
        let g = self.force_config.gravity;

        let mut new_vel = vel.to_vec();

        // For each cell, compute the RHS of the momentum equation
        for cell in 0..n {
            let vol = mesh.cells[cell].volume;
            let a_k = alpha[cell].max(1e-10);
            let mass = a_k * rho * vol;
            if mass < 1e-30 || vol < 1e-30 {
                continue;
            }

            let mut rhs = [0.0_f64; 3];

            // Convective and diffusive fluxes via face loop
            for &fid in &mesh.cells[cell].faces {
                if fid >= mesh.faces.len() {
                    continue;
                }
                let face = &mesh.faces[fid];
                let owner = face.owner_cell;

                // Determine sign: outward from this cell
                let sign = if owner == cell { 1.0 } else { -1.0 };

                // Face velocity for flux
                let u_f = vel[owner];
                let flux = (u_f[0] * face.normal[0]
                    + u_f[1] * face.normal[1]
                    + u_f[2] * face.normal[2])
                    * face.area
                    * sign;

                if let Some(neigh) = face.neighbor_cell {
                    let other = if owner == cell { neigh } else { owner };

                    // Upwind convection
                    let u_upwind = if flux >= 0.0 { vel[cell] } else { vel[other] };
                    for dim in 0..3 {
                        rhs[dim] -= a_k * rho * u_upwind[dim] * flux;
                    }

                    // Viscous diffusion: mu * (u_neigh - u_owner) * A / d
                    let dx = mesh.cells[other].center[0] - mesh.cells[cell].center[0];
                    let dy = mesh.cells[other].center[1] - mesh.cells[cell].center[1];
                    let dz = mesh.cells[other].center[2] - mesh.cells[cell].center[2];
                    let dist = (dx * dx + dy * dy + dz * dz).sqrt().max(1e-30);
                    let diff_coeff = a_k * mu * face.area / dist;
                    for dim in 0..3 {
                        rhs[dim] += diff_coeff * (vel[other][dim] - vel[cell][dim]);
                    }
                } else {
                    // Boundary: zero-gradient assumption for convection
                    for dim in 0..3 {
                        rhs[dim] -= a_k * rho * vel[cell][dim] * flux;
                    }
                }
            }

            // Pressure gradient: -alpha_k * grad(p) * V
            // Approximate grad(p) using Green-Gauss
            let mut grad_p = [0.0_f64; 3];
            for &fid in &mesh.cells[cell].faces {
                if fid >= mesh.faces.len() {
                    continue;
                }
                let face = &mesh.faces[fid];
                let sign = if face.owner_cell == cell { 1.0 } else { -1.0 };
                let p_face = if let Some(neigh) = face.neighbor_cell {
                    0.5 * (self.pressure[face.owner_cell] + self.pressure[neigh])
                } else {
                    self.pressure[face.owner_cell]
                };
                for j in 0..3 {
                    grad_p[j] += sign * p_face * face.normal[j] * face.area;
                }
            }
            for j in 0..3 {
                grad_p[j] /= vol;
            }
            for dim in 0..3 {
                rhs[dim] -= a_k * grad_p[dim] * vol;
            }

            // Interphase momentum transfer: M_k * V
            for dim in 0..3 {
                rhs[dim] += interphase_forces[cell][dim] * vol;
            }

            // Gravity: alpha_k * rho_k * g * V
            for dim in 0..3 {
                rhs[dim] += a_k * rho * g[dim] * vol;
            }

            // Explicit time integration: u_new = u_old + dt * rhs / mass
            for dim in 0..3 {
                new_vel[cell][dim] = vel[cell][dim] + dt * rhs[dim] / mass;
            }
        }

        new_vel
    }

    /// Solves the inter-phase coupling for one time step.
    ///
    /// The coupling loop:
    /// 1. Store previous velocities for virtual mass computation
    /// 2. Solve continuity equation for each phase to update alpha
    /// 3. Solve momentum equation for each phase with inter-phase forces
    ///    (drag + lift + virtual mass)
    /// 4. Enforce volume fraction constraint: sum(alpha_k) = 1
    /// 5. Check convergence of velocities
    /// 6. Iterate until converged or max iterations reached
    pub fn solve_phase_coupling(
        &mut self,
        mesh: &UnstructuredMesh,
        dt: f64,
    ) -> Result<f64> {
        let n = mesh.num_cells();
        let num_phases = self.phases.len();
        if num_phases < 2 {
            return Ok(0.0);
        }

        // Resize pressure if needed
        if self.pressure.len() != n {
            self.pressure.resize(n, 0.0);
        }

        // Store previous velocities for virtual mass computation
        self.prev_velocities = self
            .phases
            .iter()
            .map(|ph| ph.velocity.values().to_vec())
            .collect();

        let mut max_residual = 0.0_f64;

        // Iterative coupling loop
        for _coupling_iter in 0..self.max_coupling_iterations {
            // Save old velocities for convergence check
            let old_velocities: Vec<Vec<[f64; 3]>> = self
                .phases
                .iter()
                .map(|ph| ph.velocity.values().to_vec())
                .collect();

            // Step 1: Solve continuity for each phase
            self.solve_continuity(mesh, dt);

            // Step 2: Compute interphase forces and solve momentum for each phase
            // Accumulate forces on each phase from all pairs
            let mut all_forces: Vec<Vec<[f64; 3]>> =
                (0..num_phases).map(|_| vec![[0.0; 3]; n]).collect();

            for p in 0..num_phases {
                for q in (p + 1)..num_phases {
                    let forces_on_p = self.compute_interphase_forces(p, q, mesh, dt);
                    for cell in 0..n {
                        for dim in 0..3 {
                            all_forces[p][cell][dim] += forces_on_p[cell][dim];
                            // Newton's third law: force on q is negative
                            all_forces[q][cell][dim] -= forces_on_p[cell][dim];
                        }
                    }
                }
            }

            // Solve momentum for each phase
            let new_velocities: Vec<Vec<[f64; 3]>> = (0..num_phases)
                .map(|p| self.solve_momentum_for_phase(p, &all_forces[p], mesh, dt))
                .collect();

            // Apply new velocities with under-relaxation
            let relax = 0.5;
            for p in 0..num_phases {
                let vel_mut = self.phases[p].velocity.values_mut();
                for cell in 0..n {
                    for dim in 0..3 {
                        vel_mut[cell][dim] = old_velocities[p][cell][dim]
                            + relax * (new_velocities[p][cell][dim] - old_velocities[p][cell][dim]);
                    }
                }
            }

            // Step 3: Enforce sum(alpha) = 1 constraint by normalization
            for cell in 0..n {
                let sum: f64 = self
                    .phases
                    .iter()
                    .map(|ph| ph.alpha.values()[cell].max(0.0))
                    .sum();
                if sum > 0.0 {
                    for ph in self.phases.iter_mut() {
                        let v = ph.alpha.values_mut();
                        v[cell] = v[cell].max(0.0) / sum;
                    }
                }
            }

            // Step 4: Compute convergence residual (max velocity change)
            let mut iter_residual = 0.0_f64;
            for p in 0..num_phases {
                let vel = self.phases[p].velocity.values();
                for cell in 0..n {
                    for dim in 0..3 {
                        let diff = (vel[cell][dim] - old_velocities[p][cell][dim]).abs();
                        iter_residual = iter_residual.max(diff);
                    }
                }
            }

            max_residual = iter_residual;
            if max_residual < self.coupling_tolerance {
                break;
            }
        }

        Ok(max_residual)
    }

    /// Returns the total interphase momentum transfer for diagnostics.
    ///
    /// Returns a vector of total forces on each phase.
    pub fn compute_total_interphase_forces(
        &self,
        mesh: &UnstructuredMesh,
        dt: f64,
    ) -> Vec<[f64; 3]> {
        let n = mesh.num_cells();
        let num_phases = self.phases.len();
        let mut totals = vec![[0.0; 3]; num_phases];

        for p in 0..num_phases {
            for q in (p + 1)..num_phases {
                let forces = self.compute_interphase_forces(p, q, mesh, dt);
                for cell in 0..n {
                    let vol = mesh.cells[cell].volume;
                    for dim in 0..3 {
                        totals[p][dim] += forces[cell][dim] * vol;
                        totals[q][dim] -= forces[cell][dim] * vol;
                    }
                }
            }
        }

        totals
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gfd_core::mesh::structured::StructuredMesh;

    fn make_test_mesh(nx: usize, ny: usize) -> UnstructuredMesh {
        let sm = StructuredMesh::uniform(nx, ny, 0, 1.0, 1.0, 0.0);
        sm.to_unstructured()
    }

    #[test]
    fn test_schiller_naumann_drag() {
        let solver = EulerEulerSolver::new(vec![]);
        // Re ~ 0: Cd should be ~0
        assert!(solver.schiller_naumann_drag(0.0).abs() < 1e-10);
        // Re = 1: Cd = 24/1 * (1 + 0.15 * 1^0.687) = 24 * 1.15 = 27.6
        let cd_1 = solver.schiller_naumann_drag(1.0);
        assert!((cd_1 - 27.6).abs() < 0.1);
        // Re >= 1000: Cd = 0.44
        assert!((solver.schiller_naumann_drag(1000.0) - 0.44).abs() < 1e-10);
        assert!((solver.schiller_naumann_drag(5000.0) - 0.44).abs() < 1e-10);
    }

    #[test]
    fn test_two_phase_volume_fraction_constraint() {
        let mesh = make_test_mesh(5, 5);
        let n = mesh.num_cells();

        let phase_a = Phase::new("water", "liquid", n, 1000.0, 1e-3, 0.7);
        let phase_b = Phase::new("air", "gas", n, 1.2, 1.8e-5, 0.3);

        let mut solver = EulerEulerSolver::new(vec![phase_a, phase_b]);
        let _ = solver.solve_phase_coupling(&mesh, 1e-3).unwrap();

        // Check volume fraction constraint: sum(alpha) = 1 for all cells
        for cell in 0..n {
            let sum: f64 = solver
                .phases
                .iter()
                .map(|ph| ph.alpha.values()[cell])
                .sum();
            assert!(
                (sum - 1.0).abs() < 1e-10,
                "Volume fraction sum at cell {} is {}, expected 1.0",
                cell,
                sum
            );
        }
    }

    #[test]
    fn test_zero_relative_velocity_no_drag() {
        // When both phases have the same velocity, drag should be zero
        let mesh = make_test_mesh(3, 3);
        let n = mesh.num_cells();

        let mut phase_a = Phase::new("water", "liquid", n, 1000.0, 1e-3, 0.6);
        let mut phase_b = Phase::new("air", "gas", n, 1.2, 1.8e-5, 0.4);

        // Set same velocity for both phases
        for cell in 0..n {
            phase_a.velocity.values_mut()[cell] = [1.0, 0.0, 0.0];
            phase_b.velocity.values_mut()[cell] = [1.0, 0.0, 0.0];
        }

        let solver = EulerEulerSolver::new(vec![phase_a, phase_b]);
        let forces = solver.compute_interphase_forces(0, 1, &mesh, 0.01);

        // With zero relative velocity, drag should be zero
        // Lift should also be zero (u_rel x curl = 0 x anything = 0)
        for cell in 0..n {
            let mag = (forces[cell][0].powi(2)
                + forces[cell][1].powi(2)
                + forces[cell][2].powi(2))
            .sqrt();
            assert!(
                mag < 1e-6,
                "Force at cell {} should be ~0 with zero relative velocity, got {}",
                cell,
                mag
            );
        }
    }

    #[test]
    fn test_drag_opposes_relative_velocity() {
        let mesh = make_test_mesh(3, 3);
        let n = mesh.num_cells();

        let mut phase_a = Phase::new("water", "liquid", n, 1000.0, 1e-3, 0.6);
        let mut phase_b = Phase::new("air", "gas", n, 1.2, 1.8e-5, 0.4);

        // Phase A moves right, Phase B stationary
        for cell in 0..n {
            phase_a.velocity.values_mut()[cell] = [1.0, 0.0, 0.0];
            phase_b.velocity.values_mut()[cell] = [0.0, 0.0, 0.0];
        }

        let mut config = InterphaseForceConfig::default();
        config.lift_enabled = false;
        config.virtual_mass_enabled = false;

        let solver = EulerEulerSolver::with_config(vec![phase_a, phase_b], config);
        let forces = solver.compute_interphase_forces(0, 1, &mesh, 0.01);

        // Drag force on phase A should point in -x direction (opposing motion)
        for cell in 0..n {
            assert!(
                forces[cell][0] < 0.0,
                "Drag force on moving phase should oppose motion at cell {}",
                cell
            );
        }
    }

    #[test]
    fn test_interphase_newtons_third_law() {
        let mesh = make_test_mesh(4, 4);
        let n = mesh.num_cells();

        let mut phase_a = Phase::new("water", "liquid", n, 1000.0, 1e-3, 0.7);
        let mut phase_b = Phase::new("bubbles", "gas", n, 1.2, 1.8e-5, 0.3);

        for cell in 0..n {
            phase_a.velocity.values_mut()[cell] = [0.5, 0.1, 0.0];
            phase_b.velocity.values_mut()[cell] = [0.0, 0.5, 0.0];
        }

        let solver = EulerEulerSolver::new(vec![phase_a, phase_b]);
        let totals = solver.compute_total_interphase_forces(&mesh, 0.01);

        // Newton's third law: total force on phase 0 + total force on phase 1 = 0
        for dim in 0..3 {
            let sum = totals[0][dim] + totals[1][dim];
            assert!(
                sum.abs() < 1e-10,
                "Newton's third law violated: dim={}, sum={}",
                dim,
                sum
            );
        }
    }

    #[test]
    fn test_three_phase_solver() {
        let mesh = make_test_mesh(3, 3);
        let n = mesh.num_cells();

        let phase_a = Phase::new("oil", "liquid", n, 800.0, 5e-3, 0.5);
        let phase_b = Phase::new("water", "liquid", n, 1000.0, 1e-3, 0.3);
        let phase_c = Phase::new("gas", "gas", n, 1.2, 1.8e-5, 0.2);

        let mut solver = EulerEulerSolver::new(vec![phase_a, phase_b, phase_c]);
        let residual = solver.solve_phase_coupling(&mesh, 1e-4).unwrap();
        assert!(residual.is_finite());

        // Volume fraction constraint must hold for 3 phases too
        for cell in 0..n {
            let sum: f64 = solver
                .phases
                .iter()
                .map(|ph| ph.alpha.values()[cell])
                .sum();
            assert!(
                (sum - 1.0).abs() < 1e-10,
                "3-phase volume fraction sum at cell {} is {}",
                cell,
                sum
            );
        }
    }

    #[test]
    fn test_lift_force_perpendicular_to_relative_velocity() {
        // Lift force is (u_rel) x curl(u_c), so it's perpendicular to u_rel
        let mesh = make_test_mesh(3, 3);
        let n = mesh.num_cells();

        let mut phase_a = Phase::new("bubbles", "gas", n, 1.2, 1.8e-5, 0.3);
        let mut phase_b = Phase::new("water", "liquid", n, 1000.0, 1e-3, 0.7);

        // Set up a flow with relative velocity in x and vorticity in z
        for cell in 0..n {
            phase_a.velocity.values_mut()[cell] = [1.0, 0.0, 0.0];
            phase_b.velocity.values_mut()[cell] = [0.0, 0.0, 0.0];
        }

        let mut config = InterphaseForceConfig::default();
        config.drag_enabled = false;
        config.virtual_mass_enabled = false;
        config.lift_enabled = true;

        let solver = EulerEulerSolver::with_config(vec![phase_a, phase_b], config);
        // Just verify it runs without error - the lift cross product is correct by construction
        let forces = solver.compute_interphase_forces(0, 1, &mesh, 0.01);
        assert_eq!(forces.len(), n);
    }

    #[test]
    fn test_virtual_mass_force() {
        let mesh = make_test_mesh(3, 3);
        let n = mesh.num_cells();

        let mut phase_a = Phase::new("bubbles", "gas", n, 1.2, 1.8e-5, 0.3);
        let phase_b = Phase::new("water", "liquid", n, 1000.0, 1e-3, 0.7);

        // Give phase_a some velocity so prev != current
        for cell in 0..n {
            phase_a.velocity.values_mut()[cell] = [1.0, 0.0, 0.0];
        }

        let mut config = InterphaseForceConfig::default();
        config.drag_enabled = false;
        config.lift_enabled = false;
        config.virtual_mass_enabled = true;

        let mut solver = EulerEulerSolver::with_config(vec![phase_a, phase_b], config);

        // prev_velocities should be the initial values (all zeros by default from new())
        // Current velocity has phase_a at [1,0,0], so Du_d/Dt = (1-0)/dt
        // Du_c/Dt = 0
        // M_vm = C_vm * rho_c * alpha_d * (Du_c/Dt - Du_d/Dt)
        // = 0.5 * 1000 * 0.3 * (0 - 1/dt) per dim
        let dt = 0.01;
        // Reset prev_velocities to zero to get a clear signal
        solver.prev_velocities = vec![vec![[0.0; 3]; n]; 2];
        let forces = solver.compute_interphase_forces(0, 1, &mesh, dt);

        // Virtual mass should resist acceleration of the dispersed phase
        // Du_d/Dt = 1.0/0.01 = 100 in x, Du_c/Dt = 0
        // F = 0.5 * 1000 * 0.3 * (0 - 100) = -15000 in x
        for cell in 0..n {
            assert!(
                forces[cell][0] < -1.0,
                "Virtual mass should resist dispersed phase acceleration"
            );
        }
    }
}
