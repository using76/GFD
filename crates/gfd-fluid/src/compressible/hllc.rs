//! HLLC (Harten-Lax-van Leer-Contact) approximate Riemann solver.
//!
//! An extension of the HLL solver that restores the contact wave,
//! providing better resolution of contact discontinuities and shear layers.

use super::ConservativeState;
use super::roe::ConservativeFlux;

/// HLLC approximate Riemann solver.
///
/// Uses signal speed estimates S_L, S_R, and contact speed S_star to
/// define three wave-separated regions. Provides exact resolution of
/// isolated contact and shear waves.
///
/// Reference: E.F. Toro, "Riemann Solvers and Numerical Methods for
/// Fluid Dynamics", 3rd ed., Springer, 2009, Chapter 10.
pub struct HllcFlux {
    /// Ratio of specific heats (gamma).
    pub gamma: f64,
}

impl HllcFlux {
    /// Creates a new HLLC flux calculator.
    pub fn new(gamma: f64) -> Self {
        Self { gamma }
    }

    /// Estimates the wave speeds S_L, S_R, and S_star.
    ///
    /// Uses the pressure-based estimate from Toro (Section 10.5):
    /// - S_L = min(u_L - a_L, u_tilde - a_tilde)
    /// - S_R = max(u_R + a_R, u_tilde + a_tilde)
    /// - S_star = (p_R - p_L + rho_L*u_L*(S_L - u_L) - rho_R*u_R*(S_R - u_R))
    ///            / (rho_L*(S_L - u_L) - rho_R*(S_R - u_R))
    fn estimate_signal_speeds(
        &self,
        left: &ConservativeState,
        right: &ConservativeState,
        normal: [f64; 3],
    ) -> (f64, f64, f64) {
        let vel_l = left.velocity();
        let vel_r = right.velocity();

        // Normal velocity components
        let u_l = vel_l[0] * normal[0] + vel_l[1] * normal[1] + vel_l[2] * normal[2];
        let u_r = vel_r[0] * normal[0] + vel_r[1] * normal[1] + vel_r[2] * normal[2];

        // Pressures and sound speeds
        let p_l = left.pressure(self.gamma);
        let p_r = right.pressure(self.gamma);
        let a_l = (self.gamma * p_l / left.rho).sqrt();
        let a_r = (self.gamma * p_r / right.rho).sqrt();

        // Direct wave speed estimates (Davis)
        let s_l = (u_l - a_l).min(u_r - a_r);
        let s_r = (u_l + a_l).max(u_r + a_r);

        // Contact wave speed
        let numer = p_r - p_l + left.rho * u_l * (s_l - u_l) - right.rho * u_r * (s_r - u_r);
        let denom = left.rho * (s_l - u_l) - right.rho * (s_r - u_r);
        let s_star = if denom.abs() > 1e-30 {
            numer / denom
        } else {
            0.5 * (u_l + u_r)
        };

        (s_l, s_r, s_star)
    }

    /// Computes the physical flux in the given normal direction from a state.
    fn physical_flux(&self, state: &ConservativeState, normal: [f64; 3]) -> [f64; 5] {
        let vel = state.velocity();
        let u_n = vel[0] * normal[0] + vel[1] * normal[1] + vel[2] * normal[2];
        let p = state.pressure(self.gamma);

        [
            state.rho * u_n,
            state.rho_u * u_n + p * normal[0],
            state.rho_v * u_n + p * normal[1],
            state.rho_w * u_n + p * normal[2],
            (state.rho_e + p) * u_n,
        ]
    }

    /// Computes the HLLC numerical flux across a cell interface.
    ///
    /// The HLLC flux is:
    /// - F_L                          if 0 <= S_L
    /// - F*_L = F_L + S_L*(U*_L - U_L)   if S_L <= 0 <= S*
    /// - F*_R = F_R + S_R*(U*_R - U_R)   if S* <= 0 <= S_R
    /// - F_R                          if 0 >= S_R
    pub fn compute_flux(
        &self,
        left: &ConservativeState,
        right: &ConservativeState,
        face_normal: [f64; 3],
    ) -> ConservativeFlux {
        let (s_l, s_r, _s_star) = self.estimate_signal_speeds(left, right, face_normal);
        let _f_l = self.physical_flux(left, face_normal);
        let _f_r = self.physical_flux(right, face_normal);

        if s_l >= 0.0 {
            // Supersonic flow from the left
            return ConservativeFlux::new(_f_l[0], _f_l[1], _f_l[2], _f_l[3], _f_l[4]);
        }
        if s_r <= 0.0 {
            // Supersonic flow from the right
            return ConservativeFlux::new(_f_r[0], _f_r[1], _f_r[2], _f_r[3], _f_r[4]);
        }

        // Subsonic region: compute the HLLC star states
        let s_star = _s_star;

        // Helper to compute star state and flux for a given side
        let compute_star_flux = |state: &ConservativeState, s_k: f64, f_k: &[f64; 5]| -> [f64; 5] {
            let vel = state.velocity();
            let u_n = vel[0] * face_normal[0] + vel[1] * face_normal[1] + vel[2] * face_normal[2];
            let p_k = state.pressure(self.gamma);

            let coeff = state.rho * (s_k - u_n) / (s_k - s_star);
            let p_star = p_k + state.rho * (s_k - u_n) * (s_star - u_n);

            // U*_k
            let u_star = [
                coeff,
                coeff * (vel[0] + (s_star - u_n) * face_normal[0]),
                coeff * (vel[1] + (s_star - u_n) * face_normal[1]),
                coeff * (vel[2] + (s_star - u_n) * face_normal[2]),
                coeff * (state.rho_e / state.rho + (s_star - u_n) * (s_star + p_k / (state.rho * (s_k - u_n)))),
            ];

            // F*_k = F_k + S_k * (U*_k - U_k)
            let u_k = [state.rho, state.rho_u, state.rho_v, state.rho_w, state.rho_e];
            let mut f_star = [0.0; 5];
            for i in 0..5 {
                f_star[i] = f_k[i] + s_k * (u_star[i] - u_k[i]);
            }

            let _ = p_star; // p_star is embedded in the star state computation
            f_star
        };

        if s_star >= 0.0 {
            // Left star state: F*_L
            let f_star = compute_star_flux(left, s_l, &_f_l);
            ConservativeFlux::new(f_star[0], f_star[1], f_star[2], f_star[3], f_star[4])
        } else {
            // Right star state: F*_R
            let f_star = compute_star_flux(right, s_r, &_f_r);
            ConservativeFlux::new(f_star[0], f_star[1], f_star[2], f_star[3], f_star[4])
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_signal_speed_symmetry() {
        let state = ConservativeState::new(1.225, 0.0, 0.0, 0.0, 253312.5);
        let hllc = HllcFlux::new(1.4);
        let (s_l, s_r, s_star) = hllc.estimate_signal_speeds(&state, &state, [1.0, 0.0, 0.0]);
        assert!((s_star).abs() < 1e-10, "Contact speed should be ~0 for symmetric states");
        assert!((s_l + s_r).abs() < 1e-10, "Wave speeds should be symmetric");
    }
}
