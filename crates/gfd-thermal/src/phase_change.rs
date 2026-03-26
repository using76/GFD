//! Phase change (melting/solidification) models.

use gfd_core::{ScalarField, UnstructuredMesh};
use crate::Result;

/// Source terms produced by the enthalpy-porosity phase change model.
///
/// These are returned by [`EnthalpyPorosity::solve_phase_change_step`] and
/// should be added to the energy and momentum equations respectively.
pub struct PhaseChangeSourceTerms {
    /// Energy source term: S = -rho * L * d(f_l)/dt  [W/m^3 per cell].
    pub energy_source: ScalarField,
    /// Momentum damping source per velocity component [N/m^3 per cell].
    /// This is the Carman-Kozeny coefficient: C * (1-f_l)^2 / (f_l^3 + eps).
    /// Multiply by -u_i to get the actual force in direction i.
    pub momentum_damping: ScalarField,
    /// Current liquid fraction field (0 = solid, 1 = liquid).
    pub liquid_fraction: ScalarField,
}

/// Enthalpy-porosity method for melting and solidification.
///
/// Models the mushy zone as a porous medium with the liquid fraction
/// varying between 0 (fully solid) and 1 (fully liquid).
pub struct EnthalpyPorosity {
    /// Solidus temperature [K].
    pub solidus_temperature: f64,
    /// Liquidus temperature [K].
    pub liquidus_temperature: f64,
    /// Latent heat of fusion [J/kg].
    pub latent_heat: f64,
    /// Mushy zone constant (Carman-Kozeny parameter).
    pub mushy_constant: f64,
    /// Small constant to avoid division by zero in Carman-Kozeny term.
    pub epsilon: f64,
    /// Density [kg/m^3] (needed for latent heat source).
    pub density: f64,
    /// Previous liquid fraction field (for d(f_l)/dt computation).
    prev_liquid_fraction: Option<Vec<f64>>,
}

impl EnthalpyPorosity {
    /// Creates a new enthalpy-porosity phase change model.
    pub fn new(
        solidus_temperature: f64,
        liquidus_temperature: f64,
        latent_heat: f64,
    ) -> Self {
        Self {
            solidus_temperature,
            liquidus_temperature,
            latent_heat,
            mushy_constant: 1.0e5,
            epsilon: 1.0e-3,
            density: 1000.0,
            prev_liquid_fraction: None,
        }
    }

    /// Sets the density used for the latent heat source term.
    pub fn with_density(mut self, density: f64) -> Self {
        self.density = density;
        self
    }

    /// Sets the mushy zone constant (Carman-Kozeny parameter).
    pub fn with_mushy_constant(mut self, c: f64) -> Self {
        self.mushy_constant = c;
        self
    }

    /// Computes the liquid fraction from the temperature field.
    pub fn compute_liquid_fraction(
        &self,
        temperature: &ScalarField,
        _mesh: &UnstructuredMesh,
    ) -> Result<ScalarField> {
        let values = temperature.values();
        let t_s = self.solidus_temperature;
        let t_l = self.liquidus_temperature;

        let fl: Vec<f64> = values
            .iter()
            .map(|&t| {
                if t_l <= t_s {
                    // Degenerate case: isothermal phase change
                    if t >= t_s { 1.0 } else { 0.0 }
                } else {
                    ((t - t_s) / (t_l - t_s)).clamp(0.0, 1.0)
                }
            })
            .collect();

        Ok(ScalarField::new("liquid_fraction", fl))
    }

    /// Computes the Carman-Kozeny momentum damping coefficient for a given
    /// liquid fraction value:  C * (1 - f_l)^2 / (f_l^3 + epsilon)
    fn carman_kozeny_coeff(&self, f_l: f64) -> f64 {
        let one_minus_fl = 1.0 - f_l;
        self.mushy_constant * one_minus_fl * one_minus_fl / (f_l * f_l * f_l + self.epsilon)
    }

    /// Performs a full enthalpy-porosity phase change step.
    ///
    /// Given the current temperature field and a time step, computes:
    /// 1. The liquid fraction f_l(T) at every cell
    /// 2. The energy source term: S_e = -rho * L * d(f_l)/dt  [W/m^3]
    ///    (negative when melting absorbs energy, positive when solidifying releases)
    /// 3. The momentum damping coefficient from the Carman-Kozeny relation:
    ///    D = C * (1-f_l)^2 / (f_l^3 + epsilon)
    ///    The momentum source for component i is S_u,i = -D * u_i
    ///
    /// The returned source terms should be added to the energy and momentum
    /// equations by the calling solver.
    pub fn solve_phase_change_step(
        &mut self,
        temperature: &ScalarField,
        mesh: &UnstructuredMesh,
        dt: f64,
    ) -> Result<PhaseChangeSourceTerms> {
        let num_cells = mesh.num_cells();
        let liquid_fraction = self.compute_liquid_fraction(temperature, mesh)?;
        let fl_values = liquid_fraction.values();

        // Compute d(f_l)/dt using backward difference
        let mut energy_source = vec![0.0_f64; num_cells];
        if let Some(ref prev_fl) = self.prev_liquid_fraction {
            let rho = self.density;
            let latent = self.latent_heat;
            for i in 0..num_cells {
                let dfl_dt = (fl_values[i] - prev_fl[i]) / dt;
                // S_e = -rho * L * d(f_l)/dt
                // Negative sign: melting (dfl_dt > 0) absorbs energy (sink)
                energy_source[i] = -rho * latent * dfl_dt;
            }
        }
        // If no previous liquid fraction exists (first step), energy source is zero

        // Compute Carman-Kozeny momentum damping coefficient
        let mut damping = vec![0.0_f64; num_cells];
        for i in 0..num_cells {
            damping[i] = self.carman_kozeny_coeff(fl_values[i]);
        }

        // Store current liquid fraction for next step
        self.prev_liquid_fraction = Some(fl_values.to_vec());

        Ok(PhaseChangeSourceTerms {
            energy_source: ScalarField::new("phase_change_energy_source", energy_source),
            momentum_damping: ScalarField::new("phase_change_momentum_damping", damping),
            liquid_fraction,
        })
    }
}

/// Coupled energy-phase-change solver using the enthalpy-porosity method.
pub struct CoupledPhaseChangeSolver {
    /// The enthalpy-porosity phase change model.
    pub phase_change: EnthalpyPorosity,
    /// Thermal conductivity [W/(m*K)].
    pub conductivity: f64,
    /// Specific heat capacity rho*cp [J/(m^3*K)].
    pub rho_cp: f64,
    /// Maximum number of coupling iterations.
    pub max_coupling_iterations: usize,
    /// Convergence tolerance for liquid fraction change.
    pub coupling_tolerance: f64,
}

/// Result of a coupled phase change solve step.
pub struct CoupledPhaseChangeResult {
    /// Energy source terms from latent heat.
    pub energy_source: ScalarField,
    /// Momentum damping coefficients (Carman-Kozeny).
    pub momentum_damping: ScalarField,
    /// Current liquid fraction field.
    pub liquid_fraction: ScalarField,
    /// Number of coupling iterations performed.
    pub iterations: usize,
    /// Final max change in liquid fraction.
    pub liquid_fraction_residual: f64,
}

impl CoupledPhaseChangeSolver {
    /// Creates a new coupled phase change solver.
    pub fn new(
        solidus_temperature: f64,
        liquidus_temperature: f64,
        latent_heat: f64,
        conductivity: f64,
        rho_cp: f64,
    ) -> Self {
        Self {
            phase_change: EnthalpyPorosity::new(solidus_temperature, liquidus_temperature, latent_heat),
            conductivity,
            rho_cp,
            max_coupling_iterations: 20,
            coupling_tolerance: 1e-4,
        }
    }

    /// Sets the density for latent heat source computation.
    pub fn with_density(mut self, density: f64) -> Self {
        self.phase_change = self.phase_change.with_density(density);
        self
    }

    /// Sets the mushy zone constant.
    pub fn with_mushy_constant(mut self, c: f64) -> Self {
        self.phase_change = self.phase_change.with_mushy_constant(c);
        self
    }

    /// Performs a coupled energy + phase change solve step.
    ///
    /// Iterates between the energy equation and phase change until converged.
    pub fn solve_coupled_phase_change(
        &mut self,
        temperature: &mut ScalarField,
        mesh: &UnstructuredMesh,
        dt: f64,
        boundary_temps: &std::collections::HashMap<String, f64>,
    ) -> Result<CoupledPhaseChangeResult> {
        let n = mesh.num_cells();
        let mut fl_old = self.phase_change.compute_liquid_fraction(temperature, mesh)?;
        let mut fl_residual = f64::MAX;
        let mut iterations = 0;
        let mut phase_source = vec![0.0_f64; n];

        for iter in 0..self.max_coupling_iterations {
            iterations = iter + 1;
            let conductivity_per_cell = vec![self.conductivity; n];
            let rho_cp_per_cell = vec![self.rho_cp; n];
            let total_source: Vec<f64> = phase_source.clone();

            let mut solver = crate::conduction::ConductionSolver::new();
            solver.tolerance = 1e-8;
            solver.max_iterations = 500;

            let mut thermal_state = crate::ThermalState::new(n, 0.0);
            for i in 0..n {
                let _ = thermal_state.temperature.set(i, temperature.values()[i]);
            }

            solver.solve_transient_step(
                &mut thermal_state, mesh, &conductivity_per_cell,
                &rho_cp_per_cell, &total_source, dt, boundary_temps,
            )?;

            for i in 0..n {
                let _ = temperature.set(i, thermal_state.temperature.values()[i]);
            }

            let fl_new = self.phase_change.compute_liquid_fraction(temperature, mesh)?;

            fl_residual = 0.0_f64;
            for i in 0..n {
                let diff = (fl_new.values()[i] - fl_old.values()[i]).abs();
                fl_residual = fl_residual.max(diff);
            }

            if let Some(ref prev_fl) = self.phase_change.prev_liquid_fraction {
                let rho = self.phase_change.density;
                let latent = self.phase_change.latent_heat;
                for i in 0..n {
                    let dfl_dt = (fl_new.values()[i] - prev_fl[i]) / dt;
                    phase_source[i] = -rho * latent * dfl_dt;
                }
            }

            fl_old = fl_new;

            if fl_residual < self.coupling_tolerance {
                break;
            }
        }

        let result = self.phase_change.solve_phase_change_step(temperature, mesh, dt)?;

        Ok(CoupledPhaseChangeResult {
            energy_source: result.energy_source,
            momentum_damping: result.momentum_damping,
            liquid_fraction: result.liquid_fraction,
            iterations,
            liquid_fraction_residual: fl_residual,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gfd_core::mesh::cell::Cell;
    use gfd_core::mesh::face::Face;
    use gfd_core::mesh::unstructured::{BoundaryPatch, UnstructuredMesh};

    /// Creates a simple 1D mesh of `nx` cells for testing.
    fn make_1d_mesh(nx: usize, length: f64) -> UnstructuredMesh {
        let dx = length / nx as f64;
        let cross_area = 1.0;

        let mut cells = Vec::with_capacity(nx);
        for i in 0..nx {
            let cx = (i as f64 + 0.5) * dx;
            cells.push(Cell::new(i, vec![], vec![], dx, [cx, 0.5, 0.5]));
        }

        let mut faces: Vec<Face> = Vec::new();
        let mut face_id = 0usize;

        let left_face_id = face_id;
        faces.push(Face::new(face_id, vec![], 0, None, cross_area, [-1.0, 0.0, 0.0], [0.0, 0.5, 0.5]));
        face_id += 1;

        for i in 0..nx - 1 {
            let fx = (i as f64 + 1.0) * dx;
            faces.push(Face::new(face_id, vec![], i, Some(i + 1), cross_area, [1.0, 0.0, 0.0], [fx, 0.5, 0.5]));
            cells[i].faces.push(face_id);
            cells[i + 1].faces.push(face_id);
            face_id += 1;
        }

        let right_face_id = face_id;
        faces.push(Face::new(face_id, vec![], nx - 1, None, cross_area, [1.0, 0.0, 0.0], [length, 0.5, 0.5]));

        cells[0].faces.insert(0, left_face_id);
        cells[nx - 1].faces.push(right_face_id);

        let boundary_patches = vec![
            BoundaryPatch::new("left", vec![left_face_id]),
            BoundaryPatch::new("right", vec![right_face_id]),
        ];

        UnstructuredMesh::from_components(vec![], faces, cells, boundary_patches)
    }

    #[test]
    fn liquid_fraction_below_solidus() {
        let model = EnthalpyPorosity::new(500.0, 600.0, 200_000.0);
        let mesh = make_1d_mesh(3, 1.0);
        let temp = ScalarField::new("temperature", vec![400.0, 450.0, 499.9]);
        let fl = model.compute_liquid_fraction(&temp, &mesh).unwrap();
        for &v in fl.values() {
            assert_eq!(v, 0.0, "Below solidus should be fully solid");
        }
    }

    #[test]
    fn liquid_fraction_above_liquidus() {
        let model = EnthalpyPorosity::new(500.0, 600.0, 200_000.0);
        let mesh = make_1d_mesh(3, 1.0);
        let temp = ScalarField::new("temperature", vec![601.0, 700.0, 1000.0]);
        let fl = model.compute_liquid_fraction(&temp, &mesh).unwrap();
        for &v in fl.values() {
            assert_eq!(v, 1.0, "Above liquidus should be fully liquid");
        }
    }

    #[test]
    fn liquid_fraction_linear_in_mushy_zone() {
        let model = EnthalpyPorosity::new(500.0, 600.0, 200_000.0);
        let mesh = make_1d_mesh(3, 1.0);
        // T = 500 -> fl=0, T=550 -> fl=0.5, T=600 -> fl=1.0
        let temp = ScalarField::new("temperature", vec![500.0, 550.0, 600.0]);
        let fl = model.compute_liquid_fraction(&temp, &mesh).unwrap();
        let vals = fl.values();
        assert!((vals[0] - 0.0).abs() < 1e-12);
        assert!((vals[1] - 0.5).abs() < 1e-12);
        assert!((vals[2] - 1.0).abs() < 1e-12);
    }

    #[test]
    fn carman_kozeny_fully_solid() {
        let model = EnthalpyPorosity::new(500.0, 600.0, 200_000.0);
        // f_l = 0 -> C * 1 / epsilon = 1e5 / 1e-3 = 1e8
        let coeff = model.carman_kozeny_coeff(0.0);
        let expected = 1.0e5 / 1.0e-3;
        assert!((coeff - expected).abs() / expected < 1e-10,
            "Fully solid: expected {}, got {}", expected, coeff);
    }

    #[test]
    fn carman_kozeny_fully_liquid() {
        let model = EnthalpyPorosity::new(500.0, 600.0, 200_000.0);
        // f_l = 1 -> C * 0 / (1 + eps) = 0
        let coeff = model.carman_kozeny_coeff(1.0);
        assert!(coeff.abs() < 1e-10,
            "Fully liquid: damping should be ~0, got {}", coeff);
    }

    #[test]
    fn carman_kozeny_mushy_zone() {
        let model = EnthalpyPorosity::new(500.0, 600.0, 200_000.0);
        // f_l = 0.5 -> C * 0.25 / (0.125 + eps) ~ C * 0.25 / 0.126 ~ C * 1.984
        let coeff = model.carman_kozeny_coeff(0.5);
        let expected = 1.0e5 * 0.25 / (0.125 + 1e-3);
        assert!((coeff - expected).abs() / expected < 1e-10);
    }

    #[test]
    fn solve_phase_change_step_first_call_zero_energy_source() {
        let mut model = EnthalpyPorosity::new(500.0, 600.0, 200_000.0)
            .with_density(1000.0);
        let mesh = make_1d_mesh(3, 1.0);
        let temp = ScalarField::new("temperature", vec![550.0; 3]);

        let result = model.solve_phase_change_step(&temp, &mesh, 0.01).unwrap();

        // First call: no previous liquid fraction, energy source should be zero
        for &v in result.energy_source.values() {
            assert_eq!(v, 0.0, "First step energy source should be zero");
        }
        // Liquid fraction at 550 K with T_s=500, T_l=600 -> fl = 0.5
        for &v in result.liquid_fraction.values() {
            assert!((v - 0.5).abs() < 1e-12);
        }
    }

    #[test]
    fn solve_phase_change_step_melting_absorbs_energy() {
        let mut model = EnthalpyPorosity::new(500.0, 600.0, 200_000.0)
            .with_density(1000.0);
        let mesh = make_1d_mesh(3, 1.0);
        let dt = 0.1;

        // Step 1: T = 520 -> fl = 0.2
        let temp1 = ScalarField::new("temperature", vec![520.0; 3]);
        let _ = model.solve_phase_change_step(&temp1, &mesh, dt).unwrap();

        // Step 2: T = 570 -> fl = 0.7 (melting: fl increased)
        let temp2 = ScalarField::new("temperature", vec![570.0; 3]);
        let result = model.solve_phase_change_step(&temp2, &mesh, dt).unwrap();

        // dfl/dt = (0.7 - 0.2) / 0.1 = 5.0
        // S_e = -rho * L * dfl/dt = -1000 * 200000 * 5.0 = -1e9
        // Negative = energy sink (melting absorbs energy)
        for &v in result.energy_source.values() {
            assert!(v < 0.0, "Melting should produce negative (sink) energy source, got {}", v);
            let expected = -1000.0 * 200_000.0 * 5.0;
            assert!((v - expected).abs() / expected.abs() < 1e-10,
                "Expected energy source {}, got {}", expected, v);
        }
    }

    #[test]
    fn solve_phase_change_step_solidification_releases_energy() {
        let mut model = EnthalpyPorosity::new(500.0, 600.0, 200_000.0)
            .with_density(1000.0);
        let mesh = make_1d_mesh(3, 1.0);
        let dt = 0.1;

        // Step 1: T = 570 -> fl = 0.7
        let temp1 = ScalarField::new("temperature", vec![570.0; 3]);
        let _ = model.solve_phase_change_step(&temp1, &mesh, dt).unwrap();

        // Step 2: T = 520 -> fl = 0.2 (solidification: fl decreased)
        let temp2 = ScalarField::new("temperature", vec![520.0; 3]);
        let result = model.solve_phase_change_step(&temp2, &mesh, dt).unwrap();

        // dfl/dt = (0.2 - 0.7) / 0.1 = -5.0
        // S_e = -rho * L * (-5.0) = +1e9 (positive = energy source released)
        for &v in result.energy_source.values() {
            assert!(v > 0.0, "Solidification should produce positive energy source, got {}", v);
        }
    }

    #[test]
    fn momentum_damping_zero_in_liquid_large_in_solid() {
        let mut model = EnthalpyPorosity::new(500.0, 600.0, 200_000.0)
            .with_density(1000.0);
        let mesh = make_1d_mesh(3, 1.0);
        let dt = 0.01;

        // Cell 0: fully solid (T=400), Cell 1: mushy (T=550), Cell 2: fully liquid (T=700)
        let temp = ScalarField::new("temperature", vec![400.0, 550.0, 700.0]);
        let result = model.solve_phase_change_step(&temp, &mesh, dt).unwrap();

        let damping = result.momentum_damping.values();

        // Fully solid: very large damping
        assert!(damping[0] > 1e7, "Solid should have large damping, got {}", damping[0]);

        // Mushy: intermediate
        assert!(damping[1] > 0.0 && damping[1] < damping[0],
            "Mushy damping {} should be between 0 and solid damping {}", damping[1], damping[0]);

        // Fully liquid: near-zero damping
        assert!(damping[2] < 1e-5, "Liquid should have near-zero damping, got {}", damping[2]);
    }

    #[test]
    fn coupled_solver_creation() {
        let solver = CoupledPhaseChangeSolver::new(500.0, 600.0, 200_000.0, 50.0, 4.0e6);
        assert_eq!(solver.conductivity, 50.0);
        assert_eq!(solver.rho_cp, 4.0e6);
        assert_eq!(solver.phase_change.solidus_temperature, 500.0);
    }

    #[test]
    fn coupled_solver_fully_liquid_no_source() {
        let mut solver = CoupledPhaseChangeSolver::new(500.0, 600.0, 200_000.0, 50.0, 4.0e6)
            .with_density(1000.0);
        solver.max_coupling_iterations = 5;
        let mesh = make_1d_mesh(5, 1.0);
        let n = mesh.num_cells();
        let mut temp = ScalarField::new("temperature", vec![700.0; n]);
        let mut boundary_temps = std::collections::HashMap::new();
        boundary_temps.insert("left".to_string(), 700.0);
        boundary_temps.insert("right".to_string(), 700.0);
        let result = solver.solve_coupled_phase_change(&mut temp, &mesh, 0.01, &boundary_temps).unwrap();
        for &v in result.liquid_fraction.values() {
            assert!((v - 1.0).abs() < 1e-10, "Should be fully liquid, got fl={}", v);
        }
        for &v in result.momentum_damping.values() {
            assert!(v < 1e-5, "Liquid should have near-zero damping, got {}", v);
        }
    }

    #[test]
    fn coupled_solver_fully_solid_large_damping() {
        let mut solver = CoupledPhaseChangeSolver::new(500.0, 600.0, 200_000.0, 50.0, 4.0e6)
            .with_density(1000.0);
        solver.max_coupling_iterations = 5;
        let mesh = make_1d_mesh(5, 1.0);
        let n = mesh.num_cells();
        let mut temp = ScalarField::new("temperature", vec![400.0; n]);
        let mut boundary_temps = std::collections::HashMap::new();
        boundary_temps.insert("left".to_string(), 400.0);
        boundary_temps.insert("right".to_string(), 400.0);
        let result = solver.solve_coupled_phase_change(&mut temp, &mesh, 0.01, &boundary_temps).unwrap();
        for &v in result.liquid_fraction.values() {
            assert!(v.abs() < 1e-10, "Should be fully solid, got fl={}", v);
        }
        for &v in result.momentum_damping.values() {
            assert!(v > 1e7, "Solid should have large damping, got {}", v);
        }
    }

    #[test]
    fn coupled_solver_returns_iterations() {
        let mut solver = CoupledPhaseChangeSolver::new(500.0, 600.0, 200_000.0, 50.0, 4.0e6)
            .with_density(1000.0);
        solver.max_coupling_iterations = 10;
        let mesh = make_1d_mesh(3, 1.0);
        let n = mesh.num_cells();
        let mut temp = ScalarField::new("temperature", vec![550.0; n]);
        let mut boundary_temps = std::collections::HashMap::new();
        boundary_temps.insert("left".to_string(), 550.0);
        boundary_temps.insert("right".to_string(), 550.0);
        let result = solver.solve_coupled_phase_change(&mut temp, &mesh, 0.01, &boundary_temps).unwrap();
        assert!(result.iterations >= 1);
        assert!(result.liquid_fraction_residual.is_finite());
    }

    #[test]
    fn coupled_solver_mushy_zone_convergence() {
        let mut solver = CoupledPhaseChangeSolver::new(500.0, 600.0, 200_000.0, 50.0, 4.0e6)
            .with_density(1000.0);
        solver.max_coupling_iterations = 20;
        solver.coupling_tolerance = 1e-6;
        let mesh = make_1d_mesh(5, 1.0);
        let n = mesh.num_cells();
        let mut temp = ScalarField::new("temperature", vec![550.0; n]);
        let mut boundary_temps = std::collections::HashMap::new();
        boundary_temps.insert("left".to_string(), 550.0);
        boundary_temps.insert("right".to_string(), 550.0);
        let result = solver.solve_coupled_phase_change(&mut temp, &mesh, 0.01, &boundary_temps).unwrap();
        for &v in result.liquid_fraction.values() {
            assert!((v - 0.5).abs() < 0.1, "Mushy zone fl should be ~0.5, got {}", v);
        }
    }
}
