//! Local remeshing for deformed meshes.
//!
//! Identifies cells with quality below a threshold, collects their nodes, and
//! re-triangulates/re-hexahedrates the region to restore mesh quality.

use gfd_core::mesh::cell::Cell;
use gfd_core::mesh::face::Face;
use gfd_core::mesh::node::Node;
use gfd_core::mesh::unstructured::{BoundaryPatch, UnstructuredMesh};

use std::collections::{HashMap, HashSet};

use crate::Result;

/// Remesh cells that have quality below the given threshold.
///
/// Quality is measured as the ratio of the shortest edge to the longest edge
/// (1.0 = perfect, 0.0 = degenerate). Cells with quality below `quality_threshold`
/// are replaced by new cells formed by splitting the cell from its centroid to
/// each face (centroid decomposition).
///
/// # Arguments
/// * `mesh` - The mesh to remesh (modified in place).
/// * `quality_threshold` - Cells with quality below this value will be remeshed.
///
/// # Returns
/// The number of cells that were remeshed.
pub fn local_remesh(
    mesh: &mut UnstructuredMesh,
    quality_threshold: f64,
) -> Result<usize> {
    // Compute quality for each cell
    let n_cells = mesh.cells.len();
    let mut bad_cells: Vec<usize> = Vec::new();

    for ci in 0..n_cells {
        let quality = compute_cell_quality(mesh, ci);
        if quality < quality_threshold {
            bad_cells.push(ci);
        }
    }

    if bad_cells.is_empty() {
        return Ok(0);
    }

    let num_remeshed = bad_cells.len();
    let bad_set: HashSet<usize> = bad_cells.iter().copied().collect();

    // Collect all non-bad cells and build replacement cells for bad ones
    let node_positions: Vec<[f64; 3]> = mesh.nodes.iter().map(|n| n.position).collect();
    let mut new_nodes = mesh.nodes.clone();
    let mut new_cells: Vec<Cell> = Vec::new();

    // Map from old cell index to new cell indices
    let mut old_to_new_map: HashMap<usize, Vec<usize>> = HashMap::new();

    for ci in 0..n_cells {
        if bad_set.contains(&ci) {
            // Centroid decomposition: add centroid node, then create sub-cells
            // by connecting each face to the centroid
            let cell = &mesh.cells[ci];
            let centroid = cell.center;
            let centroid_id = new_nodes.len();
            new_nodes.push(Node::new(centroid_id, centroid));

            let face_node_lists = get_cell_face_nodes(cell);
            let sub_vol = cell.volume / face_node_lists.len().max(1) as f64;

            let mut new_ids = Vec::new();
            for face_nodes in &face_node_lists {
                let new_id = new_cells.len();
                // Create a new cell from the face nodes + centroid
                let mut cell_nodes = face_nodes.clone();
                cell_nodes.push(centroid_id);

                let center = compute_center_from_positions(&node_positions, &new_nodes, &cell_nodes);
                new_cells.push(Cell::new(
                    new_id,
                    cell_nodes,
                    Vec::new(),
                    sub_vol,
                    center,
                ));
                new_ids.push(new_id);
            }
            old_to_new_map.insert(ci, new_ids);
        } else {
            let new_id = new_cells.len();
            let cell = &mesh.cells[ci];
            old_to_new_map.insert(ci, vec![new_id]);
            new_cells.push(Cell::new(
                new_id,
                cell.nodes.clone(),
                Vec::new(),
                cell.volume,
                cell.center,
            ));
        }
    }

    // Rebuild face connectivity
    let (new_faces, boundary_patches) = rebuild_faces_simple(&new_nodes, &mut new_cells);

    *mesh = UnstructuredMesh::from_components(new_nodes, new_faces, new_cells, boundary_patches);

    Ok(num_remeshed)
}

/// Compute quality of a cell as min_edge / max_edge.
fn compute_cell_quality(mesh: &UnstructuredMesh, cell_idx: usize) -> f64 {
    let cell = &mesh.cells[cell_idx];
    let n = cell.nodes.len();
    if n < 2 {
        return 0.0;
    }

    let mut min_edge = f64::MAX;
    let mut max_edge = 0.0f64;

    for i in 0..n {
        for j in (i + 1)..n {
            let p1 = mesh.nodes[cell.nodes[i]].position;
            let p2 = mesh.nodes[cell.nodes[j]].position;
            let d = ((p1[0] - p2[0]).powi(2)
                + (p1[1] - p2[1]).powi(2)
                + (p1[2] - p2[2]).powi(2))
            .sqrt();
            if d > 1e-30 {
                min_edge = min_edge.min(d);
                max_edge = max_edge.max(d);
            }
        }
    }

    if max_edge < 1e-30 {
        return 0.0;
    }
    min_edge / max_edge
}

/// Get the face node lists for a cell based on its topology.
fn get_cell_face_nodes(cell: &Cell) -> Vec<Vec<usize>> {
    let cn = &cell.nodes;
    match cn.len() {
        8 => {
            // Hexahedron: 6 quad faces
            vec![
                vec![cn[0], cn[1], cn[2], cn[3]],
                vec![cn[4], cn[5], cn[6], cn[7]],
                vec![cn[0], cn[1], cn[5], cn[4]],
                vec![cn[3], cn[2], cn[6], cn[7]],
                vec![cn[0], cn[3], cn[7], cn[4]],
                vec![cn[1], cn[2], cn[6], cn[5]],
            ]
        }
        4 => {
            // Tetrahedron: 4 triangle faces
            vec![
                vec![cn[0], cn[1], cn[2]],
                vec![cn[0], cn[1], cn[3]],
                vec![cn[0], cn[2], cn[3]],
                vec![cn[1], cn[2], cn[3]],
            ]
        }
        _ => {
            // Generic: create a single "face" from all nodes
            vec![cn.clone()]
        }
    }
}

/// Compute center from a mix of original and new node positions.
fn compute_center_from_positions(
    _orig_positions: &[[f64; 3]],
    all_nodes: &[Node],
    node_ids: &[usize],
) -> [f64; 3] {
    let n = node_ids.len() as f64;
    if n < 1.0 {
        return [0.0; 3];
    }
    let mut cx = 0.0;
    let mut cy = 0.0;
    let mut cz = 0.0;
    for &nid in node_ids {
        let p = all_nodes[nid].position;
        cx += p[0];
        cy += p[1];
        cz += p[2];
    }
    [cx / n, cy / n, cz / n]
}

/// Simple face rebuild from cell-node connectivity.
fn rebuild_faces_simple(
    nodes: &[Node],
    cells: &mut Vec<Cell>,
) -> (Vec<Face>, Vec<BoundaryPatch>) {
    let mut face_map: HashMap<Vec<usize>, (usize, Vec<usize>)> = HashMap::new();
    let mut faces: Vec<Face> = Vec::new();
    let mut boundary_face_ids: Vec<usize> = Vec::new();

    // Enumerate faces per cell
    let cell_face_nodes: Vec<Vec<Vec<usize>>> = cells
        .iter()
        .map(|cell| {
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
                n_nodes => {
                    // For pyramids (5 nodes) or generic cells, create face from
                    // pairs of consecutive nodes
                    if n_nodes >= 3 {
                        let mut fl = Vec::new();
                        for i in 0..n_nodes {
                            for j in (i + 1)..n_nodes {
                                for k in (j + 1)..n_nodes {
                                    fl.push(vec![cn[i], cn[j], cn[k]]);
                                }
                            }
                        }
                        fl
                    } else {
                        vec![]
                    }
                }
            }
        })
        .collect();

    for (ci, face_list) in cell_face_nodes.iter().enumerate() {
        for fn_nodes in face_list {
            let mut sorted = fn_nodes.clone();
            sorted.sort();

            if let Some((owner_ci, _)) = face_map.get(&sorted) {
                let fid = faces.len();
                let fc = face_center(nodes, fn_nodes);
                let (area, normal) = face_area_normal(nodes, fn_nodes);
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
        let fc = face_center(nodes, fn_nodes);
        let (area, normal) = face_area_normal(nodes, fn_nodes);
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

fn face_center(nodes: &[Node], node_ids: &[usize]) -> [f64; 3] {
    let n = node_ids.len() as f64;
    let mut cx = 0.0;
    let mut cy = 0.0;
    let mut cz = 0.0;
    for &nid in node_ids {
        let p = nodes[nid].position;
        cx += p[0];
        cy += p[1];
        cz += p[2];
    }
    [cx / n, cy / n, cz / n]
}

fn face_area_normal(nodes: &[Node], node_ids: &[usize]) -> (f64, [f64; 3]) {
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
    ([nx / (2.0 * area), ny / (2.0 * area), nz / (2.0 * area)], area).into_tuple()
}

/// Helper to return (area, normal) in the correct order.
trait IntoTuple {
    fn into_tuple(self) -> (f64, [f64; 3]);
}

impl IntoTuple for ([f64; 3], f64) {
    fn into_tuple(self) -> (f64, [f64; 3]) {
        (self.1, self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gfd_core::mesh::structured::StructuredMesh;

    #[test]
    fn test_local_remesh_no_bad_cells() {
        let mut mesh = StructuredMesh::uniform(3, 3, 1, 3.0, 3.0, 1.0).to_unstructured();
        let n_before = mesh.cells.len();
        let count = local_remesh(&mut mesh, 0.01).unwrap();
        // Uniform hex cells have quality ~0.577 (1/sqrt(3)), so with threshold 0.01 none are bad
        assert_eq!(count, 0, "No cells should be below quality 0.01");
        assert_eq!(mesh.cells.len(), n_before);
    }

    #[test]
    fn test_local_remesh_all_bad() {
        let mut mesh = StructuredMesh::uniform(2, 2, 1, 2.0, 2.0, 1.0).to_unstructured();
        let n_before = mesh.cells.len();
        // With threshold 1.0, all cells will be flagged (quality < 1.0 for any non-cube)
        let count = local_remesh(&mut mesh, 1.0).unwrap();
        assert_eq!(count, n_before, "All cells should be remeshed with threshold=1.0");
        assert!(mesh.cells.len() >= n_before, "Should have at least as many cells after remeshing");
    }

    #[test]
    fn test_local_remesh_produces_valid_mesh() {
        let mut mesh = StructuredMesh::uniform(2, 1, 1, 2.0, 1.0, 1.0).to_unstructured();
        let _ = local_remesh(&mut mesh, 1.0).unwrap();
        // Check all cells have positive volume
        for cell in &mesh.cells {
            assert!(cell.volume > 0.0, "Cell {} should have positive volume", cell.id);
        }
    }

    #[test]
    fn test_cell_quality_computation() {
        let mesh = StructuredMesh::uniform(1, 1, 1, 1.0, 1.0, 1.0).to_unstructured();
        let quality = compute_cell_quality(&mesh, 0);
        // A unit cube: min edge = 1.0, max edge = sqrt(3) ~ 1.732
        // quality = 1/sqrt(3) ~ 0.577
        assert!(quality > 0.5 && quality < 0.6, "Unit cube quality should be ~0.577, got {quality}");
    }
}
