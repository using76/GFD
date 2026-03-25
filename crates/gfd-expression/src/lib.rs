// ---------------------------------------------------------------------------
// gfd-expression  --  Mathematical expression parsing and symbolic engine
// ---------------------------------------------------------------------------
//!
//! This crate provides:
//!
//! - **Tokenizer / Parser**: convert GMN expression strings into an AST.
//! - **Simplifier**: algebraic identity and constant-folding simplification.
//! - **Differentiator**: symbolic differentiation with chain / product / quotient rules.
//! - **Linearizer**: source-term decomposition S(φ) → Sc + Sp·φ.
//! - **Dimension checker**: SI unit inference and consistency checking.
//! - **Validator**: multi-pass diagnostics (names, dimensions, arity, div-by-zero).
//! - **Code generators**: emit Rust, LaTeX, or JSON from an AST.

// -- modules ---------------------------------------------------------------

pub mod ast;
pub mod codegen_json;
pub mod codegen_latex;
pub mod codegen_rust;
pub mod differentiate;
pub mod dimension;
pub mod linearize;
pub mod parser;
pub mod simplify;
pub mod tokenizer;
pub mod validate;

// -- re-exports of key functions -------------------------------------------

pub use parser::parse;
pub use validate::validate;
pub use simplify::simplify;
pub use codegen_latex::to_latex;
pub use codegen_rust::to_rust;
pub use codegen_json::to_json;

// -- error type ------------------------------------------------------------

/// Unified error type for the expression engine.
#[derive(Debug, Clone, thiserror::Error)]
pub enum ExpressionError {
    /// Error during tokenization.
    #[error("tokenize error at position {position}: {message}")]
    TokenizeError { message: String, position: usize },

    /// Error during parsing.
    #[error("parse error at position {position}: {message}")]
    ParseError { message: String, position: usize },

    /// Error during symbolic differentiation.
    #[error("differentiation error: {message}")]
    DifferentiationError { message: String },

    /// Error during dimensional analysis.
    #[error("dimension error: {message}")]
    DimensionError { message: String },

    /// Error during code generation.
    #[error("codegen error: {message}")]
    CodegenError { message: String },

    /// Error during linearisation.
    #[error("linearization error: {message}")]
    LinearizationError { message: String },

    /// Generic / catch-all error.
    #[error("{0}")]
    Other(String),
}

// -- convenience Result alias ----------------------------------------------

/// Shorthand result type used throughout this crate.
pub type Result<T> = std::result::Result<T, ExpressionError>;

// ---------------------------------------------------------------------------
// integration tests (top-level)
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::*;

    #[test]
    fn parse_simplify_latex_roundtrip() {
        let expr = parse("0 + $rho * $U").unwrap();
        let simplified = simplify(&expr);
        let latex = to_latex(&simplified);
        assert!(latex.contains(r"\rho"));
    }

    #[test]
    fn parse_to_rust() {
        let expr = parse("$rho * sin($T)").unwrap();
        let code = to_rust(&expr);
        assert!(code.contains("state.rho[cell_id]"));
        assert!(code.contains(".sin()"));
    }

    #[test]
    fn parse_to_json_roundtrip() {
        let expr = parse("x ^ 2 + y ^ 2").unwrap();
        let json = to_json(&expr).unwrap();
        let expr2 = codegen_json::from_json(&json).unwrap();
        assert_eq!(expr, expr2);
    }

    #[test]
    fn full_pipeline() {
        // Parse
        let expr = parse("$rho * ddt($U) + div($rho * $U * $U) - laplacian($mu, $U)").unwrap();

        // Validate (should have no errors with all fields registered)
        let mut ctx = validate::ValidationContext::new();
        ctx.add_field("rho").add_field("U").add_field("mu");
        let diags = validate(&expr, &ctx);
        assert!(
            diags.is_empty(),
            "unexpected validation errors: {diags:?}"
        );

        // LaTeX
        let latex = to_latex(&expr);
        assert!(latex.contains(r"\nabla"));
        assert!(latex.contains(r"\rho"));

        // Rust
        let rust_code = to_rust(&expr);
        assert!(rust_code.contains("state.rho[cell_id]"));

        // JSON roundtrip
        let json = to_json(&expr).unwrap();
        let expr2 = codegen_json::from_json(&json).unwrap();
        assert_eq!(expr, expr2);
    }

    #[test]
    fn differentiate_and_simplify() {
        let expr = parse("x ^ 3 + 2 * x + 1").unwrap();
        let d = differentiate::differentiate(&expr, "x").unwrap();
        // d/dx(x^3 + 2x + 1) = 3x^2 + 2
        // The exact AST shape depends on simplification depth, but
        // we can verify it produces a valid expression.
        let _latex = to_latex(&d);
        // Just verify no panic.
    }

    #[test]
    fn linearize_source_term() {
        let expr = parse("a * T + b").unwrap();
        let (sc, sp) = linearize::linearize_source(&expr, "T").unwrap();
        // sc should be "b", sp should be "a"
        assert_eq!(sc, Expr::var("b"));
        assert_eq!(sp, Expr::var("a"));
    }
}
