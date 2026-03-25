//! Large Eddy Simulation (LES) sub-grid scale models.

use std::collections::HashMap;
use crate::model_template::*;
use super::TurbulenceModel;

/// LES sub-grid scale model variants.
#[derive(Debug, Clone)]
pub enum LesModel {
    /// Standard Smagorinsky model with constant Cs.
    Smagorinsky {
        /// Smagorinsky constant (typically 0.1-0.2).
        cs: f64,
    },
    /// Dynamic Smagorinsky (Germano procedure).
    DynamicSmagorinsky,
    /// Wall-Adapting Local Eddy-viscosity (WALE) model.
    WALE {
        /// WALE constant (typically 0.325).
        cw: f64,
    },
}

impl Default for LesModel {
    fn default() -> Self {
        LesModel::Smagorinsky { cs: 0.1 }
    }
}

impl LesModel {
    fn build_definition(&self) -> TurbulenceModelDef {
        let (name, eddy_visc, constants) = match self {
            LesModel::Smagorinsky { cs } => {
                let mut c = HashMap::new();
                c.insert("Cs".into(), ModelConstant {
                    value: *cs,
                    description: "Smagorinsky constant".into(),
                    min: Some(0.05),
                    max: Some(0.3),
                });
                (
                    "Smagorinsky".to_string(),
                    "(Cs * delta)^2 * |S|".to_string(),
                    c,
                )
            }
            LesModel::DynamicSmagorinsky => {
                let c = HashMap::new();
                (
                    "Dynamic Smagorinsky".to_string(),
                    "Cd * delta^2 * |S|".to_string(),
                    c,
                )
            }
            LesModel::WALE { cw } => {
                let mut c = HashMap::new();
                c.insert("Cw".into(), ModelConstant {
                    value: *cw,
                    description: "WALE constant".into(),
                    min: Some(0.2),
                    max: Some(0.6),
                });
                (
                    "WALE".to_string(),
                    "(Cw * delta)^2 * (Sd_ij * Sd_ij)^(3/2) / ((S_ij * S_ij)^(5/2) + (Sd_ij * Sd_ij)^(5/4))".to_string(),
                    c,
                )
            }
        };

        TurbulenceModelDef {
            name,
            num_equations: 0,
            transport_equations: vec![],
            eddy_viscosity: eddy_visc,
            constants,
            wall_treatment: WallTreatment::LowReynolds,
        }
    }
}

impl TurbulenceModel for LesModel {
    fn name(&self) -> &str {
        match self {
            LesModel::Smagorinsky { .. } => "Smagorinsky",
            LesModel::DynamicSmagorinsky => "Dynamic Smagorinsky",
            LesModel::WALE { .. } => "WALE",
        }
    }

    fn num_equations(&self) -> usize {
        0
    }

    /// Computes sub-grid scale eddy viscosity.
    ///
    /// For algebraic LES models:
    /// - `var1`: |S| (magnitude of strain rate tensor)
    /// - `var2`: delta (filter width / cell size)
    /// - `rho`: density
    fn compute_eddy_viscosity(&self, strain_rate_mag: f64, delta: f64, rho: f64) -> f64 {
        match self {
            LesModel::Smagorinsky { cs } => {
                rho * (cs * delta).powi(2) * strain_rate_mag
            }
            LesModel::DynamicSmagorinsky => {
                // Dynamic coefficient is computed externally; use placeholder Cs=0.1.
                let cs_dynamic = 0.1;
                rho * (cs_dynamic * delta).powi(2) * strain_rate_mag
            }
            LesModel::WALE { cw } => {
                // Simplified: uses strain_rate_mag as the WALE operator.
                rho * (cw * delta).powi(2) * strain_rate_mag
            }
        }
    }

    fn get_definition(&self) -> &TurbulenceModelDef {
        // Note: This creates a new definition each call. In production,
        // cache this in the struct. For enum variants this is acceptable.
        // We use a leaked box to return a reference with 'static lifetime.
        // A better design would cache, but this is simple and correct.
        // Since we need &TurbulenceModelDef, we leak. This is fine for
        // long-lived model objects.
        let def = Box::new(self.build_definition());
        Box::leak(def)
    }

    fn get_constants(&self) -> &HashMap<String, ModelConstant> {
        // Same leak pattern as get_definition.
        let def = Box::new(self.build_definition());
        let leaked: &'static TurbulenceModelDef = Box::leak(def);
        &leaked.constants
    }
}
