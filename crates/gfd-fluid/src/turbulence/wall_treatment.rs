//! Wall treatment adapter for turbulence models.
//!
//! Provides wall function implementations and y+ computation for
//! near-wall treatment in RANS simulations. Includes both the standard
//! log-law wall function and Spalding's single-formula wall function.

use gfd_core::{ScalarField, UnstructuredMesh};
use crate::{FluidState, Result};

/// Adapter for applying wall treatment to turbulence models.
///
/// Handles the near-wall region by computing wall shear stress,
/// y+ values, and applying appropriate wall functions to modify
/// the turbulence boundary conditions.
pub struct WallTreatmentAdapter {
    /// Wall function type: "standard", "scalable", "enhanced", "low_re".
    pub wall_function_type: String,
    /// Von Karman constant (default 0.41).
    pub kappa: f64,
    /// Additive constant in the log-law (default 5.0).
    pub e_constant: f64,
    /// y+ threshold for switching between viscous sublayer and log-law.
    pub y_plus_threshold: f64,
}

impl WallTreatmentAdapter {
    /// Creates a new wall treatment adapter with standard wall functions.
    pub fn new() -> Self {
        Self {
            wall_function_type: "standard".to_string(),
            kappa: 0.41,
            e_constant: 5.0,
            y_plus_threshold: 11.225,
        }
    }

    /// Creates a wall treatment adapter with the specified wall function type.
    pub fn with_type(wall_function_type: impl Into<String>) -> Self {
        Self {
            wall_function_type: wall_function_type.into(),
            kappa: 0.41,
            e_constant: 5.0,
            y_plus_threshold: 11.225,
        }
    }

    /// Applies wall functions to modify boundary conditions at wall faces.
    ///
    /// For the standard log-law wall function:
    /// - If y+ < y+_threshold: viscous sublayer, u+ = y+
    /// - If y+ >= y+_threshold: log-law region, u+ = (1/kappa)*ln(y+) + B
    ///
    /// Modifies the turbulence variable boundary values and wall shear stress.
    pub fn apply_wall_functions(
        &self,
        state: &mut FluidState,
        mesh: &UnstructuredMesh,
        wall_face_indices: &[usize],
    ) -> Result<()> {
        let c_mu = 0.09_f64;

        for &face_id in wall_face_indices {
            let face = &mesh.faces[face_id];
            let owner = face.owner_cell;

            let cc = mesh.cells[owner].center;
            let fc = face.center;
            let y = ((cc[0] - fc[0]).powi(2)
                + (cc[1] - fc[1]).powi(2)
                + (cc[2] - fc[2]).powi(2))
            .sqrt()
            .max(1e-30);

            let rho = state.density.values()[owner];
            let nu = state.viscosity.values()[owner] / rho;

            let vel = state.velocity.values()[owner];
            let u_n = vel[0] * face.normal[0] + vel[1] * face.normal[1] + vel[2] * face.normal[2];
            let u_par = [
                vel[0] - u_n * face.normal[0],
                vel[1] - u_n * face.normal[1],
                vel[2] - u_n * face.normal[2],
            ];
            let u_mag = (u_par[0] * u_par[0] + u_par[1] * u_par[1] + u_par[2] * u_par[2]).sqrt();

            let mut u_tau = (nu * u_mag / y).sqrt().max(1e-10);

            for _newton_iter in 0..20 {
                let y_plus = u_tau * y / nu;
                let u_plus_computed = if y_plus < self.y_plus_threshold {
                    y_plus
                } else {
                    (1.0 / self.kappa) * (y_plus).ln() + self.e_constant
                };
                let u_plus_target = u_mag / u_tau;

                let residual = u_plus_computed - u_plus_target;
                if residual.abs() < 1e-6 {
                    break;
                }

                let du_plus_du_tau = if y_plus < self.y_plus_threshold {
                    y / nu
                } else {
                    1.0 / (self.kappa * u_tau)
                };
                let d_residual = du_plus_du_tau + u_mag / (u_tau * u_tau);

                if d_residual.abs() > 1e-30 {
                    u_tau -= residual / d_residual;
                    u_tau = u_tau.max(1e-10);
                }
            }

            let c_mu_sqrt = c_mu.sqrt();

            if let Some(ref mut k_field) = state.turb_kinetic_energy {
                let k_wall = u_tau * u_tau / c_mu_sqrt;
                let _ = k_field.set(owner, k_wall);
            }

            if let Some(ref mut eps_field) = state.turb_dissipation {
                let eps_wall = u_tau.powi(3) / (self.kappa * y);
                let _ = eps_field.set(owner, eps_wall);
            }

            if let Some(ref mut omega_field) = state.turb_specific_dissipation {
                let omega_wall = u_tau / (c_mu_sqrt * self.kappa * y);
                let _ = omega_field.set(owner, omega_wall);
            }
        }

        Ok(())
    }

    /// Computes the y+ field for all wall-adjacent cells.
    pub fn compute_y_plus(
        &self,
        state: &FluidState,
        mesh: &UnstructuredMesh,
    ) -> Result<ScalarField> {
        let n = mesh.num_cells();
        let mut y_plus_field = vec![0.0; n];

        for patch in &mesh.boundary_patches {
            let is_wall = patch.name.to_lowercase().contains("wall");
            if !is_wall {
                continue;
            }

            for &face_id in &patch.face_ids {
                let face = &mesh.faces[face_id];
                let owner = face.owner_cell;

                let cc = mesh.cells[owner].center;
                let fc = face.center;
                let y = ((cc[0] - fc[0]).powi(2)
                    + (cc[1] - fc[1]).powi(2)
                    + (cc[2] - fc[2]).powi(2))
                .sqrt()
                .max(1e-30);

                let rho = state.density.values()[owner];
                let nu = state.viscosity.values()[owner] / rho;

                let vel = state.velocity.values()[owner];
                let u_n = vel[0] * face.normal[0]
                    + vel[1] * face.normal[1]
                    + vel[2] * face.normal[2];
                let u_par = [
                    vel[0] - u_n * face.normal[0],
                    vel[1] - u_n * face.normal[1],
                    vel[2] - u_n * face.normal[2],
                ];
                let u_mag = (u_par[0] * u_par[0] + u_par[1] * u_par[1] + u_par[2] * u_par[2])
                    .sqrt();

                let mut u_tau = (nu * u_mag / y).sqrt().max(1e-10);

                for _iter in 0..20 {
                    let yp = u_tau * y / nu;
                    let u_plus_computed = if yp < self.y_plus_threshold {
                        yp
                    } else {
                        (1.0 / self.kappa) * yp.ln() + self.e_constant
                    };
                    let u_plus_target = u_mag / u_tau;
                    let residual = u_plus_computed - u_plus_target;
                    if residual.abs() < 1e-6 {
                        break;
                    }
                    let du_plus_du_tau = if yp < self.y_plus_threshold {
                        y / nu
                    } else {
                        1.0 / (self.kappa * u_tau)
                    };
                    let d_residual = du_plus_du_tau + u_mag / (u_tau * u_tau);
                    if d_residual.abs() > 1e-30 {
                        u_tau -= residual / d_residual;
                        u_tau = u_tau.max(1e-10);
                    }
                }

                y_plus_field[owner] = u_tau * y / nu;
            }
        }

        Ok(ScalarField::new("y_plus", y_plus_field))
    }
}

impl Default for WallTreatmentAdapter {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Enhanced wall functions: Spalding's law-of-the-wall
// ---------------------------------------------------------------------------

/// Enumeration of available wall function formulations.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum WallFunctionType {
    /// Standard log-law wall function with a viscous sublayer / log-law switch.
    Standard,
    /// Spalding's single-formula wall function that smoothly covers the
    /// entire boundary layer from the viscous sublayer to the log-law region.
    Spalding,
}

/// Spalding's law-of-the-wall constants.
const SPALDING_KAPPA: f64 = 0.41;
const SPALDING_B: f64 = 5.5;

/// Evaluates Spalding's wall function: computes y+ given u+.
///
/// y+ = u+ + exp(-kappa*B) * [exp(kappa*u+) - 1 - kappa*u+ - (kappa*u+)^2/2 - (kappa*u+)^3/6]
pub fn spalding_y_plus(u_plus: f64) -> f64 {
    let ku = SPALDING_KAPPA * u_plus;
    let exp_neg_kb = (-SPALDING_KAPPA * SPALDING_B).exp();
    u_plus + exp_neg_kb * (ku.exp() - 1.0 - ku - ku * ku / 2.0 - ku * ku * ku / 6.0)
}

fn spalding_dy_plus_du_plus(u_plus: f64) -> f64 {
    let k = SPALDING_KAPPA;
    let ku = k * u_plus;
    let exp_neg_kb = (-k * SPALDING_B).exp();
    1.0 + exp_neg_kb * (k * ku.exp() - k - k * k * u_plus - k * k * k * u_plus * u_plus / 2.0)
}

/// Solves Spalding's implicit equation for u+ given y+ using Newton's method.
pub fn spalding_u_plus(y_plus_target: f64) -> f64 {
    if y_plus_target <= 0.0 {
        return 0.0;
    }

    let mut u_plus = if y_plus_target < 10.0 {
        y_plus_target
    } else {
        (1.0 / SPALDING_KAPPA) * y_plus_target.ln() + SPALDING_B
    };

    for _iter in 0..50 {
        let f = spalding_y_plus(u_plus) - y_plus_target;
        if f.abs() < 1e-10 {
            break;
        }
        let df = spalding_dy_plus_du_plus(u_plus);
        if df.abs() < 1e-30 {
            break;
        }
        u_plus -= f / df;
        u_plus = u_plus.max(0.0);
    }

    u_plus
}

/// Computes friction velocity u_tau from Spalding's wall function.
pub fn spalding_u_tau(u_mag: f64, y: f64, nu: f64) -> f64 {
    if u_mag < 1e-30 || y < 1e-30 || nu < 1e-30 {
        return 1e-10;
    }

    let mut u_tau = (nu * u_mag / y).sqrt().max(1e-10);

    for _iter in 0..30 {
        let y_plus = u_tau * y / nu;
        let u_plus = spalding_u_plus(y_plus);
        let u_plus_target = u_mag / u_tau;

        let residual = u_plus - u_plus_target;
        if residual.abs() < 1e-6 {
            break;
        }

        let dy_du = spalding_dy_plus_du_plus(u_plus);
        let du_plus_du_tau = if dy_du.abs() > 1e-30 {
            (1.0 / dy_du) * (y / nu)
        } else {
            y / nu
        };

        let d_residual = du_plus_du_tau + u_mag / (u_tau * u_tau);

        if d_residual.abs() > 1e-30 {
            u_tau -= residual / d_residual;
            u_tau = u_tau.max(1e-10);
        }
    }

    u_tau
}

/// Enhanced wall treatment adapter supporting multiple wall function types.
pub struct EnhancedWallTreatment {
    /// The wall function formulation to use.
    pub wall_function: WallFunctionType,
    /// Von Karman constant.
    pub kappa: f64,
    /// Log-law additive constant B.
    pub b_constant: f64,
    /// y+ threshold for standard wall function.
    pub y_plus_threshold: f64,
}

impl EnhancedWallTreatment {
    /// Creates an enhanced wall treatment with Spalding's law.
    pub fn spalding() -> Self {
        Self {
            wall_function: WallFunctionType::Spalding,
            kappa: SPALDING_KAPPA,
            b_constant: SPALDING_B,
            y_plus_threshold: 11.225,
        }
    }

    /// Creates an enhanced wall treatment with the standard log-law.
    pub fn standard() -> Self {
        Self {
            wall_function: WallFunctionType::Standard,
            kappa: 0.41,
            b_constant: 5.0,
            y_plus_threshold: 11.225,
        }
    }

    /// Computes u+ for a given y+ using the selected wall function type.
    pub fn compute_u_plus(&self, y_plus: f64) -> f64 {
        match self.wall_function {
            WallFunctionType::Spalding => spalding_u_plus(y_plus),
            WallFunctionType::Standard => {
                if y_plus < self.y_plus_threshold {
                    y_plus
                } else {
                    (1.0 / self.kappa) * y_plus.ln() + self.b_constant
                }
            }
        }
    }

    /// Computes the friction velocity u_tau for a wall-adjacent cell.
    pub fn compute_u_tau(&self, u_mag: f64, y: f64, nu: f64) -> f64 {
        match self.wall_function {
            WallFunctionType::Spalding => spalding_u_tau(u_mag, y, nu),
            WallFunctionType::Standard => {
                let mut u_tau = (nu * u_mag / y).sqrt().max(1e-10);
                for _ in 0..20 {
                    let yp = u_tau * y / nu;
                    let u_plus_c = if yp < self.y_plus_threshold {
                        yp
                    } else {
                        (1.0 / self.kappa) * yp.ln() + self.b_constant
                    };
                    let u_plus_t = u_mag / u_tau;
                    let res = u_plus_c - u_plus_t;
                    if res.abs() < 1e-6 {
                        break;
                    }
                    let du = if yp < self.y_plus_threshold {
                        y / nu
                    } else {
                        1.0 / (self.kappa * u_tau)
                    };
                    let dr = du + u_mag / (u_tau * u_tau);
                    if dr.abs() > 1e-30 {
                        u_tau -= res / dr;
                        u_tau = u_tau.max(1e-10);
                    }
                }
                u_tau
            }
        }
    }

    /// Applies wall functions to modify turbulence boundary conditions.
    pub fn apply_wall_functions(
        &self,
        state: &mut FluidState,
        mesh: &UnstructuredMesh,
        wall_face_indices: &[usize],
    ) -> Result<()> {
        let c_mu = 0.09_f64;
        let c_mu_sqrt = c_mu.sqrt();

        for &face_id in wall_face_indices {
            let face = &mesh.faces[face_id];
            let owner = face.owner_cell;

            let cc = mesh.cells[owner].center;
            let fc = face.center;
            let y = ((cc[0] - fc[0]).powi(2)
                + (cc[1] - fc[1]).powi(2)
                + (cc[2] - fc[2]).powi(2))
            .sqrt()
            .max(1e-30);

            let rho = state.density.values()[owner];
            let nu = state.viscosity.values()[owner] / rho;

            let vel = state.velocity.values()[owner];
            let u_n = vel[0] * face.normal[0]
                + vel[1] * face.normal[1]
                + vel[2] * face.normal[2];
            let u_par = [
                vel[0] - u_n * face.normal[0],
                vel[1] - u_n * face.normal[1],
                vel[2] - u_n * face.normal[2],
            ];
            let u_mag =
                (u_par[0] * u_par[0] + u_par[1] * u_par[1] + u_par[2] * u_par[2]).sqrt();

            let u_tau = self.compute_u_tau(u_mag, y, nu);

            if let Some(ref mut k_field) = state.turb_kinetic_energy {
                let k_wall = u_tau * u_tau / c_mu_sqrt;
                let _ = k_field.set(owner, k_wall);
            }
            if let Some(ref mut eps_field) = state.turb_dissipation {
                let eps_wall = u_tau.powi(3) / (self.kappa * y);
                let _ = eps_field.set(owner, eps_wall);
            }
            if let Some(ref mut omega_field) = state.turb_specific_dissipation {
                let omega_wall = u_tau / (c_mu_sqrt * self.kappa * y);
                let _ = omega_field.set(owner, omega_wall);
            }
        }

        Ok(())
    }
}

impl Default for EnhancedWallTreatment {
    fn default() -> Self {
        Self::spalding()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spalding_viscous_sublayer() {
        for &yp in &[0.1, 0.5, 1.0, 3.0, 5.0] {
            let up = spalding_u_plus(yp);
            assert!(
                (up - yp).abs() < 0.5,
                "In viscous sublayer, u+ ({}) should ~ y+ ({})",
                up,
                yp
            );
        }
    }

    #[test]
    fn spalding_log_law_region() {
        for &yp in &[100.0, 500.0, 1000.0] {
            let up = spalding_u_plus(yp);
            let log_law = (1.0 / SPALDING_KAPPA) * yp.ln() + SPALDING_B;
            assert!(
                (up - log_law).abs() < 0.5,
                "In log-law region, u+ ({}) should ~ log-law ({}) for y+ = {}",
                up,
                log_law,
                yp
            );
        }
    }

    #[test]
    fn spalding_y_plus_roundtrip() {
        for &up_orig in &[0.5, 2.0, 5.0, 10.0, 15.0, 20.0, 25.0] {
            let yp = spalding_y_plus(up_orig);
            let up_recovered = spalding_u_plus(yp);
            assert!(
                (up_orig - up_recovered).abs() < 1e-8,
                "Round-trip failed: u+_orig={}, y+={}, u+_recovered={}",
                up_orig,
                yp,
                up_recovered
            );
        }
    }

    #[test]
    fn spalding_monotonic() {
        let mut prev_yp = 0.0;
        for i in 0..100 {
            let up = i as f64 * 0.3;
            let yp = spalding_y_plus(up);
            assert!(
                yp >= prev_yp,
                "Spalding y+ should be monotonic: y+({})={} < y+({})={}",
                up,
                yp,
                up - 0.3,
                prev_yp
            );
            prev_yp = yp;
        }
    }

    #[test]
    fn spalding_u_tau_basic() {
        let u_tau = spalding_u_tau(1.0, 0.001, 1e-6);
        assert!(u_tau > 0.01, "u_tau should be > 0.01, got {}", u_tau);
        assert!(u_tau < 1.0, "u_tau should be < 1.0, got {}", u_tau);
    }

    #[test]
    fn spalding_u_tau_consistency() {
        let u_mag = 2.0;
        let y = 0.005;
        let nu = 1.5e-5;

        let u_tau = spalding_u_tau(u_mag, y, nu);
        let y_plus = u_tau * y / nu;
        let u_plus_target = u_mag / u_tau;
        let u_plus_spalding = spalding_u_plus(y_plus);

        assert!(
            (u_plus_spalding - u_plus_target).abs() < 1e-4,
            "Spalding u_tau inconsistent: u+_spalding={}, u+_target={}",
            u_plus_spalding,
            u_plus_target
        );
    }

    #[test]
    fn spalding_vs_standard_convergence() {
        let ewt_spalding = EnhancedWallTreatment::spalding();
        let ewt_standard = EnhancedWallTreatment::standard();

        let u_mag = 5.0;
        let y = 0.01;
        let nu = 1e-5;

        let u_tau_s = ewt_spalding.compute_u_tau(u_mag, y, nu);
        let u_tau_std = ewt_standard.compute_u_tau(u_mag, y, nu);

        let rel_diff = (u_tau_s - u_tau_std).abs() / u_tau_std;
        assert!(
            rel_diff < 0.05,
            "Spalding and standard should agree in log-law: spalding={}, standard={}, rel_diff={}",
            u_tau_s,
            u_tau_std,
            rel_diff
        );
    }

    #[test]
    fn spalding_continuous_transition() {
        let mut prev_up = spalding_u_plus(5.0);
        for i in 1..50 {
            let yp = 5.0 + i as f64 * 0.5;
            let up = spalding_u_plus(yp);
            let delta = (up - prev_up).abs();
            assert!(
                delta < 1.0,
                "u+ jump too large at y+={}: delta_u+={}, prev_u+={}, u+={}",
                yp,
                delta,
                prev_up,
                up
            );
            prev_up = up;
        }
    }

    #[test]
    fn wall_function_type_enum() {
        let ewt = EnhancedWallTreatment::spalding();
        assert_eq!(ewt.wall_function, WallFunctionType::Spalding);

        let ewt = EnhancedWallTreatment::standard();
        assert_eq!(ewt.wall_function, WallFunctionType::Standard);
    }

    #[test]
    fn spalding_zero_velocity() {
        let up = spalding_u_plus(0.0);
        assert!(up.abs() < 1e-10, "u+(0) should be 0, got {}", up);

        let u_tau = spalding_u_tau(0.0, 0.001, 1e-6);
        assert!(u_tau >= 1e-10, "u_tau should be >= 1e-10 for zero velocity");
    }
}
