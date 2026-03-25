//! Core traits and types for source terms.

use gfd_core::mesh::cell::Cell;
use serde::{Deserialize, Serialize};
use crate::Result;

/// Identifies which equation a source term contributes to.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum EquationId {
    /// Continuity (mass conservation).
    Continuity,
    /// X-momentum.
    MomentumX,
    /// Y-momentum.
    MomentumY,
    /// Z-momentum.
    MomentumZ,
    /// Energy equation.
    Energy,
    /// Turbulent kinetic energy.
    TurbulentKE,
    /// Turbulent dissipation rate (epsilon or omega).
    TurbulentDissipation,
    /// Species transport equation by index.
    Species(usize),
    /// User-defined equation.
    Custom(String),
}

/// A linearized source term: S(phi) = Sc + Sp * phi.
///
/// - `sc`: explicit (constant) part of the source.
/// - `sp`: implicit (coefficient of phi) part of the source.
///
/// For stability, `sp` should be negative (or zero).
#[derive(Debug, Clone, Copy, Default)]
pub struct LinearizedSource {
    /// Explicit part of the source (added to RHS).
    pub sc: f64,
    /// Implicit part of the source (coefficient of phi, added to matrix diagonal).
    pub sp: f64,
}

/// Filter that determines which cells a source term applies to.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ZoneFilter {
    /// Applies to all cells.
    All,
    /// Applies only to cells in the zone with the given ID.
    ZoneId(usize),
    /// Applies only to cells in the named zone.
    ZoneName(String),
    /// Applies to cells matching the given expression.
    Expression(String),
}

/// Trait implemented by all source terms.
pub trait SourceTerm: std::fmt::Debug + Send + Sync {
    /// Computes the linearized source contribution for a given cell.
    ///
    /// - `cell`: the mesh cell where the source is evaluated.
    /// - `volume`: the volume of the cell.
    ///
    /// Returns `LinearizedSource { sc, sp }` such that
    /// the total source for the cell is `Sc + Sp * phi_P`.
    fn compute(&self, cell: &Cell, volume: f64) -> Result<LinearizedSource>;

    /// Returns which equation this source term targets.
    fn target_equation(&self) -> EquationId;

    /// Returns the zone filter for this source term.
    fn zone_filter(&self) -> &ZoneFilter;
}
