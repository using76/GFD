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
}
