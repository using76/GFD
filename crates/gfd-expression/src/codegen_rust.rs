// ---------------------------------------------------------------------------
// codegen_rust.rs  --  Emit Rust source code from an Expr AST
// ---------------------------------------------------------------------------

use crate::ast::*;

/// Convert an expression AST to a Rust source-code string.
///
/// Conventions:
/// - `FieldRef("rho")` → `state.rho[cell_id]`
/// - `Variable("x")` → `x`
/// - Transcendental functions → `f64::sin(…)` etc.
/// - Constants: `pi` → `std::f64::consts::PI`, `e` → `std::f64::consts::E`
pub fn to_rust(expr: &Expr) -> String {
    emit(expr)
}

// ---------------------------------------------------------------------------
// recursive emitter
// ---------------------------------------------------------------------------

fn emit(expr: &Expr) -> String {
    match expr {
        Expr::Number(v) => format_number(*v),

        Expr::Variable(name) => name.clone(),

        Expr::FieldRef(name) => format!("state.{name}[cell_id]"),

        Expr::Constant(name) => match name.as_str() {
            "pi" => "std::f64::consts::PI".to_string(),
            "e" => "std::f64::consts::E".to_string(),
            other => other.to_string(),
        },

        Expr::BinaryOp { op, left, right } => {
            let l = emit_paren(left, Some(*op), true);
            let r = emit_paren(right, Some(*op), false);
            let sym = match op {
                BinOp::Add => "+",
                BinOp::Sub => "-",
                BinOp::Mul => "*",
                BinOp::Div => "/",
                BinOp::Pow => return format!("{l}.powf({r})"),
            };
            format!("{l} {sym} {r}")
        }

        Expr::UnaryOp { op, operand } => {
            let inner = emit(operand);
            match op {
                UnOp::Neg => format!("-{}", emit_paren(operand, None, false)),
                UnOp::Abs => format!("{inner}.abs()"),
                UnOp::Sqrt => format!("{inner}.sqrt()"),
                UnOp::Sin => format!("{inner}.sin()"),
                UnOp::Cos => format!("{inner}.cos()"),
                UnOp::Exp => format!("{inner}.exp()"),
                UnOp::Log => format!("{inner}.ln()"),
            }
        }

        Expr::FunctionCall { name, args } => {
            let arg_strs: Vec<String> = args.iter().map(emit).collect();
            format!("{name}({})", arg_strs.join(", "))
        }

        Expr::Conditional {
            condition,
            true_val,
            false_val,
        } => {
            let c = emit(condition);
            let t = emit(true_val);
            let f = emit(false_val);
            format!("if {c} {{ {t} }} else {{ {f} }}")
        }

        Expr::DiffOp { op, operands } => {
            let args: Vec<String> = operands.iter().map(emit).collect();
            let func = match op {
                DiffOperator::TimeDerivative => "ddt",
                DiffOperator::Gradient => "grad",
                DiffOperator::Divergence => "div",
                DiffOperator::Laplacian => "laplacian",
                DiffOperator::Curl => "curl",
            };
            format!("{func}({})", args.join(", "))
        }

        Expr::TensorOp { op, operands } => {
            let args: Vec<String> = operands.iter().map(emit).collect();
            let func = match op {
                TensorOperator::Dot => "dot",
                TensorOperator::Cross => "cross",
                TensorOperator::Outer => "outer",
                TensorOperator::Trace => "trace",
                TensorOperator::Transpose => "transpose",
                TensorOperator::Symmetric => "sym",
                TensorOperator::Skew => "skew",
                TensorOperator::Magnitude => "mag",
                TensorOperator::MagnitudeSqr => "mag_sqr",
                TensorOperator::Determinant => "det",
                TensorOperator::Inverse => "inv",
            };
            format!("{func}({})", args.join(", "))
        }
    }
}

// ---------------------------------------------------------------------------
// helpers
// ---------------------------------------------------------------------------

/// Format an f64 in a way that Rust will accept as a float literal.
fn format_number(v: f64) -> String {
    if v == v.floor() && v.abs() < 1e15 {
        // Print as integer-style float to avoid things like `1_f64`
        format!("{:.1}", v) // e.g. "3.0"
    } else {
        format!("{}", v)
    }
}

/// Precedence ranking for deciding when to parenthesise.
fn precedence(op: BinOp) -> u8 {
    match op {
        BinOp::Add | BinOp::Sub => 1,
        BinOp::Mul | BinOp::Div => 2,
        BinOp::Pow => 3,
    }
}

/// Wrap `expr` in parentheses when the child's precedence is lower
/// than the parent operator's.
fn emit_paren(expr: &Expr, parent_op: Option<BinOp>, _is_left: bool) -> String {
    let inner = emit(expr);
    if let Some(pop) = parent_op {
        if let Expr::BinaryOp { op: child_op, .. } = expr {
            if precedence(*child_op) < precedence(pop) {
                return format!("({inner})");
            }
        }
    }
    inner
}

// ---------------------------------------------------------------------------
// tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::parse;

    #[test]
    fn simple_add() {
        let e = parse("1 + 2").unwrap();
        assert_eq!(to_rust(&e), "1.0 + 2.0");
    }

    #[test]
    fn field_ref_access() {
        let e = parse("$rho * $U").unwrap();
        assert_eq!(to_rust(&e), "state.rho[cell_id] * state.U[cell_id]");
    }

    #[test]
    fn function_call() {
        let e = parse("sin(x) + cos(y)").unwrap();
        let code = to_rust(&e);
        assert!(code.contains(".sin()"));
        assert!(code.contains(".cos()"));
    }

    #[test]
    fn power() {
        let e = parse("x ^ 3").unwrap();
        assert_eq!(to_rust(&e), "x.powf(3.0)");
    }

    #[test]
    fn precedence_parens() {
        let e = parse("(a + b) * c").unwrap();
        let code = to_rust(&e);
        assert!(code.contains("(a + b)"));
    }

    #[test]
    fn constant_pi() {
        let e = parse("pi * r ^ 2").unwrap();
        let code = to_rust(&e);
        assert!(code.contains("std::f64::consts::PI"));
    }
}
