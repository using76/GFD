//! Combustion and species transport solvers.

pub mod species;
pub mod reaction;

use gfd_core::{ScalarField, UnstructuredMesh};
use crate::Result;
use crate::combustion::reaction::ReactionSet;

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
///
/// Supports two combustion models:
/// - "finite_rate": Uses Arrhenius kinetics via a `ReactionSet` to compute
///   species source terms from temperature and species concentrations.
/// - "eddy_dissipation": Uses the Magnussen Eddy Dissipation Model (EDM),
///   where the reaction rate is controlled by turbulent mixing (epsilon/k).
pub struct ReactionModel {
    /// Type of reaction model: "finite_rate", "eddy_dissipation", "eddy_breakup".
    pub model_type: String,
    /// Reaction set for finite-rate chemistry (Arrhenius kinetics).
    pub reaction_set: Option<ReactionSet>,
    /// Molecular weights for each species [kg/mol].
    pub molecular_weights: Vec<f64>,
    /// Stoichiometric mass ratios for EDM: s_i for each species
    /// (mass of species i consumed per unit mass of fuel consumed).
    /// Only used by "eddy_dissipation" model.
    pub stoich_mass_ratios: Vec<f64>,
    /// EDM model constant A (default 4.0).
    pub edm_constant_a: f64,
    /// Fuel species index for EDM.
    pub fuel_index: usize,
    /// Oxidizer species index for EDM.
    pub oxidizer_index: usize,
}

impl ReactionModel {
    /// Creates a new reaction model with default settings.
    pub fn new(model_type: impl Into<String>) -> Self {
        Self {
            model_type: model_type.into(),
            reaction_set: None,
            molecular_weights: Vec::new(),
            stoich_mass_ratios: Vec::new(),
            edm_constant_a: 4.0,
            fuel_index: 0,
            oxidizer_index: 1,
        }
    }

    /// Creates a finite-rate reaction model with the given reaction set and molecular weights.
    pub fn finite_rate(reaction_set: ReactionSet, molecular_weights: Vec<f64>) -> Self {
        assert_eq!(
            reaction_set.num_species,
            molecular_weights.len(),
            "Number of species in ReactionSet must match molecular_weights length"
        );
        Self {
            model_type: "finite_rate".to_string(),
            reaction_set: Some(reaction_set),
            molecular_weights,
            stoich_mass_ratios: Vec::new(),
            edm_constant_a: 4.0,
            fuel_index: 0,
            oxidizer_index: 1,
        }
    }

    /// Creates an eddy dissipation model.
    ///
    /// # Arguments
    /// * `molecular_weights` - Molecular weights [kg/mol] for each species.
    /// * `stoich_mass_ratios` - Stoichiometric mass ratios: s_i = (nu_i * MW_i) / (nu_fuel * MW_fuel).
    ///   For the fuel itself, s_fuel = 1.0.
    /// * `fuel_index` - Index of the fuel species.
    /// * `oxidizer_index` - Index of the oxidizer species.
    /// * `edm_constant_a` - EDM constant A (typically 4.0).
    pub fn eddy_dissipation(
        molecular_weights: Vec<f64>,
        stoich_mass_ratios: Vec<f64>,
        fuel_index: usize,
        oxidizer_index: usize,
        edm_constant_a: f64,
    ) -> Self {
        assert_eq!(
            molecular_weights.len(),
            stoich_mass_ratios.len(),
            "molecular_weights and stoich_mass_ratios must have the same length"
        );
        Self {
            model_type: "eddy_dissipation".to_string(),
            reaction_set: None,
            molecular_weights,
            stoich_mass_ratios,
            edm_constant_a,
            fuel_index,
            oxidizer_index,
        }
    }

    /// Computes the reaction source terms for all species [kg/(m^3*s)].
    ///
    /// For "finite_rate": Uses Arrhenius kinetics.
    ///   1. Convert mass fractions to molar concentrations: c_i = rho * Y_i / MW_i
    ///   2. Compute net molar production rates via ReactionSet: omega_i [mol/(m^3*s)]
    ///   3. Convert to mass source: S_i = omega_i * MW_i [kg/(m^3*s)]
    ///
    /// For "eddy_dissipation": Uses the Magnussen EDM.
    ///   rate = A * rho * epsilon/k * min(Y_fuel/s_fuel, Y_ox/s_ox)
    ///   S_i = -stoich_coeff_i * rate  (negative for reactants, positive for products)
    pub fn compute_source_terms(
        &self,
        mass_fractions: &[ScalarField],
        temperature: &ScalarField,
        density: &ScalarField,
    ) -> Result<Vec<ScalarField>> {
        let n = if let Some(first) = mass_fractions.first() {
            first.values().len()
        } else {
            return Ok(Vec::new());
        };
        let num_species = mass_fractions.len();

        // Initialize source term fields (zero)
        let mut source_terms: Vec<ScalarField> = (0..num_species)
            .map(|sp| ScalarField::zeros(&format!("omega_{}", sp), n))
            .collect();

        match self.model_type.as_str() {
            "finite_rate" => {
                if let Some(ref rs) = self.reaction_set {
                    if self.molecular_weights.len() != num_species {
                        return Ok(source_terms); // Safety: cannot compute without MW
                    }
                    let mw = &self.molecular_weights;
                    let t_vals = temperature.values();
                    let rho_vals = density.values();

                    for cell in 0..n {
                        let rho = rho_vals[cell];
                        let t = t_vals[cell];
                        if t <= 0.0 || rho <= 0.0 {
                            continue;
                        }

                        // Convert mass fractions to molar concentrations [mol/m^3]
                        let concentrations: Vec<f64> = (0..num_species)
                            .map(|sp| {
                                let y = mass_fractions[sp].values()[cell].max(0.0);
                                if mw[sp] > 0.0 {
                                    rho * y / mw[sp]
                                } else {
                                    0.0
                                }
                            })
                            .collect();

                        // Compute net molar production rates [mol/(m^3*s)]
                        let net_rates = rs.compute_net_rates(t, &concentrations);

                        // Convert to mass source terms [kg/(m^3*s)]
                        for sp in 0..num_species {
                            let _ = source_terms[sp].set(cell, net_rates[sp] * mw[sp]);
                        }
                    }
                }
            }
            "eddy_dissipation" => {
                // Magnussen Eddy Dissipation Model (EDM)
                // Requires turbulence quantities k and epsilon to be provided
                // via the density field trick or separate fields.
                //
                // For cells where k > 0 and epsilon > 0:
                //   rate_fuel = A * rho * (epsilon/k) * Y_fuel / s_fuel
                //   rate_ox   = A * rho * (epsilon/k) * Y_ox / s_ox
                //   rate = min(rate_fuel, rate_ox)
                //   S_i = -s_i * rate  for reactants
                //   S_i = +s_i * rate  for products (s_i < 0 convention or separate handling)
                //
                // Since FluidState turbulence fields are not passed here, we use
                // a simplified EDM that takes k and epsilon embedded in the
                // temperature field as a proxy, or works with the available data.
                // In practice, this function is called with k and epsilon available
                // in the calling context.
                self.compute_edm_source_terms(
                    mass_fractions,
                    density,
                    n,
                    num_species,
                    &mut source_terms,
                );
            }
            _ => {
                // Unknown model type: return zero source terms (safe default)
            }
        }

        Ok(source_terms)
    }

    /// Computes EDM source terms with explicit turbulence quantities.
    ///
    /// This is the primary EDM interface where k and epsilon are provided directly.
    ///
    /// rate = A * rho * (epsilon/k) * min(Y_fuel/s_fuel, Y_ox/s_ox)
    /// S_i = -s_i * rate  (reactants consumed, products formed)
    pub fn compute_edm_source_terms_with_turbulence(
        &self,
        mass_fractions: &[ScalarField],
        density: &ScalarField,
        turb_k: &ScalarField,
        turb_epsilon: &ScalarField,
        source_terms: &mut [ScalarField],
    ) {
        let n = density.values().len();
        let num_species = mass_fractions.len();
        if self.stoich_mass_ratios.len() != num_species || num_species == 0 {
            return;
        }

        let rho_vals = density.values();
        let k_vals = turb_k.values();
        let eps_vals = turb_epsilon.values();
        let s = &self.stoich_mass_ratios;
        let a = self.edm_constant_a;
        let fi = self.fuel_index;
        let oi = self.oxidizer_index;

        for cell in 0..n {
            let rho = rho_vals[cell];
            let k = k_vals[cell];
            let eps = eps_vals[cell];

            if k <= 1e-30 || eps <= 0.0 || rho <= 0.0 {
                continue;
            }

            let y_fuel = mass_fractions[fi].values()[cell].max(0.0);
            let y_ox = mass_fractions[oi].values()[cell].max(0.0);
            let s_fuel = s[fi].max(1e-30);
            let s_ox = s[oi].max(1e-30);

            // Magnussen EDM: rate = A * rho * (epsilon/k) * min(Y_fuel/s_fuel, Y_ox/s_ox)
            let mixing_rate = a * rho * eps / k;
            let rate = mixing_rate * f64::min(y_fuel / s_fuel, y_ox / s_ox);

            // Source terms: S_i = -s_i * rate for reactants, +s_i * rate for products
            for sp in 0..num_species {
                // Negative s_i means reactant is consumed, positive means product is formed
                // Convention: stoich_mass_ratios[i] > 0 for reactants, < 0 for products
                let source = -s[sp] * rate;
                let _ = source_terms[sp].set(cell, source);
            }
        }
    }

    /// Internal helper for EDM without explicit turbulence fields.
    ///
    /// Uses a default mixing rate (epsilon/k = 1.0) as a fallback when
    /// turbulence quantities are not available. For proper EDM, use
    /// `compute_edm_source_terms_with_turbulence` instead.
    fn compute_edm_source_terms(
        &self,
        mass_fractions: &[ScalarField],
        density: &ScalarField,
        n: usize,
        num_species: usize,
        source_terms: &mut [ScalarField],
    ) {
        if self.stoich_mass_ratios.len() != num_species || num_species == 0 {
            return;
        }

        let rho_vals = density.values();
        let s = &self.stoich_mass_ratios;
        let a = self.edm_constant_a;
        let fi = self.fuel_index;
        let oi = self.oxidizer_index;

        // Default mixing rate when k/epsilon not provided
        let default_mixing_ratio = 1.0; // epsilon/k fallback

        for cell in 0..n {
            let rho = rho_vals[cell];
            if rho <= 0.0 {
                continue;
            }

            let y_fuel = mass_fractions[fi].values()[cell].max(0.0);
            let y_ox = mass_fractions[oi].values()[cell].max(0.0);
            let s_fuel = s[fi].max(1e-30);
            let s_ox = s[oi].max(1e-30);

            let mixing_rate = a * rho * default_mixing_ratio;
            let rate = mixing_rate * f64::min(y_fuel / s_fuel, y_ox / s_ox);

            for sp in 0..num_species {
                let source = -s[sp] * rate;
                let _ = source_terms[sp].set(cell, source);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::combustion::reaction::{ArrheniusReaction, ReactionSet};

    #[test]
    fn test_finite_rate_source_terms() {
        // 3-species system: A + B -> C
        // Species 0: A (fuel), Species 1: B (oxidizer), Species 2: C (product)
        let mut rs = ReactionSet::new(3);
        rs.add_reaction(ArrheniusReaction::new(
            "A + B -> C",
            1.0e6,       // A (pre-exponential)
            50_000.0,    // E_a [J/mol]
            0.0,         // beta
            vec![-1.0, -1.0, 1.0],  // stoich: A consumed, B consumed, C produced
            vec![1.0, 1.0, 0.0],    // first order in A and B
        ));

        let mw = vec![0.016, 0.032, 0.044]; // kg/mol (CH4-like, O2-like, CO2-like)
        let model = ReactionModel::finite_rate(rs, mw.clone());

        let n = 4; // 4 cells
        // Set up mass fractions: Y_A=0.1, Y_B=0.2, Y_C=0.0
        let mass_fractions = vec![
            ScalarField::new("Y_A", vec![0.1; n]),
            ScalarField::new("Y_B", vec![0.2; n]),
            ScalarField::new("Y_C", vec![0.0; n]),
        ];
        let temperature = ScalarField::new("T", vec![1500.0; n]);
        let density = ScalarField::new("rho", vec![1.0; n]);

        let sources = model.compute_source_terms(&mass_fractions, &temperature, &density).unwrap();

        assert_eq!(sources.len(), 3);
        // Species A (fuel) should be consumed (negative source)
        assert!(sources[0].values()[0] < 0.0,
            "Fuel A should have negative source term, got {}", sources[0].values()[0]);
        // Species B (oxidizer) should be consumed (negative source)
        assert!(sources[1].values()[0] < 0.0,
            "Oxidizer B should have negative source term, got {}", sources[1].values()[0]);
        // Species C (product) should be produced (positive source)
        assert!(sources[2].values()[0] > 0.0,
            "Product C should have positive source term, got {}", sources[2].values()[0]);

        // Verify mass conservation: sum of source terms * (1/MW) should be zero
        // Actually: sum of stoich * rate is already mass-conserving via MW
        // Check that |S_A/MW_A| = |S_B/MW_B| = |S_C/MW_C| for this 1:1:1 molar reaction
        let s_a = sources[0].values()[0];
        let s_b = sources[1].values()[0];
        let s_c = sources[2].values()[0];
        let molar_a = s_a / mw[0];
        let molar_b = s_b / mw[1];
        let molar_c = s_c / mw[2];
        // molar rates should be equal in magnitude: -omega_A = -omega_B = omega_C
        assert!((molar_a - molar_b).abs() < 1e-6 * molar_a.abs(),
            "Molar consumption rates should match: A={}, B={}", molar_a, molar_b);
        assert!((molar_a + molar_c).abs() < 1e-6 * molar_c.abs(),
            "Molar production rate should match consumption: A={}, C={}", molar_a, molar_c);
    }

    #[test]
    fn test_finite_rate_zero_temperature_no_reaction() {
        let mut rs = ReactionSet::new(2);
        rs.add_reaction(ArrheniusReaction::new(
            "A -> B",
            1.0e10, 100_000.0, 0.0,
            vec![-1.0, 1.0],
            vec![1.0, 0.0],
        ));
        let model = ReactionModel::finite_rate(rs, vec![0.028, 0.044]);

        let n = 2;
        let mass_fractions = vec![
            ScalarField::new("Y_A", vec![0.5; n]),
            ScalarField::new("Y_B", vec![0.5; n]),
        ];
        // Temperature = 0 should produce no reactions (skipped)
        let temperature = ScalarField::new("T", vec![0.0; n]);
        let density = ScalarField::new("rho", vec![1.2; n]);

        let sources = model.compute_source_terms(&mass_fractions, &temperature, &density).unwrap();
        for sp in 0..2 {
            for cell in 0..n {
                assert_eq!(sources[sp].values()[cell], 0.0,
                    "Source should be zero at T=0 for species {}, cell {}", sp, cell);
            }
        }
    }

    #[test]
    fn test_edm_source_terms_with_turbulence() {
        // 3-species EDM: fuel(0) + oxidizer(1) -> product(2)
        // stoich_mass_ratios: fuel=1.0, oxidizer=4.0, product=-5.0
        // (1 kg fuel + 4 kg oxidizer -> 5 kg product)
        let mw = vec![0.016, 0.032, 0.044];
        let stoich = vec![1.0, 4.0, -5.0]; // positive = reactant consumed, negative = product formed
        let model = ReactionModel::eddy_dissipation(mw, stoich, 0, 1, 4.0);

        let n = 2;
        let mass_fractions = vec![
            ScalarField::new("Y_fuel", vec![0.05; n]),
            ScalarField::new("Y_ox", vec![0.20; n]),
            ScalarField::new("Y_prod", vec![0.75; n]),
        ];
        let density = ScalarField::new("rho", vec![1.2; n]);
        let turb_k = ScalarField::new("k", vec![1.0; n]);
        let turb_eps = ScalarField::new("epsilon", vec![10.0; n]);

        let mut source_terms: Vec<ScalarField> = (0..3)
            .map(|sp| ScalarField::zeros(&format!("omega_{}", sp), n))
            .collect();

        model.compute_edm_source_terms_with_turbulence(
            &mass_fractions, &density, &turb_k, &turb_eps, &mut source_terms,
        );

        // Fuel should be consumed (negative source)
        assert!(source_terms[0].values()[0] < 0.0,
            "Fuel should be consumed, got {}", source_terms[0].values()[0]);
        // Oxidizer should be consumed (negative source)
        assert!(source_terms[1].values()[0] < 0.0,
            "Oxidizer should be consumed, got {}", source_terms[1].values()[0]);
        // Product should be produced (positive source, since stoich is -5.0, -(-5)*rate > 0)
        assert!(source_terms[2].values()[0] > 0.0,
            "Product should be produced, got {}", source_terms[2].values()[0]);

        // Verify the rate calculation:
        // rate = A * rho * eps/k * min(Y_fuel/s_fuel, Y_ox/s_ox)
        //      = 4.0 * 1.2 * 10.0/1.0 * min(0.05/1.0, 0.20/4.0)
        //      = 48.0 * min(0.05, 0.05)
        //      = 48.0 * 0.05 = 2.4
        let expected_rate = 4.0 * 1.2 * 10.0 * f64::min(0.05 / 1.0, 0.20 / 4.0);
        let expected_fuel_source = -1.0 * expected_rate; // -s_fuel * rate
        assert!((source_terms[0].values()[0] - expected_fuel_source).abs() < 1e-10,
            "Fuel source expected {}, got {}", expected_fuel_source, source_terms[0].values()[0]);
    }

    #[test]
    fn test_edm_zero_turbulence_no_reaction() {
        let model = ReactionModel::eddy_dissipation(
            vec![0.016, 0.032, 0.044],
            vec![1.0, 4.0, -5.0],
            0, 1, 4.0,
        );

        let n = 2;
        let mass_fractions = vec![
            ScalarField::new("Y_fuel", vec![0.1; n]),
            ScalarField::new("Y_ox", vec![0.2; n]),
            ScalarField::new("Y_prod", vec![0.7; n]),
        ];
        let density = ScalarField::new("rho", vec![1.2; n]);
        let turb_k = ScalarField::new("k", vec![0.0; n]); // zero k -> no mixing
        let turb_eps = ScalarField::new("epsilon", vec![10.0; n]);

        let mut source_terms: Vec<ScalarField> = (0..3)
            .map(|sp| ScalarField::zeros(&format!("omega_{}", sp), n))
            .collect();

        model.compute_edm_source_terms_with_turbulence(
            &mass_fractions, &density, &turb_k, &turb_eps, &mut source_terms,
        );

        // With k=0 (no turbulence), all sources should remain zero
        for sp in 0..3 {
            for cell in 0..n {
                assert_eq!(source_terms[sp].values()[cell], 0.0,
                    "Source should be zero when k=0 for species {}, cell {}", sp, cell);
            }
        }
    }

    #[test]
    fn test_edm_via_compute_source_terms() {
        // Test the EDM path via the main compute_source_terms method (fallback mixing rate)
        let model = ReactionModel::eddy_dissipation(
            vec![0.016, 0.032, 0.044],
            vec![1.0, 4.0, -5.0],
            0, 1, 4.0,
        );

        let n = 2;
        let mass_fractions = vec![
            ScalarField::new("Y_fuel", vec![0.05; n]),
            ScalarField::new("Y_ox", vec![0.20; n]),
            ScalarField::new("Y_prod", vec![0.75; n]),
        ];
        let temperature = ScalarField::new("T", vec![1500.0; n]);
        let density = ScalarField::new("rho", vec![1.2; n]);

        let sources = model.compute_source_terms(&mass_fractions, &temperature, &density).unwrap();

        // With fallback mixing rate = 1.0:
        // rate = A * rho * 1.0 * min(Y_fuel/s_fuel, Y_ox/s_ox)
        //      = 4.0 * 1.2 * min(0.05, 0.05) = 0.24
        assert!(sources[0].values()[0] < 0.0, "Fuel consumed");
        assert!(sources[1].values()[0] < 0.0, "Oxidizer consumed");
        assert!(sources[2].values()[0] > 0.0, "Product formed");
    }

    #[test]
    fn test_unknown_model_returns_zero() {
        let model = ReactionModel::new("unknown_model");
        let n = 3;
        let mass_fractions = vec![
            ScalarField::new("Y_A", vec![0.5; n]),
            ScalarField::new("Y_B", vec![0.5; n]),
        ];
        let temperature = ScalarField::new("T", vec![1500.0; n]);
        let density = ScalarField::new("rho", vec![1.0; n]);

        let sources = model.compute_source_terms(&mass_fractions, &temperature, &density).unwrap();
        for sp in 0..2 {
            for cell in 0..n {
                assert_eq!(sources[sp].values()[cell], 0.0);
            }
        }
    }
}
