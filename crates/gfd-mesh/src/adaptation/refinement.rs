//! Cell-based h-refinement for unstructured meshes.
//!
//! Supports splitting hexahedral cells into 8 sub-hexahedra and
//! tetrahedral cells into 8 sub-tetrahedra (red refinement).

use gfd_core::mesh::cell::Cell;
use gfd_core::mesh::face::Face;
use gfd_core::mesh::node::Node;
use gfd_core::mesh::unstructured::{BoundaryPatch, UnstructuredMesh};

use std::collections::HashMap;

/// Refine the specified cells in the mesh, returning a new mesh.
///
/// For hexahedral cells, each flagged cell is split into 8 sub-hexahedra by
/// inserting midpoints on each edge, face center, and cell center.
/// For tetrahedral cells, red refinement produces 8 sub-tetrahedra.
/// Non-flagged cells are carried over unchanged.
///
/// # Arguments
/// * `mesh` - The original mesh.
/// * `cells_to_refine` - Indices of cells to refine.
///
/// # Returns
/// A new `UnstructuredMesh` with the refined cells.
pub fn refine_cells(mesh: &UnstructuredMesh, cells_to_refine: &[usize]) -> UnstructuredMesh {
    let refine_set: std::collections::HashSet<usize> =
        cells_to_refine.iter().copied().collect();

    let mut new_nodes: Vec<Node> = mesh.nodes.clone();
    let mut new_cells: Vec<Cell> = Vec::new();

    // Cache for edge midpoints: (min_node, max_node) -> new_node_id
    let mut edge_midpoints: HashMap<(usize, usize), usize> = HashMap::new();

    let mut get_or_create_midpoint =
        |n1: usize, n2: usize, nodes: &mut Vec<Node>| -> usize {
            let key = if n1 < n2 { (n1, n2) } else { (n2, n1) };
            if let Some(&mid) = edge_midpoints.get(&key) {
                return mid;
            }
            let p1 = nodes[key.0].position;
            let p2 = nodes[key.1].position;
            let mid_pos = [
                (p1[0] + p2[0]) * 0.5,
                (p1[1] + p2[1]) * 0.5,
                (p1[2] + p2[2]) * 0.5,
            ];
            let mid_id = nodes.len();
            nodes.push(Node::new(mid_id, mid_pos));
            edge_midpoints.insert(key, mid_id);
            mid_id
        };

    for (ci, cell) in mesh.cells.iter().enumerate() {
        if !refine_set.contains(&ci) {
            // Keep unchanged
            let new_id = new_cells.len();
            new_cells.push(Cell::new(
                new_id,
                cell.nodes.clone(),
                Vec::new(), // Faces will be rebuilt
                cell.volume,
                cell.center,
            ));
            continue;
        }

        let n_nodes = cell.nodes.len();

        if n_nodes == 8 {
            // Hexahedron refinement: split into 8 sub-hexahedra
            refine_hex(cell, &mut new_nodes, &mut new_cells, &mut get_or_create_midpoint);
        } else if n_nodes == 4 {
            // Tetrahedron refinement: red refinement into 8 sub-tetrahedra
            refine_tet(cell, &mut new_nodes, &mut new_cells, &mut get_or_create_midpoint);
        } else {
            // Unsupported cell type: keep as-is
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

    // Rebuild face connectivity
    let (new_faces, boundary_patches) = rebuild_faces(&new_nodes, &mut new_cells);

    UnstructuredMesh::from_components(new_nodes, new_faces, new_cells, boundary_patches)
}

/// Refines a hexahedron into 8 sub-hexahedra.
///
/// Node numbering for original hex:
/// ```text
///     7------6
///    /|     /|
///   4------5 |
///   | 3----|-2
///   |/     |/
///   0------1
/// ```
fn refine_hex<F>(
    cell: &Cell,
    nodes: &mut Vec<Node>,
    new_cells: &mut Vec<Cell>,
    get_midpoint: &mut F,
) where
    F: FnMut(usize, usize, &mut Vec<Node>) -> usize,
{
    let n = &cell.nodes;
    // Original 8 corner nodes
    let c0 = n[0]; let c1 = n[1]; let c2 = n[2]; let c3 = n[3];
    let c4 = n[4]; let c5 = n[5]; let c6 = n[6]; let c7 = n[7];

    // 12 edge midpoints
    let e01 = get_midpoint(c0, c1, nodes);
    let e12 = get_midpoint(c1, c2, nodes);
    let e23 = get_midpoint(c2, c3, nodes);
    let e03 = get_midpoint(c0, c3, nodes);
    let e45 = get_midpoint(c4, c5, nodes);
    let e56 = get_midpoint(c5, c6, nodes);
    let e67 = get_midpoint(c6, c7, nodes);
    let e47 = get_midpoint(c4, c7, nodes);
    let e04 = get_midpoint(c0, c4, nodes);
    let e15 = get_midpoint(c1, c5, nodes);
    let e26 = get_midpoint(c2, c6, nodes);
    let e37 = get_midpoint(c3, c7, nodes);

    // 6 face centers
    let f_bottom = add_center_node(nodes, &[c0, c1, c2, c3]); // z-min
    let f_top = add_center_node(nodes, &[c4, c5, c6, c7]);    // z-max
    let f_front = add_center_node(nodes, &[c0, c1, c5, c4]);  // y-min
    let f_back = add_center_node(nodes, &[c3, c2, c6, c7]);   // y-max
    let f_left = add_center_node(nodes, &[c0, c3, c7, c4]);   // x-min
    let f_right = add_center_node(nodes, &[c1, c2, c6, c5]);  // x-max

    // 1 cell center
    let cc = add_center_node(nodes, &[c0, c1, c2, c3, c4, c5, c6, c7]);

    // Create 8 sub-hexahedra
    let sub_hexes = [
        [c0, e01, f_bottom, e03, e04, f_front, cc, f_left],
        [e01, c1, e12, f_bottom, f_front, e15, f_right, cc],
        [f_bottom, e12, c2, e23, cc, f_right, e26, f_back],
        [e03, f_bottom, e23, c3, f_left, cc, f_back, e37],
        [e04, f_front, cc, f_left, c4, e45, f_top, e47],
        [f_front, e15, f_right, cc, e45, c5, e56, f_top],
        [cc, f_right, e26, f_back, f_top, e56, c6, e67],
        [f_left, cc, f_back, e37, e47, f_top, e67, c7],
    ];

    let sub_vol = cell.volume / 8.0;

    for sub in &sub_hexes {
        let new_id = new_cells.len();
        let center = compute_center(nodes, sub);
        new_cells.push(Cell::new(
            new_id,
            sub.to_vec(),
            Vec::new(),
            sub_vol,
            center,
        ));
    }
}

/// Refines a tetrahedron into 8 sub-tetrahedra using red refinement.
///
/// Insert midpoints on all 6 edges, creating 4 corner tets and 4 interior tets
/// (formed by the octahedron in the middle, split into 4 tets).
fn refine_tet<F>(
    cell: &Cell,
    nodes: &mut Vec<Node>,
    new_cells: &mut Vec<Cell>,
    get_midpoint: &mut F,
) where
    F: FnMut(usize, usize, &mut Vec<Node>) -> usize,
{
    let n = &cell.nodes;
    let v0 = n[0]; let v1 = n[1]; let v2 = n[2]; let v3 = n[3];

    // 6 edge midpoints
    let m01 = get_midpoint(v0, v1, nodes);
    let m02 = get_midpoint(v0, v2, nodes);
    let m03 = get_midpoint(v0, v3, nodes);
    let m12 = get_midpoint(v1, v2, nodes);
    let m13 = get_midpoint(v1, v3, nodes);
    let m23 = get_midpoint(v2, v3, nodes);

    let sub_vol = cell.volume / 8.0;

    // 4 corner tetrahedra
    let corner_tets = [
        [v0, m01, m02, m03],
        [m01, v1, m12, m13],
        [m02, m12, v2, m23],
        [m03, m13, m23, v3],
    ];

    // 4 interior tetrahedra (splitting the central octahedron)
    let interior_tets = [
        [m01, m02, m03, m12],
        [m03, m12, m13, m01],
        [m03, m12, m23, m13],
        [m02, m03, m23, m12],
    ];

    for sub in corner_tets.iter().chain(interior_tets.iter()) {
        let new_id = new_cells.len();
        let center = compute_center(nodes, sub);
        new_cells.push(Cell::new(
            new_id,
            sub.to_vec(),
            Vec::new(),
            sub_vol,
            center,
        ));
    }
}

/// Adds a center node for the given set of existing node IDs.
fn add_center_node(nodes: &mut Vec<Node>, node_ids: &[usize]) -> usize {
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
    let new_id = nodes.len();
    nodes.push(Node::new(new_id, [cx / n, cy / n, cz / n]));
    new_id
}

/// Computes the center of a set of node IDs.
fn compute_center(nodes: &[Node], node_ids: &[usize]) -> [f64; 3] {
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

/// Rebuild faces from cell-node connectivity.
///
/// Creates internal faces between cells sharing a face and boundary faces
/// on the outer surface.
fn rebuild_faces(
    nodes: &[Node],
    cells: &mut Vec<Cell>,
) -> (Vec<Face>, Vec<BoundaryPatch>) {
    // For each cell, enumerate its faces as sets of node indices.
    // A face is shared if two cells have the same set of nodes.

    // Map from sorted face nodes -> (first_cell, face_nodes_ordered)
    let mut face_map: HashMap<Vec<usize>, (usize, Vec<usize>)> = HashMap::new();
    let mut faces: Vec<Face> = Vec::new();
    let mut boundary_face_ids: Vec<usize> = Vec::new();

    // Also track which faces each cell owns
    let n_cells = cells.len();

    // Enumerate faces per cell based on cell type
    let mut cell_face_nodes: Vec<Vec<Vec<usize>>> = Vec::with_capacity(n_cells);

    for cell in cells.iter() {
        let cn = &cell.nodes;
        let face_list = match cn.len() {
            8 => {
                // Hexahedron: 6 quad faces
                vec![
                    vec![cn[0], cn[1], cn[2], cn[3]], // bottom (z-min)
                    vec![cn[4], cn[5], cn[6], cn[7]], // top (z-max)
                    vec![cn[0], cn[1], cn[5], cn[4]], // front (y-min)
                    vec![cn[3], cn[2], cn[6], cn[7]], // back (y-max)
                    vec![cn[0], cn[3], cn[7], cn[4]], // left (x-min)
                    vec![cn[1], cn[2], cn[6], cn[5]], // right (x-max)
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
                // Generic: skip face generation for unsupported types
                vec![]
            }
        };
        cell_face_nodes.push(face_list);
    }

    // Build face map
    for (ci, face_list) in cell_face_nodes.iter().enumerate() {
        for fn_nodes in face_list {
            let mut sorted = fn_nodes.clone();
            sorted.sort();
            let key = sorted;

            if let Some((owner_ci, _stored_nodes)) = face_map.get(&key) {
                // Second cell sharing this face -> create internal face
                let fid = faces.len();
                let fc = face_center(nodes, fn_nodes);
                let (area, normal) = face_area_normal(nodes, fn_nodes);

                faces.push(Face::new(
                    fid,
                    fn_nodes.clone(),
                    *owner_ci,
                    Some(ci),
                    area,
                    normal,
                    fc,
                ));
                cells[*owner_ci].faces.push(fid);
                cells[ci].faces.push(fid);
                face_map.remove(&key);
            } else {
                face_map.insert(key, (ci, fn_nodes.clone()));
            }
        }
    }

    // Remaining faces in the map are boundary faces
    for (_key, (owner_ci, fn_nodes)) in &face_map {
        let fid = faces.len();
        let fc = face_center(nodes, fn_nodes);
        let (area, normal) = face_area_normal(nodes, fn_nodes);

        faces.push(Face::new(
            fid,
            fn_nodes.clone(),
            *owner_ci,
            None,
            area,
            normal,
            fc,
        ));
        cells[*owner_ci].faces.push(fid);
        boundary_face_ids.push(fid);
    }

    let mut boundary_patches = Vec::new();
    if !boundary_face_ids.is_empty() {
        boundary_patches.push(BoundaryPatch::new("boundary", boundary_face_ids));
    }

    (faces, boundary_patches)
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

    // Use Newell's method for polygon area and normal
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

    let normal = [nx / (2.0 * area), ny / (2.0 * area), nz / (2.0 * area)];
    (area, normal)
}

#[cfg(test)]
mod tests {
    use super::*;
    use gfd_core::mesh::structured::StructuredMesh;

    #[test]
    fn test_refine_no_cells() {
        let mesh = StructuredMesh::uniform(2, 2, 1, 2.0, 2.0, 1.0).to_unstructured();
        let refined = refine_cells(&mesh, &[]);
        // No cells refined, should have same number of cells
        assert_eq!(refined.cells.len(), mesh.cells.len());
    }

    #[test]
    fn test_refine_single_hex() {
        let mesh = StructuredMesh::uniform(1, 1, 1, 1.0, 1.0, 1.0).to_unstructured();
        assert_eq!(mesh.cells.len(), 1);
        let refined = refine_cells(&mesh, &[0]);
        assert_eq!(
            refined.cells.len(),
            8,
            "Refining one hex should produce 8 sub-hexes"
        );
    }

    #[test]
    fn test_refine_preserves_unrefined_cells() {
        let mesh = StructuredMesh::uniform(2, 1, 1, 2.0, 1.0, 1.0).to_unstructured();
        assert_eq!(mesh.cells.len(), 2);
        // Refine only cell 0
        let refined = refine_cells(&mesh, &[0]);
        // 8 from refined cell + 1 unchanged = 9
        assert_eq!(
            refined.cells.len(),
            9,
            "Should have 8 refined + 1 unrefined = 9 cells"
        );
    }

    #[test]
    fn test_refine_volume_conservation() {
        let mesh = StructuredMesh::uniform(1, 1, 1, 2.0, 3.0, 4.0).to_unstructured();
        let original_volume: f64 = mesh.cells.iter().map(|c| c.volume).sum();

        let refined = refine_cells(&mesh, &[0]);
        let refined_volume: f64 = refined.cells.iter().map(|c| c.volume).sum();

        assert!(
            (original_volume - refined_volume).abs() < 1e-10,
            "Total volume should be conserved: original={original_volume}, refined={refined_volume}"
        );
    }

    #[test]
    fn test_refine_all_cells() {
        let mesh = StructuredMesh::uniform(2, 2, 1, 2.0, 2.0, 1.0).to_unstructured();
        let cells_to_refine: Vec<usize> = (0..mesh.cells.len()).collect();
        let refined = refine_cells(&mesh, &cells_to_refine);
        assert_eq!(
            refined.cells.len(),
            4 * 8,
            "Refining all 4 cells should give 32 cells"
        );
    }

    #[test]
    fn test_refine_has_faces() {
        let mesh = StructuredMesh::uniform(1, 1, 1, 1.0, 1.0, 1.0).to_unstructured();
        let refined = refine_cells(&mesh, &[0]);
        assert!(
            !refined.faces.is_empty(),
            "Refined mesh should have faces"
        );
        // Each sub-hex should have faces
        for cell in &refined.cells {
            assert!(
                !cell.faces.is_empty(),
                "Each refined cell should have faces, cell {} has none",
                cell.id
            );
        }
    }
}
