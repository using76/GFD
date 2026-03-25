// ---------------------------------------------------------------------------
// dimension.rs  --  Dimensional analysis / unit checking
// ---------------------------------------------------------------------------

use std::collections::HashMap;

use crate::ast::*;
use crate::ExpressionError;

// ---------------------------------------------------------------------------
// common SI units (convenience constants)
// ---------------------------------------------------------------------------

/// Dimensionless.
pub const DIMENSIONLESS: Unit = Unit::DIMENSIONLESS;
/// Metre.
pub const METRE: Unit = Unit::new(0, 1, 0, 0, 0);
/// Second.
pub const SECOND: Unit = Unit::new(0, 0, 1, 0, 0);
/// Kilogram.
pub const KILOGRAM: Unit = Unit::new(1, 0, 0, 0, 0);
/// Kelvin.
pub const KELVIN: Unit = Unit::new(0, 0, 0, 1, 0);
/// Mole.
pub const MOLE: Unit = Unit::new(0, 0, 0, 0, 1);
/// Velocity  m/s.
pub const VELOCITY: Unit = Unit::new(0, 1, -1, 0, 0);
/// Acceleration  m/s².
pub const ACCELERATION: Unit = Unit::new(0, 1, -2, 0, 0);
/// Density  kg/m³.
pub const DENSITY: Unit = Unit::new(1, -3, 0, 0, 0);
/// Pressure / stress  kg/(m·s²) = Pa.
pub const PRESSURE: Unit = Unit::new(1, -1, -2, 0, 0);
/// Dynamic viscosity  kg/(m·s) = Pa·s.
pub const VISCOSITY: Unit = Unit::new(1, -1, -1, 0, 0);

// ---------------------------------------------------------------------------
// context
// ---------------------------------------------------------------------------

/// Maps variable / field names to their physical dimensions.
#[derive(Debug, Clone, Default)]
pub struct DimensionContext {
    dims: HashMap<String, Unit>,
}

impl DimensionContext {
    pub fn new() -> Self {
        Self::default()
    }

    /// Register a variable or field name with its unit.
    pub fn insert(&mut self, name: &str, unit: Unit) -> &mut Self {
        self.dims.insert(name.to_string(), unit);
        self
    }

    /// Look up the unit for a name.
    pub fn get(&self, name: &str) -> Option<&Unit> {
        self.dims.get(name)
    }
}

// ---------------------------------------------------------------------------
// public API
// ---------------------------------------------------------------------------

/// Infer and check the physical dimensions of `expr`.
///
/// Returns the resulting [`Unit`] if every sub-expression is dimensionally
/// consistent, or an error describing the first inconsistency found.
pub fn check_dimensions(
    expr: &Expr,
    ctx: &DimensionContext,
) -> Result<Unit, ExpressionError> {
    infer(expr, ctx)
}

// ---------------------------------------------------------------------------
// recursive inference
// ---------------------------------------------------------------------------

fn infer(expr: &Expr, ctx: &DimensionContext) -> Result<Unit, ExpressionError> {
    match expr {
        // Numbers are dimensionless (they are pure coefficients).
        Expr::Number(_) => Ok(DIMENSIONLESS),

        // Named constants (pi, e, …) are dimensionless.
        Expr::Constant(_) => Ok(DIMENSIONLESS),

        // Variables and field references: look up in context.
        Expr::Variable(name) | Expr::FieldRef(name) => ctx
            .get(name)
            .copied()
            .ok_or_else(|| ExpressionError::DimensionError {
                message: format!("unknown dimension for `{name}`"),
            }),

        Expr::BinaryOp { op, left, right } => {
            let lu = infer(left, ctx)?;
            let ru = infer(right, ctx)?;
            match op {
                // Add / Sub require identical dimensions.
                BinOp::Add | BinOp::Sub => {
                    if lu != ru {
                        Err(ExpressionError::DimensionError {
                            message: format!(
                                "dimension mismatch in {:?}: {lu:?} vs {ru:?}",
                                op
                            ),
                        })
                    } else {
                        Ok(lu)
                    }
                }
                // Mul → exponents add.
                BinOp::Mul => Ok(lu.mul(ru)),
                // Div → exponents subtract.
                BinOp::Div => Ok(lu.div(ru)),
                // Pow: exponent must be dimensionless and ideally a known integer.
                BinOp::Pow => {
                    if !ru.is_dimensionless() {
                        return Err(ExpressionError::DimensionError {
                            message: "exponent must be dimensionless".into(),
                        });
                    }
                    // If exponent is a literal integer we can compute the result.
                    if let Expr::Number(n) = right.as_ref() {
                        let ni = *n as i8;
                        Ok(lu.pow(ni))
                    } else {
                        // Cannot determine resulting dimension symbolically;
                        // assume base must be dimensionless.
                        if lu.is_dimensionless() {
                            Ok(DIMENSIONLESS)
                        } else {
                            Err(ExpressionError::DimensionError {
                                message:
                                    "non-literal exponent requires dimensionless base".into(),
                            })
                        }
                    }
                }
            }
        }

        Expr::UnaryOp { op, operand } => {
            let u = infer(operand, ctx)?;
            match op {
                // Negation preserves dimension.
                UnOp::Neg => Ok(u),
                // Abs preserves dimension.
                UnOp::Abs => Ok(u),
                // Sqrt: half the exponents (all must be even).
                UnOp::Sqrt => {
                    if u.kg % 2 != 0 || u.m % 2 != 0 || u.s % 2 != 0 || u.k % 2 != 0 || u.mol % 2 != 0 {
                        Err(ExpressionError::DimensionError {
                            message: format!(
                                "sqrt requires even dimension exponents, got {u:?}"
                            ),
                        })
                    } else {
                        Ok(Unit::new(u.kg / 2, u.m / 2, u.s / 2, u.k / 2, u.mol / 2))
                    }
                }
                // Transcendental functions require dimensionless arguments.
                UnOp::Sin | UnOp::Cos | UnOp::Exp | UnOp::Log => {
                    if !u.is_dimensionless() {
                        Err(ExpressionError::DimensionError {
                            message: format!(
                                "{op:?} requires dimensionless argument, got {u:?}"
                            ),
                        })
                    } else {
                        Ok(DIMENSIONLESS)
                    }
                }
            }
        }

        // Function calls: we don't know the signature in general.
        Expr::FunctionCall { .. } => Ok(DIMENSIONLESS),

        // Conditional: both branches must match.
        Expr::Conditional {
            true_val,
            false_val,
            ..
        } => {
            let tu = infer(true_val, ctx)?;
            let fu = infer(false_val, ctx)?;
            if tu != fu {
                Err(ExpressionError::DimensionError {
                    message: format!(
                        "conditional branches have different dimensions: {tu:?} vs {fu:?}"
                    ),
                })
            } else {
                Ok(tu)
            }
        }

        // Diff / tensor ops: delegate to simplified heuristics.
        Expr::DiffOp { .. } | Expr::TensorOp { .. } => {
            // Full treatment would require tensor-rank tracking.
            // For now, return dimensionless to avoid blocking downstream code.
            Ok(DIMENSIONLESS)
        }
    }
}

// ---------------------------------------------------------------------------
// tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::parse;

    fn ctx() -> DimensionContext {
        let mut c = DimensionContext::new();
        c.insert("rho", DENSITY);
        c.insert("U", VELOCITY);
        c.insert("p", PRESSURE);
        c.insert("mu", VISCOSITY);
        c.insert("T", KELVIN);
        c.insert("x", METRE);
        c
    }

    #[test]
    fn velocity_squared() {
        let e = parse("$U * $U").unwrap();
        let u = check_dimensions(&e, &ctx()).unwrap();
        // m/s * m/s = m²/s²
        assert_eq!(u, Unit::new(0, 2, -2, 0, 0));
    }

    #[test]
    fn pressure_consistency() {
        // rho * U * U  has same dimension as p  (kg/(m·s²))
        let e1 = parse("$rho * $U * $U").unwrap();
        let e2 = parse("$p").unwrap();
        let u1 = check_dimensions(&e1, &ctx()).unwrap();
        let u2 = check_dimensions(&e2, &ctx()).unwrap();
        assert_eq!(u1, u2);
    }

    #[test]
    fn add_mismatch() {
        let e = parse("$rho + $U").unwrap();
        assert!(check_dimensions(&e, &ctx()).is_err());
    }

    #[test]
    fn transcendental_needs_dimensionless() {
        let e = parse("sin($x)").unwrap();
        assert!(check_dimensions(&e, &ctx()).is_err());
    }

    #[test]
    fn sqrt_velocity_sq() {
        let e = parse("sqrt($U * $U)").unwrap();
        let u = check_dimensions(&e, &ctx()).unwrap();
        assert_eq!(u, VELOCITY);
    }
}
