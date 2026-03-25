//! Creep (time-dependent deformation) models.

use gfd_core::UnstructuredMesh;
use crate::{SolidState, Result};

/// Creep solver for time-dependent inelastic deformation.
///
/// Supports Norton (power law) and other creep models.
pub struct CreepSolver {
    /// Norton creep coefficient A.
    pub norton_a: f64,
    /// Norton stress exponent n.
    pub norton_n: f64,
    /// Activation energy / (gas constant) [K].
    pub q_over_r: f64,
}

impl CreepSolver {
    /// Creates a new Norton creep model.
    ///
    /// Creep strain rate: d(epsilon_cr)/dt = A * sigma^n * exp(-Q/(R*T))
    pub fn norton(a: f64, n: f64, q_over_r: f64) -> Self {
        Self {
            norton_a: a,
            norton_n: n,
            q_over_r,
        }
    }

    /// Performs one creep time step.
    pub fn solve_step(
        &self,
        _state: &mut SolidState,
        _mesh: &UnstructuredMesh,
        _temperature: &[f64],
        _dt: f64,
    ) -> Result<f64> {
        let num_cells = _state.num_cells();
        let dt = _dt;
        let a_coeff = self.norton_a;
        let n_exp = self.norton_n;
        let q_r = self.q_over_r;

        let mut max_creep_strain = 0.0_f64;

        for i in 0..num_cells {
            let stress = _state.stress.get(i).unwrap_or([[0.0; 3]; 3]);

            // Compute Von Mises equivalent stress
            let hydrostatic = (stress[0][0] + stress[1][1] + stress[2][2]) / 3.0;
            let mut j2 = 0.0_f64;
            for a in 0..3 {
                for b in 0..3 {
                    let s_ab = stress[a][b] - if a == b { hydrostatic } else { 0.0 };
                    j2 += if a == b { 0.5 } else { 1.0 } * s_ab * s_ab;
                }
            }
            let sigma_vm = (3.0 * j2).sqrt();

            // Temperature for this cell
            let temp = if i < _temperature.len() { _temperature[i] } else { 300.0 };

            // Norton creep rate: d(eps_cr)/dt = A * sigma^n * exp(-Q/(R*T))
            let creep_rate = a_coeff * sigma_vm.powf(n_exp) * (-q_r / temp.max(1.0)).exp();

            // Creep strain increment
            let d_eps_cr = creep_rate * dt;
            if d_eps_cr > max_creep_strain {
                max_creep_strain = d_eps_cr;
            }

            // Update strain: add creep strain in deviatoric direction
            if sigma_vm > 1e-30 {
                let mut strain = _state.strain.get(i).unwrap_or([[0.0; 3]; 3]);
                let factor = 1.5 * d_eps_cr / sigma_vm;
                for a in 0..3 {
                    for b in 0..3 {
                        let s_ab = stress[a][b] - if a == b { hydrostatic } else { 0.0 };
                        strain[a][b] += factor * s_ab;
                    }
                }
                let _ = _state.strain.set(i, strain);
            }
        }

        Ok(max_creep_strain)
    }
}
