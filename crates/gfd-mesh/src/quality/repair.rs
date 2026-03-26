//! Mesh repair utilities.
//!
//! Fixes common mesh defects such as negative-volume cells and high-skewness cells.

use gfd_core::mesh::cell::Cell;
use gfd_core::mesh::face::Face;
use gfd_core::mesh::node::Node;
use gfd_core::mesh::unstructured::{BoundaryPatch, UnstructuredMesh};

use std::collections::HashMap;

/// Fix cells with negative volume by flipping face orientations.
///
/// For each cell with negative volume, the node ordering is reversed to fix
/// the orientation, and the volume is made positive. Face normals are
/// recalculated after the fix.
///
/// # Arguments
/// * `mesh` - The mesh to fix (modified in place).
///
/// # Returns
/// The number of cells that were fixed.
pub fn fix_negative_volumes(mesh: &mut UnstructuredMesh) -> usize {
    let mut num_fixed = 0;

    for cell in &mut mesh.cells {
        if cell.volume < 0.0 {
            // Reverse node ordering to fix orientation
            cell.nodes.reverse();
            cell.volume = cell.volume.abs();
            num_fixed += 1;
        }
    }

    if num_fixed > 0 {
        // Recompute face normals based on corrected cell orientations
        recompute_face_normals(mesh);
    }

    num_fixed
}

/// Fix cells with high skewness by inserting the centroid and splitting.
///
/// For each cell with equiangle skewness above `max_skewness`, the cell is
/// replaced by multiple sub-cells formed by connecting each face of the
/// original cell to the cell centroid (centroid decomposition).
///
/// # Arguments
/// * `mesh` - The mesh to fix (modified in place).
/// * `max_skewness` - Maximum allowed skewness (0.0-1.0). Cells above this are split.
///
/// # Returns
/// The number of cells that were split.
pub fn fix_high_skewness(mesh: &mut UnstructuredMesh, max_skewness: f64) -> usize {
    let n_cells = mesh.cells.len();

    // Compute skewness for each cell
    let mut bad_cells: Vec<usize> = Vec::new();
    for ci in 0..n_cells {
        let skewness = compute_cell_skewness(mesh, ci);
        if skewness > max_skewness {
            bad_cells.push(ci);
        }
    }

    if bad_cells.is_empty() {
        return 0;
    }

    let num_split = bad_cells.len();
    let bad_set: std::collections::HashSet<usize> = bad_cells.iter().copied().collect();

    let mut new_nodes = mesh.nodes.clone();
    let mut new_cells: Vec<Cell> = Vec::new();

    for ci in 0..n_cells {
        if bad_set.contains(&ci) {
            let cell = &mesh.cells[ci];

            // Add centroid as new node
            let centroid_id = new_nodes.len();
            new_nodes.push(Node::new(centroid_id, cell.center));

            // Get face node lists for this cell type
            let face_lists = cell_face_node_lists(cell);
            let sub_vol = cell.volume / face_lists.len().max(1) as f64;

            for face_nodes in &face_lists {
                let new_id = new_cells.len();
                let mut sub_nodes = face_nodes.clone();
                sub_nodes.push(centroid_id);

                let center = compute_center_from_nodes(&new_nodes, &sub_nodes);
                new_cells.push(Cell::new(new_id, sub_nodes, Vec::new(), sub_vol, center));
            }
        } else {
            let new_id = new_cells.len();
            let cell = &mesh.cells[ci];
            new_cells.push(Cell::new(
                new_id,
                cell.nodes.clone(),
                Vec::new(),
                cell.volume,
                cell.center,
            ));
        }
    }

    // Rebuild faces
    let (new_faces, boundary_patches) = rebuild_faces(&new_nodes, &mut new_cells);
    *mesh = UnstructuredMesh::from_components(new_nodes, new_faces, new_cells, boundary_patches);

    num_split
}

/// Compute equiangle skewness for a cell.
fn compute_cell_skewness(mesh: &UnstructuredMesh, cell_idx: usize) -> f64 {
    let cell = &mesh.cells[cell_idx];
    let positions: Vec<[f64; 3]> = cell
        .nodes
        .iter()
        .map(|&nid| mesh.nodes[nid].position)
        .collect();

    let n = positions.len();
    if n < 3 {
        return 0.0;
    }

    let ideal_angle = match n {
        4 => 70.528779_f64.to_radians(),
        8 => 90.0_f64.to_radians(),
        _ => 90.0_f64.to_radians(),
    };

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
                let e1 = sub3(positions[j], positions[i]);
                let e2 = sub3(positions[k], positions[i]);
                let len1 = len3(e1);
                let len2 = len3(e2);
                if len1 < 1e-30 || len2 < 1e-30 {
                    continue;
                }
                let cos_a = (dot3(e1, e2) / (len1 * len2)).clamp(-1.0, 1.0);
                let angle = cos_a.acos();
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

fn sub3(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

fn dot3(a: [f64; 3], b: [f64; 3]) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

fn len3(v: [f64; 3]) -> f64 {
    dot3(v, v).sqrt()
}

fn cell_face_node_lists(cell: &Cell) -> Vec<Vec<usize>> {
    let cn = &cell.nodes;
    match cn.len() {
        8 => vec![
            vec![cn[0], cn[1], cn[2], cn[3]],
            vec![cn[4], cn[5], cn[6], cn[7]],
            vec![cn[0], cn[1], cn[5], cn[4]],
            vec![cn[3], cn[2], cn[6], cn[7]],
            vec![cn[0], cn[3], cn[7], cn[4]],
            vec![cn[1], cn[2], cn[6], cn[5]],
        ],
        4 => vec![
            vec![cn[0], cn[1], cn[2]],
            vec![cn[0], cn[1], cn[3]],
            vec![cn[0], cn[2], cn[3]],
            vec![cn[1], cn[2], cn[3]],
        ],
        _ => vec![cn.clone()],
    }
}

fn compute_center_from_nodes(nodes: &[Node], node_ids: &[usize]) -> [f64; 3] {
    let n = node_ids.len() as f64;
    if n < 1.0 {
        return [0.0; 3];
    }
    let mut c = [0.0; 3];
    for &nid in node_ids {
        let p = nodes[nid].position;
        c[0] += p[0];
        c[1] += p[1];
        c[2] += p[2];
    }
    [c[0] / n, c[1] / n, c[2] / n]
}

/// Recompute face normals after cell orientation changes.
fn recompute_face_normals(mesh: &mut UnstructuredMesh) {
    let node_positions: Vec<[f64; 3]> = mesh.nodes.iter().map(|n| n.position).collect();

    for face in &mut mesh.faces {
        let n = face.nodes.len();
        if n < 3 {
            continue;
        }

        // Recompute center
        let mut cx = 0.0;
        let mut cy = 0.0;
        let mut cz = 0.0;
        for &nid in &face.nodes {
            let p = node_positions[nid];
            cx += p[0];
            cy += p[1];
            cz += p[2];
        }
        let inv_n = 1.0 / n as f64;
        face.center = [cx * inv_n, cy * inv_n, cz * inv_n];

        // Newell's method for area and normal
        let mut nx = 0.0f64;
        let mut ny = 0.0f64;
        let mut nz = 0.0f64;
        for i in 0..n {
            let p1 = node_positions[face.nodes[i]];
            let p2 = node_positions[face.nodes[(i + 1) % n]];
            nx += (p1[1] - p2[1]) * (p1[2] + p2[2]);
            ny += (p1[2] - p2[2]) * (p1[0] + p2[0]);
            nz += (p1[0] - p2[0]) * (p1[1] + p2[1]);
        }
        let area = 0.5 * (nx * nx + ny * ny + nz * nz).sqrt();
        if area > 1e-30 {
            face.area = area;
            face.normal = [nx / (2.0 * area), ny / (2.0 * area), nz / (2.0 * area)];
        }
    }
}

fn rebuild_faces(
    nodes: &[Node],
    cells: &mut Vec<Cell>,
) -> (Vec<Face>, Vec<BoundaryPatch>) {
    let mut face_map: HashMap<Vec<usize>, (usize, Vec<usize>)> = HashMap::new();
    let mut faces: Vec<Face> = Vec::new();
    let mut boundary_face_ids: Vec<usize> = Vec::new();

    let cell_face_nodes: Vec<Vec<Vec<usize>>> = cells
        .iter()
        .map(|cell| cell_face_node_lists(cell))
        .collect();

    for (ci, face_list) in cell_face_nodes.iter().enumerate() {
        for fn_nodes in face_list {
            let mut sorted = fn_nodes.clone();
            sorted.sort();

            if let Some((owner_ci, _)) = face_map.get(&sorted) {
                let fid = faces.len();
                let fc = face_center_calc(nodes, fn_nodes);
                let (area, normal) = face_area_normal_calc(nodes, fn_nodes);
                faces.push(Face::new(fid, fn_nodes.clone(), *owner_ci, Some(ci), area, normal, fc));
                cells[*owner_ci].faces.push(fid);
                cells[ci].faces.push(fid);
                face_map.remove(&sorted);
            } else {
                face_map.insert(sorted, (ci, fn_nodes.clone()));
            }
        }
    }

    for (_key, (owner_ci, fn_nodes)) in &face_map {
        let fid = faces.len();
        let fc = face_center_calc(nodes, fn_nodes);
        let (area, normal) = face_area_normal_calc(nodes, fn_nodes);
        faces.push(Face::new(fid, fn_nodes.clone(), *owner_ci, None, area, normal, fc));
        cells[*owner_ci].faces.push(fid);
        boundary_face_ids.push(fid);
    }

    let mut patches = Vec::new();
    if !boundary_face_ids.is_empty() {
        patches.push(BoundaryPatch::new("boundary", boundary_face_ids));
    }
    (faces, patches)
}

fn face_center_calc(nodes: &[Node], node_ids: &[usize]) -> [f64; 3] {
    let n = node_ids.len() as f64;
    let mut c = [0.0; 3];
    for &nid in node_ids {
        let p = nodes[nid].position;
        c[0] += p[0];
        c[1] += p[1];
        c[2] += p[2];
    }
    [c[0] / n, c[1] / n, c[2] / n]
}

fn face_area_normal_calc(nodes: &[Node], node_ids: &[usize]) -> (f64, [f64; 3]) {
    if node_ids.len() < 3 {
        return (0.0, [0.0, 0.0, 1.0]);
    }
    let n = node_ids.len();
    let mut nx = 0.0f64;
    let mut ny = 0.0f64;
    let mut nz = 0.0f64;
    for i in 0..n {
        let p1 = nodes[node_ids[i]].position;
        let p2 = nodes[node_ids[(i + 1) % n]].position;
        nx += (p1[1] - p2[1]) * (p1[2] + p2[2]);
        ny += (p1[2] - p2[2]) * (p1[0] + p2[0]);
        nz += (p1[0] - p2[0]) * (p1[1] + p2[1]);
    }
    let area = 0.5 * (nx * nx + ny * ny + nz * nz).sqrt();
    if area < 1e-30 {
        return (0.0, [0.0, 0.0, 1.0]);
    }
    (area, [nx / (2.0 * area), ny / (2.0 * area), nz / (2.0 * area)])
}

#[cfg(test)]
mod tests {
    use super::*;
    use gfd_core::mesh::structured::StructuredMesh;

    #[test]
    fn test_fix_negative_volumes_none_negative() {
        let mut mesh = StructuredMesh::uniform(3, 3, 1, 3.0, 3.0, 1.0).to_unstructured();
        let fixed = fix_negative_volumes(&mut mesh);
        assert_eq!(fixed, 0, "Uniform mesh should have no negative volumes");
    }

    #[test]
    fn test_fix_negative_volumes_with_negative() {
        let mut mesh = StructuredMesh::uniform(2, 2, 1, 2.0, 2.0, 1.0).to_unstructured();
        // Artificially set negative volumes
        mesh.cells[0].volume = -1.0;
        mesh.cells[1].volume = -0.5;
        let fixed = fix_negative_volumes(&mut mesh);
        assert_eq!(fixed, 2);
        assert!(mesh.cells[0].volume > 0.0);
        assert!(mesh.cells[1].volume > 0.0);
    }

    #[test]
    fn test_fix_negative_volumes_preserves_positive() {
        let mut mesh = StructuredMesh::uniform(2, 1, 1, 2.0, 1.0, 1.0).to_unstructured();
        let original_vol = mesh.cells[1].volume;
        mesh.cells[0].volume = -1.0;
        fix_negative_volumes(&mut mesh);
        assert!(
            (mesh.cells[1].volume - original_vol).abs() < 1e-10,
            "Positive-volume cells should not change"
        );
    }

    #[test]
    fn test_fix_high_skewness_uniform() {
        let mut mesh = StructuredMesh::uniform(3, 3, 1, 3.0, 3.0, 1.0).to_unstructured();
        // Uniform mesh cells have moderate skewness (~0.5-0.7) but below 0.95
        let split = fix_high_skewness(&mut mesh, 0.95);
        assert_eq!(split, 0, "Uniform mesh should have no cells above 0.95 skewness");
    }

    #[test]
    fn test_fix_high_skewness_all_split() {
        let mut mesh = StructuredMesh::uniform(2, 2, 1, 2.0, 2.0, 1.0).to_unstructured();
        let n_before = mesh.cells.len();
        // Threshold 0.0 means all cells get split
        let split = fix_high_skewness(&mut mesh, 0.0);
        assert_eq!(split, n_before, "All cells should be split with threshold 0.0");
        assert!(mesh.cells.len() > n_before);
    }

    #[test]
    fn test_fix_high_skewness_preserves_volume() {
        let mut mesh = StructuredMesh::uniform(2, 1, 1, 2.0, 1.0, 1.0).to_unstructured();
        let original_volume: f64 = mesh.cells.iter().map(|c| c.volume).sum();
        fix_high_skewness(&mut mesh, 0.0);
        let new_volume: f64 = mesh.cells.iter().map(|c| c.volume).sum();
        assert!(
            (original_volume - new_volume).abs() < 1e-10,
            "Volume should be conserved: original={original_volume}, new={new_volume}"
        );
    }
}
