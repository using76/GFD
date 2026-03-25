//! Generic volume source terms.

use gfd_core::mesh::cell::Cell;
use crate::traits::{EquationId, LinearizedSource, ZoneFilter, SourceTerm};
use crate::Result;

/// A constant volume source term: S = value * volume.
///
/// The source is applied as a purely explicit term (sc = value * volume, sp = 0).
#[derive(Debug, Clone)]
pub struct ConstantVolumeSource {
    /// Source term value per unit volume [units depend on equation].
    pub value: f64,
    /// Target equation.
    pub equation: EquationId,
    /// Zone filter.
    pub zone: ZoneFilter,
}

impl ConstantVolumeSource {
    /// Creates a new constant volume source.
    pub fn new(value: f64, equation: EquationId) -> Self {
        Self {
            value,
            equation,
            zone: ZoneFilter::All,
        }
    }

    /// Sets the zone filter.
    pub fn with_zone(mut self, zone: ZoneFilter) -> Self {
        self.zone = zone;
        self
    }
}

impl SourceTerm for ConstantVolumeSource {
    fn compute(&self, _cell: &Cell, volume: f64) -> Result<LinearizedSource> {
        Ok(LinearizedSource {
            sc: self.value * volume,
            sp: 0.0,
        })
    }

    fn target_equation(&self) -> EquationId {
        self.equation.clone()
    }

    fn zone_filter(&self) -> &ZoneFilter {
        &self.zone
    }
}

/// An expression-based volume source term.
///
/// The expression is evaluated at each cell center to produce a source value.
#[derive(Debug, Clone)]
pub struct ExpressionSource {
    /// GMN expression string for the source value per unit volume.
    pub expr: String,
    /// Target equation.
    pub equation: EquationId,
    /// Zone filter.
    pub zone: ZoneFilter,
}

impl ExpressionSource {
    /// Creates a new expression source.
    pub fn new(expr: String, equation: EquationId) -> Self {
        Self {
            expr,
            equation,
            zone: ZoneFilter::All,
        }
    }

    /// Sets the zone filter.
    pub fn with_zone(mut self, zone: ZoneFilter) -> Self {
        self.zone = zone;
        self
    }
}

impl SourceTerm for ExpressionSource {
    fn compute(&self, _cell: &Cell, volume: f64) -> Result<LinearizedSource> {
        // Full implementation would:
        // 1. Parse self.expr via gfd_expression::parse
        // 2. Bind cell.center as (x, y, z) variables
        // 3. Evaluate to get a scalar value
        // 4. Optionally linearize around current solution
        //
        // Placeholder: treat the expression result as 0.
        let _expr_value = 0.0;
        Ok(LinearizedSource {
            sc: _expr_value * volume,
            sp: 0.0,
        })
    }

    fn target_equation(&self) -> EquationId {
        self.equation.clone()
    }

    fn zone_filter(&self) -> &ZoneFilter {
        &self.zone
    }
}
