//! Porous media source terms (Darcy-Forchheimer model).

use gfd_core::mesh::cell::Cell;
use crate::traits::{EquationId, LinearizedSource, ZoneFilter, SourceTerm};
use crate::Result;

/// Darcy-Forchheimer porous media model.
///
/// Source term per unit volume for the i-th momentum equation:
///
///   S_i = -(mu / alpha_i) * v_i - C2_i * 0.5 * rho * |v| * v_i
///
/// where alpha_i is the permeability (inverse of viscous_resistance)
/// and C2_i is the inertial resistance coefficient.
///
/// This is linearized as:
///   Sc = 0 (no explicit part)
///   Sp = -(mu / alpha_i + C2_i * 0.5 * rho * |v|) * volume
///
/// Since the velocity-dependent source depends on the current solution,
/// the caller must supply velocity information.
#[derive(Debug, Clone)]
pub struct DarcyForchheimer {
    /// Viscous resistance coefficients [1/m^2] for (x, y, z).
    pub viscous_resistance: [f64; 3],
    /// Inertial resistance coefficients [1/m] for (x, y, z).
    pub inertial_resistance: [f64; 3],
    /// Porosity (0-1, typically < 1).
    pub porosity: f64,
    /// Zone filter.
    pub zone: ZoneFilter,
    /// Direction this source applies to.
    direction: usize,
    /// Dynamic viscosity mu [Pa*s] (set at construction).
    mu: f64,
    /// Density rho [kg/m^3] (set at construction).
    rho: f64,
    /// Velocity magnitude |v| [m/s] (set at construction or updated).
    velocity_mag: f64,
}

impl DarcyForchheimer {
    /// Creates three porous media source terms (one per momentum direction).
    ///
    /// - `viscous_resistance`: [1/m^2] per direction.
    /// - `inertial_resistance`: [1/m] per direction.
    /// - `porosity`: porosity fraction.
    /// - `mu`: dynamic viscosity [Pa*s].
    /// - `rho`: density [kg/m^3].
    /// - `velocity_mag`: velocity magnitude [m/s].
    pub fn new(
        viscous_resistance: [f64; 3],
        inertial_resistance: [f64; 3],
        porosity: f64,
        mu: f64,
        rho: f64,
        velocity_mag: f64,
    ) -> [Self; 3] {
        [
            DarcyForchheimer {
                viscous_resistance,
                inertial_resistance,
                porosity,
                zone: ZoneFilter::All,
                direction: 0,
                mu,
                rho,
                velocity_mag,
            },
            DarcyForchheimer {
                viscous_resistance,
                inertial_resistance,
                porosity,
                zone: ZoneFilter::All,
                direction: 1,
                mu,
                rho,
                velocity_mag,
            },
            DarcyForchheimer {
                viscous_resistance,
                inertial_resistance,
                porosity,
                zone: ZoneFilter::All,
                direction: 2,
                mu,
                rho,
                velocity_mag,
            },
        ]
    }

    /// Updates the flow state (velocity magnitude, density, viscosity).
    pub fn update_state(&mut self, mu: f64, rho: f64, velocity_mag: f64) {
        self.mu = mu;
        self.rho = rho;
        self.velocity_mag = velocity_mag;
    }
}

impl SourceTerm for DarcyForchheimer {
    fn compute(&self, _cell: &Cell, volume: f64) -> Result<LinearizedSource> {
        let i = self.direction;

        // S_i = -(mu * D_i + C2_i * 0.5 * rho * |v|) * v_i
        // Linearized implicitly: Sp = -(mu * D_i + C2_i * 0.5 * rho * |v|) * volume
        let viscous_term = self.mu * self.viscous_resistance[i];
        let inertial_term = self.inertial_resistance[i] * 0.5 * self.rho * self.velocity_mag;

        let sp = -(viscous_term + inertial_term) * volume;

        Ok(LinearizedSource {
            sc: 0.0,
            sp,
        })
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
