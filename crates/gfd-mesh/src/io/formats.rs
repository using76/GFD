//! Mesh import from various file formats.
//!
//! Supports importing meshes from Gmsh MSH v2.2 format.

use gfd_core::mesh::cell::Cell;
use gfd_core::mesh::face::Face;
use gfd_core::mesh::node::Node;
use gfd_core::mesh::unstructured::{BoundaryPatch, UnstructuredMesh};

use std::collections::HashMap;

use crate::Result;

/// Import a mesh from Gmsh MSH v2.2 ASCII format.
///
/// Parses `$Nodes` and `$Elements` sections. Only volumetric elements
/// (tetrahedra, hexahedra, prisms, pyramids) are imported as cells.
/// Lower-dimensional elements (points, lines, triangles) are skipped.
///
/// Gmsh element types recognized:
/// - 4: tetrahedron (4 nodes)
/// - 5: hexahedron (8 nodes)
/// - 6: prism/wedge (6 nodes)
/// - 7: pyramid (5 nodes)
///
/// # Arguments
/// * `content` - The full text of the MSH file.
///
/// # Returns
/// An `UnstructuredMesh` with the imported mesh.
pub fn import_gmsh_msh(content: &str) -> Result<UnstructuredMesh> {
    let lines: Vec<&str> = content.lines().collect();
    let n_lines = lines.len();

    // Parse nodes
    let nodes_start = find_section(&lines, "$Nodes")?;
    let num_nodes: usize = parse_usize(lines[nodes_start + 1])?;
    let mut nodes: Vec<Node> = Vec::with_capacity(num_nodes);
    let mut gmsh_id_to_idx: HashMap<usize, usize> = HashMap::new();

    for i in 0..num_nodes {
        let line_idx = nodes_start + 2 + i;
        if line_idx >= n_lines {
            return Err(crate::MeshError::GeometryError(
                "Unexpected end of file in $Nodes section".to_string(),
            ));
        }
        let parts: Vec<&str> = lines[line_idx].split_whitespace().collect();
        if parts.len() < 4 {
            return Err(crate::MeshError::GeometryError(format!(
                "Invalid node line: '{}'",
                lines[line_idx]
            )));
        }
        let gmsh_id: usize = parse_usize(parts[0])?;
        let x: f64 = parse_f64(parts[1])?;
        let y: f64 = parse_f64(parts[2])?;
        let z: f64 = parse_f64(parts[3])?;

        let idx = nodes.len();
        gmsh_id_to_idx.insert(gmsh_id, idx);
        nodes.push(Node::new(idx, [x, y, z]));
    }

    // Parse elements
    let elems_start = find_section(&lines, "$Elements")?;
    let num_elements: usize = parse_usize(lines[elems_start + 1])?;
    let mut cells: Vec<Cell> = Vec::new();

    for i in 0..num_elements {
        let line_idx = elems_start + 2 + i;
        if line_idx >= n_lines {
            return Err(crate::MeshError::GeometryError(
                "Unexpected end of file in $Elements section".to_string(),
            ));
        }
        let parts: Vec<&str> = lines[line_idx].split_whitespace().collect();
        if parts.len() < 3 {
            continue;
        }

        let elem_type: usize = parse_usize(parts[1])?;
        let num_tags: usize = parse_usize(parts[2])?;

        // Only import volumetric elements
        let expected_nodes = match elem_type {
            4 => 4,  // tetrahedron
            5 => 8,  // hexahedron
            6 => 6,  // prism
            7 => 5,  // pyramid
            _ => continue, // skip points, lines, triangles, etc.
        };

        let node_start = 3 + num_tags;
        if parts.len() < node_start + expected_nodes {
            continue;
        }

        let mut cell_nodes = Vec::with_capacity(expected_nodes);
        for j in 0..expected_nodes {
            let gmsh_nid: usize = parse_usize(parts[node_start + j])?;
            if let Some(&idx) = gmsh_id_to_idx.get(&gmsh_nid) {
                cell_nodes.push(idx);
            } else {
                return Err(crate::MeshError::GeometryError(format!(
                    "Element references unknown node {}",
                    gmsh_nid
                )));
            }
        }

        let center = compute_center(&nodes, &cell_nodes);
        let volume = compute_cell_volume(&nodes, &cell_nodes);
        let cid = cells.len();
        cells.push(Cell::new(cid, cell_nodes, Vec::new(), volume, center));
    }

    if cells.is_empty() {
        return Err(crate::MeshError::GeometryError(
            "No volumetric elements found in MSH file".to_string(),
        ));
    }

    // Rebuild faces
    let (faces, patches) = rebuild_faces(&nodes, &mut cells);
    Ok(UnstructuredMesh::from_components(nodes, faces, cells, patches))
}

fn find_section(lines: &[&str], section: &str) -> Result<usize> {
    for (i, line) in lines.iter().enumerate() {
        if line.trim() == section {
            return Ok(i);
        }
    }
    Err(crate::MeshError::GeometryError(format!(
        "Section '{}' not found in MSH file",
        section
    )))
}

fn parse_usize(s: &str) -> Result<usize> {
    s.trim().parse::<usize>().map_err(|e| {
        crate::MeshError::GeometryError(format!("Failed to parse integer '{}': {}", s, e))
    })
}

fn parse_f64(s: &str) -> Result<f64> {
    s.trim().parse::<f64>().map_err(|e| {
        crate::MeshError::GeometryError(format!("Failed to parse float '{}': {}", s, e))
    })
}

fn compute_center(nodes: &[Node], node_ids: &[usize]) -> [f64; 3] {
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

fn compute_cell_volume(nodes: &[Node], cell_nodes: &[usize]) -> f64 {
    match cell_nodes.len() {
        4 => {
            // Tetrahedron volume
            let a = nodes[cell_nodes[0]].position;
            let b = nodes[cell_nodes[1]].position;
            let c = nodes[cell_nodes[2]].position;
            let d = nodes[cell_nodes[3]].position;
            tet_volume(a, b, c, d).abs()
        }
        8 => {
            // Hexahedron: split into 5 tetrahedra
            let p: Vec<[f64; 3]> = cell_nodes.iter().map(|&n| nodes[n].position).collect();
            let tets = [
                [0, 1, 3, 4],
                [1, 2, 3, 6],
                [1, 4, 5, 6],
                [3, 4, 6, 7],
                [1, 3, 4, 6],
            ];
            tets.iter()
                .map(|t| tet_volume(p[t[0]], p[t[1]], p[t[2]], p[t[3]]).abs())
                .sum()
        }
        _ => {
            // Rough estimate using bounding box
            let mut min = [f64::MAX; 3];
            let mut max = [f64::MIN; 3];
            for &nid in cell_nodes {
                let p = nodes[nid].position;
                for k in 0..3 {
                    min[k] = min[k].min(p[k]);
                    max[k] = max[k].max(p[k]);
                }
            }
            (max[0] - min[0]) * (max[1] - min[1]) * (max[2] - min[2])
        }
    }
}

fn tet_volume(a: [f64; 3], b: [f64; 3], c: [f64; 3], d: [f64; 3]) -> f64 {
    let ab = [b[0] - a[0], b[1] - a[1], b[2] - a[2]];
    let ac = [c[0] - a[0], c[1] - a[1], c[2] - a[2]];
    let ad = [d[0] - a[0], d[1] - a[1], d[2] - a[2]];
    let cross = [
        ac[1] * ad[2] - ac[2] * ad[1],
        ac[2] * ad[0] - ac[0] * ad[2],
        ac[0] * ad[1] - ac[1] * ad[0],
    ];
    (ab[0] * cross[0] + ab[1] * cross[1] + ab[2] * cross[2]) / 6.0
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
                6 => vec![
                    // Prism/wedge faces
                    vec![cn[0], cn[1], cn[2]],
                    vec![cn[3], cn[4], cn[5]],
                    vec![cn[0], cn[1], cn[4], cn[3]],
                    vec![cn[1], cn[2], cn[5], cn[4]],
                    vec![cn[0], cn[2], cn[5], cn[3]],
                ],
                5 => vec![
                    // Pyramid faces
                    vec![cn[0], cn[1], cn[2], cn[3]],
                    vec![cn[0], cn[1], cn[4]],
                    vec![cn[1], cn[2], cn[4]],
                    vec![cn[2], cn[3], cn[4]],
                    vec![cn[3], cn[0], cn[4]],
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

    fn sample_gmsh_msh_hex() -> String {
        // A single hexahedron: unit cube [0,1]^3
        r#"$MeshFormat
2.2 0 8
$EndMeshFormat
$Nodes
8
1 0.0 0.0 0.0
2 1.0 0.0 0.0
3 1.0 1.0 0.0
4 0.0 1.0 0.0
5 0.0 0.0 1.0
6 1.0 0.0 1.0
7 1.0 1.0 1.0
8 0.0 1.0 1.0
$EndNodes
$Elements
1
1 5 0 1 2 3 4 5 6 7 8
$EndElements"#
            .to_string()
    }

    fn sample_gmsh_msh_tet() -> String {
        // A single tetrahedron
        r#"$MeshFormat
2.2 0 8
$EndMeshFormat
$Nodes
4
1 0.0 0.0 0.0
2 1.0 0.0 0.0
3 0.0 1.0 0.0
4 0.0 0.0 1.0
$EndNodes
$Elements
1
1 4 0 1 2 3 4
$EndElements"#
            .to_string()
    }

    #[test]
    fn test_import_gmsh_hex() {
        let content = sample_gmsh_msh_hex();
        let mesh = import_gmsh_msh(&content).unwrap();
        assert_eq!(mesh.nodes.len(), 8, "Should have 8 nodes");
        assert_eq!(mesh.cells.len(), 1, "Should have 1 hex cell");
        assert_eq!(mesh.cells[0].nodes.len(), 8);
    }

    #[test]
    fn test_import_gmsh_tet() {
        let content = sample_gmsh_msh_tet();
        let mesh = import_gmsh_msh(&content).unwrap();
        assert_eq!(mesh.nodes.len(), 4);
        assert_eq!(mesh.cells.len(), 1);
        assert_eq!(mesh.cells[0].nodes.len(), 4);
    }

    #[test]
    fn test_import_gmsh_volume() {
        let content = sample_gmsh_msh_hex();
        let mesh = import_gmsh_msh(&content).unwrap();
        // Unit cube should have volume ~1.0
        assert!(
            (mesh.cells[0].volume - 1.0).abs() < 0.1,
            "Unit cube volume should be ~1.0, got {}",
            mesh.cells[0].volume
        );
    }

    #[test]
    fn test_import_gmsh_has_faces() {
        let content = sample_gmsh_msh_hex();
        let mesh = import_gmsh_msh(&content).unwrap();
        assert!(
            !mesh.faces.is_empty(),
            "Imported mesh should have reconstructed faces"
        );
        // A single hex has 6 boundary faces
        assert_eq!(
            mesh.faces.len(),
            6,
            "Single hex should have 6 faces"
        );
    }

    #[test]
    fn test_import_gmsh_missing_section() {
        let content = "$MeshFormat\n2.2 0 8\n$EndMeshFormat\n";
        let result = import_gmsh_msh(content);
        assert!(result.is_err());
    }

    #[test]
    fn test_import_gmsh_no_volume_elements() {
        // Only contains a line element (type 1)
        let content = r#"$MeshFormat
2.2 0 8
$EndMeshFormat
$Nodes
2
1 0.0 0.0 0.0
2 1.0 0.0 0.0
$EndNodes
$Elements
1
1 1 0 1 2
$EndElements"#;
        let result = import_gmsh_msh(content);
        assert!(result.is_err());
    }
}
