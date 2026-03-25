// ---------------------------------------------------------------------------
// tokenizer.rs  --  Lexical scanner for GFD / GMN expression strings
// ---------------------------------------------------------------------------

use crate::ExpressionError;
use serde::{Deserialize, Serialize};

/// A single lexical token produced by the scanner.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Token {
    /// Numeric literal (integer or float, incl. scientific notation).
    Number(f64),
    /// Plain identifier (`x`, `rho`, `sin`, …).
    Identifier(String),
    /// Field reference starting with `$` (the `$` is stripped; value is the name).
    FieldRef(String),
    /// Arithmetic / comparison operator character.
    Operator(char),
    /// `(`
    LeftParen,
    /// `)`
    RightParen,
    /// `,`
    Comma,
    /// `=`
    Equals,
    /// `;`
    Semicolon,
    /// Reserved keyword.
    Keyword(KeywordKind),
}

/// Keywords recognised by the tokenizer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum KeywordKind {
    Const,
    If,
    Switch,
}

// ---------------------------------------------------------------------------
// public API
// ---------------------------------------------------------------------------

/// Tokenize an expression string into a sequence of [`Token`]s.
pub fn tokenize(input: &str) -> Result<Vec<Token>, ExpressionError> {
    let chars: Vec<char> = input.chars().collect();
    let len = chars.len();
    let mut tokens: Vec<Token> = Vec::new();
    let mut pos: usize = 0;

    while pos < len {
        let ch = chars[pos];

        // --- skip whitespace -------------------------------------------
        if ch.is_ascii_whitespace() {
            pos += 1;
            continue;
        }

        // --- single-line comments with // ------------------------------
        if ch == '/' && pos + 1 < len && chars[pos + 1] == '/' {
            // consume until end of line
            while pos < len && chars[pos] != '\n' {
                pos += 1;
            }
            continue;
        }

        // --- field reference ($identifier) -----------------------------
        if ch == '$' {
            pos += 1; // skip '$'
            let start = pos;
            while pos < len && (chars[pos].is_alphanumeric() || chars[pos] == '_') {
                pos += 1;
            }
            if pos == start {
                return Err(ExpressionError::TokenizeError {
                    message: "expected identifier after '$'".into(),
                    position: start,
                });
            }
            let name: String = chars[start..pos].iter().collect();
            tokens.push(Token::FieldRef(name));
            continue;
        }

        // --- numeric literal -------------------------------------------
        if ch.is_ascii_digit() || (ch == '.' && pos + 1 < len && chars[pos + 1].is_ascii_digit()) {
            let start = pos;
            // integer part
            while pos < len && chars[pos].is_ascii_digit() {
                pos += 1;
            }
            // fractional part
            if pos < len && chars[pos] == '.' {
                pos += 1;
                while pos < len && chars[pos].is_ascii_digit() {
                    pos += 1;
                }
            }
            // exponent part (e / E)
            if pos < len && (chars[pos] == 'e' || chars[pos] == 'E') {
                pos += 1;
                if pos < len && (chars[pos] == '+' || chars[pos] == '-') {
                    pos += 1;
                }
                if pos >= len || !chars[pos].is_ascii_digit() {
                    return Err(ExpressionError::TokenizeError {
                        message: "expected digit in exponent".into(),
                        position: pos,
                    });
                }
                while pos < len && chars[pos].is_ascii_digit() {
                    pos += 1;
                }
            }
            let text: String = chars[start..pos].iter().collect();
            let value: f64 = text.parse().map_err(|_| ExpressionError::TokenizeError {
                message: format!("invalid number literal `{text}`"),
                position: start,
            })?;
            tokens.push(Token::Number(value));
            continue;
        }

        // --- identifier / keyword --------------------------------------
        if ch.is_ascii_alphabetic() || ch == '_' {
            let start = pos;
            while pos < len && (chars[pos].is_alphanumeric() || chars[pos] == '_') {
                pos += 1;
            }
            let word: String = chars[start..pos].iter().collect();
            let token = match word.as_str() {
                "const" => Token::Keyword(KeywordKind::Const),
                "if" => Token::Keyword(KeywordKind::If),
                "switch" => Token::Keyword(KeywordKind::Switch),
                _ => Token::Identifier(word),
            };
            tokens.push(token);
            continue;
        }

        // --- single-character tokens -----------------------------------
        match ch {
            '(' => {
                tokens.push(Token::LeftParen);
                pos += 1;
            }
            ')' => {
                tokens.push(Token::RightParen);
                pos += 1;
            }
            ',' => {
                tokens.push(Token::Comma);
                pos += 1;
            }
            '=' => {
                tokens.push(Token::Equals);
                pos += 1;
            }
            ';' => {
                tokens.push(Token::Semicolon);
                pos += 1;
            }
            '+' | '-' | '*' | '/' | '^' => {
                tokens.push(Token::Operator(ch));
                pos += 1;
            }
            _ => {
                return Err(ExpressionError::TokenizeError {
                    message: format!("unexpected character `{ch}`"),
                    position: pos,
                });
            }
        }
    }

    Ok(tokens)
}

// ---------------------------------------------------------------------------
// tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn basic_expression() {
        let tokens = tokenize("1 + 2 * x").unwrap();
        assert_eq!(
            tokens,
            vec![
                Token::Number(1.0),
                Token::Operator('+'),
                Token::Number(2.0),
                Token::Operator('*'),
                Token::Identifier("x".into()),
            ]
        );
    }

    #[test]
    fn field_ref() {
        let tokens = tokenize("$rho * $U").unwrap();
        assert_eq!(
            tokens,
            vec![
                Token::FieldRef("rho".into()),
                Token::Operator('*'),
                Token::FieldRef("U".into()),
            ]
        );
    }

    #[test]
    fn scientific_notation() {
        let tokens = tokenize("3.14e-2 + 1E+3").unwrap();
        assert_eq!(
            tokens,
            vec![
                Token::Number(3.14e-2),
                Token::Operator('+'),
                Token::Number(1e3),
            ]
        );
    }

    #[test]
    fn function_call() {
        let tokens = tokenize("sin(x) + cos(y)").unwrap();
        assert_eq!(
            tokens,
            vec![
                Token::Identifier("sin".into()),
                Token::LeftParen,
                Token::Identifier("x".into()),
                Token::RightParen,
                Token::Operator('+'),
                Token::Identifier("cos".into()),
                Token::LeftParen,
                Token::Identifier("y".into()),
                Token::RightParen,
            ]
        );
    }

    #[test]
    fn keywords() {
        let tokens = tokenize("if(x, 1, 0)").unwrap();
        assert_eq!(tokens[0], Token::Keyword(KeywordKind::If));
    }

    #[test]
    fn err_bare_dollar() {
        assert!(tokenize("$ + 1").is_err());
    }
}
