//! Momentum source terms (body forces).

use gfd_core::mesh::cell::Cell;
use crate::traits::{EquationId, LinearizedSource, ZoneFilter, SourceTerm};
use crate::Result;

/// A constant body force applied to the momentum equations.
///
/// Applies force components (fx, fy, fz) [N/m^3] to the respective
/// momentum equations. This struct produces three source terms,
/// one for each direction.
#[derive(Debug, Clone)]
pub struct BodyForce {
    /// Force per unit volume in x-direction [N/m^3].
    pub fx: f64,
    /// Force per unit volume in y-direction [N/m^3].
    pub fy: f64,
    /// Force per unit volume in z-direction [N/m^3].
    pub fz: f64,
    /// Zone filter.
    pub zone: ZoneFilter,
    /// Which momentum direction this instance targets.
    /// Use `into_source_terms()` to generate all three at once.
    direction: EquationId,
}

impl BodyForce {
    /// Creates a new body force and returns three source terms (x, y, z).
    pub fn new(fx: f64, fy: f64, fz: f64) -> [Self; 3] {
        [
            BodyForce {
                fx, fy, fz,
                zone: ZoneFilter::All,
                direction: EquationId::MomentumX,
            },
            BodyForce {
                fx, fy, fz,
                zone: ZoneFilter::All,
                direction: EquationId::MomentumY,
            },
            BodyForce {
                fx, fy, fz,
                zone: ZoneFilter::All,
                direction: EquationId::MomentumZ,
            },
        ]
    }

    /// Returns the force component for this direction.
    fn force_component(&self) -> f64 {
        match self.direction {
            EquationId::MomentumX => self.fx,
            EquationId::MomentumY => self.fy,
            EquationId::MomentumZ => self.fz,
            _ => 0.0,
        }
    }
}

impl SourceTerm for BodyForce {
    fn compute(&self, _cell: &Cell, volume: f64) -> Result<LinearizedSource> {
        Ok(LinearizedSource {
            sc: self.force_component() * volume,
            sp: 0.0,
        })
    }

    fn target_equation(&self) -> EquationId {
        self.direction.clone()
    }

    fn zone_filter(&self) -> &ZoneFilter {
        &self.zone
    }
}
