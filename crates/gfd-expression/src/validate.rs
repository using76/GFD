// ---------------------------------------------------------------------------
// validate.rs  --  Multi-pass validation of Expr trees
// ---------------------------------------------------------------------------

use std::collections::HashSet;

use serde::{Deserialize, Serialize};

use crate::ast::*;
use crate::dimension::{check_dimensions, DimensionContext};

// ---------------------------------------------------------------------------
// diagnostic types
// ---------------------------------------------------------------------------

/// Severity level for a diagnostic.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DiagnosticLevel {
    Warning,
    Error,
}

/// A single diagnostic produced during validation.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Diagnostic {
    pub level: DiagnosticLevel,
    pub message: String,
    /// Optional textual span / path indicating where in the AST the issue was found.
    pub span: Option<String>,
}

impl Diagnostic {
    pub fn error(message: impl Into<String>) -> Self {
        Diagnostic {
            level: DiagnosticLevel::Error,
            message: message.into(),
            span: None,
        }
    }

    pub fn warning(message: impl Into<String>) -> Self {
        Diagnostic {
            level: DiagnosticLevel::Warning,
            message: message.into(),
            span: None,
        }
    }

    pub fn with_span(mut self, span: impl Into<String>) -> Self {
        self.span = Some(span.into());
        self
    }
}

// ---------------------------------------------------------------------------
// validation context
// ---------------------------------------------------------------------------

/// Context supplied to the validator so it knows which names exist.
#[derive(Debug, Clone, Default)]
pub struct ValidationContext {
    /// Set of known variable names.
    pub known_variables: HashSet<String>,
    /// Set of known field names (without the `$` prefix).
    pub known_fields: HashSet<String>,
    /// Optional dimension context for unit checking.
    pub dimensions: Option<DimensionContext>,
}

impl ValidationContext {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add_variable(&mut self, name: &str) -> &mut Self {
        self.known_variables.insert(name.to_string());
        self
    }

    pub fn add_field(&mut self, name: &str) -> &mut Self {
        self.known_fields.insert(name.to_string());
        self
    }

    pub fn with_dimensions(mut self, dim_ctx: DimensionContext) -> Self {
        self.dimensions = Some(dim_ctx);
        self
    }
}

// ---------------------------------------------------------------------------
// public API
// ---------------------------------------------------------------------------

/// Run all validation passes on `expr` and return any diagnostics.
///
/// Checks performed:
/// 1. **Variable existence** -- every `Variable` / `FieldRef` must be in the context.
/// 2. **Dimension consistency** -- if a `DimensionContext` is provided.
/// 3. **Division by zero** -- literal zero in the denominator.
/// 4. **Tensor rank** -- basic arity checks on tensor/diff operators.
pub fn validate(expr: &Expr, ctx: &ValidationContext) -> Vec<Diagnostic> {
    let mut diags = Vec::new();
    check_names(expr, ctx, &mut diags);
    check_division_by_zero(expr, &mut diags);
    check_operator_arity(expr, &mut diags);

    // Dimension check (optional)
    if let Some(ref dim_ctx) = ctx.dimensions {
        if let Err(e) = check_dimensions(expr, dim_ctx) {
            diags.push(Diagnostic::error(format!("dimension check: {e}")));
        }
    }

    diags
}

// ---------------------------------------------------------------------------
// individual checks
// ---------------------------------------------------------------------------

fn check_names(expr: &Expr, ctx: &ValidationContext, diags: &mut Vec<Diagnostic>) {
    match expr {
        Expr::Variable(name) => {
            if !ctx.known_variables.is_empty() && !ctx.known_variables.contains(name) {
                // Also allow well-known math names
                let known_math = ["pi", "e", "PI", "E_CONST"];
                if !known_math.contains(&name.as_str()) {
                    diags.push(Diagnostic::warning(format!("unknown variable `{name}`")));
                }
            }
        }
        Expr::FieldRef(name) => {
            if !ctx.known_fields.is_empty() && !ctx.known_fields.contains(name) {
                diags.push(Diagnostic::error(format!("unknown field `${name}`")));
            }
        }
        // recurse into children
        Expr::BinaryOp { left, right, .. } => {
            check_names(left, ctx, diags);
            check_names(right, ctx, diags);
        }
        Expr::UnaryOp { operand, .. } => check_names(operand, ctx, diags),
        Expr::FunctionCall { args, .. } => {
            for a in args {
                check_names(a, ctx, diags);
            }
        }
        Expr::Conditional {
            condition,
            true_val,
            false_val,
        } => {
            check_names(condition, ctx, diags);
            check_names(true_val, ctx, diags);
            check_names(false_val, ctx, diags);
        }
        Expr::DiffOp { operands, .. } | Expr::TensorOp { operands, .. } => {
            for o in operands {
                check_names(o, ctx, diags);
            }
        }
        _ => {}
    }
}

fn check_division_by_zero(expr: &Expr, diags: &mut Vec<Diagnostic>) {
    match expr {
        Expr::BinaryOp {
            op: BinOp::Div,
            right,
            left,
            ..
        } => {
            if let Expr::Number(v) = right.as_ref() {
                if *v == 0.0 {
                    diags.push(Diagnostic::error("division by literal zero"));
                }
            }
            check_division_by_zero(left, diags);
            check_division_by_zero(right, diags);
        }
        Expr::BinaryOp { left, right, .. } => {
            check_division_by_zero(left, diags);
            check_division_by_zero(right, diags);
        }
        Expr::UnaryOp { operand, .. } => check_division_by_zero(operand, diags),
        Expr::FunctionCall { args, .. } => {
            for a in args {
                check_division_by_zero(a, diags);
            }
        }
        Expr::Conditional {
            condition,
            true_val,
            false_val,
        } => {
            check_division_by_zero(condition, diags);
            check_division_by_zero(true_val, diags);
            check_division_by_zero(false_val, diags);
        }
        Expr::DiffOp { operands, .. } | Expr::TensorOp { operands, .. } => {
            for o in operands {
                check_division_by_zero(o, diags);
            }
        }
        _ => {}
    }
}

fn check_operator_arity(expr: &Expr, diags: &mut Vec<Diagnostic>) {
    match expr {
        Expr::DiffOp { op, operands } => {
            let expected = match op {
                DiffOperator::TimeDerivative => 1,
                DiffOperator::Gradient => 1,
                DiffOperator::Divergence => 1,
                DiffOperator::Curl => 1,
                DiffOperator::Laplacian => 2, // laplacian(gamma, phi)
            };
            if operands.len() != expected {
                diags.push(Diagnostic::error(format!(
                    "{op:?} expects {expected} operand(s), got {}",
                    operands.len()
                )));
            }
            for o in operands {
                check_operator_arity(o, diags);
            }
        }
        Expr::TensorOp { op, operands } => {
            let expected = match op {
                TensorOperator::Dot | TensorOperator::Cross | TensorOperator::Outer => 2,
                _ => 1, // unary tensor ops
            };
            if operands.len() != expected {
                diags.push(Diagnostic::error(format!(
                    "{op:?} expects {expected} operand(s), got {}",
                    operands.len()
                )));
            }
            for o in operands {
                check_operator_arity(o, diags);
            }
        }
        Expr::BinaryOp { left, right, .. } => {
            check_operator_arity(left, diags);
            check_operator_arity(right, diags);
        }
        Expr::UnaryOp { operand, .. } => check_operator_arity(operand, diags),
        Expr::FunctionCall { args, .. } => {
            for a in args {
                check_operator_arity(a, diags);
            }
        }
        Expr::Conditional {
            condition,
            true_val,
            false_val,
        } => {
            check_operator_arity(condition, diags);
            check_operator_arity(true_val, diags);
            check_operator_arity(false_val, diags);
        }
        _ => {}
    }
}

// ---------------------------------------------------------------------------
// tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::parse;

    #[test]
    fn unknown_field() {
        let e = parse("$rho + $unknown_field").unwrap();
        let mut ctx = ValidationContext::new();
        ctx.add_field("rho");
        let diags = validate(&e, &ctx);
        assert!(diags.iter().any(|d| d.message.contains("unknown_field")));
    }

    #[test]
    fn division_by_zero_detected() {
        let e = parse("x / 0").unwrap();
        let ctx = ValidationContext::new();
        let diags = validate(&e, &ctx);
        assert!(diags
            .iter()
            .any(|d| d.message.contains("division by literal zero")));
    }

    #[test]
    fn operator_arity_mismatch() {
        // grad expects 1 operand; give it 2
        let e = Expr::DiffOp {
            op: DiffOperator::Gradient,
            operands: vec![Expr::var("a"), Expr::var("b")],
        };
        let ctx = ValidationContext::new();
        let diags = validate(&e, &ctx);
        assert!(diags.iter().any(|d| d.level == DiagnosticLevel::Error));
    }

    #[test]
    fn valid_expression_no_diags() {
        let e = parse("$rho * $U + $p").unwrap();
        let mut ctx = ValidationContext::new();
        ctx.add_field("rho").add_field("U").add_field("p");
        // No dimension context → only name & syntax checks
        let diags = validate(&e, &ctx);
        assert!(diags.is_empty(), "unexpected diagnostics: {diags:?}");
    }
}
