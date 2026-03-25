//! # gfd-discretize
//!
//! Discretization methods for the GFD solver framework.
//! Provides Finite Volume Method (FVM) and Finite Element Method (FEM)
//! discretization, plus a pipeline for converting expression ASTs into
//! discrete linear equations.

pub mod fvm;
pub mod fem;
pub mod pipeline;

use gfd_expression::ast::Expr;
use serde::{Deserialize, Serialize};
use thiserror::Error;

// ---------------------------------------------------------------------------
// Error type
// ---------------------------------------------------------------------------

/// Errors that can occur during discretization.
#[derive(Debug, Error)]
pub enum DiscretizeError {
    #[error("Unsupported operator in expression: {0}")]
    UnsupportedOperator(String),

    #[error("Mesh topology error: {0}")]
    MeshTopology(String),

    #[error("Scheme not applicable: {0}")]
    SchemeNotApplicable(String),

    #[error("Invalid configuration: {0}")]
    InvalidConfiguration(String),

    #[error("Core error: {0}")]
    Core(#[from] gfd_core::CoreError),

    #[error("Expression error: {0}")]
    Expression(#[from] gfd_expression::ExpressionError),
}

/// Convenience result alias.
pub type Result<T> = std::result::Result<T, DiscretizeError>;

// ---------------------------------------------------------------------------
// Core data structures
// ---------------------------------------------------------------------------

/// A discretized equation for a single cell, representing:
///   a_p * phi_p + sum_nb(a_nb * phi_nb) = source
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiscreteEquation {
    /// The cell index this equation belongs to.
    pub cell_id: usize,
    /// The central coefficient a_P.
    pub a_p: f64,
    /// Neighbor contributions: (neighbor_cell_id, coefficient).
    pub neighbors: Vec<(usize, f64)>,
    /// Source term (right-hand side contribution for this cell).
    pub source: f64,
}

/// Classification of terms parsed from an expression AST.
///
/// The pipeline classifies each term in the governing equation so that
/// appropriate discretization schemes can be applied.
#[derive(Debug, Clone)]
pub struct TermClassification {
    /// Temporal derivative term (e.g. ddt(rho * phi)).
    pub temporal: Option<Expr>,
    /// Convective term (e.g. div(rho * U * phi)).
    pub convection: Option<Expr>,
    /// Diffusive term (e.g. laplacian(gamma, phi)).
    pub diffusion: Option<Expr>,
    /// Source terms (explicit contributions).
    pub sources: Vec<Expr>,
}

// ---------------------------------------------------------------------------
// Re-exports
// ---------------------------------------------------------------------------

pub use fvm::FvmSchemes;
pub use fem::ElementType;
pub use pipeline::DiscretizationPipeline;
