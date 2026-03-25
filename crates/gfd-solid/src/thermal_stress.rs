//! Thermal stress analysis for thermo-mechanical coupling.

use gfd_core::{ScalarField, UnstructuredMesh};
use crate::{SolidState, Result};

/// Thermal stress solver for computing mechanical response to temperature changes.
///
/// Adds thermal strain to the total strain:
/// epsilon_total = epsilon_mechanical + epsilon_thermal
/// epsilon_thermal = alpha * (T - T_ref) * I
pub struct ThermalStressSolver {
    /// Coefficient of thermal expansion [1/K].
    pub thermal_expansion_coefficient: f64,
    /// Reference temperature [K].
    pub reference_temperature: f64,
    /// Young's modulus [Pa].
    pub youngs_modulus: f64,
    /// Poisson's ratio [-].
    pub poissons_ratio: f64,
}

impl ThermalStressSolver {
    /// Creates a new thermal stress solver.
    pub fn new(
        thermal_expansion_coefficient: f64,
        reference_temperature: f64,
        youngs_modulus: f64,
        poissons_ratio: f64,
    ) -> Self {
        Self {
            thermal_expansion_coefficient,
            reference_temperature,
            youngs_modulus,
            poissons_ratio,
        }
    }

    /// Solves for displacement and stress given a temperature field.
    ///
    /// The thermal strain acts as an initial strain that generates
    /// thermal stresses when the body is constrained.
    pub fn solve(
        &self,
        _state: &mut SolidState,
        _temperature: &ScalarField,
        _mesh: &UnstructuredMesh,
    ) -> Result<f64> {
        // 1. Compute thermal strain: epsilon_th = alpha * (T - T_ref) * I
        // 2. Compute thermal load vector: f_th = integral(B^T * C * epsilon_th * dV)
        // 3. Add to global force vector
        // 4. Solve K * u = f + f_th
        // 5. Compute total strain and subtract thermal strain for mechanical strain
        // 6. Compute stress from mechanical strain
        let num_cells = _state.num_cells();
        let alpha = self.thermal_expansion_coefficient;
        let t_ref = self.reference_temperature;
        let e = self.youngs_modulus;
        let nu = self.poissons_ratio;

        let lambda = e * nu / ((1.0 + nu) * (1.0 - 2.0 * nu));
        let g = e / (2.0 * (1.0 + nu));

        let temp_values = _temperature.values();

        let mut max_stress = 0.0_f64;

        for i in 0..num_cells {
            let t = if i < temp_values.len() { temp_values[i] } else { t_ref };

            // Thermal strain: eps_th = alpha * (T - T_ref) * I
            let eps_th = alpha * (t - t_ref);

            // Total strain
            let total_strain = _state.strain.get(i).unwrap_or([[0.0; 3]; 3]);

            // Mechanical strain = total - thermal
            let mut mech_strain = total_strain;
            for dim in 0..3 {
                mech_strain[dim][dim] -= eps_th;
            }

            // Compute stress from mechanical strain (Hooke's law)
            let trace_mech = mech_strain[0][0] + mech_strain[1][1] + mech_strain[2][2];
            let mut stress = [[0.0_f64; 3]; 3];
            for a in 0..3 {
                for b in 0..3 {
                    stress[a][b] = 2.0 * g * mech_strain[a][b];
                    if a == b {
                        stress[a][b] += lambda * trace_mech;
                    }
                }
            }

            let _ = _state.stress.set(i, stress);

            // Track max stress for convergence
            let vm = (0.5 * ((stress[0][0] - stress[1][1]).powi(2)
                + (stress[1][1] - stress[2][2]).powi(2)
                + (stress[2][2] - stress[0][0]).powi(2)
                + 6.0 * (stress[0][1].powi(2) + stress[1][2].powi(2) + stress[0][2].powi(2))))
            .sqrt();
            if vm > max_stress {
                max_stress = vm;
            }
        }

        Ok(max_stress)
    }
}
