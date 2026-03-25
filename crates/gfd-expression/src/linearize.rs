// ---------------------------------------------------------------------------
// linearize.rs  --  Source-term linearisation S(φ) → Sc + Sp·φ
// ---------------------------------------------------------------------------

use crate::ast::*;
use crate::simplify::simplify;
use crate::ExpressionError;

/// Linearise an expression `S(φ)` around the variable `phi`.
///
/// Returns `(Sc, Sp)` such that `S(φ) ≈ Sc + Sp * φ`, where:
/// - `Sc` collects all terms that are **independent** of `phi`,
/// - `Sp` collects the **coefficient** of `phi` in terms that are linear in it.
///
/// This is a syntactic decomposition suitable for implicit source-term
/// treatment in finite-volume solvers.  Non-linear terms that cannot be
/// cleanly split will appear in `Sc` evaluated symbolically.
pub fn linearize_source(
    expr: &Expr,
    phi: &str,
) -> Result<(Expr, Expr), ExpressionError> {
    let (sc, sp) = split(expr, phi);
    Ok((simplify(&sc), simplify(&sp)))
}

// ---------------------------------------------------------------------------
// internal: recursive split
// ---------------------------------------------------------------------------

/// Returns `(sc, sp)` where `expr ≈ sc + sp * phi`.
fn split(expr: &Expr, phi: &str) -> (Expr, Expr) {
    match expr {
        // Constants / numbers / field refs → pure Sc
        Expr::Number(_) | Expr::Constant(_) | Expr::FieldRef(_) => {
            (expr.clone(), Expr::num(0.0))
        }

        // Variable: if it IS phi then Sc=0, Sp=1; else Sc=var, Sp=0
        Expr::Variable(name) => {
            if name == phi {
                (Expr::num(0.0), Expr::num(1.0))
            } else {
                (expr.clone(), Expr::num(0.0))
            }
        }

        // Add / Sub are linear: split both sides independently
        Expr::BinaryOp {
            op: BinOp::Add,
            left,
            right,
        } => {
            let (lsc, lsp) = split(left, phi);
            let (rsc, rsp) = split(right, phi);
            (
                Expr::binop(BinOp::Add, lsc, rsc),
                Expr::binop(BinOp::Add, lsp, rsp),
            )
        }
        Expr::BinaryOp {
            op: BinOp::Sub,
            left,
            right,
        } => {
            let (lsc, lsp) = split(left, phi);
            let (rsc, rsp) = split(right, phi);
            (
                Expr::binop(BinOp::Sub, lsc, rsc),
                Expr::binop(BinOp::Sub, lsp, rsp),
            )
        }

        // Mul: a * b.  If one side is independent of phi and the other
        // is linear, we can factor cleanly:  (a_sc * b_sc) + (a_sc * b_sp) * phi
        // (when a is constant w.r.t. phi).
        Expr::BinaryOp {
            op: BinOp::Mul,
            left,
            right,
        } => {
            let l_dep = depends_on(left, phi);
            let r_dep = depends_on(right, phi);

            match (l_dep, r_dep) {
                // neither depends → all Sc
                (false, false) => (expr.clone(), Expr::num(0.0)),
                // only right depends: factor = left * split(right)
                (false, true) => {
                    let (rsc, rsp) = split(right, phi);
                    (
                        Expr::binop(BinOp::Mul, left.as_ref().clone(), rsc),
                        Expr::binop(BinOp::Mul, left.as_ref().clone(), rsp),
                    )
                }
                // only left depends: symmetric
                (true, false) => {
                    let (lsc, lsp) = split(left, phi);
                    (
                        Expr::binop(BinOp::Mul, lsc, right.as_ref().clone()),
                        Expr::binop(BinOp::Mul, lsp, right.as_ref().clone()),
                    )
                }
                // both depend: non-linear → put entire expression in Sc
                (true, true) => (expr.clone(), Expr::num(0.0)),
            }
        }

        // Negation: distribute through
        Expr::UnaryOp {
            op: UnOp::Neg,
            operand,
        } => {
            let (sc, sp) = split(operand, phi);
            (
                Expr::unaryop(UnOp::Neg, sc),
                Expr::unaryop(UnOp::Neg, sp),
            )
        }

        // Everything else that does not depend on phi → Sc
        _ if !depends_on(expr, phi) => (expr.clone(), Expr::num(0.0)),

        // Fallback: non-linear dependence → entire expression goes to Sc
        _ => (expr.clone(), Expr::num(0.0)),
    }
}

/// Check whether `expr` structurally contains the variable `phi`.
fn depends_on(expr: &Expr, phi: &str) -> bool {
    match expr {
        Expr::Number(_) | Expr::Constant(_) | Expr::FieldRef(_) => false,
        Expr::Variable(name) => name == phi,
        Expr::BinaryOp { left, right, .. } => depends_on(left, phi) || depends_on(right, phi),
        Expr::UnaryOp { operand, .. } => depends_on(operand, phi),
        Expr::FunctionCall { args, .. } => args.iter().any(|a| depends_on(a, phi)),
        Expr::Conditional {
            condition,
            true_val,
            false_val,
        } => depends_on(condition, phi) || depends_on(true_val, phi) || depends_on(false_val, phi),
        Expr::DiffOp { operands, .. } | Expr::TensorOp { operands, .. } => {
            operands.iter().any(|o| depends_on(o, phi))
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

    fn lin(input: &str, phi: &str) -> (Expr, Expr) {
        linearize_source(&parse(input).unwrap(), phi).unwrap()
    }

    #[test]
    fn pure_constant() {
        let (sc, sp) = lin("5", "T");
        assert_eq!(sc, Expr::num(5.0));
        assert_eq!(sp, Expr::num(0.0));
    }

    #[test]
    fn linear_variable() {
        // S = T  →  Sc=0, Sp=1
        let (sc, sp) = lin("T", "T");
        assert_eq!(sc, Expr::num(0.0));
        assert_eq!(sp, Expr::num(1.0));
    }

    #[test]
    fn linear_expression() {
        // S = a * T + b  →  Sc=b, Sp=a  (with a,b variables)
        let (sc, sp) = lin("a * T + b", "T");
        // Sc should simplify to: 0 + b = b (after simplification of a*0+b)
        assert_eq!(sc, Expr::var("b"));
        // Sp should be: a*1 + 0 = a
        assert_eq!(sp, Expr::var("a"));
    }

    #[test]
    fn negated() {
        // S = -T  →  Sc=0, Sp=-1
        let (sc, sp) = lin("-T", "T");
        assert_eq!(sc, Expr::num(0.0));
        assert_eq!(sp, Expr::num(-1.0));
    }
}
