//! Diffusive flux discretization for FVM.
//!
//! Provides both the standard orthogonal diffusion coefficient and
//! a non-orthogonal correction for meshes where cell-center-to-cell-center
//! vectors are not aligned with face normals.

use gfd_core::UnstructuredMesh;

/// Computes the diffusive flux coefficient for a face.
///
/// The diffusive contribution across a face is:
///   D = gamma * A_f / d_PN
///
/// where:
/// * `gamma` - diffusion coefficient (e.g. thermal conductivity, viscosity)
/// * `area`  - face area A_f
/// * `distance` - distance between owner and neighbor cell centers d_PN
///
/// This coefficient contributes:
///   a_P += D  (owner diagonal)
///   a_N  = D  (off-diagonal, to be subtracted from owner equation)
///   flux = D * (phi_N - phi_P)
///
/// # Panics
/// Panics if `distance` is zero.
pub fn compute_diffusive_coefficient(gamma: f64, area: f64, distance: f64) -> f64 {
    assert!(
        distance > 0.0,
        "Distance between cell centers must be positive, got {}",
        distance
    );
    gamma * area / distance
}

/// Computes the non-orthogonal correction source term for diffusion.
///
/// On non-orthogonal meshes, the standard diffusion discretization
/// `D = gamma * A / d_PN` only captures the orthogonal component of the
/// diffusive flux. The correction accounts for the non-orthogonal part.
///
/// The face area vector S_f = A_f * n_f is decomposed into:
/// - An orthogonal part Delta along d_PN: Delta = (S_f . e_PN) * e_PN
/// - A non-orthogonal part k: k = S_f - Delta
///
/// The correction for each cell is: gamma * grad(phi)_f . k
///
/// This correction is computed explicitly (deferred) and added to the RHS
/// of the diffusion equation. It can be iterated in outer correction loops.
pub fn compute_non_orthogonal_correction(
    _phi: &[f64],
    mesh: &UnstructuredMesh,
    gradient: &[[f64; 3]],
    gamma: f64,
) -> Vec<f64> {
    let n = mesh.num_cells();
    let mut correction = vec![0.0; n];

    for face in &mesh.faces {
        let owner = face.owner_cell;

        if let Some(neighbor) = face.neighbor_cell {
            let cc_o = mesh.cells[owner].center;
            let cc_n = mesh.cells[neighbor].center;

            let d_pn = [
                cc_n[0] - cc_o[0],
                cc_n[1] - cc_o[1],
                cc_n[2] - cc_o[2],
            ];
            let d_pn_mag_sq = d_pn[0] * d_pn[0] + d_pn[1] * d_pn[1] + d_pn[2] * d_pn[2];
            if d_pn_mag_sq < 1e-60 {
                continue;
            }

            let s_f = [
                face.area * face.normal[0],
                face.area * face.normal[1],
                face.area * face.normal[2],
            ];

            let d_pn_mag = d_pn_mag_sq.sqrt();
            let e_pn = [d_pn[0] / d_pn_mag, d_pn[1] / d_pn_mag, d_pn[2] / d_pn_mag];

            let s_dot_e = s_f[0] * e_pn[0] + s_f[1] * e_pn[1] + s_f[2] * e_pn[2];
            let delta = [s_dot_e * e_pn[0], s_dot_e * e_pn[1], s_dot_e * e_pn[2]];

            let k = [s_f[0] - delta[0], s_f[1] - delta[1], s_f[2] - delta[2]];

            let grad_f = [
                0.5 * (gradient[owner][0] + gradient[neighbor][0]),
                0.5 * (gradient[owner][1] + gradient[neighbor][1]),
                0.5 * (gradient[owner][2] + gradient[neighbor][2]),
            ];

            let corr = gamma * (grad_f[0] * k[0] + grad_f[1] * k[1] + grad_f[2] * k[2]);

            correction[owner] += corr;
            correction[neighbor] -= corr;
        }
    }

    correction
}

/// Computes the non-orthogonal correction with a non-uniform diffusion coefficient.
///
/// Same as `compute_non_orthogonal_correction` but accepts a per-cell diffusion
/// coefficient field. The face gamma is interpolated as the harmonic mean.
pub fn compute_non_orthogonal_correction_variable(
    _phi: &[f64],
    mesh: &UnstructuredMesh,
    gradient: &[[f64; 3]],
    gamma_field: &[f64],
) -> Vec<f64> {
    let n = mesh.num_cells();
    let mut correction = vec![0.0; n];

    for face in &mesh.faces {
        let owner = face.owner_cell;

        if let Some(neighbor) = face.neighbor_cell {
            let cc_o = mesh.cells[owner].center;
            let cc_n = mesh.cells[neighbor].center;

            let d_pn = [
                cc_n[0] - cc_o[0],
                cc_n[1] - cc_o[1],
                cc_n[2] - cc_o[2],
            ];
            let d_pn_mag_sq = d_pn[0] * d_pn[0] + d_pn[1] * d_pn[1] + d_pn[2] * d_pn[2];
            if d_pn_mag_sq < 1e-60 {
                continue;
            }

            let s_f = [
                face.area * face.normal[0],
                face.area * face.normal[1],
                face.area * face.normal[2],
            ];

            let d_pn_mag = d_pn_mag_sq.sqrt();
            let e_pn = [d_pn[0] / d_pn_mag, d_pn[1] / d_pn_mag, d_pn[2] / d_pn_mag];

            let s_dot_e = s_f[0] * e_pn[0] + s_f[1] * e_pn[1] + s_f[2] * e_pn[2];
            let delta = [s_dot_e * e_pn[0], s_dot_e * e_pn[1], s_dot_e * e_pn[2]];

            let k = [s_f[0] - delta[0], s_f[1] - delta[1], s_f[2] - delta[2]];

            let g_o = gamma_field[owner];
            let g_n = gamma_field[neighbor];
            let gamma_f = if (g_o + g_n).abs() > 1e-30 {
                2.0 * g_o * g_n / (g_o + g_n)
            } else {
                0.0
            };

            let grad_f = [
                0.5 * (gradient[owner][0] + gradient[neighbor][0]),
                0.5 * (gradient[owner][1] + gradient[neighbor][1]),
                0.5 * (gradient[owner][2] + gradient[neighbor][2]),
            ];

            let corr = gamma_f * (grad_f[0] * k[0] + grad_f[1] * k[1] + grad_f[2] * k[2]);

            correction[owner] += corr;
            correction[neighbor] -= corr;
        }
    }

    correction
}

/// Measures the non-orthogonality angle (degrees) for each internal face.
pub fn measure_non_orthogonality(mesh: &UnstructuredMesh) -> Vec<(usize, f64)> {
    let mut results = Vec::new();

    for (fi, face) in mesh.faces.iter().enumerate() {
        if let Some(neighbor) = face.neighbor_cell {
            let cc_o = mesh.cells[face.owner_cell].center;
            let cc_n = mesh.cells[neighbor].center;

            let d_pn = [
                cc_n[0] - cc_o[0],
                cc_n[1] - cc_o[1],
                cc_n[2] - cc_o[2],
            ];
            let d_pn_mag = (d_pn[0] * d_pn[0] + d_pn[1] * d_pn[1] + d_pn[2] * d_pn[2]).sqrt();
            if d_pn_mag < 1e-30 {
                continue;
            }

            let cos_theta = (face.normal[0] * d_pn[0]
                + face.normal[1] * d_pn[1]
                + face.normal[2] * d_pn[2])
                / d_pn_mag;

            let angle_rad = cos_theta.clamp(-1.0, 1.0).acos();
            results.push((fi, angle_rad.to_degrees()));
        }
    }

    results
}

#[cfg(test)]
mod tests {
    use super::*;
    use gfd_core::StructuredMesh;

    #[test]
    fn basic_diffusion_coefficient() {
        let d = compute_diffusive_coefficient(1.0, 2.0, 0.5);
        assert!((d - 4.0).abs() < 1e-12);
    }

    #[test]
    fn scaled_diffusion() {
        let d = compute_diffusive_coefficient(0.01, 1.0, 0.1);
        assert!((d - 0.1).abs() < 1e-12);
    }

    #[test]
    #[should_panic]
    fn zero_distance_panics() {
        compute_diffusive_coefficient(1.0, 1.0, 0.0);
    }

    #[test]
    fn non_orthogonal_correction_zero_on_orthogonal_mesh() {
        let mesh = StructuredMesh::uniform(5, 5, 1, 1.0, 1.0, 0.2).to_unstructured();
        let n = mesh.num_cells();
        let phi: Vec<f64> = (0..n).map(|i| mesh.cells[i].center[0]).collect();
        let gradient: Vec<[f64; 3]> = vec![[1.0, 0.0, 0.0]; n];

        let corr = compute_non_orthogonal_correction(&phi, &mesh, &gradient, 1.0);
        let max_corr = corr.iter().map(|c| c.abs()).fold(0.0_f64, f64::max);
        assert!(max_corr < 1e-10, "Expected zero on orthogonal mesh, got {}", max_corr);
    }

    #[test]
    fn non_orthogonal_correction_variable_gamma() {
        let mesh = StructuredMesh::uniform(5, 5, 1, 1.0, 1.0, 0.2).to_unstructured();
        let n = mesh.num_cells();
        let phi: Vec<f64> = (0..n).map(|i| mesh.cells[i].center[0]).collect();
        let gradient: Vec<[f64; 3]> = vec![[1.0, 0.0, 0.0]; n];
        let gamma_field = vec![2.0; n];

        let corr = compute_non_orthogonal_correction_variable(&phi, &mesh, &gradient, &gamma_field);
        let max_corr = corr.iter().map(|c| c.abs()).fold(0.0_f64, f64::max);
        assert!(max_corr < 1e-10, "Expected zero on orthogonal mesh, got {}", max_corr);
    }

    #[test]
    fn orthogonal_mesh_angle_near_zero() {
        let mesh = StructuredMesh::uniform(3, 3, 1, 1.0, 1.0, 0.2).to_unstructured();
        let angles = measure_non_orthogonality(&mesh);
        for (fi, angle) in &angles {
            assert!(*angle < 1.0, "Face {} angle {} deg on orthogonal mesh", fi, angle);
        }
    }

    #[test]
    fn correction_sums_to_zero() {
        let mesh = StructuredMesh::uniform(4, 4, 1, 1.0, 1.0, 0.2).to_unstructured();
        let n = mesh.num_cells();
        let phi: Vec<f64> = (0..n)
            .map(|i| {
                let x = mesh.cells[i].center[0];
                let y = mesh.cells[i].center[1];
                x * x + y * y
            })
            .collect();
        let gradient: Vec<[f64; 3]> = (0..n)
            .map(|i| {
                let x = mesh.cells[i].center[0];
                let y = mesh.cells[i].center[1];
                [2.0 * x, 2.0 * y, 0.0]
            })
            .collect();

        let corr = compute_non_orthogonal_correction(&phi, &mesh, &gradient, 1.0);
        let total: f64 = corr.iter().sum();
        assert!(total.abs() < 1e-10, "Correction should be conservative, sum={}", total);
    }
}
