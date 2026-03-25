//! Custom turbulence models loaded from JSON.
//!
//! The eddy viscosity expression from the model definition is parsed via
//! `gfd_expression` and evaluated numerically at runtime. Model constants
//! are resolved by name, and the special variables `var1`, `var2`, and `rho`
//! are mapped to the arguments of [`TurbulenceModel::compute_eddy_viscosity`].

use std::collections::HashMap;
use crate::model_template::{TurbulenceModelDef, ModelConstant};
use crate::builtin::TurbulenceModel;
use crate::{TurbulenceError, Result};

use gfd_expression::ast::{Expr, BinOp, UnOp};

// ---------------------------------------------------------------------------
// AST evaluator for scalar expressions
// ---------------------------------------------------------------------------

/// Variable bindings for expression evaluation.
struct EvalContext<'a> {
    /// Model constants keyed by name (e.g. "Cmu", "C1e").
    constants: &'a HashMap<String, ModelConstant>,
    /// The first turbulence variable (e.g. k, nu_tilde, |S|).
    var1: f64,
    /// The second turbulence variable (e.g. epsilon, omega, delta).
    var2: f64,
    /// Density.
    rho: f64,
}

/// Evaluates an `Expr` AST node numerically in the given context.
///
/// Supported node types:
/// - `Number`, `Constant` (pi, e)
/// - `Variable` / `FieldRef` — resolved against context
/// - `BinaryOp` (+, -, *, /, ^)
/// - `UnaryOp` (neg, abs, sqrt, sin, cos, exp, log)
/// - `FunctionCall` for `max`, `min`, `pow`
///
/// Differential and tensor operators are **not** evaluable at the scalar
/// level and return `Err`.
fn eval_expr(expr: &Expr, ctx: &EvalContext<'_>) -> std::result::Result<f64, String> {
    match expr {
        Expr::Number(v) => Ok(*v),

        Expr::Constant(name) => match name.as_str() {
            "pi" => Ok(std::f64::consts::PI),
            "e" => Ok(std::f64::consts::E),
            _ => Err(format!("unknown constant: {}", name)),
        },

        Expr::Variable(name) | Expr::FieldRef(name) => resolve_name(name, ctx),

        Expr::BinaryOp { op, left, right } => {
            let l = eval_expr(left, ctx)?;
            let r = eval_expr(right, ctx)?;
            match op {
                BinOp::Add => Ok(l + r),
                BinOp::Sub => Ok(l - r),
                BinOp::Mul => Ok(l * r),
                BinOp::Div => {
                    if r.abs() < 1e-30 {
                        Ok(0.0) // safe zero for division by near-zero
                    } else {
                        Ok(l / r)
                    }
                }
                BinOp::Pow => Ok(l.powf(r)),
            }
        }

        Expr::UnaryOp { op, operand } => {
            let v = eval_expr(operand, ctx)?;
            match op {
                UnOp::Neg => Ok(-v),
                UnOp::Abs => Ok(v.abs()),
                UnOp::Sqrt => Ok(v.max(0.0).sqrt()),
                UnOp::Sin => Ok(v.sin()),
                UnOp::Cos => Ok(v.cos()),
                UnOp::Exp => Ok(v.exp()),
                UnOp::Log => {
                    if v <= 0.0 {
                        Ok(f64::NEG_INFINITY)
                    } else {
                        Ok(v.ln())
                    }
                }
            }
        }

        Expr::FunctionCall { name, args } => {
            match name.as_str() {
                "max" if args.len() == 2 => {
                    let a = eval_expr(&args[0], ctx)?;
                    let b = eval_expr(&args[1], ctx)?;
                    Ok(a.max(b))
                }
                "min" if args.len() == 2 => {
                    let a = eval_expr(&args[0], ctx)?;
                    let b = eval_expr(&args[1], ctx)?;
                    Ok(a.min(b))
                }
                "pow" if args.len() == 2 => {
                    let base = eval_expr(&args[0], ctx)?;
                    let exp = eval_expr(&args[1], ctx)?;
                    Ok(base.powf(exp))
                }
                _ => Err(format!(
                    "unsupported function call: {}({})",
                    name,
                    args.len()
                )),
            }
        }

        Expr::Conditional { condition, true_val, false_val } => {
            let cond = eval_expr(condition, ctx)?;
            if cond > 0.0 {
                eval_expr(true_val, ctx)
            } else {
                eval_expr(false_val, ctx)
            }
        }

        Expr::DiffOp { .. } => Err("differential operators cannot be evaluated as scalars".into()),
        Expr::TensorOp { .. } => Err("tensor operators cannot be evaluated as scalars".into()),
    }
}

/// Resolves a variable name against the evaluation context.
///
/// Resolution order:
/// 1. Well-known turbulence variables: `var1`, `k`, `nu_tilde`, `var2`,
///    `epsilon`, `omega`, `delta`, `rho`.
/// 2. Model constants by exact name.
/// 3. Error if not found.
fn resolve_name(name: &str, ctx: &EvalContext<'_>) -> std::result::Result<f64, String> {
    // Well-known names
    match name {
        "var1" | "k" | "nu_tilde" => return Ok(ctx.var1),
        "var2" | "epsilon" | "omega" | "delta" => return Ok(ctx.var2),
        "rho" => return Ok(ctx.rho),
        _ => {}
    }
    // Model constants
    if let Some(mc) = ctx.constants.get(name) {
        return Ok(mc.value);
    }
    Err(format!("unresolved variable: '{}'", name))
}

// ---------------------------------------------------------------------------
// CustomTurbulenceModel
// ---------------------------------------------------------------------------

/// A user-defined turbulence model loaded from a JSON definition.
///
/// The eddy viscosity expression is parsed once at construction time and
/// evaluated numerically each time [`TurbulenceModel::compute_eddy_viscosity`]
/// is called.
#[derive(Debug, Clone)]
pub struct CustomTurbulenceModel {
    /// The underlying model definition.
    definition: TurbulenceModelDef,
    /// Pre-parsed eddy viscosity expression AST (None if parsing failed).
    eddy_viscosity_ast: Option<Expr>,
}

impl CustomTurbulenceModel {
    /// Creates a custom model from an existing definition.
    ///
    /// The eddy viscosity expression is parsed eagerly. If parsing fails,
    /// `compute_eddy_viscosity` will fall back to a safe zero.
    pub fn from_definition(definition: TurbulenceModelDef) -> Self {
        let ast = gfd_expression::parse(&definition.eddy_viscosity).ok();
        Self {
            definition,
            eddy_viscosity_ast: ast,
        }
    }
}

/// Loads a custom turbulence model from a JSON string.
///
/// The JSON should deserialize into a `TurbulenceModelDef`.
///
/// # Errors
///
/// Returns `TurbulenceError::JsonError` if deserialization fails,
/// or `TurbulenceError::CustomModelError` if the definition is empty.
pub fn load_custom_model(json: &str) -> Result<CustomTurbulenceModel> {
    let definition: TurbulenceModelDef = serde_json::from_str(json)?;
    if definition.name.is_empty() {
        return Err(TurbulenceError::CustomModelError(
            "Model name must not be empty".to_string(),
        ));
    }
    Ok(CustomTurbulenceModel::from_definition(definition))
}

impl TurbulenceModel for CustomTurbulenceModel {
    fn name(&self) -> &str {
        &self.definition.name
    }

    fn num_equations(&self) -> usize {
        self.definition.num_equations
    }

    /// Computes eddy viscosity by evaluating the model's expression string.
    ///
    /// The expression is evaluated with the following variable bindings:
    /// - `var1` / `k` / `nu_tilde` → `var1`
    /// - `var2` / `epsilon` / `omega` / `delta` → `var2`
    /// - `rho` → `rho`
    /// - All named model constants are available by their key.
    ///
    /// If the expression cannot be evaluated (parse failure, unresolved
    /// variables, unsupported operators), returns 0.0 as a safe fallback.
    fn compute_eddy_viscosity(&self, var1: f64, var2: f64, rho: f64) -> f64 {
        if let Some(ref ast) = self.eddy_viscosity_ast {
            let ctx = EvalContext {
                constants: &self.definition.constants,
                var1,
                var2,
                rho,
            };
            match eval_expr(ast, &ctx) {
                Ok(v) if v.is_finite() => v.max(0.0), // clamp to non-negative
                _ => 0.0,
            }
        } else {
            0.0
        }
    }

    fn get_definition(&self) -> &TurbulenceModelDef {
        &self.definition
    }

    fn get_constants(&self) -> &HashMap<String, ModelConstant> {
        &self.definition.constants
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model_template::WallTreatment;

    /// Helper to build a TurbulenceModelDef with a given eddy viscosity expression.
    fn make_def(expr: &str, constants: HashMap<String, ModelConstant>) -> TurbulenceModelDef {
        TurbulenceModelDef {
            name: "test_model".to_string(),
            num_equations: 2,
            transport_equations: vec![],
            eddy_viscosity: expr.to_string(),
            constants,
            wall_treatment: WallTreatment::StandardWallFunction,
        }
    }

    #[test]
    fn test_k_epsilon_expression() {
        // mu_t = rho * Cmu * k^2 / epsilon
        let mut constants = HashMap::new();
        constants.insert("Cmu".into(), ModelConstant {
            value: 0.09,
            description: "C_mu".into(),
            min: None,
            max: None,
        });
        let def = make_def("rho * Cmu * k ^ 2 / epsilon", constants);
        let model = CustomTurbulenceModel::from_definition(def);

        // k=1, epsilon=1, rho=1 => 1 * 0.09 * 1 / 1 = 0.09
        let mu_t = model.compute_eddy_viscosity(1.0, 1.0, 1.0);
        assert!((mu_t - 0.09).abs() < 1e-12, "got {}", mu_t);

        // k=2, epsilon=0.5, rho=1.2 => 1.2 * 0.09 * 4 / 0.5 = 0.864
        let mu_t2 = model.compute_eddy_viscosity(2.0, 0.5, 1.2);
        assert!((mu_t2 - 0.864).abs() < 1e-12, "got {}", mu_t2);
    }

    #[test]
    fn test_sa_like_expression() {
        // Simplified SA-like: var1 * rho (nu_tilde * rho)
        let def = make_def("var1 * rho", HashMap::new());
        let model = CustomTurbulenceModel::from_definition(def);
        let mu_t = model.compute_eddy_viscosity(0.001, 1e-5, 1.225);
        assert!((mu_t - 0.001 * 1.225).abs() < 1e-12);
    }

    #[test]
    fn test_division_by_zero_safety() {
        let mut constants = HashMap::new();
        constants.insert("Cmu".into(), ModelConstant {
            value: 0.09,
            description: "C_mu".into(),
            min: None,
            max: None,
        });
        let def = make_def("rho * Cmu * k ^ 2 / epsilon", constants);
        let model = CustomTurbulenceModel::from_definition(def);

        // epsilon = 0 => division by zero => should return 0.0 safely
        let mu_t = model.compute_eddy_viscosity(1.0, 0.0, 1.0);
        assert_eq!(mu_t, 0.0);
    }

    #[test]
    fn test_constant_expression() {
        let def = make_def("42.0", HashMap::new());
        let model = CustomTurbulenceModel::from_definition(def);
        let mu_t = model.compute_eddy_viscosity(1.0, 1.0, 1.0);
        assert!((mu_t - 42.0).abs() < 1e-12);
    }

    #[test]
    fn test_expression_with_functions() {
        // sqrt(k) * rho
        let def = make_def("sqrt(k) * rho", HashMap::new());
        let model = CustomTurbulenceModel::from_definition(def);
        let mu_t = model.compute_eddy_viscosity(4.0, 1.0, 2.0);
        assert!((mu_t - 4.0).abs() < 1e-12); // sqrt(4) * 2 = 4
    }

    #[test]
    fn test_invalid_expression_returns_zero() {
        // An expression that cannot be parsed will result in ast = None.
        let def = make_def("@#$%^INVALID", HashMap::new());
        let model = CustomTurbulenceModel::from_definition(def);
        assert!(model.eddy_viscosity_ast.is_none());
        let mu_t = model.compute_eddy_viscosity(1.0, 1.0, 1.0);
        assert_eq!(mu_t, 0.0);
    }

    #[test]
    fn test_negative_result_clamped() {
        // Expression that evaluates to negative should be clamped to 0.
        // "-1.0 * rho" => -rho => clamped to 0
        let def = make_def("-1.0 * rho", HashMap::new());
        let model = CustomTurbulenceModel::from_definition(def);
        let mu_t = model.compute_eddy_viscosity(1.0, 1.0, 1.0);
        assert_eq!(mu_t, 0.0);
    }

    #[test]
    fn test_load_custom_model_json() {
        let json = r#"{
            "name": "My k-epsilon",
            "num_equations": 2,
            "transport_equations": [],
            "eddy_viscosity": "rho * Cmu * k ^ 2 / epsilon",
            "constants": {
                "Cmu": {"value": 0.09, "description": "C_mu", "min": null, "max": null}
            },
            "wall_treatment": "StandardWallFunction"
        }"#;
        let model = load_custom_model(json).unwrap();
        assert_eq!(model.name(), "My k-epsilon");

        let mu_t = model.compute_eddy_viscosity(1.0, 1.0, 1.0);
        assert!((mu_t - 0.09).abs() < 1e-12);
    }

    #[test]
    fn test_load_empty_name_error() {
        let json = r#"{
            "name": "",
            "num_equations": 0,
            "transport_equations": [],
            "eddy_viscosity": "0",
            "constants": {},
            "wall_treatment": "LowReynolds"
        }"#;
        assert!(load_custom_model(json).is_err());
    }
}
