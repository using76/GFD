//! Buoyancy source terms.

use gfd_core::mesh::cell::Cell;
use crate::traits::{EquationId, LinearizedSource, ZoneFilter, SourceTerm};
use crate::Result;

/// Boussinesq buoyancy approximation.
///
/// Source term for the i-th momentum equation:
///   S_i = -rho_ref * beta * (T - T_ref) * g_i
///
/// Linearized:
///   Sc = rho_ref * beta * T_ref * g_i * volume   (explicit part)
///   Sp = -rho_ref * beta * g_i * volume            (implicit part, coefficient of T)
///
/// This is appropriate when density variations are small but buoyancy
/// effects are significant.
#[derive(Debug, Clone)]
pub struct BoussinesqBuoyancy {
    /// Reference density [kg/m^3].
    pub rho_ref: f64,
    /// Thermal expansion coefficient [1/K].
    pub beta: f64,
    /// Reference temperature [K].
    pub t_ref: f64,
    /// Gravity vector [m/s^2].
    pub gravity: [f64; 3],
    /// Zone filter.
    pub zone: ZoneFilter,
    /// Which momentum direction this instance targets.
    direction: usize,
}

impl BoussinesqBuoyancy {
    /// Creates three buoyancy source terms (one per momentum direction).
    pub fn new(
        rho_ref: f64,
        beta: f64,
        t_ref: f64,
        gravity: [f64; 3],
    ) -> [Self; 3] {
        [
            BoussinesqBuoyancy {
                rho_ref, beta, t_ref, gravity,
                zone: ZoneFilter::All,
                direction: 0,
            },
            BoussinesqBuoyancy {
                rho_ref, beta, t_ref, gravity,
                zone: ZoneFilter::All,
                direction: 1,
            },
            BoussinesqBuoyancy {
                rho_ref, beta, t_ref, gravity,
                zone: ZoneFilter::All,
                direction: 2,
            },
        ]
    }
}

impl SourceTerm for BoussinesqBuoyancy {
    fn compute(&self, _cell: &Cell, volume: f64) -> Result<LinearizedSource> {
        let i = self.direction;
        let g_i = self.gravity[i];

        // S = -rho_ref * beta * (T - T_ref) * g_i
        //   = -rho_ref * beta * g_i * T + rho_ref * beta * T_ref * g_i
        //   = Sp * T + Sc
        //
        // Sc = rho_ref * beta * T_ref * g_i * volume
        // Sp = -rho_ref * beta * g_i * volume
        let sc = self.rho_ref * self.beta * self.t_ref * g_i * volume;
        let sp = -self.rho_ref * self.beta * g_i * volume;

        Ok(LinearizedSource { sc, sp })
    }

    fn target_equation(&self) -> EquationId {
        match self.direction {
            0 => EquationId::MomentumX,
            1 => EquationId::MomentumY,
            2 => EquationId::MomentumZ,
            _ => unreachable!(),
        }
    }

    fn zone_filter(&self) -> &ZoneFilter {
        &self.zone
    }
}

/// Full buoyancy model for variable-density flows.
///
/// Source term: S_i = (rho - rho_ref) * g_i
///
/// Used when density is computed from the equation of state (e.g., ideal gas)
/// rather than using the Boussinesq approximation.
#[derive(Debug, Clone)]
pub struct FullBuoyancy {
    /// Reference density for the hydrostatic pressure [kg/m^3].
    pub rho_ref: f64,
    /// Gravity vector [m/s^2].
    pub gravity: [f64; 3],
    /// Zone filter.
    pub zone: ZoneFilter,
    /// Which momentum direction this instance targets.
    direction: usize,
    /// Current local density [kg/m^3] (updated each iteration).
    rho_local: f64,
}

impl FullBuoyancy {
    /// Creates three full buoyancy source terms (one per momentum direction).
    pub fn new(rho_ref: f64, gravity: [f64; 3]) -> [Self; 3] {
        [
            FullBuoyancy {
                rho_ref, gravity,
                zone: ZoneFilter::All,
                direction: 0,
                rho_local: rho_ref,
            },
            FullBuoyancy {
                rho_ref, gravity,
                zone: ZoneFilter::All,
                direction: 1,
                rho_local: rho_ref,
            },
            FullBuoyancy {
                rho_ref, gravity,
                zone: ZoneFilter::All,
                direction: 2,
                rho_local: rho_ref,
            },
        ]
    }

    /// Updates the local density for the current cell.
    pub fn set_local_density(&mut self, rho: f64) {
        self.rho_local = rho;
    }
}

impl SourceTerm for FullBuoyancy {
    fn compute(&self, _cell: &Cell, volume: f64) -> Result<LinearizedSource> {
        let i = self.direction;
        let g_i = self.gravity[i];

        // S_i = (rho - rho_ref) * g_i * volume
        // Purely explicit source (no implicit linearization on velocity).
        let sc = (self.rho_local - self.rho_ref) * g_i * volume;

        Ok(LinearizedSource { sc, sp: 0.0 })
    }

    fn target_equation(&self) -> EquationId {
        match self.direction {
            0 => EquationId::MomentumX,
            1 => EquationId::MomentumY,
            2 => EquationId::MomentumZ,
            _ => unreachable!(),
        }
    }

    fn zone_filter(&self) -> &ZoneFilter {
        &self.zone
    }
}
