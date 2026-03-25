// ---------------------------------------------------------------------------
// codegen_latex.rs  --  Emit LaTeX markup from an Expr AST
// ---------------------------------------------------------------------------

use crate::ast::*;

/// Convert an expression AST to a LaTeX math-mode string.
///
/// Conventions:
/// - `FieldRef("rho")` → `\rho` (Greek) or `\text{name}` (non-Greek)
/// - Division → `\frac{…}{…}`
/// - `Gradient` → `\nabla`, `Laplacian` → `\nabla^2`, etc.
pub fn to_latex(expr: &Expr) -> String {
    emit(expr)
}

// ---------------------------------------------------------------------------
// recursive emitter
// ---------------------------------------------------------------------------

fn emit(expr: &Expr) -> String {
    match expr {
        Expr::Number(v) => format_latex_number(*v),

        Expr::Variable(name) => greek_or_text(name),

        Expr::FieldRef(name) => greek_or_text(name),

        Expr::Constant(name) => match name.as_str() {
            "pi" => r"\pi".to_string(),
            "e" => "e".to_string(),
            other => format!(r"\text{{{other}}}"),
        },

        Expr::BinaryOp { op, left, right } => match op {
            BinOp::Add => format!("{} + {}", emit_child(left, *op), emit_child(right, *op)),
            BinOp::Sub => format!("{} - {}", emit_child(left, *op), emit_child(right, *op)),
            BinOp::Mul => {
                let l = emit_child(left, *op);
                let r = emit_child(right, *op);
                // Use \cdot between non-trivial operands, implicit juxtaposition
                // for single-symbol operands.
                if is_simple(left) && is_simple(right) {
                    format!("{l} {r}")
                } else {
                    format!("{l} \\cdot {r}")
                }
            }
            BinOp::Div => {
                let num = emit(left);
                let den = emit(right);
                format!(r"\frac{{{num}}}{{{den}}}")
            }
            BinOp::Pow => {
                let base = emit_child(left, *op);
                let exp = emit(right);
                format!("{{{base}}}^{{{exp}}}")
            }
        },

        Expr::UnaryOp { op, operand } => {
            let inner = emit(operand);
            match op {
                UnOp::Neg => format!("-{}", emit_child(operand, BinOp::Mul)),
                UnOp::Abs => format!(r"\left| {inner} \right|"),
                UnOp::Sqrt => format!(r"\sqrt{{{inner}}}"),
                UnOp::Sin => format!(r"\sin\left( {inner} \right)"),
                UnOp::Cos => format!(r"\cos\left( {inner} \right)"),
                UnOp::Exp => format!(r"\exp\left( {inner} \right)"),
                UnOp::Log => format!(r"\ln\left( {inner} \right)"),
            }
        }

        Expr::FunctionCall { name, args } => {
            let args_str: Vec<String> = args.iter().map(emit).collect();
            format!(
                r"\operatorname{{{name}}}\left( {} \right)",
                args_str.join(", ")
            )
        }

        Expr::Conditional {
            condition,
            true_val,
            false_val,
        } => {
            let c = emit(condition);
            let t = emit(true_val);
            let f = emit(false_val);
            format!(
                r"\begin{{cases}} {t} & \text{{if }} {c} \\ {f} & \text{{otherwise}} \end{{cases}}"
            )
        }

        Expr::DiffOp { op, operands } => {
            let args: Vec<String> = operands.iter().map(emit).collect();
            match op {
                DiffOperator::TimeDerivative => {
                    if let Some(a) = args.first() {
                        format!(r"\frac{{\partial {a}}}{{\partial t}}")
                    } else {
                        r"\frac{\partial}{\partial t}".to_string()
                    }
                }
                DiffOperator::Gradient => {
                    format!(r"\nabla {}", args.first().unwrap_or(&String::new()))
                }
                DiffOperator::Divergence => {
                    format!(
                        r"\nabla \cdot {}",
                        args.first().unwrap_or(&String::new())
                    )
                }
                DiffOperator::Laplacian => {
                    if args.len() == 2 {
                        format!(r"\nabla \cdot \left( {} \nabla {} \right)", args[0], args[1])
                    } else {
                        format!(
                            r"\nabla^2 {}",
                            args.first().unwrap_or(&String::new())
                        )
                    }
                }
                DiffOperator::Curl => {
                    format!(
                        r"\nabla \times {}",
                        args.first().unwrap_or(&String::new())
                    )
                }
            }
        }

        Expr::TensorOp { op, operands } => {
            let args: Vec<String> = operands.iter().map(emit).collect();
            match op {
                TensorOperator::Dot => format!(
                    "{} \\cdot {}",
                    args.first().unwrap_or(&String::new()),
                    args.get(1).unwrap_or(&String::new())
                ),
                TensorOperator::Cross => format!(
                    "{} \\times {}",
                    args.first().unwrap_or(&String::new()),
                    args.get(1).unwrap_or(&String::new())
                ),
                TensorOperator::Outer => format!(
                    "{} \\otimes {}",
                    args.first().unwrap_or(&String::new()),
                    args.get(1).unwrap_or(&String::new())
                ),
                TensorOperator::Trace => format!(
                    r"\operatorname{{tr}}\left( {} \right)",
                    args.first().unwrap_or(&String::new())
                ),
                TensorOperator::Transpose => format!(
                    "{}^T",
                    args.first().unwrap_or(&String::new())
                ),
                TensorOperator::Symmetric => format!(
                    r"\operatorname{{sym}}\left( {} \right)",
                    args.first().unwrap_or(&String::new())
                ),
                TensorOperator::Skew => format!(
                    r"\operatorname{{skew}}\left( {} \right)",
                    args.first().unwrap_or(&String::new())
                ),
                TensorOperator::Magnitude => format!(
                    r"\left| {} \right|",
                    args.first().unwrap_or(&String::new())
                ),
                TensorOperator::MagnitudeSqr => format!(
                    r"\left| {} \right|^2",
                    args.first().unwrap_or(&String::new())
                ),
                TensorOperator::Determinant => format!(
                    r"\det\left( {} \right)",
                    args.first().unwrap_or(&String::new())
                ),
                TensorOperator::Inverse => format!(
                    "{}^{{-1}}",
                    args.first().unwrap_or(&String::new())
                ),
            }
        }
    }
}

// ---------------------------------------------------------------------------
// helpers
// ---------------------------------------------------------------------------

fn format_latex_number(v: f64) -> String {
    if v == v.floor() && v.abs() < 1e15 {
        format!("{}", v as i64)
    } else {
        format!("{}", v)
    }
}

/// Map well-known names to Greek-letter LaTeX commands.
fn greek_or_text(name: &str) -> String {
    match name {
        "alpha" => r"\alpha".into(),
        "beta" => r"\beta".into(),
        "gamma" | "Gamma_coeff" => r"\gamma".into(),
        "delta" => r"\delta".into(),
        "epsilon" => r"\epsilon".into(),
        "zeta" => r"\zeta".into(),
        "eta" => r"\eta".into(),
        "theta" => r"\theta".into(),
        "kappa" => r"\kappa".into(),
        "lambda" => r"\lambda".into(),
        "mu" => r"\mu".into(),
        "nu" => r"\nu".into(),
        "rho" => r"\rho".into(),
        "sigma" => r"\sigma".into(),
        "tau" => r"\tau".into(),
        "phi" => r"\phi".into(),
        "psi" => r"\psi".into(),
        "omega" => r"\omega".into(),
        "Gamma" => r"\Gamma".into(),
        "Delta" => r"\Delta".into(),
        "Theta" => r"\Theta".into(),
        "Lambda" => r"\Lambda".into(),
        "Sigma" => r"\Sigma".into(),
        "Phi" => r"\Phi".into(),
        "Psi" => r"\Psi".into(),
        "Omega" => r"\Omega".into(),
        // Single-letter names stay as-is (T, U, p, …)
        s if s.len() == 1 => s.into(),
        // Multi-character non-Greek → wrapped in \text
        other => format!(r"\text{{{other}}}"),
    }
}

/// Returns true if the expression is "simple" (single symbol/number)
/// for deciding whether to use implicit multiplication.
fn is_simple(expr: &Expr) -> bool {
    matches!(
        expr,
        Expr::Number(_)
            | Expr::Variable(_)
            | Expr::FieldRef(_)
            | Expr::Constant(_)
    )
}

/// Wrap child in parens if needed for precedence.
fn emit_child(expr: &Expr, parent_op: BinOp) -> String {
    let s = emit(expr);
    if let Expr::BinaryOp { op: child_op, .. } = expr {
        if child_needs_parens(*child_op, parent_op) {
            return format!(r"\left( {s} \right)");
        }
    }
    s
}

fn child_needs_parens(child: BinOp, parent: BinOp) -> bool {
    let prec = |op: BinOp| -> u8 {
        match op {
            BinOp::Add | BinOp::Sub => 1,
            BinOp::Mul | BinOp::Div => 2,
            BinOp::Pow => 3,
        }
    };
    prec(child) < prec(parent)
}

// ---------------------------------------------------------------------------
// tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::parse;

    #[test]
    fn simple_fraction() {
        let e = parse("a / b").unwrap();
        let latex = to_latex(&e);
        assert_eq!(latex, r"\frac{a}{b}");
    }

    #[test]
    fn greek_letters() {
        let e = parse("$rho * $mu").unwrap();
        let latex = to_latex(&e);
        assert!(latex.contains(r"\rho"));
        assert!(latex.contains(r"\mu"));
    }

    #[test]
    fn gradient() {
        let e = parse("grad($T)").unwrap();
        let latex = to_latex(&e);
        assert!(latex.contains(r"\nabla"));
    }

    #[test]
    fn laplacian_two_arg() {
        let e = parse("laplacian($nu, $U)").unwrap();
        let latex = to_latex(&e);
        assert!(latex.contains(r"\nabla \cdot"));
        assert!(latex.contains(r"\nabla"));
    }

    #[test]
    fn time_derivative() {
        let e = parse("ddt($rho)").unwrap();
        let latex = to_latex(&e);
        assert!(latex.contains(r"\partial"));
    }

    #[test]
    fn power_and_sqrt() {
        let e = parse("sqrt(x ^ 2 + y ^ 2)").unwrap();
        let latex = to_latex(&e);
        assert!(latex.contains(r"\sqrt"));
    }

    #[test]
    fn constant_pi() {
        let e = parse("pi").unwrap();
        assert_eq!(to_latex(&e), r"\pi");
    }
}
