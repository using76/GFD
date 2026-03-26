//! Solution-based adaptive mesh refinement.
//!
//! Computes refinement flags based on gradient magnitude of a scalar field,
//! using the Green-Gauss gradient method.

use gfd_core::field::scalar::ScalarField;
use gfd_core::mesh::unstructured::UnstructuredMesh;

/// Computes refinement flags for each cell based on gradient magnitude.
///
/// A cell is flagged for refinement if `|grad(field)| * h > threshold`, where
/// `h` is the characteristic cell size (cube root of volume).
///
/// # Arguments
/// * `field` - The scalar field to analyze (one value per cell).
/// * `mesh` - The mesh on which the field is defined.
/// * `threshold` - The refinement threshold.
///
/// # Returns
/// A boolean vector with `true` for cells that should be refined.
pub fn compute_refinement_flags(
    field: &ScalarField,
    mesh: &UnstructuredMesh,
    threshold: f64,
) -> Vec<bool> {
    let n_cells = mesh.cells.len();
    let values = field.values();

    if values.len() != n_cells {
        return vec![false; n_cells];
    }

    // Compute gradient using Green-Gauss method
    let gradients = compute_green_gauss_gradient(field, mesh);

    // Compute refinement indicator for each cell
    let mut flags = vec![false; n_cells];

    for ci in 0..n_cells {
        let grad = gradients[ci];
        let grad_mag = (grad[0] * grad[0] + grad[1] * grad[1] + grad[2] * grad[2]).sqrt();

        // Characteristic cell size = cube root of volume
        let h = mesh.cells[ci].volume.cbrt();

        let indicator = grad_mag * h;
        if indicator > threshold {
            flags[ci] = true;
        }
    }

    flags
}

/// Computes refinement flags based on proximity to a geometry defined by an SDF.
///
/// Cells whose center is within `distance` of the surface (|SDF| < distance) are flagged.
///
/// # Arguments
/// * `mesh` - The mesh.
/// * `sdf` - Signed distance function.
/// * `distance` - Proximity distance threshold.
///
/// # Returns
/// A boolean vector with `true` for cells near the surface.
pub fn compute_proximity_flags(
    mesh: &UnstructuredMesh,
    sdf: &dyn Fn([f64; 3]) -> f64,
    distance: f64,
) -> Vec<bool> {
    mesh.cells
        .iter()
        .map(|cell| sdf(cell.center).abs() < distance)
        .collect()
}

/// Combines multiple flag vectors with logical OR.
pub fn combine_flags(flag_sets: &[&[bool]]) -> Vec<bool> {
    if flag_sets.is_empty() {
        return Vec::new();
    }
    let n = flag_sets[0].len();
    let mut result = vec![false; n];
    for flags in flag_sets {
        for (i, &f) in flags.iter().enumerate() {
            if i < n && f {
                result[i] = true;
            }
        }
    }
    result
}

/// Computes the Green-Gauss gradient of a scalar field.
///
/// For each cell, gradient = (1/V) * sum_faces(phi_f * S_f * n_f),
/// where phi_f is the face value (average of owner and neighbor), S_f is the face area,
/// and n_f is the outward-pointing unit normal.
fn compute_green_gauss_gradient(
    field: &ScalarField,
    mesh: &UnstructuredMesh,
) -> Vec<[f64; 3]> {
    let n_cells = mesh.cells.len();
    let values = field.values();
    let mut gradients = vec![[0.0f64; 3]; n_cells];

    for face in &mesh.faces {
        let owner = face.owner_cell;
        let phi_owner = values[owner];

        let phi_face = if let Some(neighbor) = face.neighbor_cell {
            // Internal face: interpolate linearly
            let phi_neighbor = values[neighbor];
            0.5 * (phi_owner + phi_neighbor)
        } else {
            // Boundary face: use owner value
            phi_owner
        };

        // Contribution to owner: phi_f * A * n
        let flux = phi_face * face.area;
        gradients[owner][0] += flux * face.normal[0];
        gradients[owner][1] += flux * face.normal[1];
        gradients[owner][2] += flux * face.normal[2];

        // Contribution to neighbor: -phi_f * A * n (opposite normal)
        if let Some(neighbor) = face.neighbor_cell {
            gradients[neighbor][0] -= flux * face.normal[0];
            gradients[neighbor][1] -= flux * face.normal[1];
            gradients[neighbor][2] -= flux * face.normal[2];
        }
    }

    // Divide by cell volume
    for ci in 0..n_cells {
        let vol = mesh.cells[ci].volume;
        if vol > 1e-30 {
            gradients[ci][0] /= vol;
            gradients[ci][1] /= vol;
            gradients[ci][2] /= vol;
        }
    }

    gradients
}

#[cfg(test)]
mod tests {
    use super::*;
    use gfd_core::mesh::structured::StructuredMesh;

    #[test]
    fn test_uniform_field_no_refinement() {
        let mesh = StructuredMesh::uniform(5, 5, 1, 5.0, 5.0, 1.0).to_unstructured();
        let field = ScalarField::new("T", vec![1.0; mesh.cells.len()]);
        let flags = compute_refinement_flags(&field, &mesh, 0.1);
        assert!(
            flags.iter().all(|&f| !f),
            "Uniform field should have no refinement flags"
        );
    }

    #[test]
    fn test_step_field_flags_interface() {
        let mesh = StructuredMesh::uniform(10, 1, 1, 10.0, 1.0, 1.0).to_unstructured();
        // Step function: 0 for x < 5, 1 for x >= 5
        let values: Vec<f64> = mesh
            .cells
            .iter()
            .map(|c| if c.center[0] < 5.0 { 0.0 } else { 1.0 })
            .collect();
        let field = ScalarField::new("phi", values);
        let flags = compute_refinement_flags(&field, &mesh, 0.01);
        // At least some cells near x=5 should be flagged
        let n_flagged: usize = flags.iter().filter(|&&f| f).count();
        assert!(
            n_flagged > 0,
            "Step function should flag cells near the interface"
        );
        assert!(
            n_flagged < mesh.cells.len(),
            "Not all cells should be flagged"
        );
    }

    #[test]
    fn test_proximity_flags() {
        let mesh = StructuredMesh::uniform(10, 10, 1, 10.0, 10.0, 1.0).to_unstructured();
        let sdf = |p: [f64; 3]| {
            let dx = p[0] - 5.0;
            let dy = p[1] - 5.0;
            (dx * dx + dy * dy).sqrt() - 2.0
        };
        let flags = compute_proximity_flags(&mesh, &sdf, 1.5);
        let n_flagged: usize = flags.iter().filter(|&&f| f).count();
        assert!(n_flagged > 0, "Should flag cells near the circle");
        assert!(n_flagged < mesh.cells.len(), "Not all cells should be flagged");
    }

    #[test]
    fn test_combine_flags() {
        let f1 = vec![true, false, false, true];
        let f2 = vec![false, true, false, false];
        let combined = combine_flags(&[&f1, &f2]);
        assert_eq!(combined, vec![true, true, false, true]);
    }

    #[test]
    fn test_wrong_field_size() {
        let mesh = StructuredMesh::uniform(3, 3, 1, 3.0, 3.0, 1.0).to_unstructured();
        let field = ScalarField::new("T", vec![1.0; 5]); // wrong size
        let flags = compute_refinement_flags(&field, &mesh, 0.1);
        assert!(
            flags.iter().all(|&f| !f),
            "Wrong-sized field should return no flags"
        );
    }
}
