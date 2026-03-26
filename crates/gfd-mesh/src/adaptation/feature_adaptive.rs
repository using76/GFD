//! Feature-based adaptive mesh refinement.
//!
//! Flags cells for refinement based on geometry features such as surface
//! curvature and proximity to other geometry.

use gfd_core::mesh::unstructured::UnstructuredMesh;

/// Flag cells near geometry features for refinement based on SDF curvature.
///
/// Curvature is approximated by evaluating the SDF at the cell center and at
/// offset points along each axis. Cells where the estimated curvature exceeds
/// the threshold are flagged.
///
/// # Arguments
/// * `mesh` - The mesh to evaluate.
/// * `sdf` - A signed distance function defining the geometry.
/// * `threshold` - Curvature threshold; cells with curvature above this are flagged.
///
/// # Returns
/// A boolean vector with `true` for cells that should be refined.
pub fn curvature_refinement_flags(
    mesh: &UnstructuredMesh,
    sdf: &dyn Fn([f64; 3]) -> f64,
    threshold: f64,
) -> Vec<bool> {
    let n_cells = mesh.cells.len();
    let mut flags = vec![false; n_cells];

    for (ci, cell) in mesh.cells.iter().enumerate() {
        let c = cell.center;
        let h = cell.volume.cbrt().max(1e-12);
        let eps = h * 0.1; // Small offset for finite differences

        let d0 = sdf(c);

        // Only flag cells near the surface (within a few cell lengths)
        if d0.abs() > 3.0 * h {
            continue;
        }

        // Approximate curvature via Laplacian of SDF:
        // kappa ~ (d(x+e) + d(x-e) - 2*d(x)) / e^2, summed over axes
        let mut laplacian = 0.0;
        for axis in 0..3 {
            let mut p_plus = c;
            let mut p_minus = c;
            p_plus[axis] += eps;
            p_minus[axis] -= eps;
            let d_plus = sdf(p_plus);
            let d_minus = sdf(p_minus);
            laplacian += (d_plus + d_minus - 2.0 * d0) / (eps * eps);
        }

        let curvature = laplacian.abs();
        if curvature > threshold {
            flags[ci] = true;
        }
    }

    flags
}

/// Flag cells near other cells based on a distance field (proximity refinement).
///
/// Cells whose distance field value is below a threshold are flagged, plus
/// `n_layers` additional layers of neighbor cells are also flagged (buffer region).
///
/// # Arguments
/// * `mesh` - The mesh to evaluate.
/// * `distance_field` - One value per cell representing distance to a feature.
/// * `n_layers` - Number of additional neighbor layers to flag around seed cells.
///
/// # Returns
/// A boolean vector with `true` for cells that should be refined.
pub fn proximity_refinement_flags(
    mesh: &UnstructuredMesh,
    distance_field: &[f64],
    n_layers: usize,
) -> Vec<bool> {
    let n_cells = mesh.cells.len();
    if distance_field.len() != n_cells {
        return vec![false; n_cells];
    }

    // Compute a characteristic length for the proximity threshold
    let avg_h: f64 = mesh.cells.iter().map(|c| c.volume.cbrt()).sum::<f64>() / n_cells as f64;
    let proximity_threshold = avg_h * 2.0;

    // Seed cells: those with small distance field values
    let mut flags = vec![false; n_cells];
    for ci in 0..n_cells {
        if distance_field[ci].abs() < proximity_threshold {
            flags[ci] = true;
        }
    }

    // Build cell adjacency from faces
    let mut cell_neighbors: Vec<Vec<usize>> = vec![Vec::new(); n_cells];
    for face in &mesh.faces {
        if let Some(nb) = face.neighbor_cell {
            let owner = face.owner_cell;
            if !cell_neighbors[owner].contains(&nb) {
                cell_neighbors[owner].push(nb);
            }
            if !cell_neighbors[nb].contains(&owner) {
                cell_neighbors[nb].push(owner);
            }
        }
    }

    // Expand flagged region by n_layers
    for _ in 0..n_layers {
        let current_flags = flags.clone();
        for ci in 0..n_cells {
            if current_flags[ci] {
                for &nb in &cell_neighbors[ci] {
                    flags[nb] = true;
                }
            }
        }
    }

    flags
}

#[cfg(test)]
mod tests {
    use super::*;
    use gfd_core::mesh::structured::StructuredMesh;

    #[test]
    fn test_curvature_flags_sphere() {
        let mesh = StructuredMesh::uniform(10, 10, 1, 10.0, 10.0, 1.0).to_unstructured();
        // Sphere SDF centered at (5,5,0.5) with radius 2
        let sdf = |p: [f64; 3]| {
            let dx = p[0] - 5.0;
            let dy = p[1] - 5.0;
            let dz = p[2] - 0.5;
            (dx * dx + dy * dy + dz * dz).sqrt() - 2.0
        };
        let flags = curvature_refinement_flags(&mesh, &sdf, 0.1);
        let n_flagged: usize = flags.iter().filter(|&&f| f).count();
        assert!(
            n_flagged > 0,
            "Should flag some cells near the sphere surface"
        );
        assert!(
            n_flagged < mesh.cells.len(),
            "Should not flag all cells"
        );
    }

    #[test]
    fn test_curvature_flags_flat_plane() {
        let mesh = StructuredMesh::uniform(5, 5, 1, 5.0, 5.0, 1.0).to_unstructured();
        // Flat plane SDF: curvature is zero everywhere
        let sdf = |p: [f64; 3]| p[1] - 2.5;
        let flags = curvature_refinement_flags(&mesh, &sdf, 1.0);
        let n_flagged: usize = flags.iter().filter(|&&f| f).count();
        assert_eq!(
            n_flagged, 0,
            "Flat plane should have zero curvature, no flags"
        );
    }

    #[test]
    fn test_proximity_flags_basic() {
        let mesh = StructuredMesh::uniform(10, 1, 1, 10.0, 1.0, 1.0).to_unstructured();
        // Distance field: distance from x=5
        let distance_field: Vec<f64> = mesh
            .cells
            .iter()
            .map(|c| (c.center[0] - 5.0).abs())
            .collect();
        let flags = proximity_refinement_flags(&mesh, &distance_field, 0);
        let n_flagged: usize = flags.iter().filter(|&&f| f).count();
        assert!(n_flagged > 0, "Should flag cells near x=5");
        assert!(n_flagged < mesh.cells.len(), "Should not flag all cells");
    }

    #[test]
    fn test_proximity_flags_with_layers() {
        let mesh = StructuredMesh::uniform(10, 1, 1, 10.0, 1.0, 1.0).to_unstructured();
        let distance_field: Vec<f64> = mesh
            .cells
            .iter()
            .map(|c| (c.center[0] - 5.0).abs())
            .collect();
        let flags_0 = proximity_refinement_flags(&mesh, &distance_field, 0);
        let flags_2 = proximity_refinement_flags(&mesh, &distance_field, 2);
        let n0: usize = flags_0.iter().filter(|&&f| f).count();
        let n2: usize = flags_2.iter().filter(|&&f| f).count();
        assert!(
            n2 >= n0,
            "More layers should flag at least as many cells: n0={n0}, n2={n2}"
        );
    }

    #[test]
    fn test_proximity_flags_wrong_size() {
        let mesh = StructuredMesh::uniform(5, 5, 1, 5.0, 5.0, 1.0).to_unstructured();
        let flags = proximity_refinement_flags(&mesh, &[1.0, 2.0], 0);
        assert!(
            flags.iter().all(|&f| !f),
            "Wrong-sized distance field should return no flags"
        );
    }
}
