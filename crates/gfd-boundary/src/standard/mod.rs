//! Standard boundary condition implementations.

use gfd_core::mesh::face::Face;
use gfd_core::mesh::cell::Cell;
use crate::traits::{BoundaryCondition, BoundaryConditionType};

/// Large number used for the penalty method in Dirichlet conditions.
const LARGE_VALUE: f64 = 1.0e30;

// ---------------------------------------------------------------------------
// FixedValue (Dirichlet)
// ---------------------------------------------------------------------------

/// Fixed-value (Dirichlet) boundary condition.
///
/// Sets the field value at the boundary face to a prescribed value
/// using the penalty method: a_p += LARGE, b += LARGE * value.
#[derive(Debug, Clone)]
pub struct FixedValue {
    /// The prescribed boundary value.
    pub value: f64,
}

impl FixedValue {
    /// Creates a new fixed-value BC.
    pub fn new(value: f64) -> Self {
        Self { value }
    }
}

impl BoundaryCondition for FixedValue {
    fn apply_coefficients(
        &self,
        a_p: &mut f64,
        b: &mut f64,
        _face: &Face,
        _cell: &Cell,
    ) {
        *a_p += LARGE_VALUE;
        *b += LARGE_VALUE * self.value;
    }

    fn bc_type(&self) -> BoundaryConditionType {
        BoundaryConditionType::Dirichlet
    }

    fn name(&self) -> &str {
        "fixedValue"
    }
}

// ---------------------------------------------------------------------------
// FixedGradient (Neumann)
// ---------------------------------------------------------------------------

/// Fixed-gradient (Neumann) boundary condition.
///
/// Prescribes the normal gradient at the boundary: dphi/dn = gradient.
/// Contribution: b += gradient * face_area.
#[derive(Debug, Clone)]
pub struct FixedGradient {
    /// The prescribed normal gradient value.
    pub gradient: f64,
}

impl FixedGradient {
    /// Creates a new fixed-gradient BC.
    pub fn new(gradient: f64) -> Self {
        Self { gradient }
    }
}

impl BoundaryCondition for FixedGradient {
    fn apply_coefficients(
        &self,
        _a_p: &mut f64,
        b: &mut f64,
        face: &Face,
        _cell: &Cell,
    ) {
        *b += self.gradient * face.area;
    }

    fn bc_type(&self) -> BoundaryConditionType {
        BoundaryConditionType::Neumann
    }

    fn name(&self) -> &str {
        "fixedGradient"
    }
}

// ---------------------------------------------------------------------------
// ZeroGradient
// ---------------------------------------------------------------------------

/// Zero-gradient boundary condition.
///
/// Equivalent to FixedGradient(0). The boundary value is extrapolated
/// from the interior cell. No modification to coefficients is needed.
#[derive(Debug, Clone, Default)]
pub struct ZeroGradient;

impl BoundaryCondition for ZeroGradient {
    fn apply_coefficients(
        &self,
        _a_p: &mut f64,
        _b: &mut f64,
        _face: &Face,
        _cell: &Cell,
    ) {
        // No modification: value is extrapolated from the interior.
    }

    fn bc_type(&self) -> BoundaryConditionType {
        BoundaryConditionType::Neumann
    }

    fn name(&self) -> &str {
        "zeroGradient"
    }
}

// ---------------------------------------------------------------------------
// NoSlip
// ---------------------------------------------------------------------------

/// No-slip wall boundary condition for velocity.
///
/// Equivalent to FixedValue(0.0) for each velocity component.
#[derive(Debug, Clone, Default)]
pub struct NoSlip;

impl BoundaryCondition for NoSlip {
    fn apply_coefficients(
        &self,
        a_p: &mut f64,
        b: &mut f64,
        _face: &Face,
        _cell: &Cell,
    ) {
        // Same as FixedValue(0.0): penalty method with value = 0.
        *a_p += LARGE_VALUE;
        // b += LARGE_VALUE * 0.0 (no change to b needed).
        let _ = b; // Suppress unused warning; b is intentionally unchanged.
    }

    fn bc_type(&self) -> BoundaryConditionType {
        BoundaryConditionType::Dirichlet
    }

    fn name(&self) -> &str {
        "noSlip"
    }
}

// ---------------------------------------------------------------------------
// Symmetry
// ---------------------------------------------------------------------------

/// Symmetry plane boundary condition.
///
/// For scalar fields: equivalent to zero gradient.
/// For vector fields: the normal component is zeroed out.
/// This implementation handles the scalar/general case (zero gradient).
#[derive(Debug, Clone, Default)]
pub struct Symmetry;

impl BoundaryCondition for Symmetry {
    fn apply_coefficients(
        &self,
        _a_p: &mut f64,
        _b: &mut f64,
        _face: &Face,
        _cell: &Cell,
    ) {
        // For scalar equations, symmetry is equivalent to zero gradient.
        // Vector symmetry (zeroing normal component) is handled at a
        // higher level in the discretization.
    }

    fn bc_type(&self) -> BoundaryConditionType {
        BoundaryConditionType::Symmetry
    }

    fn name(&self) -> &str {
        "symmetry"
    }
}

// ---------------------------------------------------------------------------
// RobinBC
// ---------------------------------------------------------------------------

/// Robin (mixed) boundary condition.
///
/// a * phi + b_coeff * dphi/dn = c
///
/// This can represent a combination of Dirichlet and Neumann conditions.
/// Applied as:
///   a_p += a * face_area / (b_coeff + epsilon)
///   b   += c * face_area / (b_coeff + epsilon)
#[derive(Debug, Clone)]
pub struct RobinBC {
    /// Coefficient of phi.
    pub a: f64,
    /// Coefficient of dphi/dn.
    pub b_coeff: f64,
    /// Right-hand side constant.
    pub c: f64,
}

impl RobinBC {
    /// Creates a new Robin BC.
    pub fn new(a: f64, b_coeff: f64, c: f64) -> Self {
        Self { a, b_coeff, c }
    }
}

impl BoundaryCondition for RobinBC {
    fn apply_coefficients(
        &self,
        a_p: &mut f64,
        b: &mut f64,
        face: &Face,
        _cell: &Cell,
    ) {
        let denom = self.b_coeff.abs() + 1.0e-30; // Avoid division by zero.
        let ratio = face.area / denom;
        *a_p += self.a * ratio;
        *b += self.c * ratio;
    }

    fn bc_type(&self) -> BoundaryConditionType {
        BoundaryConditionType::Robin
    }

    fn name(&self) -> &str {
        "robin"
    }
}

// ---------------------------------------------------------------------------
// ConvectiveBC
// ---------------------------------------------------------------------------

/// Convective heat transfer boundary condition.
///
/// -k * dT/dn = h * (T - T_inf)
///
/// Applied as:
///   a_p += htc * face_area
///   b   += htc * face_area * t_inf
#[derive(Debug, Clone)]
pub struct ConvectiveBC {
    /// Heat transfer coefficient h [W/(m^2*K)].
    pub htc: f64,
    /// Free-stream / ambient temperature T_inf [K].
    pub t_inf: f64,
}

impl ConvectiveBC {
    /// Creates a new convective BC.
    pub fn new(htc: f64, t_inf: f64) -> Self {
        Self { htc, t_inf }
    }
}

impl BoundaryCondition for ConvectiveBC {
    fn apply_coefficients(
        &self,
        a_p: &mut f64,
        b: &mut f64,
        face: &Face,
        _cell: &Cell,
    ) {
        let ha = self.htc * face.area;
        *a_p += ha;
        *b += ha * self.t_inf;
    }

    fn bc_type(&self) -> BoundaryConditionType {
        BoundaryConditionType::Convective
    }

    fn name(&self) -> &str {
        "convective"
    }
}
