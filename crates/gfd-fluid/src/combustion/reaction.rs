//! Chemical reaction models for combustion.
//!
//! Provides Arrhenius kinetics and reaction set management.

/// Universal gas constant [J/(mol*K)].
const R_UNIVERSAL: f64 = 8.314_462_618_153_24;

/// A single chemical reaction described by the Arrhenius rate law.
///
/// The rate constant is: k = A * T^beta * exp(-E_a / (R * T))
///
/// where A is the pre-exponential factor, beta is the temperature
/// exponent, E_a is the activation energy, and R is the universal
/// gas constant.
#[derive(Debug, Clone)]
pub struct ArrheniusReaction {
    /// Reaction name or identifier.
    pub name: String,
    /// Pre-exponential factor A [units depend on reaction order].
    pub pre_exponential: f64,
    /// Activation energy E_a [J/mol].
    pub activation_energy: f64,
    /// Temperature exponent beta [-].
    pub temperature_exponent: f64,
    /// Stoichiometric coefficients for each species.
    /// Negative values for reactants, positive for products.
    pub stoich_coefficients: Vec<f64>,
    /// Concentration exponents for each species in the rate law.
    pub concentration_exponents: Vec<f64>,
}

impl ArrheniusReaction {
    /// Creates a new Arrhenius reaction.
    pub fn new(
        name: impl Into<String>,
        pre_exponential: f64,
        activation_energy: f64,
        temperature_exponent: f64,
        stoich_coefficients: Vec<f64>,
        concentration_exponents: Vec<f64>,
    ) -> Self {
        Self {
            name: name.into(),
            pre_exponential,
            activation_energy,
            temperature_exponent,
            stoich_coefficients,
            concentration_exponents,
        }
    }

    /// Computes the Arrhenius reaction rate constant at the given temperature.
    ///
    /// k(T) = A * T^beta * exp(-E_a / (R * T))
    ///
    /// # Arguments
    /// * `temperature` - Temperature in Kelvin [K].
    ///
    /// # Returns
    /// The rate constant k with units depending on the reaction order.
    pub fn rate_constant(&self, temperature: f64) -> f64 {
        assert!(temperature > 0.0, "Temperature must be positive");
        self.pre_exponential
            * temperature.powf(self.temperature_exponent)
            * (-self.activation_energy / (R_UNIVERSAL * temperature)).exp()
    }

    /// Computes the reaction rate at the given temperature and species concentrations.
    ///
    /// omega = k(T) * product(c_i^n_i)
    ///
    /// where c_i are the species concentrations [mol/m^3] and n_i are the
    /// concentration exponents (reaction orders with respect to each species).
    ///
    /// # Arguments
    /// * `temperature` - Temperature in Kelvin [K].
    /// * `concentrations` - Molar concentrations of each species [mol/m^3].
    ///
    /// # Returns
    /// The volumetric reaction rate [mol/(m^3*s)].
    pub fn compute_reaction_rate(&self, temperature: f64, concentrations: &[f64]) -> f64 {
        assert_eq!(
            concentrations.len(),
            self.concentration_exponents.len(),
            "Number of concentrations must match number of species"
        );

        let k = self.rate_constant(temperature);

        let concentration_product: f64 = concentrations
            .iter()
            .zip(self.concentration_exponents.iter())
            .map(|(&c, &n)| {
                if n == 0.0 {
                    1.0
                } else {
                    c.max(0.0).powf(n)
                }
            })
            .product();

        k * concentration_product
    }
}

/// A set of chemical reactions.
///
/// Manages multiple reactions and computes the net species source
/// terms by summing contributions from all reactions.
#[derive(Debug, Clone)]
pub struct ReactionSet {
    /// Collection of reactions.
    pub reactions: Vec<ArrheniusReaction>,
    /// Number of species involved.
    pub num_species: usize,
}

impl ReactionSet {
    /// Creates a new empty reaction set for the given number of species.
    pub fn new(num_species: usize) -> Self {
        Self {
            reactions: Vec::new(),
            num_species,
        }
    }

    /// Adds a reaction to the set.
    pub fn add_reaction(&mut self, reaction: ArrheniusReaction) {
        assert_eq!(
            reaction.stoich_coefficients.len(),
            self.num_species,
            "Stoichiometric coefficients must have length equal to num_species"
        );
        self.reactions.push(reaction);
    }

    /// Computes the net source term for each species from all reactions.
    ///
    /// omega_i = sum_r(nu_ir * omega_r)
    ///
    /// where nu_ir is the stoichiometric coefficient of species i in
    /// reaction r, and omega_r is the rate of reaction r.
    ///
    /// # Returns
    /// A vector of net production rates for each species [mol/(m^3*s)].
    pub fn compute_net_rates(
        &self,
        temperature: f64,
        concentrations: &[f64],
    ) -> Vec<f64> {
        let mut net_rates = vec![0.0; self.num_species];

        for reaction in &self.reactions {
            let rate = reaction.compute_reaction_rate(temperature, concentrations);
            for (i, &nu) in reaction.stoich_coefficients.iter().enumerate() {
                net_rates[i] += nu * rate;
            }
        }

        net_rates
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_arrhenius_rate_constant() {
        // H2 + O2 -> H2O type reaction
        let reaction = ArrheniusReaction::new(
            "test",
            1.0e10,        // A
            80_000.0,      // E_a in J/mol
            0.0,           // beta
            vec![-1.0, -0.5, 1.0],
            vec![1.0, 0.5, 0.0],
        );

        let k_1000 = reaction.rate_constant(1000.0);
        let k_2000 = reaction.rate_constant(2000.0);

        // Rate should increase with temperature
        assert!(k_2000 > k_1000, "Rate constant should increase with temperature");

        // Check the formula: k = A * exp(-Ea/(R*T))
        let expected = 1.0e10 * (-80_000.0 / (R_UNIVERSAL * 1000.0)).exp();
        assert!(
            (k_1000 - expected).abs() / expected < 1e-10,
            "Rate constant should match Arrhenius formula"
        );
    }

    #[test]
    fn test_reaction_rate_with_concentrations() {
        let reaction = ArrheniusReaction::new(
            "A + B -> C",
            1.0e6,
            50_000.0,
            0.0,
            vec![-1.0, -1.0, 1.0],
            vec![1.0, 1.0, 0.0],
        );

        let t = 1500.0;
        let conc = vec![0.1, 0.2, 0.05]; // mol/m^3
        let rate = reaction.compute_reaction_rate(t, &conc);

        let k = reaction.rate_constant(t);
        let expected = k * 0.1 * 0.2; // first order in both A and B
        assert!(
            (rate - expected).abs() / expected < 1e-10,
            "Reaction rate should equal k * [A] * [B]"
        );
    }

    #[test]
    fn test_reaction_set_net_rates() {
        let mut set = ReactionSet::new(3);

        // A -> B (forward)
        set.add_reaction(ArrheniusReaction::new(
            "forward",
            1.0e8,
            40_000.0,
            0.0,
            vec![-1.0, 1.0, 0.0],
            vec![1.0, 0.0, 0.0],
        ));

        let rates = set.compute_net_rates(1000.0, &[1.0, 0.0, 0.0]);
        assert!(rates[0] < 0.0, "Reactant A should be consumed");
        assert!(rates[1] > 0.0, "Product B should be produced");
        assert_eq!(rates[2], 0.0, "Species C should be unchanged");
    }
}
