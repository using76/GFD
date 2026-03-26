//! Mixture multiphase model implementation.
//!
//! The mixture model solves for mixture-averaged properties using a single
//! momentum equation, with volume fraction transport for secondary phases.
//! The drift velocity between phases is computed using an algebraic slip model.

use gfd_core::{ScalarField, VectorField, UnstructuredMesh};
use crate::Result;

/// A single phase in the mixture model.
#[derive(Debug, Clone)]
pub struct MixturePhase {
    pub name: String,
    pub density: f64,
    pub viscosity: f64,
    pub diameter: f64,
    pub alpha: ScalarField,
}

impl MixturePhase {
    pub fn new(
        name: impl Into<String>, density: f64, viscosity: f64,
        diameter: f64, num_cells: usize, initial_alpha: f64,
    ) -> Self {
        let name = name.into();
        Self {
            alpha: ScalarField::new(&format!("alpha_{}", name), vec![initial_alpha; num_cells]),
            name, density, viscosity, diameter,
        }
    }
}

/// Mixture multiphase solver.
pub struct MixtureSolver {
    pub phases: Vec<MixturePhase>,
    pub primary_phase: usize,
    pub gravity: [f64; 3],
    pub alpha_relaxation: f64,
    pub mixture_velocity: VectorField,
    pub mixture_density: ScalarField,
    pub mixture_viscosity: ScalarField,
}

impl MixtureSolver {
    pub fn new(phases: Vec<MixturePhase>, primary_phase: usize) -> Self {
        let n = if phases.is_empty() { 0 } else { phases[0].alpha.values().len() };
        let mut solver = Self {
            phases, primary_phase,
            gravity: [0.0, -9.81, 0.0],
            alpha_relaxation: 0.5,
            mixture_velocity: VectorField::zeros("U_mixture", n),
            mixture_density: ScalarField::ones("rho_mixture", n),
            mixture_viscosity: ScalarField::new("mu_mixture", vec![1e-3; n]),
        };
        solver.compute_mixture_properties();
        solver
    }

    fn num_cells(&self) -> usize { self.mixture_density.values().len() }

    pub fn compute_mixture_properties(&mut self) {
        let n = self.num_cells();
        let rho_m = self.mixture_density.values_mut();
        for i in 0..n { rho_m[i] = 0.0; }
        for phase in &self.phases {
            let alpha = phase.alpha.values();
            for i in 0..n { rho_m[i] += alpha[i] * phase.density; }
        }
        let mu_m = self.mixture_viscosity.values_mut();
        for i in 0..n { mu_m[i] = 0.0; }
        for phase in &self.phases {
            let alpha = phase.alpha.values();
            for i in 0..n { mu_m[i] += alpha[i] * phase.viscosity; }
        }
    }

    pub fn compute_mixture_velocity(&mut self, phase_velocities: &[VectorField]) {
        let n = self.num_cells();
        let u_m = self.mixture_velocity.values_mut();
        for i in 0..n { u_m[i] = [0.0, 0.0, 0.0]; }
        for (k, phase) in self.phases.iter().enumerate() {
            if k >= phase_velocities.len() { break; }
            let alpha = phase.alpha.values();
            let u_k = phase_velocities[k].values();
            for i in 0..n {
                let w = alpha[i] * phase.density;
                for d in 0..3 { u_m[i][d] += w * u_k[i][d]; }
            }
        }
        let rho_m = self.mixture_density.values();
        let u_m = self.mixture_velocity.values_mut();
        for i in 0..n {
            let inv = 1.0 / rho_m[i].max(1e-30);
            for d in 0..3 { u_m[i][d] *= inv; }
        }
    }

    pub fn compute_drift_velocity(&self) -> Vec<Vec<[f64; 3]>> {
        let n = self.num_cells();
        let np = self.phases.len();
        let rho_m = self.mixture_density.values();
        let mu_m = self.mixture_viscosity.values();

        let mut slip: Vec<Vec<[f64; 3]>> = Vec::with_capacity(np);
        for phase in &self.phases {
            let mut u_slip = vec![[0.0; 3]; n];
            let alpha = phase.alpha.values();
            for i in 0..n {
                let tau_p = phase.density * phase.diameter * phase.diameter
                    / (18.0 * mu_m[i].max(1e-30));
                let drho = (phase.density - rho_m[i]) / rho_m[i].max(1e-30);
                if alpha[i] > 1e-10 {
                    for d in 0..3 { u_slip[i][d] = tau_p * self.gravity[d] * drho; }
                }
            }
            slip.push(u_slip);
        }

        let mut avg = vec![[0.0; 3]; n];
        for (k, phase) in self.phases.iter().enumerate() {
            let alpha = phase.alpha.values();
            for i in 0..n {
                let w = alpha[i] * phase.density;
                for d in 0..3 { avg[i][d] += w * slip[k][i][d]; }
            }
        }
        for i in 0..n {
            let inv = 1.0 / rho_m[i].max(1e-30);
            for d in 0..3 { avg[i][d] *= inv; }
        }

        let mut drift = Vec::with_capacity(np);
        for k in 0..np {
            let mut ud = vec![[0.0; 3]; n];
            for i in 0..n {
                for d in 0..3 { ud[i][d] = slip[k][i][d] - avg[i][d]; }
            }
            drift.push(ud);
        }
        drift
    }

    pub fn solve_volume_fraction(&mut self, mesh: &UnstructuredMesh, dt: f64) -> Result<()> {
        let n = mesh.num_cells();
        let drift_velocities = self.compute_drift_velocity();
        let u_m = self.mixture_velocity.values().to_vec();

        for (k, phase) in self.phases.iter_mut().enumerate() {
            if k == self.primary_phase { continue; }
            let alpha_old = phase.alpha.values().to_vec();
            let mut alpha_new = alpha_old.clone();
            let mut net_flux = vec![0.0; n];

            for face in &mesh.faces {
                let owner = face.owner_cell;
                if let Some(neighbor) = face.neighbor_cell {
                    let mut u_f = [0.0; 3];
                    for d in 0..3 {
                        u_f[d] = 0.5 * (u_m[owner][d] + u_m[neighbor][d])
                            + 0.5 * (drift_velocities[k][owner][d] + drift_velocities[k][neighbor][d]);
                    }
                    let flux = (u_f[0]*face.normal[0] + u_f[1]*face.normal[1]
                        + u_f[2]*face.normal[2]) * face.area;
                    let af = if flux >= 0.0 { alpha_old[owner] } else { alpha_old[neighbor] };
                    net_flux[owner] += flux * af;
                    net_flux[neighbor] -= flux * af;
                } else {
                    let mut u_f = [0.0; 3];
                    for d in 0..3 { u_f[d] = u_m[owner][d] + drift_velocities[k][owner][d]; }
                    let flux = (u_f[0]*face.normal[0] + u_f[1]*face.normal[1]
                        + u_f[2]*face.normal[2]) * face.area;
                    if flux >= 0.0 { net_flux[owner] += flux * alpha_old[owner]; }
                }
            }

            for i in 0..n {
                alpha_new[i] = alpha_old[i] - dt / mesh.cells[i].volume * net_flux[i];
                alpha_new[i] = alpha_new[i].clamp(0.0, 1.0);
                alpha_new[i] = self.alpha_relaxation * alpha_new[i]
                    + (1.0 - self.alpha_relaxation) * alpha_old[i];
            }
            phase.alpha.values_mut().copy_from_slice(&alpha_new);
        }

        let mut alpha_primary = vec![1.0; n];
        for (k, phase) in self.phases.iter().enumerate() {
            if k == self.primary_phase { continue; }
            let ak = phase.alpha.values();
            for i in 0..n { alpha_primary[i] -= ak[i]; }
        }
        for i in 0..n { alpha_primary[i] = alpha_primary[i].clamp(0.0, 1.0); }
        self.phases[self.primary_phase].alpha.values_mut().copy_from_slice(&alpha_primary);
        self.compute_mixture_properties();
        Ok(())
    }

    pub fn solve_mixture_momentum(&mut self, mesh: &UnstructuredMesh, dt: f64) -> Result<()> {
        let n = mesh.num_cells();
        let rho_m = self.mixture_density.values().to_vec();
        let mu_m = self.mixture_viscosity.values().to_vec();
        let u_old = self.mixture_velocity.values().to_vec();
        let mut u_new = u_old.clone();
        let mut mom = vec![[0.0; 3]; n];

        for face in &mesh.faces {
            let owner = face.owner_cell;
            if let Some(neighbor) = face.neighbor_cell {
                let mut u_f = [0.0; 3];
                for d in 0..3 { u_f[d] = 0.5 * (u_old[owner][d] + u_old[neighbor][d]); }
                let flux = (u_f[0]*face.normal[0] + u_f[1]*face.normal[1]
                    + u_f[2]*face.normal[2]) * face.area;
                let rho_f = 0.5 * (rho_m[owner] + rho_m[neighbor]);
                let u_up = if flux >= 0.0 { u_old[owner] } else { u_old[neighbor] };
                for d in 0..3 {
                    let c = rho_f * flux * u_up[d];
                    mom[owner][d] += c;
                    mom[neighbor][d] -= c;
                }
                let cc_o = mesh.cells[owner].center;
                let cc_n = mesh.cells[neighbor].center;
                let dist = ((cc_o[0]-cc_n[0]).powi(2) + (cc_o[1]-cc_n[1]).powi(2)
                    + (cc_o[2]-cc_n[2]).powi(2)).sqrt().max(1e-30);
                let mu_f = 0.5 * (mu_m[owner] + mu_m[neighbor]);
                let dc = mu_f * face.area / dist;
                for d in 0..3 {
                    let diff = dc * (u_old[neighbor][d] - u_old[owner][d]);
                    mom[owner][d] -= diff;
                    mom[neighbor][d] += diff;
                }
            }
        }

        for i in 0..n {
            let inv = 1.0 / (rho_m[i].max(1e-30) * mesh.cells[i].volume);
            for d in 0..3 {
                u_new[i][d] = u_old[i][d] + dt * (-inv * mom[i][d] + self.gravity[d]);
            }
        }
        self.mixture_velocity.values_mut().copy_from_slice(&u_new);
        Ok(())
    }

    pub fn solve_step(&mut self, mesh: &UnstructuredMesh, dt: f64) -> Result<()> {
        self.solve_volume_fraction(mesh, dt)?;
        self.solve_mixture_momentum(mesh, dt)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gfd_core::StructuredMesh;

    fn make_mesh() -> UnstructuredMesh {
        StructuredMesh::uniform(10, 1, 1, 1.0, 0.1, 0.1).to_unstructured()
    }

    #[test]
    fn mixture_properties_two_phases() {
        let n = 10;
        let w = MixturePhase::new("water", 1000.0, 1e-3, 0.0, n, 0.8);
        let s = MixturePhase::new("sand", 2500.0, 1e-2, 1e-3, n, 0.2);
        let solver = MixtureSolver::new(vec![w, s], 0);
        for &rho in solver.mixture_density.values() {
            assert!((rho - 1300.0).abs() < 1e-10);
        }
        for &mu in solver.mixture_viscosity.values() {
            assert!((mu - 2.8e-3).abs() < 1e-12);
        }
    }

    #[test]
    fn volume_fraction_constraint() {
        let n = 10;
        let mesh = make_mesh();
        let w = MixturePhase::new("water", 1000.0, 1e-3, 0.0, n, 0.7);
        let s = MixturePhase::new("sand", 2500.0, 1e-2, 1e-3, n, 0.2);
        let a = MixturePhase::new("air", 1.225, 1.8e-5, 0.0, n, 0.1);
        let mut solver = MixtureSolver::new(vec![w, s, a], 0);
        solver.solve_volume_fraction(&mesh, 0.001).unwrap();
        for i in 0..n {
            let sum: f64 = solver.phases.iter().map(|p| p.alpha.values()[i]).sum();
            assert!((sum - 1.0).abs() < 1e-10);
        }
    }

    #[test]
    fn drift_zero_equal_density() {
        let n = 10;
        let a = MixturePhase::new("a", 1000.0, 1e-3, 1e-3, n, 0.5);
        let b = MixturePhase::new("b", 1000.0, 1e-3, 1e-3, n, 0.5);
        let solver = MixtureSolver::new(vec![a, b], 0);
        let drift = solver.compute_drift_velocity();
        for k in 0..2 {
            for i in 0..n {
                let mag = (drift[k][i][0].powi(2)+drift[k][i][1].powi(2)+drift[k][i][2].powi(2)).sqrt();
                assert!(mag < 1e-15);
            }
        }
    }

    #[test]
    fn drift_nonzero_density_diff() {
        let n = 10;
        let w = MixturePhase::new("water", 1000.0, 1e-3, 0.0, n, 0.9);
        let s = MixturePhase::new("sand", 2500.0, 1e-2, 1e-3, n, 0.1);
        let solver = MixtureSolver::new(vec![w, s], 0);
        let drift = solver.compute_drift_velocity();
        let dy: f64 = drift[1].iter().map(|d| d[1].abs()).sum();
        assert!(dy > 0.0);
    }

    #[test]
    fn solve_step_runs() {
        let n = 10;
        let mesh = make_mesh();
        let w = MixturePhase::new("water", 1000.0, 1e-3, 0.0, n, 0.8);
        let s = MixturePhase::new("sand", 2500.0, 1e-2, 1e-3, n, 0.2);
        let mut solver = MixtureSolver::new(vec![w, s], 0);
        solver.solve_step(&mesh, 1e-4).unwrap();
        for i in 0..n {
            let sum: f64 = solver.phases.iter().map(|p| p.alpha.values()[i]).sum();
            assert!((sum - 1.0).abs() < 1e-10);
        }
    }

    #[test]
    fn mixture_velocity_calc() {
        let n = 10;
        let w = MixturePhase::new("water", 1000.0, 1e-3, 0.0, n, 0.5);
        let s = MixturePhase::new("sand", 2500.0, 1e-2, 1e-3, n, 0.5);
        let mut solver = MixtureSolver::new(vec![w, s], 0);
        let mut vw = VectorField::zeros("Uw", n);
        let vs = VectorField::zeros("Us", n);
        for i in 0..n { vw.values_mut()[i] = [1.0, 0.0, 0.0]; }
        solver.compute_mixture_velocity(&[vw, vs]);
        let expected = 500.0 / 1750.0;
        for i in 0..n {
            assert!((solver.mixture_velocity.values()[i][0] - expected).abs() < 1e-10);
        }
    }
}
