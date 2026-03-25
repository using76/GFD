// ---------------------------------------------------------------------------
// ast.rs  --  Abstract Syntax Tree node types for GFD expressions
// ---------------------------------------------------------------------------

use serde::{Deserialize, Serialize};

// ---- core expression tree ------------------------------------------------

/// Top-level expression node.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Expr {
    /// Literal floating-point number.
    Number(f64),
    /// Named variable (e.g. `x`, `T`).
    Variable(String),
    /// Field reference using `$` prefix (e.g. `$rho`, `$U`).
    FieldRef(String),
    /// Named mathematical constant (`pi`, `e`, …).
    Constant(String),
    /// Binary operation.
    BinaryOp {
        op: BinOp,
        left: Box<Expr>,
        right: Box<Expr>,
    },
    /// Unary operation / built-in function with a single argument.
    UnaryOp {
        op: UnOp,
        operand: Box<Expr>,
    },
    /// Generic function call (e.g. `max(a, b)`).
    FunctionCall {
        name: String,
        args: Vec<Expr>,
    },
    /// Ternary conditional: `if(cond, true_val, false_val)`.
    Conditional {
        condition: Box<Expr>,
        true_val: Box<Expr>,
        false_val: Box<Expr>,
    },
    /// Differential operator applied to operands.
    DiffOp {
        op: DiffOperator,
        operands: Vec<Expr>,
    },
    /// Tensor operator applied to operands.
    TensorOp {
        op: TensorOperator,
        operands: Vec<Expr>,
    },
}

// ---- operator enums ------------------------------------------------------

/// Binary arithmetic / power operators.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BinOp {
    Add,
    Sub,
    Mul,
    Div,
    Pow,
}

/// Unary operators / single-argument built-in functions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum UnOp {
    Neg,
    Abs,
    Sqrt,
    Sin,
    Cos,
    Exp,
    Log,
}

/// Differential operators used in PDE notation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DiffOperator {
    /// ∂/∂t
    TimeDerivative,
    /// ∇
    Gradient,
    /// ∇·
    Divergence,
    /// ∇²  (∇·∇)
    Laplacian,
    /// ∇×
    Curl,
}

/// Tensor algebra operators.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TensorOperator {
    Dot,
    Cross,
    Outer,
    Trace,
    Transpose,
    Symmetric,
    Skew,
    Magnitude,
    MagnitudeSqr,
    Determinant,
    Inverse,
}

// ---- dimensional analysis ------------------------------------------------

/// SI unit expressed as a product of base-dimension powers.
///
/// For example, velocity (m/s) is represented as `Unit { kg: 0, m: 1, s: -1, k: 0, mol: 0 }`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Unit {
    pub kg: i8,
    pub m: i8,
    pub s: i8,
    pub k: i8,
    pub mol: i8,
}

impl Unit {
    /// Dimensionless (all powers zero).
    pub const DIMENSIONLESS: Unit = Unit {
        kg: 0,
        m: 0,
        s: 0,
        k: 0,
        mol: 0,
    };

    pub const fn new(kg: i8, m: i8, s: i8, k: i8, mol: i8) -> Self {
        Unit { kg, m, s, k, mol }
    }

    /// Multiply two units (add exponents).
    pub const fn mul(self, other: Unit) -> Unit {
        Unit {
            kg: self.kg + other.kg,
            m: self.m + other.m,
            s: self.s + other.s,
            k: self.k + other.k,
            mol: self.mol + other.mol,
        }
    }

    /// Divide two units (subtract exponents).
    pub const fn div(self, other: Unit) -> Unit {
        Unit {
            kg: self.kg - other.kg,
            m: self.m - other.m,
            s: self.s - other.s,
            k: self.k - other.k,
            mol: self.mol - other.mol,
        }
    }

    /// Raise a unit to an integer power.
    pub const fn pow(self, n: i8) -> Unit {
        Unit {
            kg: self.kg * n,
            m: self.m * n,
            s: self.s * n,
            k: self.k * n,
            mol: self.mol * n,
        }
    }

    /// Returns `true` when the unit is dimensionless.
    pub const fn is_dimensionless(self) -> bool {
        self.kg == 0 && self.m == 0 && self.s == 0 && self.k == 0 && self.mol == 0
    }
}

// ---- convenience constructors --------------------------------------------

impl Expr {
    /// Shorthand for `Expr::Number`.
    pub fn num(v: f64) -> Self {
        Expr::Number(v)
    }

    /// Shorthand for `Expr::Variable`.
    pub fn var(name: &str) -> Self {
        Expr::Variable(name.to_string())
    }

    /// Shorthand for `Expr::FieldRef`.
    pub fn field(name: &str) -> Self {
        Expr::FieldRef(name.to_string())
    }

    /// Shorthand: build a binary op node.
    pub fn binop(op: BinOp, left: Expr, right: Expr) -> Self {
        Expr::BinaryOp {
            op,
            left: Box::new(left),
            right: Box::new(right),
        }
    }

    /// Shorthand: build a unary op node.
    pub fn unaryop(op: UnOp, operand: Expr) -> Self {
        Expr::UnaryOp {
            op,
            operand: Box::new(operand),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unit_arithmetic() {
        let m = Unit::new(0, 1, 0, 0, 0);
        let s = Unit::new(0, 0, 1, 0, 0);
        let velocity = m.div(s);
        assert_eq!(velocity, Unit::new(0, 1, -1, 0, 0));

        let accel = velocity.div(s);
        assert_eq!(accel, Unit::new(0, 1, -2, 0, 0));

        let area = m.pow(2);
        assert_eq!(area, Unit::new(0, 2, 0, 0, 0));
    }

    #[test]
    fn expr_construction() {
        let e = Expr::binop(BinOp::Add, Expr::num(1.0), Expr::var("x"));
        match &e {
            Expr::BinaryOp { op, left, right } => {
                assert_eq!(*op, BinOp::Add);
                assert_eq!(**left, Expr::Number(1.0));
                assert_eq!(**right, Expr::Variable("x".into()));
            }
            _ => panic!("unexpected variant"),
        }
    }

    #[test]
    fn serde_roundtrip() {
        let e = Expr::binop(
            BinOp::Mul,
            Expr::field("rho"),
            Expr::unaryop(UnOp::Sqrt, Expr::var("T")),
        );
        let json = serde_json::to_string(&e).unwrap();
        let e2: Expr = serde_json::from_str(&json).unwrap();
        assert_eq!(e, e2);
    }
}
