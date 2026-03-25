//! P1 radiation model.
//!
//! Approximates the radiative transfer equation using the first-order
//! spherical harmonics expansion, resulting in a diffusion equation
//! for the incident radiation G.

use gfd_core::{ScalarField, UnstructuredMesh};
use crate::radiation::RadiationModel;
use crate::Result;

/// P1 radiation model.
///
/// Solves: div(Gamma * grad(G)) - a * G + 4 * a * sigma * T^4 = 0
/// where G is the incident radiation, a is the absorption coefficient,
/// Gamma = 1/(3*(a + sigma_s)), and sigma is Stefan-Boltzmann constant.
pub struct P1Radiation {
    /// Absorption coefficient [1/m].
    pub absorption_coefficient: f64,
    /// Scattering coefficient [1/m].
    pub scattering_coefficient: f64,
}

impl P1Radiation {
    /// Creates a new P1 radiation model.
    pub fn new(absorption_coefficient: f64, scattering_coefficient: f64) -> Self {
        Self {
            absorption_coefficient,
            scattering_coefficient,
        }
    }
}

impl RadiationModel for P1Radiation {
    fn solve(
        &mut self,
        temperature: &ScalarField,
        mesh: &UnstructuredMesh,
    ) -> Result<ScalarField> {
        let a = self.absorption_coefficient;
        let sigma_s = self.scattering_coefficient;
        let sigma_sb = 5.670374419e-8; // Stefan-Boltzmann constant [W/(m^2*K^4)]
        let gamma = 1.0 / (3.0 * (a + sigma_s));

        let num_cells = mesh.num_cells();
        let values = temperature.values();

        // Solve: div(Gamma * grad(G)) - a*G + 4*a*sigma*T^4 = 0
        // Approximate G iteratively; start with G = 4*sigma*T^4
        let mut g = vec![0.0_f64; num_cells];
        for i in 0..num_cells {
            let t = values[i];
            g[i] = 4.0 * sigma_sb * t * t * t * t;
        }

        // Simple Jacobi-like iteration for the diffusion equation
        for _iter in 0..100 {
            let mut g_new = vec![0.0_f64; num_cells];

            for cell_id in 0..num_cells {
                let vol = mesh.cells[cell_id].volume;
                let t = values[cell_id];
                let emission = 4.0 * a * sigma_sb * t * t * t * t * vol;

                let mut sum_flux = 0.0;
                let mut sum_coeff = 0.0;

                for &face_id in &mesh.cells[cell_id].faces {
                    let face = &mesh.faces[face_id];
                    if let Some(neighbor) = face.neighbor_cell {
                        let other = if neighbor == cell_id {
                            face.owner_cell
                        } else {
                            neighbor
                        };
                        let dx = mesh.cells[other].center[0] - mesh.cells[cell_id].center[0];
                        let dy = mesh.cells[other].center[1] - mesh.cells[cell_id].center[1];
                        let dz = mesh.cells[other].center[2] - mesh.cells[cell_id].center[2];
                        let dist = (dx * dx + dy * dy + dz * dz).sqrt().max(1e-30);
                        let diff_coeff = gamma * face.area / dist;
                        sum_flux += diff_coeff * g[other];
                        sum_coeff += diff_coeff;
                    }
                }

                let denom = sum_coeff + a * vol;
                if denom.abs() > 1e-30 {
                    g_new[cell_id] = (sum_flux + emission) / denom;
                } else {
                    g_new[cell_id] = g[cell_id];
                }
            }

            g = g_new;
        }

        // Compute radiative source: S_rad = a * (G - 4*sigma*T^4)
        let mut source = vec![0.0_f64; num_cells];
        for i in 0..num_cells {
            let t = values[i];
            source[i] = a * (g[i] - 4.0 * sigma_sb * t * t * t * t);
        }

        Ok(ScalarField::new("radiative_source", source))
    }

    fn name(&self) -> &str {
        "P1"
    }
}
