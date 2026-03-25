// ---------------------------------------------------------------------------
// differentiate.rs  --  Symbolic differentiation of Expr trees
// ---------------------------------------------------------------------------

use crate::ast::*;
use crate::simplify::simplify;
use crate::ExpressionError;

/// Symbolically differentiate `expr` with respect to `var`.
///
/// The result is automatically simplified via [`simplify`].
pub fn differentiate(expr: &Expr, var: &str) -> Result<Expr, ExpressionError> {
    let raw = diff(expr, var)?;
    Ok(simplify(&raw))
}

// ---------------------------------------------------------------------------
// recursive core
// ---------------------------------------------------------------------------

fn diff(expr: &Expr, var: &str) -> Result<Expr, ExpressionError> {
    match expr {
        // d(c)/dx = 0
        Expr::Number(_) | Expr::Constant(_) | Expr::FieldRef(_) => Ok(Expr::num(0.0)),

        // d(x)/dx = 1,  d(y)/dx = 0
        Expr::Variable(name) => {
            if name == var {
                Ok(Expr::num(1.0))
            } else {
                Ok(Expr::num(0.0))
            }
        }

        // d(f ⊕ g)/dx
        Expr::BinaryOp { op, left, right } => diff_binop(*op, left, right, var),

        // d(op(f))/dx  =  op'(f) · f'
        Expr::UnaryOp { op, operand } => diff_unaryop(*op, operand, var),

        // Generic function call -- not differentiable in general
        Expr::FunctionCall { name, .. } => Err(ExpressionError::DifferentiationError {
            message: format!("cannot differentiate generic function `{name}`"),
        }),

        // Conditional -- differentiate both branches
        Expr::Conditional {
            condition,
            true_val,
            false_val,
        } => Ok(Expr::Conditional {
            condition: condition.clone(),
            true_val: Box::new(diff(true_val, var)?),
            false_val: Box::new(diff(false_val, var)?),
        }),

        // DiffOp / TensorOp -- not supported yet
        Expr::DiffOp { .. } | Expr::TensorOp { .. } => Err(
            ExpressionError::DifferentiationError {
                message: "differentiation of differential/tensor operators not yet supported"
                    .into(),
            },
        ),
    }
}

// ---------------------------------------------------------------------------
// binary operations
// ---------------------------------------------------------------------------

fn diff_binop(
    op: BinOp,
    left: &Expr,
    right: &Expr,
    var: &str,
) -> Result<Expr, ExpressionError> {
    let dl = diff(left, var)?;
    let dr = diff(right, var)?;

    match op {
        // d(f+g) = df + dg
        BinOp::Add => Ok(Expr::binop(BinOp::Add, dl, dr)),

        // d(f-g) = df - dg
        BinOp::Sub => Ok(Expr::binop(BinOp::Sub, dl, dr)),

        // d(f*g) = f*dg + g*df   (product rule)
        BinOp::Mul => Ok(Expr::binop(
            BinOp::Add,
            Expr::binop(BinOp::Mul, left.clone(), dr),
            Expr::binop(BinOp::Mul, right.clone(), dl),
        )),

        // d(f/g) = (g*df - f*dg) / g²   (quotient rule)
        BinOp::Div => Ok(Expr::binop(
            BinOp::Div,
            Expr::binop(
                BinOp::Sub,
                Expr::binop(BinOp::Mul, right.clone(), dl),
                Expr::binop(BinOp::Mul, left.clone(), dr),
            ),
            Expr::binop(BinOp::Pow, right.clone(), Expr::num(2.0)),
        )),

        // d(f^g):
        //   if g is constant w.r.t. var:  g * f^(g-1) * df        (power rule)
        //   if f is constant w.r.t. var:  f^g * ln(f) * dg        (exponential rule)
        //   general:                      f^g * (dg*ln(f) + g*df/f)
        BinOp::Pow => {
            let f_const = is_const(left, var);
            let g_const = is_const(right, var);

            if g_const {
                // g * f^(g-1) * f'
                Ok(Expr::binop(
                    BinOp::Mul,
                    Expr::binop(
                        BinOp::Mul,
                        right.clone(),
                        Expr::binop(
                            BinOp::Pow,
                            left.clone(),
                            Expr::binop(BinOp::Sub, right.clone(), Expr::num(1.0)),
                        ),
                    ),
                    dl,
                ))
            } else if f_const {
                // f^g * ln(f) * g'
                Ok(Expr::binop(
                    BinOp::Mul,
                    Expr::binop(
                        BinOp::Mul,
                        Expr::binop(BinOp::Pow, left.clone(), right.clone()),
                        Expr::unaryop(UnOp::Log, left.clone()),
                    ),
                    dr,
                ))
            } else {
                // General case: f^g * (g' * ln(f) + g * f' / f)
                Ok(Expr::binop(
                    BinOp::Mul,
                    Expr::binop(BinOp::Pow, left.clone(), right.clone()),
                    Expr::binop(
                        BinOp::Add,
                        Expr::binop(BinOp::Mul, dr, Expr::unaryop(UnOp::Log, left.clone())),
                        Expr::binop(
                            BinOp::Mul,
                            right.clone(),
                            Expr::binop(BinOp::Div, dl, left.clone()),
                        ),
                    ),
                ))
            }
        }
    }
}

// ---------------------------------------------------------------------------
// unary operations  (chain rule)
// ---------------------------------------------------------------------------

fn diff_unaryop(op: UnOp, operand: &Expr, var: &str) -> Result<Expr, ExpressionError> {
    let df = diff(operand, var)?;

    let outer_deriv = match op {
        // d(-f) = -f'
        UnOp::Neg => return Ok(Expr::unaryop(UnOp::Neg, df)),

        // d|f| – not differentiable everywhere, but: sign(f) * f'
        // We approximate sign(f) as f / |f|.
        UnOp::Abs => {
            return Ok(Expr::binop(
                BinOp::Mul,
                Expr::binop(
                    BinOp::Div,
                    operand.clone(),
                    Expr::unaryop(UnOp::Abs, operand.clone()),
                ),
                df,
            ));
        }

        // d(sqrt(f)) = f' / (2 * sqrt(f))
        UnOp::Sqrt => {
            return Ok(Expr::binop(
                BinOp::Div,
                df,
                Expr::binop(
                    BinOp::Mul,
                    Expr::num(2.0),
                    Expr::unaryop(UnOp::Sqrt, operand.clone()),
                ),
            ));
        }

        // d(sin(f)) = cos(f) * f'
        UnOp::Sin => Expr::unaryop(UnOp::Cos, operand.clone()),

        // d(cos(f)) = -sin(f) * f'
        UnOp::Cos => Expr::unaryop(UnOp::Neg, Expr::unaryop(UnOp::Sin, operand.clone())),

        // d(exp(f)) = exp(f) * f'
        UnOp::Exp => Expr::unaryop(UnOp::Exp, operand.clone()),

        // d(ln(f)) = f' / f
        UnOp::Log => {
            return Ok(Expr::binop(BinOp::Div, df, operand.clone()));
        }
    };

    // chain rule: outer' * inner'
    Ok(Expr::binop(BinOp::Mul, outer_deriv, df))
}

// ---------------------------------------------------------------------------
// utility
// ---------------------------------------------------------------------------

/// Returns `true` when `expr` does not depend on `var`.
fn is_const(expr: &Expr, var: &str) -> bool {
    match expr {
        Expr::Number(_) | Expr::Constant(_) | Expr::FieldRef(_) => true,
        Expr::Variable(name) => name != var,
        Expr::BinaryOp { left, right, .. } => is_const(left, var) && is_const(right, var),
        Expr::UnaryOp { operand, .. } => is_const(operand, var),
        Expr::FunctionCall { args, .. } => args.iter().all(|a| is_const(a, var)),
        Expr::Conditional {
            condition,
            true_val,
            false_val,
        } => is_const(condition, var) && is_const(true_val, var) && is_const(false_val, var),
        Expr::DiffOp { operands, .. } | Expr::TensorOp { operands, .. } => {
            operands.iter().all(|o| is_const(o, var))
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

    fn d(input: &str, var: &str) -> Expr {
        differentiate(&parse(input).unwrap(), var).unwrap()
    }

    #[test]
    fn const_zero() {
        assert_eq!(d("5", "x"), Expr::num(0.0));
    }

    #[test]
    fn identity() {
        assert_eq!(d("x", "x"), Expr::num(1.0));
    }

    #[test]
    fn different_var() {
        assert_eq!(d("y", "x"), Expr::num(0.0));
    }

    #[test]
    fn sum_rule() {
        // d(x + x)/dx = 1 + 1 = 2
        assert_eq!(d("x + x", "x"), Expr::num(2.0));
    }

    #[test]
    fn product_rule() {
        // d(x * x)/dx = x*1 + x*1 = 2*x  (after simplification: x + x)
        let result = d("x * x", "x");
        // We expect simplified form: x + x  (or could be 2*x depending on simplifier)
        // Our simplifier doesn't combine like terms, so expect Add(x, x)
        assert_eq!(
            result,
            Expr::binop(BinOp::Add, Expr::var("x"), Expr::var("x"))
        );
    }

    #[test]
    fn power_rule() {
        // d(x^3)/dx = 3 * x^2  (simplified: the trailing `* 1` is folded away)
        let result = d("x ^ 3", "x");
        assert_eq!(
            result,
            Expr::binop(
                BinOp::Mul,
                Expr::num(3.0),
                Expr::binop(BinOp::Pow, Expr::var("x"), Expr::num(2.0)),
            )
        );
    }

    #[test]
    fn chain_rule_sin() {
        // d(sin(x))/dx = cos(x) * 1 → cos(x)  (simplified)
        let result = d("sin(x)", "x");
        assert_eq!(result, Expr::unaryop(UnOp::Cos, Expr::var("x")));
    }

    #[test]
    fn chain_rule_exp() {
        // d(exp(x))/dx = exp(x)
        let result = d("exp(x)", "x");
        assert_eq!(result, Expr::unaryop(UnOp::Exp, Expr::var("x")));
    }

    #[test]
    fn negation() {
        // d(-x)/dx = -1  → simplified to Number(-1.0)
        let result = d("-x", "x");
        assert_eq!(result, Expr::num(-1.0));
    }
}
