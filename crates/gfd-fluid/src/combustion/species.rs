//! Species transport solver for combustion simulations.
//!
//! Solves the transport equations for individual chemical species
//! mass fractions, including convection, diffusion, and reaction source terms.

use gfd_core::{ScalarField, VectorField, UnstructuredMesh};
use crate::Result;

/// Solver for species transport equations in reacting flows.
///
/// Solves the species mass fraction equation for each species i:
/// d(rho*Y_i)/dt + div(rho*U*Y_i) = div(rho*D_i*grad(Y_i)) + omega_i
///
/// where Y_i is the mass fraction, D_i the diffusion coefficient,
/// and omega_i the reaction source term for species i.
pub struct SpeciesTransportSolver {
    /// Mass fraction fields for each species.
    pub mass_fractions: Vec<ScalarField>,
    /// Species names.
    pub species_names: Vec<String>,
    /// Diffusion coefficients for each species [m^2/s].
    pub diffusivities: Vec<f64>,
    /// Schmidt number for turbulent diffusion (default 0.7).
    pub turbulent_schmidt: f64,
}

impl SpeciesTransportSolver {
    /// Creates a new species transport solver.
    pub fn new(
        species_names: Vec<String>,
        diffusivities: Vec<f64>,
        num_cells: usize,
    ) -> Self {
        assert_eq!(species_names.len(), diffusivities.len(),
            "Number of species names must match number of diffusivities");

        let mass_fractions = species_names
            .iter()
            .map(|name| ScalarField::zeros(&format!("Y_{}", name), num_cells))
            .collect();

        Self {
            mass_fractions,
            species_names,
            diffusivities,
            turbulent_schmidt: 0.7,
        }
    }

    /// Returns the number of species.
    pub fn num_species(&self) -> usize {
        self.species_names.len()
    }

    /// Solves the species transport equations for one time step.
    ///
    /// For each species i (except the last, which is computed from
    /// the constraint sum(Y_i) = 1):
    /// 1. Assemble the convection-diffusion equation
    /// 2. Add reaction source terms
    /// 3. Solve the linear system
    /// 4. Bound mass fractions to [0, 1]
    pub fn solve_species(
        &mut self,
        velocity: &VectorField,
        density: &ScalarField,
        mesh: &UnstructuredMesh,
        dt: f64,
        reaction_rates: &[ScalarField],
    ) -> Result<()> {
        let n = mesh.num_cells();
        let num_species = self.mass_fractions.len();
        if num_species == 0 {
            return Ok(());
        }

        // Solve for each species except the last (constraint: sum(Y_i) = 1)
        let solve_count = if num_species > 1 { num_species - 1 } else { num_species };

        for sp in 0..solve_count {
            let y_old = self.mass_fractions[sp].values().to_vec();
            let mut y_new = y_old.clone();
            let diff = self.diffusivities[sp];

            // Accumulate net flux per cell
            let mut net_flux = vec![0.0; n];

            for face in &mesh.faces {
                let owner = face.owner_cell;

                if let Some(neighbor) = face.neighbor_cell {
                    // Face velocity and density (interpolated)
                    let vel_o = velocity.values()[owner];
                    let vel_n = velocity.values()[neighbor];
                    let rho_o = density.values()[owner];
                    let rho_n = density.values()[neighbor];
                    let rho_f = 0.5 * (rho_o + rho_n);
                    let u_f = [
                        0.5 * (vel_o[0] + vel_n[0]),
                        0.5 * (vel_o[1] + vel_n[1]),
                        0.5 * (vel_o[2] + vel_n[2]),
                    ];

                    // Convective mass flux
                    let mass_flux = rho_f
                        * (u_f[0] * face.normal[0]
                            + u_f[1] * face.normal[1]
                            + u_f[2] * face.normal[2])
                        * face.area;

                    // Upwind for convective flux
                    let y_f = if mass_flux >= 0.0 {
                        y_old[owner]
                    } else {
                        y_old[neighbor]
                    };
                    let conv_flux = mass_flux * y_f;

                    // Diffusive flux: rho * D * (Y_n - Y_o) / dist * area
                    let co = mesh.cells[owner].center;
                    let cn = mesh.cells[neighbor].center;
                    let dist = ((co[0] - cn[0]).powi(2)
                        + (co[1] - cn[1]).powi(2)
                        + (co[2] - cn[2]).powi(2))
                    .sqrt()
                    .max(1e-30);
                    let diff_flux =
                        rho_f * diff * (y_old[neighbor] - y_old[owner]) / dist * face.area;

                    // Total flux: convective (out of owner) - diffusive (into owner)
                    net_flux[owner] += conv_flux - diff_flux;
                    net_flux[neighbor] -= conv_flux - diff_flux;
                } else {
                    // Boundary: zero-gradient (no diffusive flux, convective uses owner value)
                    let vel_o = velocity.values()[owner];
                    let rho_o = density.values()[owner];
                    let mass_flux = rho_o
                        * (vel_o[0] * face.normal[0]
                            + vel_o[1] * face.normal[1]
                            + vel_o[2] * face.normal[2])
                        * face.area;
                    let conv_flux = mass_flux * y_old[owner];
                    net_flux[owner] += conv_flux;
                }
            }

            // Time integration: explicit Euler
            // d(rho*Y)/dt = -net_flux/V + omega
            for i in 0..n {
                let vol = mesh.cells[i].volume;
                let rho_i = density.values()[i];
                if vol > 0.0 && rho_i > 0.0 {
                    let source = if sp < reaction_rates.len() {
                        reaction_rates[sp].values()[i]
                    } else {
                        0.0
                    };
                    y_new[i] = y_old[i] + dt / (rho_i * vol) * (-net_flux[i] + source * vol);
                }
                y_new[i] = y_new[i].clamp(0.0, 1.0);
            }

            self.mass_fractions[sp].values_mut().copy_from_slice(&y_new);
        }

        // Compute last species from constraint: Y_N = 1 - sum(Y_i, i=0..N-2)
        if num_species > 1 {
            for cell in 0..n {
                let sum: f64 = (0..num_species - 1)
                    .map(|sp| self.mass_fractions[sp].values()[cell])
                    .sum();
                let _ = self.mass_fractions[num_species - 1].set(cell, (1.0 - sum).clamp(0.0, 1.0));
            }
        }

        // Renormalize
        self.normalize_mass_fractions();

        Ok(())
    }

    /// Ensures that mass fractions sum to 1 and are bounded in [0, 1].
    pub fn normalize_mass_fractions(&mut self) {
        let num_cells = if let Some(first) = self.mass_fractions.first() {
            first.values().len()
        } else {
            return;
        };

        for cell in 0..num_cells {
            // Clamp all to [0, 1]
            for mf in self.mass_fractions.iter_mut() {
                let val = mf.values_mut();
                val[cell] = val[cell].clamp(0.0, 1.0);
            }

            // Normalize so sum = 1
            let sum: f64 = self.mass_fractions.iter().map(|mf| mf.values()[cell]).sum();
            if sum > 0.0 {
                for mf in self.mass_fractions.iter_mut() {
                    mf.values_mut()[cell] /= sum;
                }
            }
        }
    }
}
