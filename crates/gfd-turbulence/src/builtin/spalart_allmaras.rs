//! Spalart-Allmaras one-equation turbulence model.

use std::collections::HashMap;
use crate::model_template::*;
use super::TurbulenceModel;

/// Spalart-Allmaras one-equation turbulence model.
#[derive(Debug, Clone)]
pub struct SpalartAllmaras {
    /// cb1 constant.
    pub cb1: f64,
    /// cb2 constant.
    pub cb2: f64,
    /// cv1 constant.
    pub cv1: f64,
    /// sigma constant.
    pub sigma: f64,
    /// von Karman constant.
    pub kappa: f64,
    /// cw1 derived constant.
    pub cw1: f64,
    /// cw2 constant.
    pub cw2: f64,
    /// cw3 constant.
    pub cw3: f64,
    /// Cached model definition.
    definition: TurbulenceModelDef,
}

impl Default for SpalartAllmaras {
    fn default() -> Self {
        Self::new()
    }
}

impl SpalartAllmaras {
    /// Creates a new SA model with standard constants.
    pub fn new() -> Self {
        let cb1 = 0.1355;
        let cb2 = 0.622;
        let cv1 = 7.1;
        let sigma = 2.0 / 3.0;
        let kappa = 0.41;
        let cw1 = cb1 / (kappa * kappa) + (1.0 + cb2) / sigma;
        let cw2 = 0.3;
        let cw3 = 2.0;

        let constants = Self::build_constants(cb1, cb2, cv1, sigma, kappa, cw1, cw2, cw3);

        let mut boundary_defaults = HashMap::new();
        boundary_defaults.insert("wall".to_string(), "fixedValue 0".to_string());
        boundary_defaults.insert("inlet".to_string(), "fixedValue 3*nu".to_string());
        boundary_defaults.insert("outlet".to_string(), "zeroGradient".to_string());

        let transport_eq = TransportEquationDef {
            variable_name: "nuTilde".to_string(),
            equation_str: "ddt(nuTilde) + div(U, nuTilde) - (1/sigma) * (laplacian((nu + nuTilde), nuTilde) + cb2 * mag(grad(nuTilde))^2) = cb1 * S_hat * nuTilde - cw1 * fw * (nuTilde / d)^2".to_string(),
            diffusion_coeff: "(nu + nuTilde) / sigma".to_string(),
            production: "cb1 * S_hat * nuTilde".to_string(),
            destruction: "cw1 * fw * (nuTilde / d)^2".to_string(),
            boundary_defaults,
        };

        let definition = TurbulenceModelDef {
            name: "Spalart-Allmaras".to_string(),
            num_equations: 1,
            transport_equations: vec![transport_eq],
            eddy_viscosity: "nuTilde * fv1".to_string(),
            constants: constants.clone(),
            wall_treatment: WallTreatment::StandardWallFunction,
        };

        Self {
            cb1, cb2, cv1, sigma, kappa, cw1, cw2, cw3,
            definition,
        }
    }

    fn build_constants(
        cb1: f64, cb2: f64, cv1: f64, sigma: f64, kappa: f64,
        cw1: f64, cw2: f64, cw3: f64,
    ) -> HashMap<String, ModelConstant> {
        let mut c = HashMap::new();
        c.insert("cb1".into(), ModelConstant { value: cb1, description: "Production coefficient".into(), min: Some(0.0), max: Some(1.0) });
        c.insert("cb2".into(), ModelConstant { value: cb2, description: "Diffusion coefficient".into(), min: Some(0.0), max: Some(2.0) });
        c.insert("cv1".into(), ModelConstant { value: cv1, description: "Viscous damping constant".into(), min: Some(1.0), max: Some(20.0) });
        c.insert("sigma".into(), ModelConstant { value: sigma, description: "Turbulent Prandtl number".into(), min: Some(0.1), max: Some(2.0) });
        c.insert("kappa".into(), ModelConstant { value: kappa, description: "von Karman constant".into(), min: Some(0.3), max: Some(0.5) });
        c.insert("cw1".into(), ModelConstant { value: cw1, description: "Destruction coefficient (derived)".into(), min: None, max: None });
        c.insert("cw2".into(), ModelConstant { value: cw2, description: "Destruction constant".into(), min: Some(0.0), max: Some(1.0) });
        c.insert("cw3".into(), ModelConstant { value: cw3, description: "Destruction constant".into(), min: Some(1.0), max: Some(4.0) });
        c
    }

    /// Computes fv1 = chi^3 / (chi^3 + cv1^3) where chi = nu_tilde / nu.
    pub fn fv1(&self, nu_tilde: f64, nu: f64) -> f64 {
        if nu <= 0.0 {
            return 0.0;
        }
        let chi = nu_tilde / nu;
        let chi3 = chi * chi * chi;
        let cv13 = self.cv1 * self.cv1 * self.cv1;
        chi3 / (chi3 + cv13)
    }
}

impl TurbulenceModel for SpalartAllmaras {
    fn name(&self) -> &str {
        "Spalart-Allmaras"
    }

    fn num_equations(&self) -> usize {
        1
    }

    /// Computes eddy viscosity: nu_t = nu_tilde * fv1.
    ///
    /// - `var1`: nu_tilde (modified turbulent viscosity)
    /// - `var2`: nu (molecular kinematic viscosity)
    /// - `rho`: density (used to return mu_t = rho * nu_t)
    fn compute_eddy_viscosity(&self, nu_tilde: f64, nu: f64, rho: f64) -> f64 {
        let fv1 = self.fv1(nu_tilde, nu);
        rho * nu_tilde * fv1
    }

    fn get_definition(&self) -> &TurbulenceModelDef {
        &self.definition
    }

    fn get_constants(&self) -> &HashMap<String, ModelConstant> {
        &self.definition.constants
    }
}
