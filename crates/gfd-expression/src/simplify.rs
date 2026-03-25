// ---------------------------------------------------------------------------
// simplify.rs  --  Algebraic simplification of Expr trees
// ---------------------------------------------------------------------------

use crate::ast::*;

/// Apply basic algebraic simplification rules to an expression.
///
/// The function recurses bottom-up: children are simplified first, then the
/// parent node is pattern-matched against a set of identity / folding rules.
pub fn simplify(expr: &Expr) -> Expr {
    match expr {
        // -- leaf nodes (no children to simplify) --------------------------
        Expr::Number(_) | Expr::Variable(_) | Expr::FieldRef(_) | Expr::Constant(_) => {
            expr.clone()
        }

        // -- binary operations ---------------------------------------------
        Expr::BinaryOp { op, left, right } => {
            let l = simplify(left);
            let r = simplify(right);
            simplify_binop(*op, l, r)
        }

        // -- unary operations ----------------------------------------------
        Expr::UnaryOp { op, operand } => {
            let inner = simplify(operand);
            simplify_unaryop(*op, inner)
        }

        // -- function call: simplify arguments -----------------------------
        Expr::FunctionCall { name, args } => Expr::FunctionCall {
            name: name.clone(),
            args: args.iter().map(simplify).collect(),
        },

        // -- conditional ---------------------------------------------------
        Expr::Conditional {
            condition,
            true_val,
            false_val,
        } => Expr::Conditional {
            condition: Box::new(simplify(condition)),
            true_val: Box::new(simplify(true_val)),
            false_val: Box::new(simplify(false_val)),
        },

        // -- diff / tensor ops: simplify operands --------------------------
        Expr::DiffOp { op, operands } => Expr::DiffOp {
            op: *op,
            operands: operands.iter().map(simplify).collect(),
        },
        Expr::TensorOp { op, operands } => Expr::TensorOp {
            op: *op,
            operands: operands.iter().map(simplify).collect(),
        },
    }
}

// ---------------------------------------------------------------------------
// helpers
// ---------------------------------------------------------------------------

fn is_zero(e: &Expr) -> bool {
    matches!(e, Expr::Number(v) if *v == 0.0)
}

fn is_one(e: &Expr) -> bool {
    matches!(e, Expr::Number(v) if *v == 1.0)
}

/// Try to extract a constant f64 from a Number node.
fn as_number(e: &Expr) -> Option<f64> {
    if let Expr::Number(v) = e {
        Some(*v)
    } else {
        None
    }
}

/// Structurally compare two expressions (uses PartialEq derive).
fn same(a: &Expr, b: &Expr) -> bool {
    a == b
}

// ---------------------------------------------------------------------------

fn simplify_binop(op: BinOp, l: Expr, r: Expr) -> Expr {
    // --- constant folding -------------------------------------------------
    if let (Some(a), Some(b)) = (as_number(&l), as_number(&r)) {
        let result = match op {
            BinOp::Add => a + b,
            BinOp::Sub => a - b,
            BinOp::Mul => a * b,
            BinOp::Div if b != 0.0 => a / b,
            BinOp::Pow => a.powf(b),
            _ => return Expr::binop(op, l, r), // e.g. div by zero – keep symbolic
        };
        return Expr::Number(result);
    }

    match op {
        // ---- addition ----------------------------------------------------
        BinOp::Add => {
            // 0 + x → x
            if is_zero(&l) {
                return r;
            }
            // x + 0 → x
            if is_zero(&r) {
                return l;
            }
            // x + (-x) → 0  (detects UnaryOp Neg)
            if let Expr::UnaryOp {
                op: UnOp::Neg,
                operand,
            } = &r
            {
                if same(&l, operand) {
                    return Expr::num(0.0);
                }
            }
            Expr::binop(BinOp::Add, l, r)
        }

        // ---- subtraction -------------------------------------------------
        BinOp::Sub => {
            // x - 0 → x
            if is_zero(&r) {
                return l;
            }
            // 0 - x → -x
            if is_zero(&l) {
                return Expr::unaryop(UnOp::Neg, r);
            }
            // x - x → 0
            if same(&l, &r) {
                return Expr::num(0.0);
            }
            Expr::binop(BinOp::Sub, l, r)
        }

        // ---- multiplication ----------------------------------------------
        BinOp::Mul => {
            // 0 * x or x * 0 → 0
            if is_zero(&l) || is_zero(&r) {
                return Expr::num(0.0);
            }
            // 1 * x → x
            if is_one(&l) {
                return r;
            }
            // x * 1 → x
            if is_one(&r) {
                return l;
            }
            Expr::binop(BinOp::Mul, l, r)
        }

        // ---- division ----------------------------------------------------
        BinOp::Div => {
            // 0 / x → 0  (x presumed non-zero)
            if is_zero(&l) {
                return Expr::num(0.0);
            }
            // x / 1 → x
            if is_one(&r) {
                return l;
            }
            // x / x → 1
            if same(&l, &r) {
                return Expr::num(1.0);
            }
            Expr::binop(BinOp::Div, l, r)
        }

        // ---- exponentiation ----------------------------------------------
        BinOp::Pow => {
            // x ^ 0 → 1
            if is_zero(&r) {
                return Expr::num(1.0);
            }
            // x ^ 1 → x
            if is_one(&r) {
                return l;
            }
            // 0 ^ x → 0  (x > 0 assumed)
            if is_zero(&l) {
                return Expr::num(0.0);
            }
            // 1 ^ x → 1
            if is_one(&l) {
                return Expr::num(1.0);
            }
            Expr::binop(BinOp::Pow, l, r)
        }
    }
}

fn simplify_unaryop(op: UnOp, inner: Expr) -> Expr {
    match op {
        // --(-x) → x
        UnOp::Neg => {
            if let Expr::UnaryOp {
                op: UnOp::Neg,
                operand,
            } = &inner
            {
                return (**operand).clone();
            }
            // -(0) → 0
            if is_zero(&inner) {
                return Expr::num(0.0);
            }
            // -(literal) → fold
            if let Some(v) = as_number(&inner) {
                return Expr::num(-v);
            }
            Expr::unaryop(UnOp::Neg, inner)
        }
        // Constant-fold single-arg math functions
        _ => {
            if let Some(v) = as_number(&inner) {
                let result = match op {
                    UnOp::Abs => v.abs(),
                    UnOp::Sqrt if v >= 0.0 => v.sqrt(),
                    UnOp::Sin => v.sin(),
                    UnOp::Cos => v.cos(),
                    UnOp::Exp => v.exp(),
                    UnOp::Log if v > 0.0 => v.ln(),
                    _ => return Expr::unaryop(op, inner),
                };
                return Expr::Number(result);
            }
            Expr::unaryop(op, inner)
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

    fn simplified(input: &str) -> Expr {
        simplify(&parse(input).unwrap())
    }

    #[test]
    fn zero_add() {
        assert_eq!(simplified("0 + x"), Expr::var("x"));
        assert_eq!(simplified("x + 0"), Expr::var("x"));
    }

    #[test]
    fn zero_mul() {
        assert_eq!(simplified("0 * x"), Expr::num(0.0));
        assert_eq!(simplified("x * 0"), Expr::num(0.0));
    }

    #[test]
    fn one_mul() {
        assert_eq!(simplified("1 * x"), Expr::var("x"));
        assert_eq!(simplified("x * 1"), Expr::var("x"));
    }

    #[test]
    fn sub_self() {
        assert_eq!(simplified("x - x"), Expr::num(0.0));
    }

    #[test]
    fn div_self() {
        assert_eq!(simplified("x / x"), Expr::num(1.0));
    }

    #[test]
    fn constant_folding() {
        assert_eq!(simplified("2 + 3"), Expr::num(5.0));
        assert_eq!(simplified("6 / 2"), Expr::num(3.0));
        assert_eq!(simplified("2 ^ 10"), Expr::num(1024.0));
    }

    #[test]
    fn nested_negation() {
        assert_eq!(simplified("--x"), Expr::var("x"));
    }

    #[test]
    fn pow_identities() {
        assert_eq!(simplified("x ^ 0"), Expr::num(1.0));
        assert_eq!(simplified("x ^ 1"), Expr::var("x"));
        assert_eq!(simplified("1 ^ x"), Expr::num(1.0));
    }

    #[test]
    fn composite() {
        // (0 + x) * (1 + 0) → x * 1 → x
        assert_eq!(simplified("(0 + x) * (1 + 0)"), Expr::var("x"));
    }
}
