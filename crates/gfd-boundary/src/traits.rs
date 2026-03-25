//! Core traits and types for boundary conditions.

use gfd_core::mesh::face::Face;
use gfd_core::mesh::cell::Cell;
use serde::{Deserialize, Serialize};

/// Classification of a boundary condition.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum BoundaryConditionType {
    /// Fixed-value (Dirichlet) condition.
    Dirichlet,
    /// Fixed-gradient (Neumann) condition.
    Neumann,
    /// Robin (mixed) condition: a*phi + b*dphi/dn = c.
    Robin,
    /// Convective heat transfer condition.
    Convective,
    /// Radiative heat transfer condition.
    Radiative,
    /// Periodic (cyclic) condition.
    Periodic,
    /// Symmetry plane condition.
    Symmetry,
    /// User-defined custom condition.
    Custom(String),
}

/// Trait implemented by all boundary conditions.
///
/// Boundary conditions modify the linear system coefficients (a_p and b)
/// for the cell adjacent to the boundary face.
pub trait BoundaryCondition: std::fmt::Debug + Send + Sync {
    /// Applies this boundary condition by modifying the linear system
    /// coefficients for the owner cell of the given face.
    ///
    /// - `a_p`: diagonal coefficient of the cell equation (modified in place).
    /// - `b`: source term / right-hand side (modified in place).
    /// - `face`: the boundary face.
    /// - `cell`: the cell adjacent to the boundary face.
    fn apply_coefficients(
        &self,
        a_p: &mut f64,
        b: &mut f64,
        face: &Face,
        cell: &Cell,
    );

    /// Returns the type of this boundary condition.
    fn bc_type(&self) -> BoundaryConditionType;

    /// Returns the human-readable name of this boundary condition.
    fn name(&self) -> &str;
}
