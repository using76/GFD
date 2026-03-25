//! k-epsilon two-equation turbulence model family.

use std::collections::HashMap;
use crate::model_template::*;
use super::TurbulenceModel;

/// Variant of the k-epsilon model.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KEpsilonVariant {
    /// Standard k-epsilon (Launder & Spalding, 1974).
    Standard,
    /// RNG k-epsilon (Yakhot & Orszag, 1986).
    RNG,
    /// Realizable k-epsilon (Shih et al., 1995).
    Realizable,
}

/// k-epsilon two-equation turbulence model.
#[derive(Debug, Clone)]
pub struct KEpsilon {
    /// Model variant.
    pub variant: KEpsilonVariant,
    /// C_mu constant.
    pub c_mu: f64,
    /// C_1epsilon constant.
    pub c_1e: f64,
    /// C_2epsilon constant.
    pub c_2e: f64,
    /// Turbulent Prandtl number for k.
    pub sigma_k: f64,
    /// Turbulent Prandtl number for epsilon.
    pub sigma_e: f64,
    /// Cached model definition.
    definition: TurbulenceModelDef,
}

impl Default for KEpsilon {
    fn default() -> Self {
        Self::standard()
    }
}

impl KEpsilon {
    /// Creates the standard k-epsilon model.
    pub fn standard() -> Self {
        Self::with_variant(KEpsilonVariant::Standard)
    }

    /// Creates the RNG k-epsilon model.
    pub fn rng() -> Self {
        Self::with_variant(KEpsilonVariant::RNG)
    }

    /// Creates the Realizable k-epsilon model.
    pub fn realizable() -> Self {
        Self::with_variant(KEpsilonVariant::Realizable)
    }

    /// Creates a k-epsilon model of the given variant.
    pub fn with_variant(variant: KEpsilonVariant) -> Self {
        let (c_mu, c_1e, c_2e, sigma_k, sigma_e) = match variant {
            KEpsilonVariant::Standard   => (0.09, 1.44, 1.92, 1.0, 1.3),
            KEpsilonVariant::RNG        => (0.0845, 1.42, 1.68, 0.7194, 0.7194),
            KEpsilonVariant::Realizable => (0.09, 1.44, 1.9, 1.0, 1.2),
        };

        let constants = Self::build_constants(c_mu, c_1e, c_2e, sigma_k, sigma_e);
        let definition = Self::build_definition(variant, &constants, c_mu);

        Self {
            variant,
            c_mu,
            c_1e,
            c_2e,
            sigma_k,
            sigma_e,
            definition,
        }
    }

    fn build_constants(
        c_mu: f64, c_1e: f64, c_2e: f64, sigma_k: f64, sigma_e: f64,
    ) -> HashMap<String, ModelConstant> {
        let mut c = HashMap::new();
        c.insert("Cmu".into(), ModelConstant { value: c_mu, description: "Eddy viscosity coefficient".into(), min: Some(0.0), max: Some(0.2) });
        c.insert("C1e".into(), ModelConstant { value: c_1e, description: "Epsilon production coefficient".into(), min: Some(1.0), max: Some(2.0) });
        c.insert("C2e".into(), ModelConstant { value: c_2e, description: "Epsilon destruction coefficient".into(), min: Some(1.0), max: Some(3.0) });
        c.insert("sigma_k".into(), ModelConstant { value: sigma_k, description: "Turbulent Prandtl number for k".into(), min: Some(0.5), max: Some(2.0) });
        c.insert("sigma_e".into(), ModelConstant { value: sigma_e, description: "Turbulent Prandtl number for epsilon".into(), min: Some(0.5), max: Some(2.0) });
        c
    }

    fn build_definition(
        variant: KEpsilonVariant,
        constants: &HashMap<String, ModelConstant>,
        _c_mu: f64,
    ) -> TurbulenceModelDef {
        let variant_name = match variant {
            KEpsilonVariant::Standard => "Standard k-epsilon",
            KEpsilonVariant::RNG => "RNG k-epsilon",
            KEpsilonVariant::Realizable => "Realizable k-epsilon",
        };

        let mut k_bc = HashMap::new();
        k_bc.insert("wall".into(), "fixedValue 0".into());
        k_bc.insert("inlet".into(), "fixedValue".into());
        k_bc.insert("outlet".into(), "zeroGradient".into());

        let k_eq = TransportEquationDef {
            variable_name: "k".to_string(),
            equation_str: "ddt(rho, k) + div(rho * U, k) - laplacian((mu + mu_t / sigma_k), k) = G_k - rho * epsilon".to_string(),
            diffusion_coeff: "(mu + mu_t / sigma_k)".to_string(),
            production: "G_k".to_string(),
            destruction: "rho * epsilon".to_string(),
            boundary_defaults: k_bc,
        };

        let mut e_bc = HashMap::new();
        e_bc.insert("wall".into(), "epsilonWallFunction".into());
        e_bc.insert("inlet".into(), "fixedValue".into());
        e_bc.insert("outlet".into(), "zeroGradient".into());

        let e_eq = TransportEquationDef {
            variable_name: "epsilon".to_string(),
            equation_str: "ddt(rho, epsilon) + div(rho * U, epsilon) - laplacian((mu + mu_t / sigma_e), epsilon) = C1e * (epsilon / k) * G_k - C2e * rho * epsilon^2 / k".to_string(),
            diffusion_coeff: "(mu + mu_t / sigma_e)".to_string(),
            production: "C1e * (epsilon / k) * G_k".to_string(),
            destruction: "C2e * rho * epsilon^2 / k".to_string(),
            boundary_defaults: e_bc,
        };

        TurbulenceModelDef {
            name: variant_name.to_string(),
            num_equations: 2,
            transport_equations: vec![k_eq, e_eq],
            eddy_viscosity: "rho * Cmu * k^2 / epsilon".to_string(),
            constants: constants.clone(),
            wall_treatment: WallTreatment::StandardWallFunction,
        }
    }
}

impl TurbulenceModel for KEpsilon {
    fn name(&self) -> &str {
        &self.definition.name
    }

    fn num_equations(&self) -> usize {
        2
    }

    /// Computes eddy viscosity: mu_t = rho * C_mu * k^2 / epsilon.
    ///
    /// - `var1`: k (turbulent kinetic energy)
    /// - `var2`: epsilon (turbulent dissipation rate)
    /// - `rho`: density
    fn compute_eddy_viscosity(&self, k: f64, epsilon: f64, rho: f64) -> f64 {
        if epsilon <= 0.0 || k < 0.0 {
            return 0.0;
        }
        rho * self.c_mu * k * k / epsilon
    }

    fn get_definition(&self) -> &TurbulenceModelDef {
        &self.definition
    }

    fn get_constants(&self) -> &HashMap<String, ModelConstant> {
        &self.definition.constants
    }
}
