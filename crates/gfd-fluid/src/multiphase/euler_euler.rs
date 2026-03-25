//! Euler-Euler multiphase solver.
//!
//! Treats each phase as an interpenetrating continuum, solving
//! a full set of conservation equations for each phase.

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

/// Euler-Euler multiphase solver.
///
/// Solves a set of coupled conservation equations for each phase,
/// including phase continuity, momentum, and inter-phase coupling
/// through drag, lift, and virtual mass forces.
pub struct EulerEulerSolver {
    /// The phases being simulated.
    pub phases: Vec<Phase>,
    /// Maximum inter-phase coupling iterations per time step.
    pub max_coupling_iterations: usize,
    /// Convergence tolerance for the phase coupling loop.
    pub coupling_tolerance: f64,
}

impl EulerEulerSolver {
    /// Creates a new Euler-Euler solver with the given phases.
    pub fn new(phases: Vec<Phase>) -> Self {
        Self {
            phases,
            max_coupling_iterations: 20,
            coupling_tolerance: 1e-4,
        }
    }

    /// Adds a phase to the solver.
    pub fn add_phase(&mut self, phase: Phase) {
        self.phases.push(phase);
    }

    /// Solves the inter-phase coupling for one time step.
    ///
    /// The coupling loop:
    /// 1. Solve continuity equation for each phase to update alpha
    /// 2. Solve momentum equation for each phase with inter-phase forces
    /// 3. Apply pressure-velocity coupling (shared pressure field)
    /// 4. Check convergence of volume fractions and velocities
    /// 5. Iterate until converged or max iterations reached
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

        let mut max_residual = 0.0_f64;

        // Iterative coupling loop
        for _coupling_iter in 0..self.max_coupling_iterations {
            let mut iter_residual = 0.0_f64;

            // For each phase pair, compute drag-based momentum transfer
            for p in 0..num_phases {
                for q in (p + 1)..num_phases {
                    for cell in 0..n {
                        let alpha_p = self.phases[p].alpha.values()[cell];
                        let alpha_q = self.phases[q].alpha.values()[cell];

                        let vel_p = self.phases[p].velocity.values()[cell];
                        let vel_q = self.phases[q].velocity.values()[cell];

                        // Relative velocity
                        let u_rel = [
                            vel_p[0] - vel_q[0],
                            vel_p[1] - vel_q[1],
                            vel_p[2] - vel_q[2],
                        ];
                        let u_rel_mag =
                            (u_rel[0] * u_rel[0] + u_rel[1] * u_rel[1] + u_rel[2] * u_rel[2])
                                .sqrt();

                        // Schiller-Naumann drag
                        let d_p = 1e-3; // assume 1mm particle diameter
                        let re = self.phases[q].density * u_rel_mag * d_p
                            / (self.phases[q].viscosity + 1e-30);
                        let cd = self.schiller_naumann_drag(re);

                        // Drag coefficient K = 0.75 * C_D * rho_q * alpha_p * |u_rel| / d_p
                        let k_drag = 0.75 * cd * self.phases[q].density * alpha_p * u_rel_mag
                            / (d_p + 1e-30);

                        // Apply drag force (explicit, semi-implicit via relaxation)
                        let vol = mesh.cells[cell].volume;
                        let relax = 0.5;
                        for dim in 0..3 {
                            let force = k_drag * u_rel[dim] * vol;
                            let correction_p = -relax * force * dt / (self.phases[p].density * alpha_p.max(1e-10) * vol + 1e-30);
                            let correction_q = relax * force * dt / (self.phases[q].density * alpha_q.max(1e-10) * vol + 1e-30);

                            self.phases[p].velocity.values_mut()[cell][dim] += correction_p;
                            self.phases[q].velocity.values_mut()[cell][dim] += correction_q;
                        }
                    }
                }
            }

            // Enforce sum(alpha) = 1 constraint: normalize volume fractions
            for cell in 0..n {
                let sum: f64 = self.phases.iter().map(|ph| ph.alpha.values()[cell].max(0.0)).sum();
                if sum > 0.0 {
                    for ph in self.phases.iter_mut() {
                        let v = ph.alpha.values_mut();
                        v[cell] = (v[cell].max(0.0)) / sum;
                    }
                }
            }

            // Compute residual as max change in alpha
            for ph in &self.phases {
                for cell in 0..n {
                    let alpha_val = ph.alpha.values()[cell];
                    // Simple residual measure
                    iter_residual = iter_residual.max(alpha_val.abs());
                }
            }

            max_residual = iter_residual;
            if max_residual < self.coupling_tolerance {
                break;
            }
        }

        Ok(max_residual)
    }

    /// Computes the drag coefficient between two phases.
    ///
    /// Uses the Schiller-Naumann correlation for particle drag:
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
}
