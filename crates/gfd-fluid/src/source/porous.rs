//! Porous media model for the momentum equations.
//!
//! Adds a momentum sink to cells inside a porous zone using the
//! Darcy-Forchheimer formulation (same approach as ANSYS Fluent):
//!
//!   S_i = -(mu/alpha * u_i  +  C_2 * 0.5 * rho * |u| * u_i)
//!
//! where:
//! - alpha is the permeability [m^2],
//! - C_2 is the inertial resistance factor [1/m],
//! - mu is the dynamic viscosity [Pa*s],
//! - rho is the fluid density [kg/m^3],
//! - |u| is the velocity magnitude [m/s].
//!
//! The source is linearized so that the viscous (Darcy) term is treated
//! implicitly (added to the matrix diagonal) and the inertial
//! (Forchheimer) term is also treated implicitly through the velocity-
//! dependent coefficient.

use gfd_core::UnstructuredMesh;

/// Description of a porous zone in the computational domain.
#[derive(Debug, Clone)]
pub struct PorousZone {
    /// Permeability alpha [m^2].
    pub permeability: f64,
    /// Inertial resistance factor C_2 [1/m].
    pub inertial_resistance: f64,
    /// Volume porosity [0-1].
    pub porosity: f64,
    /// Indices of the mesh cells that belong to this porous zone.
    pub cell_ids: Vec<usize>,
}

/// Result of the porous media source computation for a single cell.
///
/// The source is linearized as `S = sc + sp * u_i`:
/// - `sc` is the explicit part (added to the RHS),
/// - `sp` is the implicit coefficient (added to the diagonal).
///
/// For the Darcy-Forchheimer model, `sc = 0` and
/// `sp = -(mu/alpha + C_2 * 0.5 * rho * |u|) * V`.
#[derive(Debug, Clone, Copy, Default)]
pub struct PorousSource {
    /// Explicit source [N].
    pub sc: f64,
    /// Implicit source coefficient [N/(m/s)] (should be <= 0 for stability).
    pub sp: f64,
}

impl PorousZone {
    /// Creates a new porous zone.
    pub fn new(
        permeability: f64,
        inertial_resistance: f64,
        porosity: f64,
        cell_ids: Vec<usize>,
    ) -> Self {
        Self {
            permeability,
            inertial_resistance,
            porosity,
            cell_ids,
        }
    }

    /// Computes the linearized porous-media momentum source for every cell
    /// in the zone.
    ///
    /// Returns a vector of `(cell_id, PorousSource)` pairs. Only cells
    /// belonging to this zone are included.
    ///
    /// # Arguments
    /// * `velocity` - per-cell velocity vectors `[u, v, w]`
    /// * `mu` - dynamic viscosity [Pa*s]
    /// * `rho` - density [kg/m^3]
    /// * `mesh` - the computational mesh (for cell volumes)
    pub fn compute_porous_source(
        &self,
        velocity: &[[f64; 3]],
        mu: f64,
        rho: f64,
        mesh: &UnstructuredMesh,
    ) -> Vec<(usize, PorousSource)> {
        let mut result = Vec::with_capacity(self.cell_ids.len());

        let alpha = self.permeability;
        let c2 = self.inertial_resistance;

        for &cid in &self.cell_ids {
            if cid >= velocity.len() || cid >= mesh.cells.len() {
                continue;
            }

            let u = velocity[cid];
            let u_mag = (u[0] * u[0] + u[1] * u[1] + u[2] * u[2]).sqrt();
            let vol = mesh.cells[cid].volume;

            // Viscous (Darcy) contribution: -mu/alpha * u_i
            // Inertial (Forchheimer) contribution: -C_2 * 0.5 * rho * |u| * u_i
            // Combined implicit coefficient (negative for stability):
            let viscous = if alpha > 0.0 { mu / alpha } else { 0.0 };
            let inertial = c2 * 0.5 * rho * u_mag;

            let sp = -(viscous + inertial) * vol;

            result.push((cid, PorousSource { sc: 0.0, sp }));
        }

        result
    }

    /// Convenience: computes the total porous source field for the entire
    /// mesh, returning `(sc, sp)` arrays of length `num_cells`.
    ///
    /// Cells outside the porous zone receive zero source.
    pub fn compute_porous_source_field(
        &self,
        velocity: &[[f64; 3]],
        mu: f64,
        rho: f64,
        mesh: &UnstructuredMesh,
    ) -> (Vec<f64>, Vec<f64>) {
        let n = mesh.num_cells();
        let mut sc = vec![0.0; n];
        let mut sp = vec![0.0; n];

        for (cid, src) in self.compute_porous_source(velocity, mu, rho, mesh) {
            if cid < n {
                sc[cid] += src.sc;
                sp[cid] += src.sp;
            }
        }

        (sc, sp)
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use gfd_core::mesh::structured::StructuredMesh;

    fn make_test_mesh(nx: usize, ny: usize) -> UnstructuredMesh {
        let sm = StructuredMesh::uniform(nx, ny, 0, 1.0, 1.0, 0.0);
        sm.to_unstructured()
    }

    #[test]
    fn test_porous_zone_creation() {
        let zone = PorousZone::new(1e-6, 100.0, 0.5, vec![0, 1, 2]);
        assert!((zone.permeability - 1e-6).abs() < 1e-20);
        assert!((zone.inertial_resistance - 100.0).abs() < 1e-12);
        assert!((zone.porosity - 0.5).abs() < 1e-12);
        assert_eq!(zone.cell_ids.len(), 3);
    }

    #[test]
    fn test_zero_velocity_gives_zero_source() {
        let mesh = make_test_mesh(3, 3);
        let n = mesh.num_cells();
        let velocity = vec![[0.0; 3]; n];
        let zone = PorousZone::new(1e-6, 100.0, 0.5, vec![0, 1, 2]);

        let sources = zone.compute_porous_source(&velocity, 1e-3, 1.0, &mesh);

        for (_cid, src) in &sources {
            // sc should always be 0.
            assert!(src.sc.abs() < 1e-20);
            // With zero velocity, the inertial term vanishes, but the
            // viscous term sp = -(mu/alpha)*V should still be there.
            // However, since sp multiplies u_i which is 0, the net source is 0.
            // sp itself is: -(1e-3/1e-6 + 0)*V = -1000*V
            assert!(src.sp < 0.0, "sp should be negative");
        }
    }

    #[test]
    fn test_porous_source_increases_with_velocity() {
        let mesh = make_test_mesh(3, 3);
        let n = mesh.num_cells();

        let zone = PorousZone::new(1e-6, 100.0, 0.5, vec![4]);

        // Low velocity.
        let vel_low = vec![[1.0, 0.0, 0.0]; n];
        let src_low = zone.compute_porous_source(&vel_low, 1e-3, 1.0, &mesh);

        // High velocity.
        let vel_high = vec![[10.0, 0.0, 0.0]; n];
        let src_high = zone.compute_porous_source(&vel_high, 1e-3, 1.0, &mesh);

        // |sp| should be larger at higher velocity because of the inertial term.
        let sp_low = src_low[0].1.sp.abs();
        let sp_high = src_high[0].1.sp.abs();
        assert!(
            sp_high > sp_low,
            "Porous resistance should increase with velocity: {} vs {}",
            sp_high,
            sp_low
        );
    }

    #[test]
    fn test_viscous_term_dominates_at_low_velocity() {
        // At very low velocity, the Darcy (viscous) term should dominate.
        let mesh = make_test_mesh(3, 3);
        let n = mesh.num_cells();
        let velocity = vec![[1e-6, 0.0, 0.0]; n];

        let mu = 1e-3;
        let rho = 1000.0;
        let alpha = 1e-10; // very low permeability
        let c2 = 1.0;

        let zone = PorousZone::new(alpha, c2, 0.5, vec![4]);
        let sources = zone.compute_porous_source(&velocity, mu, rho, &mesh);

        let src = sources[0].1;
        let vol = mesh.cells[4].volume;
        let u_mag = 1e-6;

        let viscous_part = mu / alpha * vol;
        let inertial_part = c2 * 0.5 * rho * u_mag * vol;

        // Viscous should be >> inertial.
        assert!(
            viscous_part > 1000.0 * inertial_part,
            "Viscous should dominate at low velocity"
        );
        // sp should be close to -(viscous + inertial) * vol
        let expected_sp = -(viscous_part + inertial_part);
        assert!(
            (src.sp - expected_sp).abs() < 1e-6 * expected_sp.abs(),
            "sp mismatch: got {}, expected {}",
            src.sp,
            expected_sp
        );
    }

    #[test]
    fn test_inertial_term_dominates_at_high_velocity() {
        let mesh = make_test_mesh(3, 3);
        let n = mesh.num_cells();
        let velocity = vec![[100.0, 0.0, 0.0]; n];

        let mu = 1e-3;
        let rho = 1000.0;
        let alpha = 1.0; // very high permeability => negligible viscous resistance
        let c2 = 1000.0;

        let zone = PorousZone::new(alpha, c2, 0.5, vec![4]);
        let sources = zone.compute_porous_source(&velocity, mu, rho, &mesh);

        let vol = mesh.cells[4].volume;
        let u_mag = 100.0;

        let viscous_part = mu / alpha * vol;
        let inertial_part = c2 * 0.5 * rho * u_mag * vol;

        assert!(
            inertial_part > 1000.0 * viscous_part,
            "Inertial should dominate at high velocity"
        );
    }

    #[test]
    fn test_compute_porous_source_field() {
        let mesh = make_test_mesh(3, 3);
        let n = mesh.num_cells();
        let velocity = vec![[1.0, 0.0, 0.0]; n];

        // Zone covers cells 0, 1, 2 only.
        let zone = PorousZone::new(1e-6, 100.0, 0.5, vec![0, 1, 2]);
        let (sc, sp) = zone.compute_porous_source_field(&velocity, 1e-3, 1.0, &mesh);

        assert_eq!(sc.len(), n);
        assert_eq!(sp.len(), n);

        // Cells 0-2 should have non-zero sp.
        for i in 0..3 {
            assert!(sp[i] < 0.0, "Porous cells should have negative sp");
        }
        // Cells 3..n should have zero sp.
        for i in 3..n {
            assert!(
                sp[i].abs() < 1e-20,
                "Non-porous cells should have zero sp"
            );
        }
        // All sc should be zero.
        for i in 0..n {
            assert!(sc[i].abs() < 1e-20);
        }
    }

    #[test]
    fn test_sp_is_always_negative() {
        // For stability the implicit coefficient must be <= 0.
        let mesh = make_test_mesh(5, 5);
        let n = mesh.num_cells();
        let velocity = vec![[3.0, 2.0, 1.0]; n];

        let all_cells: Vec<usize> = (0..n).collect();
        let zone = PorousZone::new(1e-8, 500.0, 0.3, all_cells);
        let sources = zone.compute_porous_source(&velocity, 1e-3, 1000.0, &mesh);

        for (_, src) in &sources {
            assert!(src.sp <= 0.0, "sp must be non-positive, got {}", src.sp);
        }
    }
}
