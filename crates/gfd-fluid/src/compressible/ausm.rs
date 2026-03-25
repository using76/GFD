//! AUSM+ (Advection Upstream Splitting Method) flux scheme.
//!
//! Splits the inviscid flux into convective and pressure contributions,
//! each using a separate upwinding approach based on the Mach number.

use super::ConservativeState;
use super::roe::ConservativeFlux;

/// AUSM+ flux splitting scheme.
///
/// The AUSM family splits the interface flux into:
/// F_{1/2} = M_{1/2} * a_{1/2} * Phi_{1/2} + P_{1/2}
///
/// where M_{1/2} is the interface Mach number from Mach splitting,
/// Phi contains the convective quantities, and P_{1/2} is the split pressure.
///
/// Reference: M.-S. Liou, "A Sequel to AUSM: AUSM+", J. Comp. Phys. 129, 1996.
pub struct AusmPlusFlux {
    /// Ratio of specific heats (gamma).
    pub gamma: f64,
    /// AUSM+ parameter alpha (default 3/16 for AUSM+).
    pub alpha: f64,
    /// AUSM+ parameter beta (default 1/8).
    pub beta: f64,
}

impl AusmPlusFlux {
    /// Creates a new AUSM+ flux calculator with default parameters.
    pub fn new(gamma: f64) -> Self {
        Self {
            gamma,
            alpha: 3.0 / 16.0,
            beta: 1.0 / 8.0,
        }
    }

    /// Mach number splitting function M_plus (M >= 0 contribution).
    ///
    /// For |M| <= 1:  M+ = 0.25*(M+1)^2 + beta*(M^2 - 1)^2
    /// For |M| > 1:   M+ = 0.5*(M + |M|)
    pub fn mach_plus(&self, mach: f64) -> f64 {
        if mach.abs() <= 1.0 {
            0.25 * (mach + 1.0).powi(2) + self.beta * (mach * mach - 1.0).powi(2)
        } else {
            0.5 * (mach + mach.abs())
        }
    }

    /// Mach number splitting function M_minus (M < 0 contribution).
    ///
    /// For |M| <= 1:  M- = -0.25*(M-1)^2 - beta*(M^2 - 1)^2
    /// For |M| > 1:   M- = 0.5*(M - |M|)
    pub fn mach_minus(&self, mach: f64) -> f64 {
        if mach.abs() <= 1.0 {
            -0.25 * (mach - 1.0).powi(2) - self.beta * (mach * mach - 1.0).powi(2)
        } else {
            0.5 * (mach - mach.abs())
        }
    }

    /// Pressure splitting function P_plus.
    ///
    /// For |M| <= 1:  P+ = 0.25*(M+1)^2*(2 - M) + alpha*M*(M^2 - 1)^2
    /// For |M| > 1:   P+ = 0.5*(1 + sign(M))
    pub fn pressure_plus(&self, mach: f64) -> f64 {
        if mach.abs() <= 1.0 {
            0.25 * (mach + 1.0).powi(2) * (2.0 - mach)
                + self.alpha * mach * (mach * mach - 1.0).powi(2)
        } else {
            if mach >= 0.0 { 1.0 } else { 0.0 }
        }
    }

    /// Pressure splitting function P_minus.
    ///
    /// For |M| <= 1:  P- = 0.25*(M-1)^2*(2 + M) - alpha*M*(M^2 - 1)^2
    /// For |M| > 1:   P- = 0.5*(1 - sign(M))
    pub fn pressure_minus(&self, mach: f64) -> f64 {
        if mach.abs() <= 1.0 {
            0.25 * (mach - 1.0).powi(2) * (2.0 + mach)
                - self.alpha * mach * (mach * mach - 1.0).powi(2)
        } else {
            if mach <= 0.0 { 1.0 } else { 0.0 }
        }
    }

    /// Computes the speed of sound at the interface.
    fn interface_speed_of_sound(&self, left: &ConservativeState, right: &ConservativeState) -> f64 {
        let p_l = left.pressure(self.gamma);
        let p_r = right.pressure(self.gamma);
        let a_l = (self.gamma * p_l / left.rho).sqrt();
        let a_r = (self.gamma * p_r / right.rho).sqrt();
        // Simple average for the interface speed of sound
        0.5 * (a_l + a_r)
    }

    /// Computes the AUSM+ numerical flux across a cell interface.
    ///
    /// Steps:
    /// 1. Compute the left/right normal velocities and Mach numbers
    /// 2. Split the interface Mach number: M_{1/2} = M+_L + M-_R
    /// 3. Split the interface pressure: p_{1/2} = P+_L * p_L + P-_R * p_R
    /// 4. Assemble the convective flux from the upwind state
    /// 5. Add the pressure flux contribution
    pub fn compute_flux(
        &self,
        left: &ConservativeState,
        right: &ConservativeState,
        face_normal: [f64; 3],
    ) -> ConservativeFlux {
        let vel_l = left.velocity();
        let vel_r = right.velocity();

        // Normal velocity components
        let u_n_l = vel_l[0] * face_normal[0] + vel_l[1] * face_normal[1] + vel_l[2] * face_normal[2];
        let u_n_r = vel_r[0] * face_normal[0] + vel_r[1] * face_normal[1] + vel_r[2] * face_normal[2];

        // Interface speed of sound and Mach numbers
        let a_half = self.interface_speed_of_sound(left, right);
        let mach_l = u_n_l / a_half;
        let mach_r = u_n_r / a_half;

        // Interface Mach number
        let mach_half = self.mach_plus(mach_l) + self.mach_minus(mach_r);

        // Interface pressure
        let p_l = left.pressure(self.gamma);
        let p_r = right.pressure(self.gamma);
        let p_half = self.pressure_plus(mach_l) * p_l + self.pressure_minus(mach_r) * p_r;

        // Select upwind state for convective flux
        let (rho, rho_u, rho_v, rho_w, rho_e) = if mach_half >= 0.0 {
            (left.rho, left.rho_u, left.rho_v, left.rho_w, left.rho_e + p_l)
        } else {
            (right.rho, right.rho_u, right.rho_v, right.rho_w, right.rho_e + p_r)
        };

        let mass_flux = a_half * mach_half;

        ConservativeFlux::new(
            mass_flux * rho,
            mass_flux * rho_u + p_half * face_normal[0],
            mass_flux * rho_v + p_half * face_normal[1],
            mass_flux * rho_w + p_half * face_normal[2],
            mass_flux * rho_e,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mach_splitting_sum() {
        let ausm = AusmPlusFlux::new(1.4);
        // For subsonic Mach, M+ + M- should equal M
        for &m in &[-0.8, -0.5, 0.0, 0.3, 0.9] {
            let sum = ausm.mach_plus(m) + ausm.mach_minus(m);
            assert!(
                (sum - m).abs() < 1e-12,
                "M+(M) + M-(M) should equal M, got {} for M={}",
                sum,
                m
            );
        }
    }

    #[test]
    fn test_pressure_splitting_sum() {
        let ausm = AusmPlusFlux::new(1.4);
        // P+ + P- should equal 1 for any Mach number
        for &m in &[-2.0, -0.8, 0.0, 0.5, 1.5] {
            let sum = ausm.pressure_plus(m) + ausm.pressure_minus(m);
            assert!(
                (sum - 1.0).abs() < 1e-12,
                "P+(M) + P-(M) should equal 1, got {} for M={}",
                sum,
                m
            );
        }
    }
}
