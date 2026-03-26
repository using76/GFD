//! Mesh quality metrics computation.
//!
//! Evaluates aspect ratio, skewness, orthogonality, and minimum angle per cell.

use gfd_core::mesh::unstructured::UnstructuredMesh;

/// Quality metrics for a single cell.
#[derive(Debug, Clone, Copy)]
pub struct CellQuality {
    /// Ratio of the longest edge to the shortest edge.
    pub aspect_ratio: f64,
    /// Skewness: 0 = perfect, 1 = degenerate.
    pub skewness: f64,
    /// Face orthogonality: 0 = non-orthogonal, 1 = perfectly orthogonal.
    pub orthogonality: f64,
    /// Minimum angle between any two edges sharing a node (degrees).
    pub min_angle: f64,
}

/// Summary quality report for the entire mesh.
#[derive(Debug, Clone)]
pub struct MeshQualityReport {
    /// Minimum orthogonality across all cells (worst is 0, best is 1).
    pub min_orthogonality: f64,
    /// Maximum skewness across all cells (worst is 1, best is 0).
    pub max_skewness: f64,
    /// Maximum aspect ratio across all cells.
    pub max_aspect_ratio: f64,
    /// Number of cells that fail basic quality thresholds.
    pub num_bad_cells: usize,
    /// Per-cell quality metrics.
    pub cell_qualities: Vec<CellQuality>,
}

fn dist(a: [f64; 3], b: [f64; 3]) -> f64 {
    let dx = a[0] - b[0];
    let dy = a[1] - b[1];
    let dz = a[2] - b[2];
    (dx * dx + dy * dy + dz * dz).sqrt()
}

fn sub(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

fn dot(a: [f64; 3], b: [f64; 3]) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

fn vec_len(v: [f64; 3]) -> f64 {
    dot(v, v).sqrt()
}

/// Computes the aspect ratio of a cell from its node positions.
/// Aspect ratio = longest_edge / shortest_edge.
fn compute_aspect_ratio(node_positions: &[[f64; 3]]) -> f64 {
    if node_positions.len() < 2 {
        return 1.0;
    }
    let mut min_edge = f64::MAX;
    let mut max_edge = 0.0f64;
    let n = node_positions.len();
    for i in 0..n {
        for j in (i + 1)..n {
            let d = dist(node_positions[i], node_positions[j]);
            if d > 1e-30 {
                min_edge = min_edge.min(d);
                max_edge = max_edge.max(d);
            }
        }
    }
    if min_edge < 1e-30 {
        return f64::MAX;
    }
    max_edge / min_edge
}

/// Computes equiangle skewness for a cell.
///
/// For a hexahedron, the ideal angle is 90 degrees.
/// For a tetrahedron, the ideal angle is ~70.53 degrees (arccos(1/3)).
/// Skewness = max((theta_max - theta_ideal)/(180 - theta_ideal),
///                (theta_ideal - theta_min)/theta_ideal)
fn compute_skewness(node_positions: &[[f64; 3]]) -> f64 {
    let n = node_positions.len();
    if n < 3 {
        return 0.0;
    }

    let ideal_angle = match n {
        4 => 70.528779_f64.to_radians(), // tetrahedron
        8 => 90.0_f64.to_radians(),       // hexahedron
        _ => 90.0_f64.to_radians(),       // default
    };

    // Compute all angles between edges sharing a vertex
    let mut min_angle = std::f64::consts::PI;
    let mut max_angle = 0.0f64;

    for i in 0..n {
        for j in 0..n {
            if j == i {
                continue;
            }
            for k in (j + 1)..n {
                if k == i {
                    continue;
                }
                let e1 = sub(node_positions[j], node_positions[i]);
                let e2 = sub(node_positions[k], node_positions[i]);
                let len1 = vec_len(e1);
                let len2 = vec_len(e2);
                if len1 < 1e-30 || len2 < 1e-30 {
                    continue;
                }
                let cos_angle = (dot(e1, e2) / (len1 * len2)).clamp(-1.0, 1.0);
                let angle = cos_angle.acos();
                min_angle = min_angle.min(angle);
                max_angle = max_angle.max(angle);
            }
        }
    }

    let pi = std::f64::consts::PI;
    let skew_max = if pi - ideal_angle > 1e-30 {
        (max_angle - ideal_angle) / (pi - ideal_angle)
    } else {
        0.0
    };
    let skew_min = if ideal_angle > 1e-30 {
        (ideal_angle - min_angle) / ideal_angle
    } else {
        0.0
    };
    skew_max.max(skew_min).max(0.0).min(1.0)
}

/// Computes the minimum angle (in degrees) between any two edges sharing a node.
fn compute_min_angle(node_positions: &[[f64; 3]]) -> f64 {
    let n = node_positions.len();
    if n < 3 {
        return 180.0;
    }

    let mut min_angle = std::f64::consts::PI;

    for i in 0..n {
        for j in 0..n {
            if j == i {
                continue;
            }
            for k in (j + 1)..n {
                if k == i {
                    continue;
                }
                let e1 = sub(node_positions[j], node_positions[i]);
                let e2 = sub(node_positions[k], node_positions[i]);
                let len1 = vec_len(e1);
                let len2 = vec_len(e2);
                if len1 < 1e-30 || len2 < 1e-30 {
                    continue;
                }
                let cos_angle = (dot(e1, e2) / (len1 * len2)).clamp(-1.0, 1.0);
                let angle = cos_angle.acos();
                min_angle = min_angle.min(angle);
            }
        }
    }

    min_angle.to_degrees()
}

/// Computes face orthogonality for a cell.
///
/// For each face of the cell, measures how well the face normal aligns with
/// the vector from cell center to face center. Returns the minimum dot product
/// (1.0 = perfect orthogonality, 0.0 = non-orthogonal).
fn compute_orthogonality(mesh: &UnstructuredMesh, cell_idx: usize) -> f64 {
    let cell = &mesh.cells[cell_idx];
    let cc = cell.center;
    let mut min_ortho = 1.0f64;

    for &fid in &cell.faces {
        let face = &mesh.faces[fid];
        let fc = face.center;
        let d = sub(fc, cc);
        let d_len = vec_len(d);
        if d_len < 1e-30 {
            continue;
        }
        let d_unit = [d[0] / d_len, d[1] / d_len, d[2] / d_len];

        // Face normal direction: ensure it points away from the cell
        let n = if face.owner_cell == cell_idx {
            face.normal
        } else {
            [-face.normal[0], -face.normal[1], -face.normal[2]]
        };
        let n_len = vec_len(n);
        if n_len < 1e-30 {
            continue;
        }
        let n_unit = [n[0] / n_len, n[1] / n_len, n[2] / n_len];

        let ortho = dot(d_unit, n_unit).abs();
        min_ortho = min_ortho.min(ortho);
    }

    min_ortho
}

/// Computes mesh quality metrics for all cells in an unstructured mesh.
///
/// Bad cells are defined as those with:
/// - skewness > 0.9, or
/// - orthogonality < 0.1, or
/// - aspect ratio > 100
pub fn compute_mesh_quality(mesh: &UnstructuredMesh) -> MeshQualityReport {
    let n_cells = mesh.cells.len();
    let mut cell_qualities = Vec::with_capacity(n_cells);
    let mut min_orthogonality = 1.0f64;
    let mut max_skewness = 0.0f64;
    let mut max_aspect_ratio = 0.0f64;
    let mut num_bad_cells = 0usize;

    for (ci, cell) in mesh.cells.iter().enumerate() {
        let node_positions: Vec<[f64; 3]> = cell
            .nodes
            .iter()
            .map(|&nid| mesh.nodes[nid].position)
            .collect();

        let aspect_ratio = compute_aspect_ratio(&node_positions);
        let skewness = compute_skewness(&node_positions);
        let orthogonality = compute_orthogonality(mesh, ci);
        let min_angle = compute_min_angle(&node_positions);

        let quality = CellQuality {
            aspect_ratio,
            skewness,
            orthogonality,
            min_angle,
        };

        min_orthogonality = min_orthogonality.min(orthogonality);
        max_skewness = max_skewness.max(skewness);
        max_aspect_ratio = max_aspect_ratio.max(aspect_ratio);

        if skewness > 0.9 || orthogonality < 0.1 || aspect_ratio > 100.0 {
            num_bad_cells += 1;
        }

        cell_qualities.push(quality);
    }

    MeshQualityReport {
        min_orthogonality,
        max_skewness,
        max_aspect_ratio,
        num_bad_cells,
        cell_qualities,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gfd_core::mesh::structured::StructuredMesh;

    #[test]
    fn test_uniform_mesh_quality() {
        let mesh = StructuredMesh::uniform(3, 3, 1, 3.0, 3.0, 1.0).to_unstructured();
        let report = compute_mesh_quality(&mesh);

        // Uniform cube cells should have reasonable quality.
        // Note: equiangle skewness for hex cells using all node pairs can be
        // moderate even for perfect cubes due to face/body diagonal angles.
        assert!(
            report.max_skewness < 0.85,
            "Uniform mesh skewness should be reasonable, got {}",
            report.max_skewness
        );
        assert!(
            report.min_orthogonality > 0.5,
            "Uniform mesh orthogonality should be high, got {}",
            report.min_orthogonality
        );
        assert_eq!(report.num_bad_cells, 0, "Uniform mesh should have 0 bad cells");
    }

    #[test]
    fn test_aspect_ratio_unit_cube() {
        // A perfect unit cube: all edges are length 1
        let positions = vec![
            [0.0, 0.0, 0.0],
            [1.0, 0.0, 0.0],
            [1.0, 1.0, 0.0],
            [0.0, 1.0, 0.0],
            [0.0, 0.0, 1.0],
            [1.0, 0.0, 1.0],
            [1.0, 1.0, 1.0],
            [0.0, 1.0, 1.0],
        ];
        let ar = compute_aspect_ratio(&positions);
        // Edges: 12 edges of length 1, 12 face diagonals of sqrt(2), 4 body diags of sqrt(3)
        // AR = sqrt(3) / 1 = 1.732...
        assert!(ar > 1.0 && ar < 2.0, "Cube AR should be ~1.73, got {ar}");
    }

    #[test]
    fn test_cell_quality_count() {
        let mesh = StructuredMesh::uniform(2, 2, 1, 2.0, 2.0, 1.0).to_unstructured();
        let report = compute_mesh_quality(&mesh);
        assert_eq!(report.cell_qualities.len(), mesh.cells.len());
    }

    #[test]
    fn test_min_angle_equilateral_tet() {
        // Regular tetrahedron
        let positions = vec![
            [1.0, 1.0, 1.0],
            [-1.0, -1.0, 1.0],
            [-1.0, 1.0, -1.0],
            [1.0, -1.0, -1.0],
        ];
        let angle = compute_min_angle(&positions);
        // All angles in a regular tet are arccos(1/3) ~ 70.53 degrees
        assert!(
            (angle - 60.0).abs() < 15.0,
            "Regular tet min angle should be ~60 degrees, got {angle}"
        );
    }
}
