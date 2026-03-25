// ---------------------------------------------------------------------------
// parser.rs  --  Recursive-descent parser: token stream → Expr AST
// ---------------------------------------------------------------------------

use crate::ast::*;
use crate::tokenizer::{tokenize, KeywordKind, Token};
use crate::ExpressionError;

// ---------------------------------------------------------------------------
// public API
// ---------------------------------------------------------------------------

/// Parse an expression string into an [`Expr`] AST.
pub fn parse(input: &str) -> Result<Expr, ExpressionError> {
    let tokens = tokenize(input)?;
    let mut parser = Parser::new(tokens);
    let expr = parser.parse_expr()?;
    if parser.pos < parser.tokens.len() {
        return Err(ExpressionError::ParseError {
            message: format!(
                "unexpected token after expression: {:?}",
                parser.tokens[parser.pos]
            ),
            position: parser.pos,
        });
    }
    Ok(expr)
}

// ---------------------------------------------------------------------------
// internal parser state
// ---------------------------------------------------------------------------

struct Parser {
    tokens: Vec<Token>,
    pos: usize,
}

impl Parser {
    fn new(tokens: Vec<Token>) -> Self {
        Parser { tokens, pos: 0 }
    }

    // -- helpers -----------------------------------------------------------

    fn peek(&self) -> Option<&Token> {
        self.tokens.get(self.pos)
    }

    fn advance(&mut self) -> Option<Token> {
        if self.pos < self.tokens.len() {
            let tok = self.tokens[self.pos].clone();
            self.pos += 1;
            Some(tok)
        } else {
            None
        }
    }

    fn expect_token(&mut self, expected: &Token) -> Result<(), ExpressionError> {
        match self.advance() {
            Some(ref tok) if tok == expected => Ok(()),
            other => Err(ExpressionError::ParseError {
                message: format!("expected {expected:?}, got {other:?}"),
                position: self.pos,
            }),
        }
    }

    // -- grammar rules (lowest → highest precedence) -----------------------

    /// expr  =  additive
    fn parse_expr(&mut self) -> Result<Expr, ExpressionError> {
        self.parse_additive()
    }

    /// additive  =  multiplicative ( ('+' | '-') multiplicative )*
    fn parse_additive(&mut self) -> Result<Expr, ExpressionError> {
        let mut left = self.parse_multiplicative()?;
        loop {
            match self.peek() {
                Some(Token::Operator('+')) => {
                    self.advance();
                    let right = self.parse_multiplicative()?;
                    left = Expr::binop(BinOp::Add, left, right);
                }
                Some(Token::Operator('-')) => {
                    self.advance();
                    let right = self.parse_multiplicative()?;
                    left = Expr::binop(BinOp::Sub, left, right);
                }
                _ => break,
            }
        }
        Ok(left)
    }

    /// multiplicative  =  power ( ('*' | '/') power )*
    fn parse_multiplicative(&mut self) -> Result<Expr, ExpressionError> {
        let mut left = self.parse_power()?;
        loop {
            match self.peek() {
                Some(Token::Operator('*')) => {
                    self.advance();
                    let right = self.parse_power()?;
                    left = Expr::binop(BinOp::Mul, left, right);
                }
                Some(Token::Operator('/')) => {
                    self.advance();
                    let right = self.parse_power()?;
                    left = Expr::binop(BinOp::Div, left, right);
                }
                _ => break,
            }
        }
        Ok(left)
    }

    /// power  =  unary ( '^' power )?        (right-associative)
    fn parse_power(&mut self) -> Result<Expr, ExpressionError> {
        let base = self.parse_unary()?;
        if let Some(Token::Operator('^')) = self.peek() {
            self.advance();
            let exp = self.parse_power()?; // right-associative recursion
            Ok(Expr::binop(BinOp::Pow, base, exp))
        } else {
            Ok(base)
        }
    }

    /// unary  =  '-' unary  |  primary
    fn parse_unary(&mut self) -> Result<Expr, ExpressionError> {
        if let Some(Token::Operator('-')) = self.peek() {
            self.advance();
            let operand = self.parse_unary()?;
            // optimisation: fold double negation at parse time
            if let Expr::UnaryOp {
                op: UnOp::Neg,
                operand: inner,
            } = &operand
            {
                return Ok((**inner).clone());
            }
            Ok(Expr::unaryop(UnOp::Neg, operand))
        } else {
            self.parse_primary()
        }
    }

    /// primary  =  Number
    ///           |  FieldRef
    ///           |  'if' '(' expr ',' expr ',' expr ')'
    ///           |  Identifier '(' arg_list ')'      -- function call
    ///           |  Identifier                        -- variable / constant
    ///           |  '(' expr ')'
    fn parse_primary(&mut self) -> Result<Expr, ExpressionError> {
        match self.peek().cloned() {
            Some(Token::Number(v)) => {
                self.advance();
                Ok(Expr::Number(v))
            }
            Some(Token::FieldRef(name)) => {
                self.advance();
                Ok(Expr::FieldRef(name))
            }
            // keyword: if(cond, true_val, false_val)
            Some(Token::Keyword(KeywordKind::If)) => {
                self.advance();
                self.expect_token(&Token::LeftParen)?;
                let cond = self.parse_expr()?;
                self.expect_token(&Token::Comma)?;
                let tv = self.parse_expr()?;
                self.expect_token(&Token::Comma)?;
                let fv = self.parse_expr()?;
                self.expect_token(&Token::RightParen)?;
                Ok(Expr::Conditional {
                    condition: Box::new(cond),
                    true_val: Box::new(tv),
                    false_val: Box::new(fv),
                })
            }
            Some(Token::Identifier(name)) => {
                self.advance();
                // Is this a function call?
                if let Some(Token::LeftParen) = self.peek() {
                    self.advance(); // consume '('
                    let args = self.parse_arg_list()?;
                    self.expect_token(&Token::RightParen)?;
                    // Map well-known names to specific AST nodes
                    return self.build_call(name, args);
                }
                // Named constants
                match name.as_str() {
                    "pi" | "PI" => Ok(Expr::Constant("pi".into())),
                    "e" | "E_CONST" => Ok(Expr::Constant("e".into())),
                    _ => Ok(Expr::Variable(name)),
                }
            }
            Some(Token::LeftParen) => {
                self.advance();
                let inner = self.parse_expr()?;
                self.expect_token(&Token::RightParen)?;
                Ok(inner)
            }
            other => Err(ExpressionError::ParseError {
                message: format!("unexpected token in primary position: {other:?}"),
                position: self.pos,
            }),
        }
    }

    /// arg_list  =  ε  |  expr ( ',' expr )*
    fn parse_arg_list(&mut self) -> Result<Vec<Expr>, ExpressionError> {
        let mut args = Vec::new();
        if let Some(Token::RightParen) = self.peek() {
            return Ok(args);
        }
        args.push(self.parse_expr()?);
        while let Some(Token::Comma) = self.peek() {
            self.advance();
            args.push(self.parse_expr()?);
        }
        Ok(args)
    }

    // -- call resolution ---------------------------------------------------

    /// Map a function name + args to the appropriate AST node.
    fn build_call(&self, name: String, args: Vec<Expr>) -> Result<Expr, ExpressionError> {
        // ---- single-argument unary ops --------------------------------
        macro_rules! unary {
            ($op:expr, $args:expr) => {{
                if $args.len() != 1 {
                    return Err(ExpressionError::ParseError {
                        message: format!("`{}` expects 1 argument, got {}", name, $args.len()),
                        position: self.pos,
                    });
                }
                Ok(Expr::unaryop($op, $args.into_iter().next().unwrap()))
            }};
        }

        match name.as_str() {
            // unary math functions
            "sin" => unary!(UnOp::Sin, args),
            "cos" => unary!(UnOp::Cos, args),
            "exp" => unary!(UnOp::Exp, args),
            "log" | "ln" => unary!(UnOp::Log, args),
            "sqrt" => unary!(UnOp::Sqrt, args),
            "abs" => unary!(UnOp::Abs, args),
            "neg" => unary!(UnOp::Neg, args),

            // differential operators
            "ddt" | "time_derivative" => Ok(Expr::DiffOp {
                op: DiffOperator::TimeDerivative,
                operands: args,
            }),
            "grad" | "gradient" => Ok(Expr::DiffOp {
                op: DiffOperator::Gradient,
                operands: args,
            }),
            "div" | "divergence" => Ok(Expr::DiffOp {
                op: DiffOperator::Divergence,
                operands: args,
            }),
            "laplacian" => Ok(Expr::DiffOp {
                op: DiffOperator::Laplacian,
                operands: args,
            }),
            "curl" => Ok(Expr::DiffOp {
                op: DiffOperator::Curl,
                operands: args,
            }),

            // tensor operators
            "dot" => Ok(Expr::TensorOp {
                op: TensorOperator::Dot,
                operands: args,
            }),
            "cross" => Ok(Expr::TensorOp {
                op: TensorOperator::Cross,
                operands: args,
            }),
            "outer" => Ok(Expr::TensorOp {
                op: TensorOperator::Outer,
                operands: args,
            }),
            "tr" | "trace" => Ok(Expr::TensorOp {
                op: TensorOperator::Trace,
                operands: args,
            }),
            "transpose" => Ok(Expr::TensorOp {
                op: TensorOperator::Transpose,
                operands: args,
            }),
            "sym" | "symmetric" => Ok(Expr::TensorOp {
                op: TensorOperator::Symmetric,
                operands: args,
            }),
            "skew" => Ok(Expr::TensorOp {
                op: TensorOperator::Skew,
                operands: args,
            }),
            "mag" | "magnitude" => Ok(Expr::TensorOp {
                op: TensorOperator::Magnitude,
                operands: args,
            }),
            "magSqr" => Ok(Expr::TensorOp {
                op: TensorOperator::MagnitudeSqr,
                operands: args,
            }),
            "det" | "determinant" => Ok(Expr::TensorOp {
                op: TensorOperator::Determinant,
                operands: args,
            }),
            "inv" | "inverse" => Ok(Expr::TensorOp {
                op: TensorOperator::Inverse,
                operands: args,
            }),

            // everything else: generic function call
            _ => Ok(Expr::FunctionCall { name, args }),
        }
    }
}

// ---------------------------------------------------------------------------
// tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn literal_number() {
        assert_eq!(parse("42").unwrap(), Expr::Number(42.0));
    }

    #[test]
    fn simple_add() {
        let e = parse("1 + 2").unwrap();
        assert_eq!(
            e,
            Expr::binop(BinOp::Add, Expr::num(1.0), Expr::num(2.0))
        );
    }

    #[test]
    fn precedence_mul_add() {
        // 1 + 2 * 3  →  1 + (2*3)
        let e = parse("1 + 2 * 3").unwrap();
        assert_eq!(
            e,
            Expr::binop(
                BinOp::Add,
                Expr::num(1.0),
                Expr::binop(BinOp::Mul, Expr::num(2.0), Expr::num(3.0)),
            )
        );
    }

    #[test]
    fn precedence_pow() {
        // 2 ^ 3 ^ 2  →  2 ^ (3^2)  (right-assoc)
        let e = parse("2 ^ 3 ^ 2").unwrap();
        assert_eq!(
            e,
            Expr::binop(
                BinOp::Pow,
                Expr::num(2.0),
                Expr::binop(BinOp::Pow, Expr::num(3.0), Expr::num(2.0)),
            )
        );
    }

    #[test]
    fn parentheses() {
        let e = parse("(1 + 2) * 3").unwrap();
        assert_eq!(
            e,
            Expr::binop(
                BinOp::Mul,
                Expr::binop(BinOp::Add, Expr::num(1.0), Expr::num(2.0)),
                Expr::num(3.0),
            )
        );
    }

    #[test]
    fn negation() {
        let e = parse("-x").unwrap();
        assert_eq!(e, Expr::unaryop(UnOp::Neg, Expr::var("x")));
    }

    #[test]
    fn double_negation_folded() {
        let e = parse("--x").unwrap();
        assert_eq!(e, Expr::var("x"));
    }

    #[test]
    fn field_ref_in_expr() {
        let e = parse("$rho * $U").unwrap();
        assert_eq!(
            e,
            Expr::binop(BinOp::Mul, Expr::field("rho"), Expr::field("U"))
        );
    }

    #[test]
    fn function_sin() {
        let e = parse("sin(x)").unwrap();
        assert_eq!(e, Expr::unaryop(UnOp::Sin, Expr::var("x")));
    }

    #[test]
    fn function_laplacian() {
        let e = parse("laplacian($nu, $U)").unwrap();
        assert_eq!(
            e,
            Expr::DiffOp {
                op: DiffOperator::Laplacian,
                operands: vec![Expr::field("nu"), Expr::field("U")],
            }
        );
    }

    #[test]
    fn conditional() {
        let e = parse("if(x, 1, 0)").unwrap();
        assert_eq!(
            e,
            Expr::Conditional {
                condition: Box::new(Expr::var("x")),
                true_val: Box::new(Expr::num(1.0)),
                false_val: Box::new(Expr::num(0.0)),
            }
        );
    }

    #[test]
    fn complex_expression() {
        let e = parse("$rho * ddt($U) + div($rho * $U * $U) - laplacian($mu, $U)").unwrap();
        // Just ensure it parses without error and has the right top-level structure.
        match e {
            Expr::BinaryOp { op: BinOp::Sub, .. } => {} // top-level is subtraction
            other => panic!("unexpected top-level node: {other:?}"),
        }
    }

    #[test]
    fn constant_pi() {
        let e = parse("pi * r ^ 2").unwrap();
        match &e {
            Expr::BinaryOp { left, .. } => {
                assert_eq!(**left, Expr::Constant("pi".into()));
            }
            _ => panic!("unexpected"),
        }
    }

    #[test]
    fn generic_function() {
        let e = parse("max(a, b)").unwrap();
        assert_eq!(
            e,
            Expr::FunctionCall {
                name: "max".into(),
                args: vec![Expr::var("a"), Expr::var("b")],
            }
        );
    }

    #[test]
    fn err_trailing_token() {
        assert!(parse("1 + 2 )").is_err());
    }
}
