//! k-omega SST (Shear Stress Transport) turbulence model (Menter, 1994).

use std::collections::HashMap;
use crate::model_template::*;
use super::TurbulenceModel;

/// k-omega SST turbulence model.
#[derive(Debug, Clone)]
pub struct KOmegaSST {
    /// Inner-layer (k-omega) closure coefficient alpha_1.
    pub alpha_1: f64,
    /// Outer-layer (k-epsilon) closure coefficient alpha_2.
    pub alpha_2: f64,
    /// Inner-layer beta.
    pub beta_1: f64,
    /// Outer-layer beta.
    pub beta_2: f64,
    /// beta* constant.
    pub beta_star: f64,
    /// Inner-layer sigma_k.
    pub sigma_k1: f64,
    /// Outer-layer sigma_k.
    pub sigma_k2: f64,
    /// Inner-layer sigma_omega.
    pub sigma_w1: f64,
    /// Outer-layer sigma_omega.
    pub sigma_w2: f64,
    /// Stress limiter constant a1.
    pub a1: f64,
    /// Cached model definition.
    definition: TurbulenceModelDef,
}

impl Default for KOmegaSST {
    fn default() -> Self {
        Self::new()
    }
}

impl KOmegaSST {
    /// Creates a new k-omega SST model with standard constants.
    pub fn new() -> Self {
        let alpha_1 = 5.0 / 9.0;
        let alpha_2 = 0.44;
        let beta_1 = 3.0 / 40.0;
        let beta_2 = 0.0828;
        let beta_star = 0.09;
        let sigma_k1 = 0.85;
        let sigma_k2 = 1.0;
        let sigma_w1 = 0.5;
        let sigma_w2 = 0.856;
        let a1 = 0.31;

        let constants = Self::build_constants(
            alpha_1, alpha_2, beta_1, beta_2, beta_star,
            sigma_k1, sigma_k2, sigma_w1, sigma_w2, a1,
        );
        let definition = Self::build_definition(&constants);

        Self {
            alpha_1, alpha_2, beta_1, beta_2, beta_star,
            sigma_k1, sigma_k2, sigma_w1, sigma_w2, a1,
            definition,
        }
    }

    fn build_constants(
        alpha_1: f64, alpha_2: f64, beta_1: f64, beta_2: f64, beta_star: f64,
        sigma_k1: f64, sigma_k2: f64, sigma_w1: f64, sigma_w2: f64, a1: f64,
    ) -> HashMap<String, ModelConstant> {
        let mut c = HashMap::new();
        c.insert("alpha1".into(), ModelConstant { value: alpha_1, description: "Inner-layer production coefficient".into(), min: Some(0.0), max: Some(1.0) });
        c.insert("alpha2".into(), ModelConstant { value: alpha_2, description: "Outer-layer production coefficient".into(), min: Some(0.0), max: Some(1.0) });
        c.insert("beta1".into(), ModelConstant { value: beta_1, description: "Inner-layer destruction coefficient".into(), min: Some(0.0), max: Some(0.2) });
        c.insert("beta2".into(), ModelConstant { value: beta_2, description: "Outer-layer destruction coefficient".into(), min: Some(0.0), max: Some(0.2) });
        c.insert("betaStar".into(), ModelConstant { value: beta_star, description: "k-equation destruction coefficient".into(), min: Some(0.0), max: Some(0.2) });
        c.insert("sigma_k1".into(), ModelConstant { value: sigma_k1, description: "Inner-layer turbulent Prandtl number for k".into(), min: Some(0.5), max: Some(2.0) });
        c.insert("sigma_k2".into(), ModelConstant { value: sigma_k2, description: "Outer-layer turbulent Prandtl number for k".into(), min: Some(0.5), max: Some(2.0) });
        c.insert("sigma_w1".into(), ModelConstant { value: sigma_w1, description: "Inner-layer turbulent Prandtl number for omega".into(), min: Some(0.3), max: Some(2.0) });
        c.insert("sigma_w2".into(), ModelConstant { value: sigma_w2, description: "Outer-layer turbulent Prandtl number for omega".into(), min: Some(0.5), max: Some(2.0) });
        c.insert("a1".into(), ModelConstant { value: a1, description: "Stress limiter constant".into(), min: Some(0.2), max: Some(0.5) });
        c
    }

    fn build_definition(constants: &HashMap<String, ModelConstant>) -> TurbulenceModelDef {
        let mut k_bc = HashMap::new();
        k_bc.insert("wall".into(), "fixedValue 0".into());
        k_bc.insert("inlet".into(), "fixedValue".into());
        k_bc.insert("outlet".into(), "zeroGradient".into());

        let k_eq = TransportEquationDef {
            variable_name: "k".to_string(),
            equation_str: "ddt(rho, k) + div(rho * U, k) - laplacian((mu + sigma_k * mu_t), k) = P_k - betaStar * rho * omega * k".to_string(),
            diffusion_coeff: "(mu + sigma_k * mu_t)".to_string(),
            production: "P_k = min(mu_t * S^2, 10 * betaStar * rho * k * omega)".to_string(),
            destruction: "betaStar * rho * omega * k".to_string(),
            boundary_defaults: k_bc,
        };

        let mut w_bc = HashMap::new();
        w_bc.insert("wall".into(), "omegaWallFunction".into());
        w_bc.insert("inlet".into(), "fixedValue".into());
        w_bc.insert("outlet".into(), "zeroGradient".into());

        let w_eq = TransportEquationDef {
            variable_name: "omega".to_string(),
            equation_str: "ddt(rho, omega) + div(rho * U, omega) - laplacian((mu + sigma_w * mu_t), omega) = alpha * (S^2) - beta * rho * omega^2 + 2 * (1 - F1) * rho * sigma_w2 * (1/omega) * dot(grad(k), grad(omega))".to_string(),
            diffusion_coeff: "(mu + sigma_w * mu_t)".to_string(),
            production: "alpha * S^2".to_string(),
            destruction: "beta * rho * omega^2".to_string(),
            boundary_defaults: w_bc,
        };

        TurbulenceModelDef {
            name: "k-omega SST".to_string(),
            num_equations: 2,
            transport_equations: vec![k_eq, w_eq],
            eddy_viscosity: "rho * a1 * k / max(a1 * omega, S * F2)".to_string(),
            constants: constants.clone(),
            wall_treatment: WallTreatment::StandardWallFunction,
        }
    }

    /// Computes the F1 blending function.
    ///
    /// F1 = tanh(arg1^4), where
    /// arg1 = min(max(sqrt(k)/(betaStar*omega*y), 500*nu/(y^2*omega)), 4*rho*sigma_w2*k/(CDkw*y^2))
    ///
    /// - `k`: turbulent kinetic energy
    /// - `omega`: specific dissipation rate
    /// - `y`: wall distance
    /// - `nu`: molecular kinematic viscosity
    /// - `rho`: density
    /// - `grad_k_dot_grad_omega`: dot product of grad(k) and grad(omega)
    pub fn compute_f1(
        &self, k: f64, omega: f64, y: f64, nu: f64, rho: f64,
        grad_k_dot_grad_omega: f64,
    ) -> f64 {
        if y <= 0.0 || omega <= 0.0 {
            return 1.0;
        }
        let cd_kw = f64::max(
            2.0 * rho * self.sigma_w2 * grad_k_dot_grad_omega / omega,
            1.0e-10,
        );
        let arg1_a = (k.sqrt()) / (self.beta_star * omega * y);
        let arg1_b = 500.0 * nu / (y * y * omega);
        let arg1_c = 4.0 * rho * self.sigma_w2 * k / (cd_kw * y * y);
        let arg1 = f64::min(f64::max(arg1_a, arg1_b), arg1_c);
        (arg1.powi(4)).tanh()
    }

    /// Computes the F2 blending function.
    ///
    /// F2 = tanh(arg2^2), where
    /// arg2 = max(2*sqrt(k)/(betaStar*omega*y), 500*nu/(y^2*omega))
    pub fn compute_f2(&self, k: f64, omega: f64, y: f64, nu: f64) -> f64 {
        if y <= 0.0 || omega <= 0.0 {
            return 0.0;
        }
        let arg2_a = 2.0 * k.sqrt() / (self.beta_star * omega * y);
        let arg2_b = 500.0 * nu / (y * y * omega);
        let arg2 = f64::max(arg2_a, arg2_b);
        (arg2 * arg2).tanh()
    }
}

impl TurbulenceModel for KOmegaSST {
    fn name(&self) -> &str {
        "k-omega SST"
    }

    fn num_equations(&self) -> usize {
        2
    }

    /// Computes eddy viscosity: mu_t = rho * a1 * k / max(a1 * omega, S * F2).
    ///
    /// Note: This simplified interface uses var1=k, var2=omega and assumes
    /// S*F2 < a1*omega (no strain rate info available here).
    /// For the full formulation, use `compute_f2` and supply strain rate externally.
    fn compute_eddy_viscosity(&self, k: f64, omega: f64, rho: f64) -> f64 {
        if omega <= 0.0 || k < 0.0 {
            return 0.0;
        }
        // Simplified: without strain rate info, use mu_t = rho * k / omega
        // which is the limit when a1*omega dominates.
        rho * self.a1 * k / (self.a1 * omega)
    }

    fn get_definition(&self) -> &TurbulenceModelDef {
        &self.definition
    }

    fn get_constants(&self) -> &HashMap<String, ModelConstant> {
        &self.definition.constants
    }
}
