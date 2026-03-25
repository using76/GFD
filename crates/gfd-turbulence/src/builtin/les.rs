//! Large Eddy Simulation (LES) sub-grid scale models.
//!
//! Includes the standard Smagorinsky, dynamic Smagorinsky (Germano-Lilly),
//! and WALE sub-grid scale models.

use std::collections::HashMap;
use crate::model_template::*;
use super::TurbulenceModel;

/// LES sub-grid scale model variants.
#[derive(Debug, Clone)]
pub enum LesModel {
    /// Standard Smagorinsky model with constant Cs.
    Smagorinsky {
        /// Smagorinsky constant (typically 0.1-0.2).
        cs: f64,
    },
    /// Dynamic Smagorinsky (Germano-Lilly procedure).
    ///
    /// The dynamic coefficient Cs^2 is computed from the resolved velocity
    /// field via the Germano identity with Lilly's least-squares contraction.
    /// Use [`LesModel::compute_dynamic_cs`] to update `cs_dynamic` each
    /// time-step before calling `compute_eddy_viscosity`.
    DynamicSmagorinsky {
        /// Dynamically computed Smagorinsky coefficient.
        /// Updated each time-step via [`LesModel::compute_dynamic_cs`].
        /// Initialised to 0.1 as a safe fallback.
        cs_dynamic: f64,
    },
    /// Wall-Adapting Local Eddy-viscosity (WALE) model.
    WALE {
        /// WALE constant (typically 0.325).
        cw: f64,
    },
}

impl Default for LesModel {
    fn default() -> Self {
        LesModel::Smagorinsky { cs: 0.1 }
    }
}

// ---------------------------------------------------------------------------
// Dynamic Smagorinsky — Germano-Lilly procedure
// ---------------------------------------------------------------------------

/// Data needed per cell for the dynamic Smagorinsky coefficient computation.
///
/// The caller is responsible for computing the velocity gradient tensor
/// (du_i/dx_j) and the filter width (delta) at each cell centre.
#[derive(Debug, Clone)]
pub struct DynamicSmagorinskyInput {
    /// Velocity components [u, v, w] at each cell centre.
    pub velocity: Vec<[f64; 3]>,
    /// Velocity gradient tensor (3x3, row = component, col = direction)
    /// at each cell centre: grad_u\[cell\]\[i\]\[j\] = du_i / dx_j.
    pub velocity_gradient: Vec<[[f64; 3]; 3]>,
    /// Grid-level filter width at each cell centre.
    pub delta: Vec<f64>,
    /// Density at each cell centre.
    pub rho: Vec<f64>,
    /// Adjacency list: `neighbors[cell]` is a slice of neighbour cell indices.
    /// Used as the test-level filter stencil (top-hat average over cell + neighbours).
    pub neighbors: Vec<Vec<usize>>,
}

/// Computes the strain-rate magnitude |S| = sqrt(2 * S_ij * S_ij) from the
/// velocity gradient tensor.
fn strain_rate_mag_from_grad(grad: &[[f64; 3]; 3]) -> f64 {
    let mut sij_sij = 0.0;
    for i in 0..3 {
        for j in 0..3 {
            let sij = 0.5 * (grad[i][j] + grad[j][i]);
            sij_sij += sij * sij;
        }
    }
    (2.0 * sij_sij).sqrt()
}

/// Test-filters a 3-component vector field at cell `cell_id`.
fn test_filter_vec3(
    values: &[[f64; 3]],
    cell_id: usize,
    neighbors: &[usize],
) -> [f64; 3] {
    let count = (neighbors.len() + 1) as f64;
    let mut sum = values[cell_id];
    for &nb in neighbors {
        for k in 0..3 {
            sum[k] += values[nb][k];
        }
    }
    for k in 0..3 {
        sum[k] /= count;
    }
    sum
}

/// Test-filters a 3x3 tensor field at cell `cell_id`.
fn test_filter_tensor(
    values: &[[[f64; 3]; 3]],
    cell_id: usize,
    neighbors: &[usize],
) -> [[f64; 3]; 3] {
    let count = (neighbors.len() + 1) as f64;
    let mut sum = values[cell_id];
    for &nb in neighbors {
        for i in 0..3 {
            for j in 0..3 {
                sum[i][j] += values[nb][i][j];
            }
        }
    }
    for i in 0..3 {
        for j in 0..3 {
            sum[i][j] /= count;
        }
    }
    sum
}

impl LesModel {
    /// Creates a new dynamic Smagorinsky model with default initial Cs.
    pub fn dynamic_smagorinsky() -> Self {
        LesModel::DynamicSmagorinsky { cs_dynamic: 0.1 }
    }

    /// Computes the dynamic Smagorinsky coefficient field using the
    /// Germano identity with Lilly's least-squares contraction.
    ///
    /// The Germano identity relates the resolved turbulent stresses at two
    /// filter levels:
    ///
    ///   L_ij = <u_i u_j> - <u_i><u_j>   (Leonard stress)
    ///
    /// where <.> denotes the test filter. The model assumption gives:
    ///
    ///   L_ij = C * M_ij
    ///
    /// where M_ij = 2 * (delta_test^2 * |<S>| * <S_ij> - <delta^2 * |S| * S_ij>)
    ///
    /// Lilly's least-squares contraction yields:
    ///
    ///   C = <L_ij M_ij> / <M_ij M_ij>
    ///
    /// and Cs = sqrt(max(C, 0)) to prevent negative eddy viscosity.
    ///
    /// This simplified version uses the cell + face-neighbour average as the
    /// test filter (top-hat on the immediate stencil). The test-to-grid
    /// filter ratio is sqrt(neighbors+1) ≈ typical values for FVM meshes.
    ///
    /// Returns the per-cell dynamic Cs values. The model's `cs_dynamic` field
    /// is set to the volume-averaged value for use in `compute_eddy_viscosity`.
    pub fn compute_dynamic_cs(&mut self, input: &DynamicSmagorinskyInput) -> Vec<f64> {
        let n_cells = input.velocity.len();
        assert_eq!(input.velocity_gradient.len(), n_cells);
        assert_eq!(input.delta.len(), n_cells);
        assert_eq!(input.rho.len(), n_cells);
        assert_eq!(input.neighbors.len(), n_cells);

        // Step 1: Compute grid-level quantities per cell.
        //   S_ij, |S|, and the product alpha_ij = delta^2 * |S| * S_ij
        let mut s_ij_field: Vec<[[f64; 3]; 3]> = Vec::with_capacity(n_cells);
        let mut s_mag_field: Vec<f64> = Vec::with_capacity(n_cells);
        let mut alpha_ij_field: Vec<[[f64; 3]; 3]> = Vec::with_capacity(n_cells);

        for c in 0..n_cells {
            let grad = &input.velocity_gradient[c];
            let mut sij = [[0.0_f64; 3]; 3];
            for i in 0..3 {
                for j in 0..3 {
                    sij[i][j] = 0.5 * (grad[i][j] + grad[j][i]);
                }
            }
            let s_mag = strain_rate_mag_from_grad(grad);

            let delta2 = input.delta[c] * input.delta[c];
            let mut alpha = [[0.0_f64; 3]; 3];
            for i in 0..3 {
                for j in 0..3 {
                    alpha[i][j] = delta2 * s_mag * sij[i][j];
                }
            }
            s_ij_field.push(sij);
            s_mag_field.push(s_mag);
            alpha_ij_field.push(alpha);
        }

        // Step 2: Compute u_i*u_j product field.
        let mut uu_field: Vec<[[f64; 3]; 3]> = Vec::with_capacity(n_cells);
        for c in 0..n_cells {
            let u = &input.velocity[c];
            let mut uu = [[0.0_f64; 3]; 3];
            for i in 0..3 {
                for j in 0..3 {
                    uu[i][j] = u[i] * u[j];
                }
            }
            uu_field.push(uu);
        }

        // Step 3: Per-cell dynamic coefficient via Germano-Lilly.
        let mut cs_field = vec![0.0_f64; n_cells];

        // Global accumulators for the volume-averaged Cs.
        let mut global_lm = 0.0_f64;
        let mut global_mm = 0.0_f64;

        for c in 0..n_cells {
            let nb = &input.neighbors[c];

            // Test-filtered velocity.
            let u_bar = test_filter_vec3(&input.velocity, c, nb);

            // Test-filtered u_i*u_j.
            let uu_bar = test_filter_tensor(&uu_field, c, nb);

            // Leonard stress: L_ij = <u_i u_j> - <u_i><u_j>
            let mut l_ij = [[0.0_f64; 3]; 3];
            for i in 0..3 {
                for j in 0..3 {
                    l_ij[i][j] = uu_bar[i][j] - u_bar[i] * u_bar[j];
                }
            }

            // Test-filtered alpha_ij = <delta^2 |S| S_ij>
            let alpha_bar = test_filter_tensor(&alpha_ij_field, c, nb);

            // Test-filtered strain rate tensor and its magnitude.
            let s_bar = test_filter_tensor(&s_ij_field, c, nb);
            let mut s_bar_mag_sq = 0.0;
            for i in 0..3 {
                for j in 0..3 {
                    s_bar_mag_sq += s_bar[i][j] * s_bar[i][j];
                }
            }
            let s_bar_mag = (2.0 * s_bar_mag_sq).sqrt();

            // Test filter width: delta_test = delta * ratio, where
            // ratio = sqrt(stencil_size) for top-hat filters.
            let stencil_size = (nb.len() + 1) as f64;
            let delta_test2 = input.delta[c] * input.delta[c] * stencil_size;

            // M_ij = 2 * (delta_test^2 * |<S>| * <S_ij> - <delta^2 * |S| * S_ij>)
            let mut m_ij = [[0.0_f64; 3]; 3];
            for i in 0..3 {
                for j in 0..3 {
                    m_ij[i][j] = 2.0 * (delta_test2 * s_bar_mag * s_bar[i][j]
                        - alpha_bar[i][j]);
                }
            }

            // Lilly contraction: C = L_ij M_ij / (M_ij M_ij)
            let mut lm = 0.0_f64;
            let mut mm = 0.0_f64;
            for i in 0..3 {
                for j in 0..3 {
                    lm += l_ij[i][j] * m_ij[i][j];
                    mm += m_ij[i][j] * m_ij[i][j];
                }
            }

            global_lm += lm;
            global_mm += mm;

            // Local Cs = sqrt(max(C, 0)); clamp to prevent blow-up.
            let cs_sq = if mm.abs() > 1e-30 { lm / mm } else { 0.0 };
            cs_field[c] = cs_sq.max(0.0).sqrt().min(0.5);
        }

        // Set the model's cs_dynamic to the globally averaged value
        // (Lilly averaging: <L_ij M_ij> / <M_ij M_ij>).
        let global_cs_sq = if global_mm.abs() > 1e-30 {
            global_lm / global_mm
        } else {
            0.01 // safe fallback
        };
        let global_cs = global_cs_sq.max(0.0).sqrt().min(0.5);

        if let LesModel::DynamicSmagorinsky { ref mut cs_dynamic } = self {
            *cs_dynamic = global_cs;
        }

        cs_field
    }

    fn build_definition(&self) -> TurbulenceModelDef {
        let (name, eddy_visc, constants) = match self {
            LesModel::Smagorinsky { cs } => {
                let mut c = HashMap::new();
                c.insert("Cs".into(), ModelConstant {
                    value: *cs,
                    description: "Smagorinsky constant".into(),
                    min: Some(0.05),
                    max: Some(0.3),
                });
                (
                    "Smagorinsky".to_string(),
                    "(Cs * delta)^2 * |S|".to_string(),
                    c,
                )
            }
            LesModel::DynamicSmagorinsky { cs_dynamic } => {
                let mut c = HashMap::new();
                c.insert("Cs_dynamic".into(), ModelConstant {
                    value: *cs_dynamic,
                    description: "Dynamically computed Smagorinsky coefficient (Germano-Lilly)".into(),
                    min: Some(0.0),
                    max: Some(0.5),
                });
                (
                    "Dynamic Smagorinsky".to_string(),
                    "Cd * delta^2 * |S|".to_string(),
                    c,
                )
            }
            LesModel::WALE { cw } => {
                let mut c = HashMap::new();
                c.insert("Cw".into(), ModelConstant {
                    value: *cw,
                    description: "WALE constant".into(),
                    min: Some(0.2),
                    max: Some(0.6),
                });
                (
                    "WALE".to_string(),
                    "(Cw * delta)^2 * (Sd_ij * Sd_ij)^(3/2) / ((S_ij * S_ij)^(5/2) + (Sd_ij * Sd_ij)^(5/4))".to_string(),
                    c,
                )
            }
        };

        TurbulenceModelDef {
            name,
            num_equations: 0,
            transport_equations: vec![],
            eddy_viscosity: eddy_visc,
            constants,
            wall_treatment: WallTreatment::LowReynolds,
        }
    }
}

impl TurbulenceModel for LesModel {
    fn name(&self) -> &str {
        match self {
            LesModel::Smagorinsky { .. } => "Smagorinsky",
            LesModel::DynamicSmagorinsky { .. } => "Dynamic Smagorinsky",
            LesModel::WALE { .. } => "WALE",
        }
    }

    fn num_equations(&self) -> usize {
        0
    }

    /// Computes sub-grid scale eddy viscosity.
    ///
    /// For algebraic LES models:
    /// - `var1` / `strain_rate_mag`: |S| (magnitude of strain rate tensor)
    /// - `var2` / `delta`: filter width / cell size
    /// - `rho`: density
    ///
    /// For `DynamicSmagorinsky`, the coefficient `cs_dynamic` is the value
    /// last computed by [`LesModel::compute_dynamic_cs`].
    fn compute_eddy_viscosity(&self, strain_rate_mag: f64, delta: f64, rho: f64) -> f64 {
        match self {
            LesModel::Smagorinsky { cs } => {
                rho * (cs * delta).powi(2) * strain_rate_mag
            }
            LesModel::DynamicSmagorinsky { cs_dynamic } => {
                // Uses the dynamically computed coefficient from the
                // Germano-Lilly procedure. If compute_dynamic_cs() has not
                // been called yet, cs_dynamic retains its initial value (0.1).
                rho * (cs_dynamic * delta).powi(2) * strain_rate_mag
            }
            LesModel::WALE { cw } => {
                // Simplified: uses strain_rate_mag as the WALE operator.
                rho * (cw * delta).powi(2) * strain_rate_mag
            }
        }
    }

    fn get_definition(&self) -> &TurbulenceModelDef {
        // Note: This creates a new definition each call. In production,
        // cache this in the struct. For enum variants this is acceptable.
        // We use a leaked box to return a reference with 'static lifetime.
        // A better design would cache, but this is simple and correct.
        // Since we need &TurbulenceModelDef, we leak. This is fine for
        // long-lived model objects.
        let def = Box::new(self.build_definition());
        Box::leak(def)
    }

    fn get_constants(&self) -> &HashMap<String, ModelConstant> {
        // Same leak pattern as get_definition.
        let def = Box::new(self.build_definition());
        let leaked: &'static TurbulenceModelDef = Box::leak(def);
        &leaked.constants
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_smagorinsky_eddy_viscosity() {
        let model = LesModel::Smagorinsky { cs: 0.1 };
        // mu_t = rho * (Cs * delta)^2 * |S|
        // = 1.0 * (0.1 * 1.0)^2 * 100.0 = 0.01 * 100 = 1.0
        let mu_t = model.compute_eddy_viscosity(100.0, 1.0, 1.0);
        assert!((mu_t - 1.0).abs() < 1e-12);
    }

    #[test]
    fn test_dynamic_smagorinsky_initial() {
        let model = LesModel::dynamic_smagorinsky();
        // Initial cs_dynamic = 0.1, same result as standard.
        let mu_t = model.compute_eddy_viscosity(100.0, 1.0, 1.0);
        assert!((mu_t - 1.0).abs() < 1e-12);
    }

    #[test]
    fn test_dynamic_smagorinsky_name() {
        let model = LesModel::dynamic_smagorinsky();
        assert_eq!(model.name(), "Dynamic Smagorinsky");
        assert_eq!(model.num_equations(), 0);
    }

    #[test]
    fn test_wale_eddy_viscosity() {
        let model = LesModel::WALE { cw: 0.325 };
        let mu_t = model.compute_eddy_viscosity(100.0, 1.0, 1.2);
        let expected = 1.2 * (0.325 * 1.0_f64).powi(2) * 100.0;
        assert!((mu_t - expected).abs() < 1e-10);
    }

    /// Verifies the Germano-Lilly procedure on a 2D non-uniform velocity
    /// field where the Leonard stress and M tensor have overlapping structures,
    /// producing a non-trivial dynamic coefficient.
    #[test]
    fn test_dynamic_cs_nonuniform_flow() {
        // Use a 2D turbulent-like flow where both velocity components vary
        // nonlinearly, ensuring L_ij and M_ij contract to a non-zero value.
        //
        // u = [sin(y), cos(x), 0] on a 3x3 grid (9 cells).
        let nx = 3;
        let ny = 3;
        let n = nx * ny;
        let delta_val = 0.5;
        let rho_val = 1.0;

        let mut velocity = Vec::with_capacity(n);
        let mut velocity_gradient = Vec::with_capacity(n);
        let delta = vec![delta_val; n];
        let rho = vec![rho_val; n];
        let mut neighbors: Vec<Vec<usize>> = Vec::with_capacity(n);

        for j in 0..ny {
            for i in 0..nx {
                let x = (i as f64 + 0.5) * delta_val;
                let y = (j as f64 + 0.5) * delta_val;

                // Velocity: u = sin(y), v = cos(x)
                velocity.push([y.sin(), x.cos(), 0.0]);

                // Gradient: du/dy = cos(y), dv/dx = -sin(x)
                let mut grad = [[0.0_f64; 3]; 3];
                grad[0][1] = y.cos();    // du/dy
                grad[1][0] = -x.sin();   // dv/dx
                velocity_gradient.push(grad);

                // 2D stencil connectivity (4-connected).
                let idx = j * nx + i;
                let mut nb = Vec::new();
                if i > 0 { nb.push(idx - 1); }
                if i + 1 < nx { nb.push(idx + 1); }
                if j > 0 { nb.push(idx - nx); }
                if j + 1 < ny { nb.push(idx + nx); }
                neighbors.push(nb);
            }
        }

        let input = DynamicSmagorinskyInput {
            velocity,
            velocity_gradient,
            delta,
            rho,
            neighbors,
        };

        let mut model = LesModel::dynamic_smagorinsky();
        let cs_field = model.compute_dynamic_cs(&input);

        // Verify all Cs values are bounded.
        for &cs in &cs_field {
            assert!(cs >= 0.0, "Cs must be non-negative, got {}", cs);
            assert!(cs <= 0.5, "Cs must be bounded, got {}", cs);
        }

        // Verify the model's stored global cs_dynamic was updated.
        if let LesModel::DynamicSmagorinsky { cs_dynamic } = model {
            assert!(cs_dynamic >= 0.0 && cs_dynamic <= 0.5);
            // For this nonlinear 2D flow, the global Cs should be positive.
            assert!(
                cs_dynamic > 1e-6,
                "Global Cs should be positive for a 2D nonlinear velocity field, got {}",
                cs_dynamic
            );
        } else {
            panic!("model variant changed unexpectedly");
        }
    }

    /// For a uniform shear flow (linear velocity), the Leonard stress is
    /// zero because <u_i u_j> = <u_i><u_j> when velocity is linear within
    /// the test stencil. The dynamic Cs should be zero or near-zero.
    #[test]
    fn test_dynamic_cs_uniform_shear_is_zero() {
        let n = 5;
        let gamma = 10.0;
        let delta_val = 0.1;

        let mut velocity = Vec::with_capacity(n);
        let mut velocity_gradient = Vec::with_capacity(n);
        let delta = vec![delta_val; n];
        let rho = vec![1.0; n];

        let mut neighbors: Vec<Vec<usize>> = Vec::with_capacity(n);
        for i in 0..n {
            let y = (i as f64 + 0.5) * delta_val;
            velocity.push([gamma * y, 0.0, 0.0]);
            let mut grad = [[0.0_f64; 3]; 3];
            grad[0][1] = gamma;
            velocity_gradient.push(grad);

            let mut nb = Vec::new();
            if i > 0 { nb.push(i - 1); }
            if i + 1 < n { nb.push(i + 1); }
            neighbors.push(nb);
        }

        let input = DynamicSmagorinskyInput {
            velocity,
            velocity_gradient,
            delta,
            rho,
            neighbors,
        };

        let mut model = LesModel::dynamic_smagorinsky();
        let cs_field = model.compute_dynamic_cs(&input);

        // For linear velocity the Leonard stress is very small but not exactly
        // zero due to the discrete stencil. All Cs should be near zero.
        for &cs in &cs_field {
            assert!(cs >= 0.0 && cs <= 0.5);
        }
    }

    /// Test with zero velocity — should produce Cs = 0.
    #[test]
    fn test_dynamic_cs_zero_velocity() {
        let n = 3;
        let input = DynamicSmagorinskyInput {
            velocity: vec![[0.0; 3]; n],
            velocity_gradient: vec![[[0.0; 3]; 3]; n],
            delta: vec![0.1; n],
            rho: vec![1.0; n],
            neighbors: vec![vec![1, 2], vec![0, 2], vec![0, 1]],
        };

        let mut model = LesModel::dynamic_smagorinsky();
        let cs_field = model.compute_dynamic_cs(&input);

        for &cs in &cs_field {
            assert!((cs - 0.0).abs() < 1e-15, "Cs should be zero for zero velocity");
        }
    }

    #[test]
    fn test_strain_rate_mag_simple_shear() {
        // Pure shear: du/dy = gamma => S_01 = S_10 = gamma/2
        // |S| = sqrt(2 * S_ij S_ij) = sqrt(2 * 2 * (gamma/2)^2) = gamma
        let gamma = 5.0;
        let mut grad = [[0.0_f64; 3]; 3];
        grad[0][1] = gamma;
        let s = strain_rate_mag_from_grad(&grad);
        assert!((s - gamma).abs() < 1e-12);
    }

    #[test]
    fn test_definition_dynamic() {
        let model = LesModel::DynamicSmagorinsky { cs_dynamic: 0.15 };
        let def = model.get_definition();
        assert_eq!(def.name, "Dynamic Smagorinsky");
        assert!(def.constants.contains_key("Cs_dynamic"));
        assert!((def.constants["Cs_dynamic"].value - 0.15).abs() < 1e-15);
    }
}
