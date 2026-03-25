//! Reynolds Stress Model (RSM) — full second-moment closure.
//!
//! Solves transport equations for all six independent components of the
//! Reynolds stress tensor (R_ij) plus the dissipation rate epsilon,
//! yielding a 7-equation model.

use std::collections::HashMap;
use crate::model_template::*;
use super::TurbulenceModel;

/// Reynolds Stress Model (RSM).
///
/// A full second-moment closure that solves individual transport equations for
/// each component of the Reynolds stress tensor instead of relying on the
/// Boussinesq eddy-viscosity hypothesis.
#[derive(Debug, Clone)]
pub struct ReynoldsStressModel {
    /// Slow pressure-strain constant (C1, Rotta model).
    pub c1: f64,
    /// Rapid pressure-strain constant (C2, isotropisation of production).
    pub c2: f64,
    /// Wall-reflection slow pressure-strain constant.
    pub c1_ps: f64,
    /// Wall-reflection rapid pressure-strain constant.
    pub c2_ps: f64,
    /// Turbulent Prandtl number for k (diffusion of R_ij).
    pub sigma_k: f64,
    /// Turbulent Prandtl number for epsilon.
    pub sigma_epsilon: f64,
    /// Eddy-viscosity coefficient (used for approximating diffusion).
    pub c_mu: f64,
    /// Cached model definition.
    definition: TurbulenceModelDef,
}

impl Default for ReynoldsStressModel {
    fn default() -> Self {
        Self::new()
    }
}

impl ReynoldsStressModel {
    /// Creates a standard RSM with default LRR constants.
    pub fn new() -> Self {
        let c1 = 1.8;
        let c2 = 0.6;
        let c1_ps = 0.5;
        let c2_ps = 0.3;
        let sigma_k = 1.0;
        let sigma_epsilon = 1.3;
        let c_mu = 0.09;

        let constants = Self::build_constants(c1, c2, c1_ps, c2_ps, sigma_k, sigma_epsilon, c_mu);
        let definition = Self::build_definition(&constants);

        Self {
            c1,
            c2,
            c1_ps,
            c2_ps,
            sigma_k,
            sigma_epsilon,
            c_mu,
            definition,
        }
    }

    fn build_constants(
        c1: f64,
        c2: f64,
        c1_ps: f64,
        c2_ps: f64,
        sigma_k: f64,
        sigma_epsilon: f64,
        c_mu: f64,
    ) -> HashMap<String, ModelConstant> {
        let mut c = HashMap::new();
        c.insert("C1".into(), ModelConstant {
            value: c1,
            description: "Slow pressure-strain constant (Rotta)".into(),
            min: Some(1.0),
            max: Some(3.0),
        });
        c.insert("C2".into(), ModelConstant {
            value: c2,
            description: "Rapid pressure-strain constant (isotropisation of production)".into(),
            min: Some(0.0),
            max: Some(1.0),
        });
        c.insert("C1_ps".into(), ModelConstant {
            value: c1_ps,
            description: "Wall-reflection slow pressure-strain constant".into(),
            min: Some(0.0),
            max: Some(1.0),
        });
        c.insert("C2_ps".into(), ModelConstant {
            value: c2_ps,
            description: "Wall-reflection rapid pressure-strain constant".into(),
            min: Some(0.0),
            max: Some(1.0),
        });
        c.insert("sigma_k".into(), ModelConstant {
            value: sigma_k,
            description: "Turbulent Prandtl number for Reynolds stress diffusion".into(),
            min: Some(0.5),
            max: Some(2.0),
        });
        c.insert("sigma_epsilon".into(), ModelConstant {
            value: sigma_epsilon,
            description: "Turbulent Prandtl number for epsilon".into(),
            min: Some(0.5),
            max: Some(2.0),
        });
        c.insert("Cmu".into(), ModelConstant {
            value: c_mu,
            description: "Eddy viscosity coefficient (used in diffusion approximation)".into(),
            min: Some(0.0),
            max: Some(0.2),
        });
        c
    }

    fn build_definition(constants: &HashMap<String, ModelConstant>) -> TurbulenceModelDef {
        // Six Reynolds stress components: R_xx, R_yy, R_zz, R_xy, R_xz, R_yz
        let stress_components = ["R_xx", "R_yy", "R_zz", "R_xy", "R_xz", "R_yz"];
        let mut transport_equations = Vec::with_capacity(7);

        for comp in &stress_components {
            let mut bc = HashMap::new();
            bc.insert("wall".into(), "fixedValue 0".into());
            bc.insert("inlet".into(), "fixedValue".into());
            bc.insert("outlet".into(), "zeroGradient".into());

            transport_equations.push(TransportEquationDef {
                variable_name: comp.to_string(),
                equation_str: format!(
                    "ddt(rho, {c}) + div(rho * U, {c}) - laplacian((mu + mu_t / sigma_k), {c}) = P_{c} + phi_{c} - epsilon_{c}",
                    c = comp
                ),
                diffusion_coeff: "(mu + mu_t / sigma_k)".to_string(),
                production: format!("P_{}", comp),
                destruction: format!("epsilon_{}", comp),
                boundary_defaults: bc,
            });
        }

        // Epsilon equation
        let mut e_bc = HashMap::new();
        e_bc.insert("wall".into(), "epsilonWallFunction".into());
        e_bc.insert("inlet".into(), "fixedValue".into());
        e_bc.insert("outlet".into(), "zeroGradient".into());

        transport_equations.push(TransportEquationDef {
            variable_name: "epsilon".to_string(),
            equation_str: "ddt(rho, epsilon) + div(rho * U, epsilon) - laplacian((mu + mu_t / sigma_epsilon), epsilon) = C1 * (epsilon / k) * 0.5 * P_kk - C2 * rho * epsilon^2 / k".to_string(),
            diffusion_coeff: "(mu + mu_t / sigma_epsilon)".to_string(),
            production: "C1 * (epsilon / k) * 0.5 * P_kk".to_string(),
            destruction: "C2 * rho * epsilon^2 / k".to_string(),
            boundary_defaults: e_bc,
        });

        TurbulenceModelDef {
            name: "Reynolds Stress Model (LRR)".to_string(),
            num_equations: 7,
            transport_equations,
            eddy_viscosity: "rho * Cmu * k^2 / epsilon".to_string(),
            constants: constants.clone(),
            wall_treatment: WallTreatment::StandardWallFunction,
        }
    }
}

impl TurbulenceModel for ReynoldsStressModel {
    fn name(&self) -> &str {
        &self.definition.name
    }

    fn num_equations(&self) -> usize {
        7
    }

    /// Computes the eddy viscosity: mu_t = rho * C_mu * k^2 / epsilon.
    ///
    /// In an RSM the eddy viscosity is only used for approximating the
    /// diffusion term; the Reynolds stresses themselves are solved directly.
    ///
    /// - `var1`: k (turbulent kinetic energy, 0.5 * trace(R_ij))
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rsm_defaults() {
        let rsm = ReynoldsStressModel::new();
        assert_eq!(rsm.num_equations(), 7);
        assert_eq!(rsm.name(), "Reynolds Stress Model (LRR)");
    }

    #[test]
    fn test_rsm_eddy_viscosity() {
        let rsm = ReynoldsStressModel::new();
        let mu_t = rsm.compute_eddy_viscosity(1.0, 1.0, 1.0);
        assert!((mu_t - 0.09).abs() < 1e-10);
    }

    #[test]
    fn test_rsm_zero_epsilon() {
        let rsm = ReynoldsStressModel::new();
        assert_eq!(rsm.compute_eddy_viscosity(1.0, 0.0, 1.0), 0.0);
    }

    #[test]
    fn test_rsm_constants_count() {
        let rsm = ReynoldsStressModel::new();
        assert_eq!(rsm.get_constants().len(), 7);
    }

    #[test]
    fn test_rsm_definition_equations() {
        let rsm = ReynoldsStressModel::new();
        let def = rsm.get_definition();
        assert_eq!(def.transport_equations.len(), 7);
        // First six are Reynolds stress components, last is epsilon
        assert_eq!(def.transport_equations[6].variable_name, "epsilon");
    }
}
