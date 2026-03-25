//! Custom expression-based boundary conditions.

use std::collections::HashMap;

use gfd_core::mesh::face::Face;
use gfd_core::mesh::cell::Cell;
use gfd_expression::ast::{BinOp, Expr, UnOp};
use crate::traits::{BoundaryCondition, BoundaryConditionType};

/// Large number used for the penalty method.
const LARGE_VALUE: f64 = 1.0e30;

/// A boundary condition defined by a mathematical expression string.
///
/// The expression is evaluated at each boundary face location to determine
/// the prescribed value. Requires the gfd-expression engine for evaluation.
#[derive(Debug, Clone)]
pub struct ExpressionBC {
    /// The expression string in GMN format.
    pub expression: String,
    /// Descriptive name for this BC.
    pub bc_name: String,
}

impl ExpressionBC {
    /// Creates a new expression-based BC.
    pub fn new(expression: String, name: String) -> Self {
        Self {
            expression,
            bc_name: name,
        }
    }

    /// Evaluates the expression at the given face location.
    ///
    /// Parses `self.expression` via `gfd_expression::parse`, binds the face
    /// centroid coordinates as variables `x`, `y`, `z`, and evaluates the AST.
    /// Returns the computed boundary value at the face centroid.
    pub fn evaluate_at_face(&self, face: &Face) -> f64 {
        // Parse the expression string into an AST
        let ast = match gfd_expression::parse(&self.expression) {
            Ok(expr) => expr,
            Err(_) => return 0.0,
        };

        // Bind face center coordinates as variables
        let mut vars = HashMap::new();
        vars.insert("x".to_string(), face.center[0]);
        vars.insert("y".to_string(), face.center[1]);
        vars.insert("z".to_string(), face.center[2]);

        // Evaluate the AST with the variable bindings
        evaluate_expr(&ast, &vars)
    }
}

impl BoundaryCondition for ExpressionBC {
    fn apply_coefficients(
        &self,
        a_p: &mut f64,
        b: &mut f64,
        face: &Face,
        _cell: &Cell,
    ) {
        // Evaluate the expression at the face centroid and apply as Dirichlet.
        let value = self.evaluate_at_face(face);
        *a_p += LARGE_VALUE;
        *b += LARGE_VALUE * value;
    }

    fn bc_type(&self) -> BoundaryConditionType {
        BoundaryConditionType::Custom(self.expression.clone())
    }

    fn name(&self) -> &str {
        &self.bc_name
    }
}

// ---------------------------------------------------------------------------
// Simple recursive AST evaluator
// ---------------------------------------------------------------------------

/// Evaluate an expression AST with the given variable bindings.
///
/// Handles numbers, variables (looked up in `vars`), constants (pi, e),
/// binary operations (+, -, *, /, ^), unary functions (sin, cos, exp, log,
/// sqrt, abs, neg), and conditionals.
///
/// Differential and tensor operators, field references, and unknown
/// variables evaluate to 0.0 as a safe fallback.
fn evaluate_expr(expr: &Expr, vars: &HashMap<String, f64>) -> f64 {
    match expr {
        Expr::Number(v) => *v,

        Expr::Variable(name) => {
            vars.get(name.as_str()).copied().unwrap_or(0.0)
        }

        Expr::FieldRef(_) => {
            // Field references are not spatial coordinates; return 0.0
            0.0
        }

        Expr::Constant(name) => match name.as_str() {
            "pi" => std::f64::consts::PI,
            "e" => std::f64::consts::E,
            _ => 0.0,
        },

        Expr::BinaryOp { op, left, right } => {
            let l = evaluate_expr(left, vars);
            let r = evaluate_expr(right, vars);
            match op {
                BinOp::Add => l + r,
                BinOp::Sub => l - r,
                BinOp::Mul => l * r,
                BinOp::Div => {
                    if r.abs() > 1e-30 {
                        l / r
                    } else {
                        0.0
                    }
                }
                BinOp::Pow => l.powf(r),
            }
        }

        Expr::UnaryOp { op, operand } => {
            let v = evaluate_expr(operand, vars);
            match op {
                UnOp::Neg => -v,
                UnOp::Abs => v.abs(),
                UnOp::Sqrt => v.sqrt(),
                UnOp::Sin => v.sin(),
                UnOp::Cos => v.cos(),
                UnOp::Exp => v.exp(),
                UnOp::Log => v.ln(),
            }
        }

        Expr::FunctionCall { name, args } => {
            // Evaluate common multi-argument functions
            let vals: Vec<f64> = args.iter().map(|a| evaluate_expr(a, vars)).collect();
            match name.as_str() {
                "max" if vals.len() == 2 => vals[0].max(vals[1]),
                "min" if vals.len() == 2 => vals[0].min(vals[1]),
                "pow" if vals.len() == 2 => vals[0].powf(vals[1]),
                _ => vals.first().copied().unwrap_or(0.0),
            }
        }

        Expr::Conditional {
            condition,
            true_val,
            false_val,
        } => {
            let cond = evaluate_expr(condition, vars);
            if cond > 0.0 {
                evaluate_expr(true_val, vars)
            } else {
                evaluate_expr(false_val, vars)
            }
        }

        // Differential and tensor operators are not directly evaluable
        // to a scalar at a point; return 0.0
        Expr::DiffOp { .. } | Expr::TensorOp { .. } => 0.0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_face(center: [f64; 3]) -> Face {
        Face::new(0, vec![], 0, None, 1.0, [1.0, 0.0, 0.0], center)
    }

    fn make_cell() -> Cell {
        Cell::new(0, vec![], vec![0], 1.0, [0.5, 0.5, 0.5])
    }

    #[test]
    fn constant_expression() {
        let bc = ExpressionBC::new("42.0".to_string(), "test".to_string());
        let face = make_face([1.0, 2.0, 3.0]);
        let val = bc.evaluate_at_face(&face);
        assert!((val - 42.0).abs() < 1e-10);
    }

    #[test]
    fn linear_x_expression() {
        let bc = ExpressionBC::new("2.0 * x + 1.0".to_string(), "linear".to_string());

        let face1 = make_face([0.0, 0.0, 0.0]);
        assert!((bc.evaluate_at_face(&face1) - 1.0).abs() < 1e-10);

        let face2 = make_face([3.0, 0.0, 0.0]);
        assert!((bc.evaluate_at_face(&face2) - 7.0).abs() < 1e-10);
    }

    #[test]
    fn quadratic_expression() {
        let bc = ExpressionBC::new("x ^ 2 + y ^ 2".to_string(), "quadratic".to_string());

        let face = make_face([3.0, 4.0, 0.0]);
        let val = bc.evaluate_at_face(&face);
        assert!((val - 25.0).abs() < 1e-10);
    }

    #[test]
    fn trig_expression() {
        let bc = ExpressionBC::new("sin(x)".to_string(), "sine".to_string());

        let face = make_face([std::f64::consts::FRAC_PI_2, 0.0, 0.0]);
        let val = bc.evaluate_at_face(&face);
        assert!((val - 1.0).abs() < 1e-10);
    }

    #[test]
    fn xyz_expression() {
        let bc = ExpressionBC::new("x + 2.0 * y + 3.0 * z".to_string(), "xyz".to_string());
        let face = make_face([1.0, 2.0, 3.0]);
        let val = bc.evaluate_at_face(&face);
        // 1 + 4 + 9 = 14
        assert!((val - 14.0).abs() < 1e-10);
    }

    #[test]
    fn apply_coefficients_dirichlet() {
        let bc = ExpressionBC::new("100.0".to_string(), "hot_wall".to_string());
        let face = make_face([0.0, 0.0, 0.0]);
        let cell = make_cell();

        let mut a_p = 0.0;
        let mut b = 0.0;
        bc.apply_coefficients(&mut a_p, &mut b, &face, &cell);

        assert!(a_p > 1e20, "a_p should be large");
        assert!((b / a_p - 100.0).abs() < 1e-10, "b/a_p should equal the BC value");
    }

    #[test]
    fn invalid_expression_returns_zero() {
        let bc = ExpressionBC::new("!!!invalid".to_string(), "bad".to_string());
        let face = make_face([1.0, 2.0, 3.0]);
        let val = bc.evaluate_at_face(&face);
        assert!((val - 0.0).abs() < 1e-10);
    }

    #[test]
    fn bc_type_is_custom() {
        let bc = ExpressionBC::new("x".to_string(), "test".to_string());
        match bc.bc_type() {
            BoundaryConditionType::Custom(expr) => assert_eq!(expr, "x"),
            _ => panic!("expected Custom BC type"),
        }
    }
}
