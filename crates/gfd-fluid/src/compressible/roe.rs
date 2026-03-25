//! Roe's approximate Riemann solver for compressible flow.
//!
//! Computes the numerical flux at a cell interface using Roe-averaged
//! quantities and wave decomposition.

use super::ConservativeState;

/// Conservative flux vector at a cell interface.
#[derive(Debug, Clone, Copy)]
pub struct ConservativeFlux {
    /// Mass flux [kg/(m^2*s)].
    pub mass: f64,
    /// x-momentum flux [Pa].
    pub momentum_x: f64,
    /// y-momentum flux [Pa].
    pub momentum_y: f64,
    /// z-momentum flux [Pa].
    pub momentum_z: f64,
    /// Energy flux [W/m^2].
    pub energy: f64,
}

impl ConservativeFlux {
    /// Creates a new conservative flux vector.
    pub fn new(mass: f64, momentum_x: f64, momentum_y: f64, momentum_z: f64, energy: f64) -> Self {
        Self {
            mass,
            momentum_x,
            momentum_y,
            momentum_z,
            energy,
        }
    }

    /// Returns the flux as a 5-element array.
    pub fn as_array(&self) -> [f64; 5] {
        [self.mass, self.momentum_x, self.momentum_y, self.momentum_z, self.energy]
    }
}

/// Roe's approximate Riemann solver.
///
/// Uses Roe-averaged quantities to linearize the Euler equations at each
/// cell interface, then computes the upwind flux by decomposing the jump
/// into characteristic waves.
///
/// Reference: P.L. Roe, "Approximate Riemann Solvers, Parameter Vectors,
/// and Difference Schemes", J. Comp. Phys. 43, 1981.
pub struct RoeFlux {
    /// Ratio of specific heats (gamma).
    pub gamma: f64,
    /// Entropy fix parameter (Harten-Hyman correction).
    pub entropy_fix_coefficient: f64,
}

impl RoeFlux {
    /// Creates a new Roe flux calculator.
    pub fn new(gamma: f64) -> Self {
        Self {
            gamma,
            entropy_fix_coefficient: 0.1,
        }
    }

    /// Creates a new Roe flux calculator with a custom entropy fix coefficient.
    pub fn with_entropy_fix(gamma: f64, entropy_fix_coefficient: f64) -> Self {
        Self {
            gamma,
            entropy_fix_coefficient,
        }
    }

    /// Computes the Roe-averaged state between left and right states.
    ///
    /// The Roe averages are:
    /// - rho_avg = sqrt(rho_L * rho_R)
    /// - u_avg   = (sqrt(rho_L)*u_L + sqrt(rho_R)*u_R) / (sqrt(rho_L) + sqrt(rho_R))
    /// - H_avg   = (sqrt(rho_L)*H_L + sqrt(rho_R)*H_R) / (sqrt(rho_L) + sqrt(rho_R))
    fn roe_averages(&self, left: &ConservativeState, right: &ConservativeState) -> RoeAveragedState {
        let sqrt_rho_l = left.rho.sqrt();
        let sqrt_rho_r = right.rho.sqrt();
        let denom = sqrt_rho_l + sqrt_rho_r;

        let rho_avg = sqrt_rho_l * sqrt_rho_r;

        let vel_l = left.velocity();
        let vel_r = right.velocity();

        let u_avg = (sqrt_rho_l * vel_l[0] + sqrt_rho_r * vel_r[0]) / denom;
        let v_avg = (sqrt_rho_l * vel_l[1] + sqrt_rho_r * vel_r[1]) / denom;
        let w_avg = (sqrt_rho_l * vel_l[2] + sqrt_rho_r * vel_r[2]) / denom;

        // Total enthalpy H = (rho*E + p) / rho
        let p_l = left.pressure(self.gamma);
        let p_r = right.pressure(self.gamma);
        let h_l = (left.rho_e + p_l) / left.rho;
        let h_r = (right.rho_e + p_r) / right.rho;
        let h_avg = (sqrt_rho_l * h_l + sqrt_rho_r * h_r) / denom;

        // Speed of sound from averaged quantities
        let ke_avg = 0.5 * (u_avg * u_avg + v_avg * v_avg + w_avg * w_avg);
        let a_sq = (self.gamma - 1.0) * (h_avg - ke_avg);
        let a_avg = if a_sq > 0.0 { a_sq.sqrt() } else { 0.0 };

        RoeAveragedState {
            rho: rho_avg,
            u: u_avg,
            v: v_avg,
            w: w_avg,
            h: h_avg,
            a: a_avg,
        }
    }

    /// Computes the numerical flux across a cell interface using Roe's method.
    ///
    /// F_roe = 0.5 * (F_L + F_R) - 0.5 * sum_k(|lambda_k| * alpha_k * r_k)
    ///
    /// where lambda_k are the eigenvalues, alpha_k the wave strengths, and
    /// r_k the right eigenvectors of the Roe-averaged Jacobian matrix.
    pub fn compute_flux(
        &self,
        left_state: &ConservativeState,
        right_state: &ConservativeState,
        face_normal: [f64; 3],
    ) -> ConservativeFlux {
        let avg = self.roe_averages(left_state, right_state);
        let nx = face_normal[0];
        let ny = face_normal[1];
        let nz = face_normal[2];

        // Physical fluxes in normal direction for left state
        let vel_l = left_state.velocity();
        let p_l = left_state.pressure(self.gamma);
        let u_n_l = vel_l[0] * nx + vel_l[1] * ny + vel_l[2] * nz;
        let f_l = [
            left_state.rho * u_n_l,
            left_state.rho_u * u_n_l + p_l * nx,
            left_state.rho_v * u_n_l + p_l * ny,
            left_state.rho_w * u_n_l + p_l * nz,
            (left_state.rho_e + p_l) * u_n_l,
        ];

        // Physical fluxes in normal direction for right state
        let vel_r = right_state.velocity();
        let p_r = right_state.pressure(self.gamma);
        let u_n_r = vel_r[0] * nx + vel_r[1] * ny + vel_r[2] * nz;
        let f_r = [
            right_state.rho * u_n_r,
            right_state.rho_u * u_n_r + p_r * nx,
            right_state.rho_v * u_n_r + p_r * ny,
            right_state.rho_w * u_n_r + p_r * nz,
            (right_state.rho_e + p_r) * u_n_r,
        ];

        // Roe-averaged normal velocity
        let v_n = avg.u * nx + avg.v * ny + avg.w * nz;
        let a = avg.a;

        // Eigenvalues
        let lambda = [v_n - a, v_n, v_n, v_n, v_n + a];

        // Entropy fix (Harten-Hyman)
        let eps_fix = self.entropy_fix_coefficient * a;
        let abs_lambda: Vec<f64> = lambda.iter().map(|&l| {
            if l.abs() < eps_fix {
                (l * l + eps_fix * eps_fix) / (2.0 * eps_fix)
            } else {
                l.abs()
            }
        }).collect();

        // Jump in conservative variables
        let d_rho = right_state.rho - left_state.rho;
        let _d_rho_u = right_state.rho_u - left_state.rho_u;
        let _d_rho_v = right_state.rho_v - left_state.rho_v;
        let _d_rho_w = right_state.rho_w - left_state.rho_w;
        let _d_rho_e = right_state.rho_e - left_state.rho_e;

        // Jump in primitive-like variables
        let d_u = vel_r[0] - vel_l[0];
        let d_v = vel_r[1] - vel_l[1];
        let d_w = vel_r[2] - vel_l[2];
        let d_p = p_r - p_l;
        let d_v_n = d_u * nx + d_v * ny + d_w * nz;

        // Wave strengths
        let alpha1 = (d_p - avg.rho * a * d_v_n) / (2.0 * a * a);
        let alpha2 = d_rho - d_p / (a * a);
        let alpha5 = (d_p + avg.rho * a * d_v_n) / (2.0 * a * a);

        // Tangential velocity jumps
        let d_v_t1 = d_u - d_v_n * nx;
        let d_v_t2 = d_v - d_v_n * ny;
        let d_v_t3 = d_w - d_v_n * nz;

        // Compute dissipation: sum |lambda_k| * alpha_k * r_k
        // Wave 1: lambda = v_n - a
        let r1 = [
            1.0,
            avg.u - a * nx,
            avg.v - a * ny,
            avg.w - a * nz,
            avg.h - a * v_n,
        ];

        // Wave 2,3,4: lambda = v_n (entropy and shear waves)
        // Entropy wave
        let r2 = [
            1.0,
            avg.u,
            avg.v,
            avg.w,
            0.5 * (avg.u * avg.u + avg.v * avg.v + avg.w * avg.w),
        ];
        // Shear waves combined
        let r_shear = [
            0.0,
            d_v_t1,
            d_v_t2,
            d_v_t3,
            avg.u * d_v_t1 + avg.v * d_v_t2 + avg.w * d_v_t3,
        ];

        // Wave 5: lambda = v_n + a
        let r5 = [
            1.0,
            avg.u + a * nx,
            avg.v + a * ny,
            avg.w + a * nz,
            avg.h + a * v_n,
        ];

        let mut dissipation = [0.0; 5];
        for k in 0..5 {
            dissipation[k] = abs_lambda[0] * alpha1 * r1[k]
                + abs_lambda[1] * (alpha2 * r2[k] + avg.rho * r_shear[k])
                + abs_lambda[4] * alpha5 * r5[k];
        }

        // F_roe = 0.5*(F_L + F_R) - 0.5*dissipation
        ConservativeFlux::new(
            0.5 * (f_l[0] + f_r[0]) - 0.5 * dissipation[0],
            0.5 * (f_l[1] + f_r[1]) - 0.5 * dissipation[1],
            0.5 * (f_l[2] + f_r[2]) - 0.5 * dissipation[2],
            0.5 * (f_l[3] + f_r[3]) - 0.5 * dissipation[3],
            0.5 * (f_l[4] + f_r[4]) - 0.5 * dissipation[4],
        )
    }
}

/// Roe-averaged state at a cell interface.
#[derive(Debug, Clone, Copy)]
struct RoeAveragedState {
    /// Roe-averaged density.
    pub rho: f64,
    /// Roe-averaged x-velocity.
    pub u: f64,
    /// Roe-averaged y-velocity.
    pub v: f64,
    /// Roe-averaged z-velocity.
    pub w: f64,
    /// Roe-averaged total enthalpy.
    pub h: f64,
    /// Roe-averaged speed of sound.
    pub a: f64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_roe_averages_symmetric() {
        let state = ConservativeState::new(1.0, 100.0, 0.0, 0.0, 250000.0);
        let flux = RoeFlux::new(1.4);
        let avg = flux.roe_averages(&state, &state);
        assert!((avg.rho - 1.0).abs() < 1e-12);
        assert!((avg.u - 100.0).abs() < 1e-12);
    }
}
