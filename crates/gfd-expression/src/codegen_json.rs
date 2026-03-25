// ---------------------------------------------------------------------------
// codegen_json.rs  --  Serialize Expr AST to JSON via serde
// ---------------------------------------------------------------------------

use crate::ast::Expr;
use crate::ExpressionError;

/// Serialize an expression AST to a pretty-printed JSON string.
pub fn to_json(expr: &Expr) -> Result<String, ExpressionError> {
    serde_json::to_string_pretty(expr).map_err(|e| ExpressionError::CodegenError {
        message: format!("JSON serialization failed: {e}"),
    })
}

/// Deserialize an expression AST from a JSON string.
pub fn from_json(json: &str) -> Result<Expr, ExpressionError> {
    serde_json::from_str(json).map_err(|e| ExpressionError::CodegenError {
        message: format!("JSON deserialization failed: {e}"),
    })
}

// ---------------------------------------------------------------------------
// tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::parse;

    #[test]
    fn roundtrip() {
        let expr = parse("$rho * $U + sin(x)").unwrap();
        let json = to_json(&expr).unwrap();
        let expr2 = from_json(&json).unwrap();
        assert_eq!(expr, expr2);
    }

    #[test]
    fn json_contains_expected_keys() {
        let expr = parse("1 + 2").unwrap();
        let json = to_json(&expr).unwrap();
        assert!(json.contains("BinaryOp"));
        assert!(json.contains("Add"));
    }
}
