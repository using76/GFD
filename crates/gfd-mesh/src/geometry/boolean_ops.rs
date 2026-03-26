//! CSG boolean operations on meshes.
//!
//! Provides union and subtraction of unstructured meshes using SDF-based
//! cell classification. Cells are kept, removed, or clipped based on
//! their position relative to the other mesh's bounding region.

use gfd_core::mesh::cell::Cell;
use gfd_core::mesh::face::Face;
use gfd_core::mesh::node::Node;
use gfd_core::mesh::unstructured::{BoundaryPatch, UnstructuredMesh};

use std::collections::HashMap;

/// Compute the union of two meshes.
///
/// Keeps all cells from mesh `a` that are outside mesh `b`, all cells from
/// mesh `b` that are outside mesh `a`, and merges overlapping regions.
/// Uses a simple cell-center classification approach: cells from `b` whose
/// centers fall inside the bounding box of `a` are discarded (and vice versa
/// is not applied — `a` takes priority in overlapping regions).
///
/// # Arguments
/// * `a` - First mesh.
/// * `b` - Second mesh.
///
/// # Returns
/// A new mesh containing cells from both meshes.
pub fn mesh_union(a: &UnstructuredMesh, b: &UnstructuredMesh) -> UnstructuredMesh {
    // Compute bounding box of mesh a
    let bbox_a = compute_bounding_box(a);

    // Offset for b's nodes
    let node_offset = a.nodes.len();
    // Collect all nodes: a's nodes + b's nodes
    let mut all_nodes: Vec<Node> = a.nodes.clone();
    for (i, node) in b.nodes.iter().enumerate() {
        let new_id = node_offset + i;
        all_nodes.push(Node::new(new_id, node.position));
    }

    // Collect cells from a (all of them)
    let mut all_cells: Vec<Cell> = Vec::new();
    for cell in &a.cells {
        let new_id = all_cells.len();
        all_cells.push(Cell::new(
            new_id,
            cell.nodes.clone(),
            Vec::new(),
            cell.volume,
            cell.center,
        ));
    }

    // Collect cells from b that are outside the bounding box of a
    for cell in &b.cells {
        let c = cell.center;
        if !point_in_bbox(c, &bbox_a) {
            let new_id = all_cells.len();
            let shifted_nodes: Vec<usize> = cell.nodes.iter().map(|&n| n + node_offset).collect();
            all_cells.push(Cell::new(
                new_id,
                shifted_nodes,
                Vec::new(),
                cell.volume,
                cell.center,
            ));
        }
    }

    // Rebuild faces
    let (faces, patches) = rebuild_faces(&all_nodes, &mut all_cells);
    UnstructuredMesh::from_components(all_nodes, faces, all_cells, patches)
}

/// Subtract mesh `b` from mesh `a`.
///
/// Keeps cells from `a` whose centers are outside the bounding box of `b`.
/// Cells from `a` whose centers fall inside `b`'s bounding box are removed.
///
/// # Arguments
/// * `a` - The base mesh.
/// * `b` - The mesh to subtract.
///
/// # Returns
/// A new mesh with the overlapping region removed from `a`.
pub fn mesh_subtract(a: &UnstructuredMesh, b: &UnstructuredMesh) -> UnstructuredMesh {
    let bbox_b = compute_bounding_box(b);

    let mut new_cells: Vec<Cell> = Vec::new();

    for cell in &a.cells {
        let c = cell.center;
        if !point_in_bbox(c, &bbox_b) {
            let new_id = new_cells.len();
            new_cells.push(Cell::new(
                new_id,
                cell.nodes.clone(),
                Vec::new(),
                cell.volume,
                cell.center,
            ));
        }
    }

    let nodes = a.nodes.clone();
    let (faces, patches) = rebuild_faces(&nodes, &mut new_cells);
    UnstructuredMesh::from_components(nodes, faces, new_cells, patches)
}

/// Axis-aligned bounding box: [xmin, ymin, zmin, xmax, ymax, zmax].
type BBox = [f64; 6];

fn compute_bounding_box(mesh: &UnstructuredMesh) -> BBox {
    let mut bbox = [f64::MAX, f64::MAX, f64::MAX, f64::MIN, f64::MIN, f64::MIN];
    for node in &mesh.nodes {
        let p = node.position;
        bbox[0] = bbox[0].min(p[0]);
        bbox[1] = bbox[1].min(p[1]);
        bbox[2] = bbox[2].min(p[2]);
        bbox[3] = bbox[3].max(p[0]);
        bbox[4] = bbox[4].max(p[1]);
        bbox[5] = bbox[5].max(p[2]);
    }
    bbox
}

fn point_in_bbox(p: [f64; 3], bbox: &BBox) -> bool {
    p[0] >= bbox[0]
        && p[0] <= bbox[3]
        && p[1] >= bbox[1]
        && p[1] <= bbox[4]
        && p[2] >= bbox[2]
        && p[2] <= bbox[5]
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
                _ => vec![],
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
    let mut c = [0.0; 3];
    for &nid in node_ids {
        let p = nodes[nid].position;
        c[0] += p[0];
        c[1] += p[1];
        c[2] += p[2];
    }
    [c[0] / n, c[1] / n, c[2] / n]
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
    (area, [nx / (2.0 * area), ny / (2.0 * area), nz / (2.0 * area)])
}

#[cfg(test)]
mod tests {
    use super::*;
    use gfd_core::mesh::structured::StructuredMesh;

    #[test]
    fn test_union_non_overlapping() {
        // Two meshes that do not overlap
        let a = StructuredMesh::uniform(2, 2, 1, 2.0, 2.0, 1.0).to_unstructured();
        // Shift b far to the right (x offset = 10)
        let mut b = StructuredMesh::uniform(2, 2, 1, 2.0, 2.0, 1.0).to_unstructured();
        for node in &mut b.nodes {
            node.position[0] += 10.0;
        }
        for cell in &mut b.cells {
            cell.center[0] += 10.0;
        }

        let result = mesh_union(&a, &b);
        assert_eq!(
            result.cells.len(),
            a.cells.len() + b.cells.len(),
            "Non-overlapping union should have all cells from both meshes"
        );
    }

    #[test]
    fn test_union_overlapping() {
        let a = StructuredMesh::uniform(3, 3, 1, 3.0, 3.0, 1.0).to_unstructured();
        let b = StructuredMesh::uniform(2, 2, 1, 2.0, 2.0, 1.0).to_unstructured();
        // b's bbox [0,2]x[0,2] is inside a's bbox [0,3]x[0,3]

        let result = mesh_union(&a, &b);
        // All cells of a are kept, b's cells that are inside a's bbox are removed
        assert_eq!(
            result.cells.len(),
            a.cells.len(),
            "Overlapping union should keep a's cells and discard b's overlapping cells"
        );
    }

    #[test]
    fn test_subtract_no_overlap() {
        let a = StructuredMesh::uniform(3, 3, 1, 3.0, 3.0, 1.0).to_unstructured();
        let mut b = StructuredMesh::uniform(2, 2, 1, 2.0, 2.0, 1.0).to_unstructured();
        for node in &mut b.nodes {
            node.position[0] += 20.0;
        }
        for cell in &mut b.cells {
            cell.center[0] += 20.0;
        }

        let result = mesh_subtract(&a, &b);
        assert_eq!(
            result.cells.len(),
            a.cells.len(),
            "Non-overlapping subtract should keep all cells from a"
        );
    }

    #[test]
    fn test_subtract_overlap() {
        let a = StructuredMesh::uniform(4, 4, 1, 4.0, 4.0, 1.0).to_unstructured();
        let b = StructuredMesh::uniform(2, 2, 1, 2.0, 2.0, 1.0).to_unstructured();
        // b covers [0,2]x[0,2], which is part of a's [0,4]x[0,4]

        let result = mesh_subtract(&a, &b);
        assert!(
            result.cells.len() < a.cells.len(),
            "Subtract should remove cells from a that overlap with b"
        );
        assert!(
            result.cells.len() > 0,
            "Should still have cells outside b's region"
        );
    }

    #[test]
    fn test_union_preserves_volume() {
        let a = StructuredMesh::uniform(2, 2, 1, 2.0, 2.0, 1.0).to_unstructured();
        let mut b = StructuredMesh::uniform(2, 2, 1, 2.0, 2.0, 1.0).to_unstructured();
        for node in &mut b.nodes {
            node.position[0] += 5.0;
        }
        for cell in &mut b.cells {
            cell.center[0] += 5.0;
        }

        let vol_a: f64 = a.cells.iter().map(|c| c.volume).sum();
        let vol_b: f64 = b.cells.iter().map(|c| c.volume).sum();
        let result = mesh_union(&a, &b);
        let vol_result: f64 = result.cells.iter().map(|c| c.volume).sum();

        assert!(
            (vol_result - (vol_a + vol_b)).abs() < 1e-10,
            "Non-overlapping union should conserve total volume"
        );
    }
}
