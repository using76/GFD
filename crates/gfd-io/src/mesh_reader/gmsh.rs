//! Gmsh mesh file reader (.msh format).
//!
//! Supports Gmsh v2.2 ASCII format, the most widely used Gmsh interchange format.

use std::collections::HashMap;
use std::fs;
use std::io::{BufRead, BufReader};

use gfd_core::mesh::cell::Cell;
use gfd_core::mesh::face::Face;
use gfd_core::mesh::node::Node;
use gfd_core::mesh::unstructured::{BoundaryPatch, UnstructuredMesh};

use crate::mesh_reader::MeshReader;
use crate::IoError;

/// Reader for Gmsh .msh files (format version 2.x and 4.x).
pub struct GmshReader {
    /// Gmsh file format version to expect.
    pub format_version: u32,
}

impl GmshReader {
    /// Creates a new Gmsh reader.
    pub fn new() -> Self {
        Self { format_version: 2 }
    }

    /// Creates a Gmsh reader for a specific format version.
    pub fn with_version(format_version: u32) -> Self {
        Self { format_version }
    }
}

impl Default for GmshReader {
    fn default() -> Self {
        Self::new()
    }
}

impl MeshReader for GmshReader {
    fn read(&self, path: &str) -> crate::Result<UnstructuredMesh> {
        read_gmsh(path)
    }
}

// ---------------------------------------------------------------------------
// Public entry point
// ---------------------------------------------------------------------------

/// Reads a Gmsh v2.2 ASCII `.msh` file and builds an [`UnstructuredMesh`].
pub fn read_gmsh(path: &str) -> Result<UnstructuredMesh, IoError> {
    let content = fs::read_to_string(path).map_err(|e| {
        if e.kind() == std::io::ErrorKind::NotFound {
            IoError::FileNotFound(path.to_string())
        } else {
            IoError::StdIo(e)
        }
    })?;
    parse_gmsh_content(&content)
}

/// Parses the content of a Gmsh v2.2 ASCII `.msh` string.
pub fn parse_gmsh_content(content: &str) -> Result<UnstructuredMesh, IoError> {
    let reader = BufReader::new(content.as_bytes());
    let lines: Vec<String> = reader.lines().map(|l| l.unwrap()).collect();

    // ----- Parse sections ---------------------------------------------------
    let nodes = parse_nodes(&lines)?;
    let raw_elements = parse_elements(&lines)?;

    // ----- Determine dimensionality -----------------------------------------
    // If there are 3-D volume elements (tet, hex, wedge, pyramid) use those as
    // cells and surface elements as boundary faces. Otherwise fall back to 2-D
    // (triangles / quads are cells, lines are boundary edges).
    let has_3d_cells = raw_elements.iter().any(|e| matches!(e.elem_type, 4 | 5 | 6 | 7));

    let (volume_elems, surface_elems): (Vec<_>, Vec<_>) = if has_3d_cells {
        raw_elements
            .iter()
            .partition(|e| matches!(e.elem_type, 4 | 5 | 6 | 7))
    } else {
        raw_elements
            .iter()
            .partition(|e| matches!(e.elem_type, 2 | 3))
    };

    // ----- Build node-id mapping (Gmsh ids may not be 0-based) ---------------
    // gmsh_id -> 0-based index
    let mut id_map: HashMap<usize, usize> = HashMap::new();
    let mut mesh_nodes: Vec<Node> = Vec::with_capacity(nodes.len());
    for (idx, n) in nodes.iter().enumerate() {
        id_map.insert(n.0, idx);
        mesh_nodes.push(Node::new(idx, [n.1, n.2, n.3]));
    }

    // Helper: map gmsh node ids to 0-based indices.
    let map_nodes = |gmsh_ids: &[usize]| -> Result<Vec<usize>, IoError> {
        gmsh_ids
            .iter()
            .map(|gid| {
                id_map
                    .get(gid)
                    .copied()
                    .ok_or_else(|| IoError::ParseError(format!("Unknown node id {gid}")))
            })
            .collect()
    };

    // ----- Build cells (volume elements) with temporary empty face lists -----
    let mut cells: Vec<Cell> = Vec::with_capacity(volume_elems.len());
    for (cell_idx, elem) in volume_elems.iter().enumerate() {
        let cell_nodes = map_nodes(&elem.nodes)?;
        let center = compute_centroid(&mesh_nodes, &cell_nodes);
        let volume = compute_volume(elem.elem_type, &mesh_nodes, &cell_nodes);
        cells.push(Cell::new(cell_idx, cell_nodes, Vec::new(), volume, center));
    }

    // ----- Generate faces from cells ----------------------------------------
    // A face signature is a sorted tuple of 0-based node indices.
    // We map each signature to the face index and track the first cell that
    // created it (owner).
    struct FaceInfo {
        face_idx: usize,
        owner_cell: usize,
    }

    let mut face_map: HashMap<Vec<usize>, FaceInfo> = HashMap::new();
    let mut faces: Vec<Face> = Vec::new();

    for (cell_idx, elem) in volume_elems.iter().enumerate() {
        let cell_nodes = map_nodes(&elem.nodes)?;
        let local_faces = cell_face_definitions(elem.elem_type);
        for local_face in &local_faces {
            let face_nodes: Vec<usize> = local_face.iter().map(|&li| cell_nodes[li]).collect();
            let mut sig = face_nodes.clone();
            sig.sort();

            if let Some(info) = face_map.get(&sig) {
                // This face already exists -- it becomes an internal face.
                let fid = info.face_idx;
                let owner = info.owner_cell;
                // The owner is the cell with the smaller index.
                let (o, n) = if owner < cell_idx {
                    (owner, cell_idx)
                } else {
                    (cell_idx, owner)
                };
                faces[fid].owner_cell = o;
                faces[fid].neighbor_cell = Some(n);
                // Ensure normal points from owner to neighbor.
                // Recompute with correct orientation from owner's perspective.
                let owner_center = cells[o].center;
                orient_face_normal(&mut faces[fid], &mesh_nodes, owner_center);

                cells[cell_idx].faces.push(fid);
            } else {
                // New face -- boundary until proven otherwise.
                let fid = faces.len();
                let (area, normal, center) =
                    compute_face_geometry(&mesh_nodes, &face_nodes);

                // Orient normal outward from this cell.
                let mut face = Face::new(
                    fid,
                    face_nodes.clone(),
                    cell_idx,
                    None,
                    area,
                    normal,
                    center,
                );
                orient_face_normal(&mut face, &mesh_nodes, cells[cell_idx].center);

                faces.push(face);
                face_map.insert(sig, FaceInfo { face_idx: fid, owner_cell: cell_idx });
                cells[cell_idx].faces.push(fid);
            }
        }
    }

    // ----- Build boundary patches from surface elements ----------------------
    // Surface elements carry a physical group tag. Group boundary faces by that
    // tag and create one BoundaryPatch per group.
    //
    // For each surface element we look up its face signature in `face_map`; if
    // found that face is on the boundary.
    let mut patch_faces: HashMap<usize, Vec<usize>> = HashMap::new();

    for elem in &surface_elems {
        let face_nodes = map_nodes(&elem.nodes)?;
        let mut sig = face_nodes.clone();
        sig.sort();
        if let Some(info) = face_map.get(&sig) {
            let phys_tag = elem.physical_tag;
            patch_faces.entry(phys_tag).or_default().push(info.face_idx);
        }
    }

    // Collect boundary patches, sorted by physical tag for determinism.
    let mut sorted_tags: Vec<usize> = patch_faces.keys().copied().collect();
    sorted_tags.sort();
    let boundary_patches: Vec<BoundaryPatch> = sorted_tags
        .into_iter()
        .map(|tag| {
            let face_ids = patch_faces.remove(&tag).unwrap();
            BoundaryPatch::new(format!("patch_{tag}"), face_ids)
        })
        .collect();

    // ----- Collect any boundary faces not claimed by surface elements --------
    // If there are boundary faces that were not identified by surface elements,
    // group them into a default patch.
    let claimed: std::collections::HashSet<usize> = boundary_patches
        .iter()
        .flat_map(|p| p.face_ids.iter().copied())
        .collect();
    let unclaimed_boundary: Vec<usize> = faces
        .iter()
        .filter(|f| f.is_boundary() && !claimed.contains(&f.id))
        .map(|f| f.id)
        .collect();

    let mut all_patches = boundary_patches;
    if !unclaimed_boundary.is_empty() {
        all_patches.push(BoundaryPatch::new("default", unclaimed_boundary));
    }

    Ok(UnstructuredMesh::from_components(
        mesh_nodes, faces, cells, all_patches,
    ))
}

// ===========================================================================
// Internal data types
// ===========================================================================

/// A raw element as read from the Gmsh file.
#[derive(Debug)]
struct RawElement {
    #[allow(dead_code)]
    id: usize,
    elem_type: usize,
    physical_tag: usize,
    nodes: Vec<usize>,
}

/// A raw node as read from the Gmsh file: (id, x, y, z).
type RawNode = (usize, f64, f64, f64);

// ===========================================================================
// Section parsers
// ===========================================================================

fn find_section(lines: &[String], start_tag: &str) -> Option<usize> {
    lines.iter().position(|l| l.trim() == start_tag)
}

fn parse_nodes(lines: &[String]) -> Result<Vec<RawNode>, IoError> {
    let start = find_section(lines, "$Nodes")
        .ok_or_else(|| IoError::ParseError("Missing $Nodes section".into()))?;
    let num_nodes: usize = lines[start + 1]
        .trim()
        .parse()
        .map_err(|_| IoError::ParseError("Cannot parse node count".into()))?;

    let mut nodes = Vec::with_capacity(num_nodes);
    for i in 0..num_nodes {
        let line = &lines[start + 2 + i];
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() < 4 {
            return Err(IoError::ParseError(format!(
                "Node line too short: '{line}'"
            )));
        }
        let id: usize = parts[0]
            .parse()
            .map_err(|_| IoError::ParseError(format!("Bad node id: '{}'", parts[0])))?;
        let x: f64 = parts[1]
            .parse()
            .map_err(|_| IoError::ParseError(format!("Bad coord: '{}'", parts[1])))?;
        let y: f64 = parts[2]
            .parse()
            .map_err(|_| IoError::ParseError(format!("Bad coord: '{}'", parts[2])))?;
        let z: f64 = parts[3]
            .parse()
            .map_err(|_| IoError::ParseError(format!("Bad coord: '{}'", parts[3])))?;
        nodes.push((id, x, y, z));
    }
    Ok(nodes)
}

fn num_nodes_for_type(elem_type: usize) -> Result<usize, IoError> {
    match elem_type {
        1 => Ok(2),   // 2-node line
        2 => Ok(3),   // 3-node triangle
        3 => Ok(4),   // 4-node quad
        4 => Ok(4),   // 4-node tetrahedron
        5 => Ok(8),   // 8-node hexahedron
        6 => Ok(6),   // 6-node wedge/prism
        7 => Ok(5),   // 5-node pyramid
        15 => Ok(1),  // 1-node point
        _ => Err(IoError::ParseError(format!(
            "Unsupported Gmsh element type {elem_type}"
        ))),
    }
}

fn parse_elements(lines: &[String]) -> Result<Vec<RawElement>, IoError> {
    let start = find_section(lines, "$Elements")
        .ok_or_else(|| IoError::ParseError("Missing $Elements section".into()))?;
    let num_elements: usize = lines[start + 1]
        .trim()
        .parse()
        .map_err(|_| IoError::ParseError("Cannot parse element count".into()))?;

    let mut elements = Vec::with_capacity(num_elements);
    for i in 0..num_elements {
        let line = &lines[start + 2 + i];
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() < 4 {
            return Err(IoError::ParseError(format!(
                "Element line too short: '{line}'"
            )));
        }
        let id: usize = parts[0]
            .parse()
            .map_err(|_| IoError::ParseError("Bad element id".into()))?;
        let elem_type: usize = parts[1]
            .parse()
            .map_err(|_| IoError::ParseError("Bad element type".into()))?;
        let num_tags: usize = parts[2]
            .parse()
            .map_err(|_| IoError::ParseError("Bad num_tags".into()))?;

        // Skip point elements.
        if elem_type == 15 {
            continue;
        }

        let nn = num_nodes_for_type(elem_type)?;
        let expected_len = 3 + num_tags + nn;
        if parts.len() < expected_len {
            return Err(IoError::ParseError(format!(
                "Element line has {} tokens, expected at least {expected_len}: '{line}'",
                parts.len()
            )));
        }

        let physical_tag: usize = if num_tags > 0 {
            parts[3]
                .parse()
                .map_err(|_| IoError::ParseError("Bad physical tag".into()))?
        } else {
            0
        };

        let node_start = 3 + num_tags;
        let nodes: Result<Vec<usize>, _> = parts[node_start..node_start + nn]
            .iter()
            .map(|s| {
                s.parse::<usize>()
                    .map_err(|_| IoError::ParseError(format!("Bad node ref: '{s}'")))
            })
            .collect();

        elements.push(RawElement {
            id,
            elem_type,
            physical_tag,
            nodes: nodes?,
        });
    }
    Ok(elements)
}

// ===========================================================================
// Face definitions per cell type
// ===========================================================================

/// Returns the local-index face definitions for the given Gmsh element type.
/// Each inner `Vec` lists the local node indices that form one face.
fn cell_face_definitions(elem_type: usize) -> Vec<Vec<usize>> {
    match elem_type {
        // Tetrahedron: 4 triangular faces
        4 => vec![
            vec![0, 1, 2],
            vec![0, 1, 3],
            vec![0, 2, 3],
            vec![1, 2, 3],
        ],
        // Hexahedron: 6 quad faces
        // Gmsh node ordering: 0-1-2-3 (bottom), 4-5-6-7 (top)
        5 => vec![
            vec![0, 1, 2, 3], // bottom
            vec![4, 5, 6, 7], // top
            vec![0, 1, 5, 4], // front
            vec![2, 3, 7, 6], // back
            vec![0, 3, 7, 4], // left
            vec![1, 2, 6, 5], // right
        ],
        // Wedge / prism: 5 faces (2 triangles + 3 quads)
        // Gmsh node ordering: 0-1-2 (bottom tri), 3-4-5 (top tri)
        6 => vec![
            vec![0, 1, 2],       // bottom triangle
            vec![3, 4, 5],       // top triangle
            vec![0, 1, 4, 3],    // quad
            vec![1, 2, 5, 4],    // quad
            vec![0, 2, 5, 3],    // quad
        ],
        // Pyramid: 5 faces (1 quad base + 4 triangles)
        // Gmsh node ordering: 0-1-2-3 (base quad), 4 (apex)
        7 => vec![
            vec![0, 1, 2, 3], // base quad
            vec![0, 1, 4],    // triangle
            vec![1, 2, 4],    // triangle
            vec![2, 3, 4],    // triangle
            vec![0, 3, 4],    // triangle
        ],
        // 2-D: triangle → 3 edge-faces
        2 => vec![
            vec![0, 1],
            vec![1, 2],
            vec![0, 2],
        ],
        // 2-D: quad → 4 edge-faces
        3 => vec![
            vec![0, 1],
            vec![1, 2],
            vec![2, 3],
            vec![0, 3],
        ],
        _ => Vec::new(),
    }
}

// ===========================================================================
// Geometry helpers
// ===========================================================================

fn compute_centroid(nodes: &[Node], indices: &[usize]) -> [f64; 3] {
    let n = indices.len() as f64;
    let mut c = [0.0; 3];
    for &i in indices {
        let p = nodes[i].position;
        c[0] += p[0];
        c[1] += p[1];
        c[2] += p[2];
    }
    c[0] /= n;
    c[1] /= n;
    c[2] /= n;
    c
}

fn cross(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

fn dot(a: [f64; 3], b: [f64; 3]) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

fn sub(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

fn mag(v: [f64; 3]) -> f64 {
    (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt()
}

fn scale(v: [f64; 3], s: f64) -> [f64; 3] {
    [v[0] * s, v[1] * s, v[2] * s]
}

fn add3(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}

/// Compute area, outward normal (unit), and centroid for a face given by node
/// indices. Handles both triangular and quad faces (and degenerate 2-node
/// "edge" faces in 2D).
fn compute_face_geometry(
    nodes: &[Node],
    face_nodes: &[usize],
) -> (f64, [f64; 3], [f64; 3]) {
    let center = compute_centroid(nodes, face_nodes);

    match face_nodes.len() {
        2 => {
            // 2-D edge: "area" is edge length, normal is perpendicular in the
            // xy-plane.
            let p0 = nodes[face_nodes[0]].position;
            let p1 = nodes[face_nodes[1]].position;
            let dx = p1[0] - p0[0];
            let dy = p1[1] - p0[1];
            let len = (dx * dx + dy * dy).sqrt();
            let normal = if len > 1e-30 {
                [dy / len, -dx / len, 0.0]
            } else {
                [0.0, 0.0, 0.0]
            };
            (len, normal, center)
        }
        3 => {
            let p0 = nodes[face_nodes[0]].position;
            let p1 = nodes[face_nodes[1]].position;
            let p2 = nodes[face_nodes[2]].position;
            let v1 = sub(p1, p0);
            let v2 = sub(p2, p0);
            let n = cross(v1, v2);
            let m = mag(n);
            let area = 0.5 * m;
            let unit_n = if m > 1e-30 {
                scale(n, 1.0 / m)
            } else {
                [0.0, 0.0, 0.0]
            };
            (area, unit_n, center)
        }
        _ => {
            // General polygon: split into triangles from centroid.
            let n_pts = face_nodes.len();
            let mut total_normal = [0.0; 3];
            let mut total_area = 0.0;
            for i in 0..n_pts {
                let j = (i + 1) % n_pts;
                let pi = nodes[face_nodes[i]].position;
                let pj = nodes[face_nodes[j]].position;
                let vi = sub(pi, center);
                let vj = sub(pj, center);
                let tri_n = cross(vi, vj);
                let tri_area = 0.5 * mag(tri_n);
                total_area += tri_area;
                total_normal = add3(total_normal, scale(tri_n, 0.5));
            }
            let m = mag(total_normal);
            let unit_n = if m > 1e-30 {
                scale(total_normal, 1.0 / m)
            } else {
                [0.0, 0.0, 0.0]
            };
            (total_area, unit_n, center)
        }
    }
}

/// Make sure the face normal points away from `cell_center`.
fn orient_face_normal(face: &mut Face, _nodes: &[Node], cell_center: [f64; 3]) {
    let fc = face.center;
    let outward = sub(fc, cell_center);
    if dot(outward, face.normal) < 0.0 {
        face.normal = scale(face.normal, -1.0);
    }
}

/// Compute the volume of a cell.
fn compute_volume(elem_type: usize, nodes: &[Node], cell_nodes: &[usize]) -> f64 {
    match elem_type {
        4 => {
            // Tetrahedron: V = |det([v1-v0, v2-v0, v3-v0])| / 6
            let p0 = nodes[cell_nodes[0]].position;
            let p1 = nodes[cell_nodes[1]].position;
            let p2 = nodes[cell_nodes[2]].position;
            let p3 = nodes[cell_nodes[3]].position;
            let a = sub(p1, p0);
            let b = sub(p2, p0);
            let c = sub(p3, p0);
            (dot(a, cross(b, c))).abs() / 6.0
        }
        5 => {
            // Hexahedron: subdivide into 5 tets and sum volumes.
            // Standard decomposition of a hex into 5 tetrahedra.
            hex_volume(nodes, cell_nodes)
        }
        6 => {
            // Wedge/prism: subdivide into 3 tets.
            wedge_volume(nodes, cell_nodes)
        }
        7 => {
            // Pyramid: subdivide into 2 tets.
            pyramid_volume(nodes, cell_nodes)
        }
        2 | 3 => {
            // 2-D elements: area (treat as "volume" for the 2D solver).
            compute_2d_area(elem_type, nodes, cell_nodes)
        }
        _ => 0.0,
    }
}

fn tet_vol(p0: [f64; 3], p1: [f64; 3], p2: [f64; 3], p3: [f64; 3]) -> f64 {
    let a = sub(p1, p0);
    let b = sub(p2, p0);
    let c = sub(p3, p0);
    (dot(a, cross(b, c))).abs() / 6.0
}

fn hex_volume(nodes: &[Node], cn: &[usize]) -> f64 {
    // Decompose hex (0-7) into 5 non-overlapping tetrahedra.
    // Gmsh hex node ordering: 0-1-2-3 (bottom), 4-5-6-7 (top).
    // Standard 5-tet decomposition:
    //   (0,1,3,4), (1,2,3,6), (1,4,5,6), (3,4,6,7), (1,3,4,6)
    let p = |i: usize| nodes[cn[i]].position;
    tet_vol(p(0), p(1), p(3), p(4))
        + tet_vol(p(1), p(2), p(3), p(6))
        + tet_vol(p(1), p(4), p(5), p(6))
        + tet_vol(p(3), p(4), p(6), p(7))
        + tet_vol(p(1), p(3), p(4), p(6))
}

fn wedge_volume(nodes: &[Node], cn: &[usize]) -> f64 {
    // Wedge (0,1,2,3,4,5) -> 3 tets
    let p = |i: usize| nodes[cn[i]].position;
    tet_vol(p(0), p(1), p(2), p(3))
        + tet_vol(p(1), p(2), p(3), p(4))
        + tet_vol(p(2), p(3), p(4), p(5))
}

fn pyramid_volume(nodes: &[Node], cn: &[usize]) -> f64 {
    // Pyramid (0,1,2,3,4): split quad base into 2 tris
    let p = |i: usize| nodes[cn[i]].position;
    tet_vol(p(0), p(1), p(2), p(4)) + tet_vol(p(0), p(2), p(3), p(4))
}

fn compute_2d_area(elem_type: usize, nodes: &[Node], cn: &[usize]) -> f64 {
    match elem_type {
        2 => {
            // Triangle area
            let p0 = nodes[cn[0]].position;
            let p1 = nodes[cn[1]].position;
            let p2 = nodes[cn[2]].position;
            0.5 * mag(cross(sub(p1, p0), sub(p2, p0)))
        }
        3 => {
            // Quad area: split into 2 triangles
            let p0 = nodes[cn[0]].position;
            let p1 = nodes[cn[1]].position;
            let p2 = nodes[cn[2]].position;
            let p3 = nodes[cn[3]].position;
            0.5 * mag(cross(sub(p1, p0), sub(p2, p0)))
                + 0.5 * mag(cross(sub(p2, p0), sub(p3, p0)))
        }
        _ => 0.0,
    }
}

// ===========================================================================
// Tests
// ===========================================================================

#[cfg(test)]
mod tests {
    use super::*;

    /// A minimal Gmsh v2.2 file containing a single tetrahedron with boundary
    /// faces on all 4 surfaces.
    const SINGLE_TET_MSH: &str = "\
$MeshFormat
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
5
1 2 2 1 1 1 2 3
2 2 2 1 1 1 2 4
3 2 2 2 2 1 3 4
4 2 2 2 2 2 3 4
5 4 2 0 1 1 2 3 4
$EndElements
";

    #[test]
    fn test_parse_single_tet() {
        let mesh = parse_gmsh_content(SINGLE_TET_MSH).expect("Failed to parse single tet");

        // 4 nodes
        assert_eq!(mesh.num_nodes(), 4);

        // 1 cell (the tetrahedron)
        assert_eq!(mesh.num_cells(), 1);

        // 4 faces (all boundary for a single tet)
        assert_eq!(mesh.num_faces(), 4);
        for face in &mesh.faces {
            assert!(face.is_boundary(), "All faces of a single tet should be boundary");
        }

        // Volume of the unit corner tet = 1/6
        let vol = mesh.cells[0].volume;
        assert!((vol - 1.0 / 6.0).abs() < 1e-12, "Tet volume should be 1/6, got {vol}");

        // Centroid should be (0.25, 0.25, 0.25)
        let c = mesh.cells[0].center;
        assert!((c[0] - 0.25).abs() < 1e-12);
        assert!((c[1] - 0.25).abs() < 1e-12);
        assert!((c[2] - 0.25).abs() < 1e-12);

        // Boundary patches: physical groups 1 and 2 from surface elements,
        // plus the unclaimed boundary faces.
        assert!(
            !mesh.boundary_patches.is_empty(),
            "Should have at least one boundary patch"
        );
    }

    /// A minimal 2-D mesh: two triangles forming a square [0,1]x[0,1].
    const TWO_TRI_2D_MSH: &str = "\
$MeshFormat
2.2 0 8
$EndMeshFormat
$Nodes
4
1 0.0 0.0 0.0
2 1.0 0.0 0.0
3 1.0 1.0 0.0
4 0.0 1.0 0.0
$EndNodes
$Elements
6
1 1 2 1 1 1 2
2 1 2 2 1 2 3
3 1 2 3 1 3 4
4 1 2 4 1 4 1
5 2 2 0 1 1 2 3
6 2 2 0 1 1 3 4
$EndElements
";

    #[test]
    fn test_parse_2d_triangles() {
        let mesh = parse_gmsh_content(TWO_TRI_2D_MSH).expect("Failed to parse 2D mesh");

        assert_eq!(mesh.num_nodes(), 4);
        assert_eq!(mesh.num_cells(), 2);

        // Each triangle has 3 edges; the shared diagonal becomes 1 internal face.
        // Total = 3 + 3 - 1 = 5 faces.
        assert_eq!(mesh.num_faces(), 5);

        // Exactly 1 internal face.
        let internal_count = mesh.faces.iter().filter(|f| !f.is_boundary()).count();
        assert_eq!(internal_count, 1, "Should have exactly 1 internal face (shared edge)");

        // Total area = 0.5 + 0.5 = 1.0.
        let total_area: f64 = mesh.cells.iter().map(|c| c.volume).sum();
        assert!(
            (total_area - 1.0).abs() < 1e-12,
            "Total area should be 1.0, got {total_area}"
        );
    }
}
