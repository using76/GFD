//! Combustion and species transport solvers.

pub mod species;
pub mod reaction;

use gfd_core::{ScalarField, UnstructuredMesh};
use crate::Result;

/// Species transport solver.
///
/// Solves the species mass fraction transport equation:
/// d(rho*Y_i)/dt + div(rho*U*Y_i) = div(rho*D_i*grad(Y_i)) + omega_i
pub struct SpeciesTransport {
    /// Names of the species.
    pub species_names: Vec<String>,
    /// Diffusion coefficients for each species [m^2/s].
    pub diffusion_coefficients: Vec<f64>,
}

impl SpeciesTransport {
    /// Creates a new species transport solver.
    pub fn new(species_names: Vec<String>, diffusion_coefficients: Vec<f64>) -> Self {
        Self {
            species_names,
            diffusion_coefficients,
        }
    }

    /// Solves the species transport equations for one time step.
    pub fn solve_step(
        &self,
        mass_fractions: &mut [ScalarField],
        mesh: &UnstructuredMesh,
        dt: f64,
    ) -> Result<()> {
        let n = mesh.num_cells();
        let num_species = mass_fractions.len();

        // Solve for each species except the last (constraint: sum = 1)
        let solve_count = if num_species > 1 { num_species - 1 } else { num_species };

        for sp in 0..solve_count {
            let y_old = mass_fractions[sp].values().to_vec();
            let mut y_new = y_old.clone();
            let diff = self.diffusion_coefficients[sp];

            // Accumulate fluxes per cell
            let mut net_flux = vec![0.0; n];

            for face in &mesh.faces {
                let owner = face.owner_cell;

                if let Some(neighbor) = face.neighbor_cell {
                    // Diffusive flux: D * grad(Y) . A_f ~ D * (Y_n - Y_o) / dist * area
                    let dist = {
                        let co = mesh.cells[owner].center;
                        let cn = mesh.cells[neighbor].center;
                        ((co[0] - cn[0]).powi(2)
                            + (co[1] - cn[1]).powi(2)
                            + (co[2] - cn[2]).powi(2))
                        .sqrt()
                        .max(1e-30)
                    };
                    let diff_flux = diff * (y_old[neighbor] - y_old[owner]) / dist * face.area;
                    net_flux[owner] += diff_flux;
                    net_flux[neighbor] -= diff_flux;
                }
                // Boundary: zero-gradient (no flux)
            }

            // Time integration: explicit Euler
            for i in 0..n {
                let vol = mesh.cells[i].volume;
                if vol > 0.0 {
                    y_new[i] = y_old[i] + dt / vol * net_flux[i];
                }
                y_new[i] = y_new[i].clamp(0.0, 1.0);
            }

            mass_fractions[sp].values_mut().copy_from_slice(&y_new);
        }

        // Compute last species from constraint: Y_N = 1 - sum(Y_i)
        if num_species > 1 {
            for cell in 0..n {
                let sum: f64 = (0..num_species - 1)
                    .map(|sp| mass_fractions[sp].values()[cell])
                    .sum();
                let _ = mass_fractions[num_species - 1].set(cell, (1.0 - sum).clamp(0.0, 1.0));
            }
        }

        Ok(())
    }
}

/// Reaction model for computing species source terms.
pub struct ReactionModel {
    /// Type of reaction model: "finite_rate", "eddy_dissipation", "eddy_breakup".
    pub model_type: String,
}

impl ReactionModel {
    /// Creates a new reaction model.
    pub fn new(model_type: impl Into<String>) -> Self {
        Self {
            model_type: model_type.into(),
        }
    }

    /// Computes the reaction source terms for all species.
    pub fn compute_source_terms(
        &self,
        mass_fractions: &[ScalarField],
        _temperature: &ScalarField,
        _density: &ScalarField,
    ) -> Result<Vec<ScalarField>> {
        let n = if let Some(first) = mass_fractions.first() {
            first.values().len()
        } else {
            return Ok(Vec::new());
        };
        let num_species = mass_fractions.len();

        // Initialize source term fields (zero)
        let source_terms: Vec<ScalarField> = (0..num_species)
            .map(|sp| ScalarField::zeros(&format!("omega_{}", sp), n))
            .collect();

        // For "finite_rate" model, compute Arrhenius-type source terms
        // For simplicity, return zero source terms for other model types
        if self.model_type == "finite_rate" {
            // The actual computation would involve reaction rate constants
            // and species concentrations. For now, we provide zero source terms
            // as a safe default (no reactions occur without a ReactionSet).
        }
        // "eddy_dissipation" and "eddy_breakup" models also return zero here
        // as they require additional turbulence information.

        Ok(source_terms)
    }
}
