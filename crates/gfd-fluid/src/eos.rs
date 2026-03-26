//! Equation of State (EOS) models for compressible flow.
//!
//! Provides thermodynamic closures relating pressure, density, temperature,
//! internal energy, and speed of sound.

/// Equation of state models for computing thermodynamic properties.
///
/// Each variant implements the full thermodynamic closure required by the
/// compressible flow solver.
#[derive(Debug, Clone)]
pub enum EquationOfState {
    /// Constant density (incompressible assumption).
    IncompressibleConstant {
        /// Fixed density [kg/m^3].
        density: f64,
    },

    /// Ideal (calorically perfect) gas: p = rho * R * T.
    IdealGas {
        /// Specific gas constant R [J/(kg*K)].
        gas_constant: f64,
        /// Ratio of specific heats gamma = cp/cv.
        specific_heat_ratio: f64,
    },

    /// Stiffened gas EOS: p = (gamma - 1) * rho * e - gamma * p_inf
    StiffenedGas {
        /// Effective ratio of specific heats.
        gamma: f64,
        /// Stiffening pressure [Pa].
        p_inf: f64,
    },

    /// Barotropic liquid: rho = rho0 + (p - p0) / c0^2
    BarotropicLiquid {
        /// Reference density [kg/m^3].
        rho0: f64,
        /// Reference speed of sound [m/s].
        c0: f64,
        /// Reference pressure [Pa].
        p0: f64,
    },
}

impl EquationOfState {
    /// Computes density from pressure and temperature.
    pub fn density(&self, pressure: f64, temperature: f64) -> f64 {
        match self {
            EquationOfState::IncompressibleConstant { density } => *density,
            EquationOfState::IdealGas { gas_constant, .. } => {
                pressure / (gas_constant * temperature)
            }
            EquationOfState::StiffenedGas { gamma, p_inf } => {
                let cv = 1.0;
                (pressure + gamma * p_inf) / ((gamma - 1.0) * cv * temperature)
            }
            EquationOfState::BarotropicLiquid { rho0, c0, p0 } => {
                rho0 + (pressure - p0) / (c0 * c0)
            }
        }
    }

    /// Computes pressure from density and internal energy.
    pub fn pressure(&self, density: f64, internal_energy: f64) -> f64 {
        match self {
            EquationOfState::IncompressibleConstant { .. } => 0.0,
            EquationOfState::IdealGas { specific_heat_ratio, .. } => {
                density * (specific_heat_ratio - 1.0) * internal_energy
            }
            EquationOfState::StiffenedGas { gamma, p_inf } => {
                (gamma - 1.0) * density * internal_energy - gamma * p_inf
            }
            EquationOfState::BarotropicLiquid { rho0, c0, p0 } => {
                p0 + (density - rho0) * c0 * c0
            }
        }
    }

    /// Computes temperature from density and pressure.
    pub fn temperature(&self, density: f64, pressure: f64) -> f64 {
        match self {
            EquationOfState::IncompressibleConstant { .. } => 0.0,
            EquationOfState::IdealGas { gas_constant, .. } => {
                pressure / (density * gas_constant)
            }
            EquationOfState::StiffenedGas { gamma, p_inf } => {
                let cv = 1.0;
                (pressure + gamma * p_inf) / ((gamma - 1.0) * density * cv)
            }
            EquationOfState::BarotropicLiquid { .. } => 0.0,
        }
    }

    /// Computes speed of sound from density and pressure.
    pub fn speed_of_sound(&self, density: f64, pressure: f64) -> f64 {
        match self {
            EquationOfState::IncompressibleConstant { .. } => f64::INFINITY,
            EquationOfState::IdealGas { specific_heat_ratio, .. } => {
                (specific_heat_ratio * pressure / density).sqrt()
            }
            EquationOfState::StiffenedGas { gamma, p_inf } => {
                (gamma * (pressure + p_inf) / density).sqrt()
            }
            EquationOfState::BarotropicLiquid { c0, .. } => *c0,
        }
    }

    /// Computes specific internal energy from density and pressure.
    pub fn internal_energy(&self, density: f64, pressure: f64) -> f64 {
        match self {
            EquationOfState::IncompressibleConstant { .. } => 0.0,
            EquationOfState::IdealGas { specific_heat_ratio, .. } => {
                pressure / (density * (specific_heat_ratio - 1.0))
            }
            EquationOfState::StiffenedGas { gamma, p_inf } => {
                (pressure + gamma * p_inf) / ((gamma - 1.0) * density)
            }
            EquationOfState::BarotropicLiquid { rho0, c0, .. } => {
                pressure / density + 0.5 * c0 * c0 * ((density / rho0) - 1.0).powi(2) / density
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ideal_gas_density_from_pressure_temperature() {
        let eos = EquationOfState::IdealGas { gas_constant: 287.0, specific_heat_ratio: 1.4 };
        let rho = eos.density(101325.0, 288.15);
        let expected = 101325.0 / (287.0 * 288.15);
        assert!((rho - expected).abs() < 1e-6);
    }

    #[test]
    fn ideal_gas_pressure_from_density_energy() {
        let eos = EquationOfState::IdealGas { gas_constant: 287.0, specific_heat_ratio: 1.4 };
        let e = 101325.0 / (1.225 * 0.4);
        let p = eos.pressure(1.225, e);
        assert!((p - 101325.0).abs() < 1.0);
    }

    #[test]
    fn ideal_gas_temperature_from_density_pressure() {
        let eos = EquationOfState::IdealGas { gas_constant: 287.0, specific_heat_ratio: 1.4 };
        let t = eos.temperature(1.225, 101325.0);
        let expected = 101325.0 / (1.225 * 287.0);
        assert!((t - expected).abs() < 1e-6);
    }

    #[test]
    fn ideal_gas_speed_of_sound() {
        let eos = EquationOfState::IdealGas { gas_constant: 287.0, specific_heat_ratio: 1.4 };
        let c = eos.speed_of_sound(1.225, 101325.0);
        let expected = (1.4_f64 * 101325.0 / 1.225).sqrt();
        assert!((c - expected).abs() < 1e-6);
    }

    #[test]
    fn ideal_gas_internal_energy() {
        let eos = EquationOfState::IdealGas { gas_constant: 287.0, specific_heat_ratio: 1.4 };
        let e = eos.internal_energy(1.225, 101325.0);
        let expected = 101325.0 / (1.225 * 0.4);
        assert!((e - expected).abs() < 1e-6);
    }

    #[test]
    fn ideal_gas_round_trip_pressure() {
        let eos = EquationOfState::IdealGas { gas_constant: 287.0, specific_heat_ratio: 1.4 };
        let e = eos.internal_energy(2.5, 200000.0);
        let p = eos.pressure(2.5, e);
        assert!((p - 200000.0).abs() < 1e-6);
    }

    #[test]
    fn ideal_gas_round_trip_density() {
        let eos = EquationOfState::IdealGas { gas_constant: 287.0, specific_heat_ratio: 1.4 };
        let t = eos.temperature(1.225, 101325.0);
        let rho = eos.density(101325.0, t);
        assert!((rho - 1.225).abs() < 1e-10);
    }

    #[test]
    fn incompressible_constant_density() {
        let eos = EquationOfState::IncompressibleConstant { density: 998.0 };
        assert_eq!(eos.density(101325.0, 300.0), 998.0);
        assert_eq!(eos.density(0.0, 0.0), 998.0);
    }

    #[test]
    fn incompressible_speed_of_sound_infinite() {
        let eos = EquationOfState::IncompressibleConstant { density: 998.0 };
        assert!(eos.speed_of_sound(998.0, 101325.0).is_infinite());
    }

    #[test]
    fn stiffened_gas_pressure_from_density_energy() {
        let eos = EquationOfState::StiffenedGas { gamma: 4.4, p_inf: 6.0e8 };
        let p = eos.pressure(1000.0, 1.0e6);
        let expected = 3.4 * 1000.0 * 1.0e6 - 4.4 * 6.0e8;
        assert!((p - expected).abs() < 1.0);
    }

    #[test]
    fn stiffened_gas_speed_of_sound() {
        let eos = EquationOfState::StiffenedGas { gamma: 4.4, p_inf: 6.0e8 };
        let c = eos.speed_of_sound(1000.0, 101325.0);
        let expected = (4.4_f64 * (101325.0 + 6.0e8) / 1000.0).sqrt();
        assert!((c - expected).abs() < 1e-6);
    }

    #[test]
    fn stiffened_gas_round_trip_pressure() {
        let eos = EquationOfState::StiffenedGas { gamma: 4.4, p_inf: 6.0e8 };
        let e = eos.internal_energy(1000.0, 1.0e9);
        let p = eos.pressure(1000.0, e);
        assert!((p - 1.0e9).abs() < 1.0);
    }

    #[test]
    fn barotropic_density_at_reference() {
        let eos = EquationOfState::BarotropicLiquid { rho0: 998.0, c0: 1500.0, p0: 101325.0 };
        let rho = eos.density(101325.0, 300.0);
        assert!((rho - 998.0).abs() < 1e-10);
    }

    #[test]
    fn barotropic_density_above_reference() {
        let eos = EquationOfState::BarotropicLiquid { rho0: 998.0, c0: 1500.0, p0: 101325.0 };
        let dp = 1.0e6;
        let rho = eos.density(101325.0 + dp, 300.0);
        let expected = 998.0 + dp / (1500.0 * 1500.0);
        assert!((rho - expected).abs() < 1e-10);
    }

    #[test]
    fn barotropic_pressure_from_density() {
        let eos = EquationOfState::BarotropicLiquid { rho0: 998.0, c0: 1500.0, p0: 101325.0 };
        let p = eos.pressure(998.5, 0.0);
        let expected = 101325.0 + 0.5 * 1500.0 * 1500.0;
        assert!((p - expected).abs() < 1e-6);
    }

    #[test]
    fn barotropic_speed_of_sound_constant() {
        let eos = EquationOfState::BarotropicLiquid { rho0: 998.0, c0: 1500.0, p0: 101325.0 };
        assert_eq!(eos.speed_of_sound(998.0, 101325.0), 1500.0);
        assert_eq!(eos.speed_of_sound(1000.0, 200000.0), 1500.0);
    }

    #[test]
    fn barotropic_round_trip_pressure() {
        let eos = EquationOfState::BarotropicLiquid { rho0: 998.0, c0: 1500.0, p0: 101325.0 };
        let rho = eos.density(500000.0, 300.0);
        let p = eos.pressure(rho, 0.0);
        assert!((p - 500000.0).abs() < 1e-6);
    }
}
