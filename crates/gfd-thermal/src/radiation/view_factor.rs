//! View factor (surface-to-surface) radiation model.

use gfd_core::{ScalarField, UnstructuredMesh};
use crate::radiation::RadiationModel;
use crate::Result;

/// View factor radiation model for surface-to-surface radiation.
///
/// Computes radiative exchange between surface elements using
/// precomputed view factor matrices.
pub struct ViewFactorRadiation {
    /// Precomputed view factor matrix F[i][j] (row = emitting, col = receiving).
    pub view_factors: Vec<Vec<f64>>,
    /// Surface emissivities.
    pub emissivities: Vec<f64>,
}

impl ViewFactorRadiation {
    /// Creates a new view factor radiation model.
    pub fn new(view_factors: Vec<Vec<f64>>, emissivities: Vec<f64>) -> Self {
        Self {
            view_factors,
            emissivities,
        }
    }
}

impl RadiationModel for ViewFactorRadiation {
    fn solve(
        &mut self,
        temperature: &ScalarField,
        mesh: &UnstructuredMesh,
    ) -> Result<ScalarField> {
        let sigma_sb = 5.670374419e-8; // Stefan-Boltzmann constant
        let num_cells = mesh.num_cells();
        let values = temperature.values();

        // Number of surfaces = number of rows in view factor matrix
        let n_surfaces = self.view_factors.len();

        // Compute radiosity for each surface using view factors:
        // q_rad_i = epsilon_i * sigma * T_i^4 + (1-epsilon_i) * sum_j(F_ij * q_rad_j)
        // Simplified: assume epsilon = 1 (blackbody) so q_rad_i = sigma * T_i^4

        // Compute net radiative heat flux for each surface
        let mut surface_heat_flux = vec![0.0_f64; n_surfaces];
        for i in 0..n_surfaces {
            if i >= values.len() {
                break;
            }
            let ti = values[i];
            let ei = if i < self.emissivities.len() {
                self.emissivities[i]
            } else {
                1.0
            };
            let ebi = sigma_sb * ti * ti * ti * ti;

            // Net heat leaving surface i = emitted - absorbed from other surfaces
            let mut irradiation = 0.0;
            for j in 0..n_surfaces {
                if j >= values.len() || i >= self.view_factors.len() {
                    break;
                }
                if j < self.view_factors[i].len() {
                    let tj = values[j];
                    let ebj = sigma_sb * tj * tj * tj * tj;
                    irradiation += self.view_factors[i][j] * ebj;
                }
            }
            surface_heat_flux[i] = ei * (ebi - irradiation);
        }

        // Map surface heat fluxes back to a volumetric source (W/m^3)
        // For cells that are not surfaces, source = 0
        let mut source = vec![0.0_f64; num_cells];
        for i in 0..n_surfaces.min(num_cells) {
            let vol = mesh.cells[i].volume;
            if vol > 1e-30 {
                source[i] = -surface_heat_flux[i] / vol;
            }
        }

        Ok(ScalarField::new("radiative_source_vf", source))
    }

    fn name(&self) -> &str {
        "ViewFactor"
    }
}
