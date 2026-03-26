//! 3D Tetrahedral mesh generation using the Bowyer-Watson algorithm.
//!
//! Extends the 2D Delaunay triangulation to 3D: produces a tetrahedral mesh
//! from a set of 3D points using the Bowyer-Watson insertion algorithm with
//! circumsphere tests.

use std::collections::{HashMap, HashSet};

use gfd_core::mesh::cell::Cell;
use gfd_core::mesh::face::Face;
use gfd_core::mesh::node::Node;
use gfd_core::mesh::unstructured::{BoundaryPatch, UnstructuredMesh};

use crate::{MeshError, Result};

/// A tetrahedron represented by four point indices.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct Tet {
    v: [usize; 4],
}

impl Tet {
    fn new(a: usize, b: usize, c: usize, d: usize) -> Self {
        Self { v: [a, b, c, d] }
    }

    /// Returns true if this tet contains the given vertex index.
    fn contains_vertex(&self, v: usize) -> bool {
        self.v[0] == v || self.v[1] == v || self.v[2] == v || self.v[3] == v
    }

    /// Returns the four triangular faces as sorted triples.
    fn faces(&self) -> [[usize; 3]; 4] {
        let mut faces = [
            [self.v[0], self.v[1], self.v[2]],
            [self.v[0], self.v[1], self.v[3]],
            [self.v[0], self.v[2], self.v[3]],
            [self.v[1], self.v[2], self.v[3]],
        ];
        for f in &mut faces {
            f.sort();
        }
        faces
    }
}

/// Compute circumsphere center and radius squared for a tetrahedron.
///
/// Returns `(cx, cy, cz, r2)`.
fn circumsphere(
    p0: [f64; 3],
    p1: [f64; 3],
    p2: [f64; 3],
    p3: [f64; 3],
) -> (f64, f64, f64, f64) {
    // Translate to p0 as origin
    let a = [p1[0] - p0[0], p1[1] - p0[1], p1[2] - p0[2]];
    let b = [p2[0] - p0[0], p2[1] - p0[1], p2[2] - p0[2]];
    let c = [p3[0] - p0[0], p3[1] - p0[1], p3[2] - p0[2]];

    let a2 = a[0] * a[0] + a[1] * a[1] + a[2] * a[2];
    let b2 = b[0] * b[0] + b[1] * b[1] + b[2] * b[2];
    let c2 = c[0] * c[0] + c[1] * c[1] + c[2] * c[2];

    let det = 2.0
        * (a[0] * (b[1] * c[2] - b[2] * c[1])
            - a[1] * (b[0] * c[2] - b[2] * c[0])
            + a[2] * (b[0] * c[1] - b[1] * c[0]));

    if det.abs() < 1e-30 {
        return (0.0, 0.0, 0.0, f64::MAX);
    }

    let ux = (a2 * (b[1] * c[2] - b[2] * c[1])
        - b2 * (a[1] * c[2] - a[2] * c[1])
        + c2 * (a[1] * b[2] - a[2] * b[1]))
        / det;
    let uy = -(a2 * (b[0] * c[2] - b[2] * c[0])
        - b2 * (a[0] * c[2] - a[2] * c[0])
        + c2 * (a[0] * b[2] - a[2] * b[0]))
        / det;
    let uz = (a2 * (b[0] * c[1] - b[1] * c[0])
        - b2 * (a[0] * c[1] - a[1] * c[0])
        + c2 * (a[0] * b[1] - a[1] * b[0]))
        / det;

    let cx = ux + p0[0];
    let cy = uy + p0[1];
    let cz = uz + p0[2];
    let r2 = ux * ux + uy * uy + uz * uz;

    (cx, cy, cz, r2)
}

/// 3D Delaunay tetrahedral mesher using the Bowyer-Watson algorithm.
pub struct TetMesher3D {
    /// Input points (x, y, z).
    points: Vec<[f64; 3]>,
    /// Maximum tetrahedron volume constraint (None = no constraint).
    max_volume: Option<f64>,
}

impl TetMesher3D {
    /// Create a new 3D tetrahedral mesher with the given points.
    pub fn new(points: Vec<[f64; 3]>) -> Self {
        Self {
            points,
            max_volume: None,
        }
    }

    /// Set maximum allowed tetrahedron volume for refinement.
    pub fn with_max_volume(mut self, volume: f64) -> Self {
        self.max_volume = Some(volume);
        self
    }

    /// Run the 3D Bowyer-Watson algorithm and return tetrahedra as index quads.
    ///
    /// The returned indices refer to `self.points`.
    pub fn tetrahedralize(&self) -> Result<Vec<[usize; 4]>> {
        if self.points.len() < 4 {
            return Err(MeshError::InvalidParameters(
                "Need at least 4 points for 3D Delaunay tetrahedralization".to_string(),
            ));
        }

        // Find bounding box
        let (mut xmin, mut xmax) = (f64::MAX, f64::MIN);
        let (mut ymin, mut ymax) = (f64::MAX, f64::MIN);
        let (mut zmin, mut zmax) = (f64::MAX, f64::MIN);
        for p in &self.points {
            xmin = xmin.min(p[0]);
            xmax = xmax.max(p[0]);
            ymin = ymin.min(p[1]);
            ymax = ymax.max(p[1]);
            zmin = zmin.min(p[2]);
            zmax = zmax.max(p[2]);
        }
        let dx = xmax - xmin;
        let dy = ymax - ymin;
        let dz = zmax - zmin;
        let dmax = dx.max(dy).max(dz);
        let mid = [
            (xmin + xmax) / 2.0,
            (ymin + ymax) / 2.0,
            (zmin + zmax) / 2.0,
        ];

        let n = self.points.len();
        let margin = 20.0 * dmax;

        // Super-tetrahedron: 4 vertices far away
        let mut all_points: Vec<[f64; 3]> = self.points.clone();
        all_points.push([mid[0] - margin, mid[1] - margin, mid[2] - margin]); // n
        all_points.push([mid[0] + margin, mid[1] - margin, mid[2] - margin]); // n+1
        all_points.push([mid[0], mid[1] + margin, mid[2] - margin]);          // n+2
        all_points.push([mid[0], mid[1], mid[2] + margin]);                   // n+3

        let super_tet = Tet::new(n, n + 1, n + 2, n + 3);
        let mut tets = vec![super_tet];

        // Insert points one by one
        for pt_idx in 0..n {
            let px = all_points[pt_idx][0];
            let py = all_points[pt_idx][1];
            let pz = all_points[pt_idx][2];

            // Find all tets whose circumsphere contains the point
            let mut bad_tets = Vec::new();
            for (i, tet) in tets.iter().enumerate() {
                let (cx, cy, cz, r2) = circumsphere(
                    all_points[tet.v[0]],
                    all_points[tet.v[1]],
                    all_points[tet.v[2]],
                    all_points[tet.v[3]],
                );
                let dist2 = (px - cx).powi(2) + (py - cy).powi(2) + (pz - cz).powi(2);
                if dist2 < r2 + 1e-10 {
                    bad_tets.push(i);
                }
            }

            // Find boundary faces of the cavity (faces shared by exactly one bad tet)
            let mut face_count: HashMap<[usize; 3], usize> = HashMap::new();
            for &bi in &bad_tets {
                for face in tets[bi].faces() {
                    *face_count.entry(face).or_insert(0) += 1;
                }
            }

            let boundary_faces: Vec<[usize; 3]> = face_count
                .iter()
                .filter(|(_, &count)| count == 1)
                .map(|(f, _)| *f)
                .collect();

            // Remove bad tets (reverse order to preserve indices)
            let mut bad_sorted = bad_tets;
            bad_sorted.sort_unstable_by(|a, b| b.cmp(a));
            for bi in bad_sorted {
                tets.swap_remove(bi);
            }

            // Create new tets connecting the point to each boundary face
            for face in &boundary_faces {
                tets.push(Tet::new(pt_idx, face[0], face[1], face[2]));
            }
        }

        // Remove tets sharing vertices with the super-tetrahedron
        tets.retain(|tet| {
            !tet.contains_vertex(n)
                && !tet.contains_vertex(n + 1)
                && !tet.contains_vertex(n + 2)
                && !tet.contains_vertex(n + 3)
        });

        // Ensure consistent orientation (positive volume)
        let result: Vec<[usize; 4]> = tets
            .iter()
            .map(|tet| {
                let v = tet.v;
                let p0 = &self.points[v[0]];
                let p1 = &self.points[v[1]];
                let p2 = &self.points[v[2]];
                let p3 = &self.points[v[3]];

                // Compute signed volume
                let a = [p1[0] - p0[0], p1[1] - p0[1], p1[2] - p0[2]];
                let b = [p2[0] - p0[0], p2[1] - p0[1], p2[2] - p0[2]];
                let c = [p3[0] - p0[0], p3[1] - p0[1], p3[2] - p0[2]];
                let det = a[0] * (b[1] * c[2] - b[2] * c[1])
                    - a[1] * (b[0] * c[2] - b[2] * c[0])
                    + a[2] * (b[0] * c[1] - b[1] * c[0]);

                if det >= 0.0 {
                    v
                } else {
                    // Swap two vertices to flip orientation
                    [v[0], v[2], v[1], v[3]]
                }
            })
            .collect();

        Ok(result)
    }

    /// Build a 3D tetrahedral mesh as an UnstructuredMesh.
    pub fn build(&self) -> Result<UnstructuredMesh> {
        let tets = self.tetrahedralize()?;
        let n_pts = self.points.len();
        let n_tets = tets.len();

        // 1. Nodes
        let mut nodes = Vec::with_capacity(n_pts);
        for (i, p) in self.points.iter().enumerate() {
            nodes.push(Node::new(i, *p));
        }

        // Build face-to-tet adjacency
        let mut face_tets: HashMap<[usize; 3], Vec<usize>> = HashMap::new();
        for (ti, tet) in tets.iter().enumerate() {
            let tet_faces = tet_faces_sorted(*tet);
            for f in &tet_faces {
                face_tets.entry(*f).or_default().push(ti);
            }
        }

        // 2. Build cells
        let mut cells = Vec::with_capacity(n_tets);
        for (ti, tet) in tets.iter().enumerate() {
            let p0 = &self.points[tet[0]];
            let p1 = &self.points[tet[1]];
            let p2 = &self.points[tet[2]];
            let p3 = &self.points[tet[3]];

            let cx = (p0[0] + p1[0] + p2[0] + p3[0]) / 4.0;
            let cy = (p0[1] + p1[1] + p2[1] + p3[1]) / 4.0;
            let cz = (p0[2] + p1[2] + p2[2] + p3[2]) / 4.0;

            let vol = tet_volume(p0, p1, p2, p3);

            cells.push(Cell::new(
                ti,
                tet.to_vec(),
                Vec::new(),
                vol,
                [cx, cy, cz],
            ));
        }

        // 3. Build faces
        let mut faces: Vec<Face> = Vec::new();
        let mut boundary_face_ids = Vec::new();
        let mut created_faces: HashSet<[usize; 3]> = HashSet::new();

        for tet in &tets {
            let tet_fs = tet_faces_sorted(*tet);
            for f in &tet_fs {
                if created_faces.contains(f) {
                    continue;
                }
                created_faces.insert(*f);

                let face_id = faces.len();
                let face_nodes = vec![f[0], f[1], f[2]];

                let p0 = &self.points[f[0]];
                let p1 = &self.points[f[1]];
                let p2 = &self.points[f[2]];

                // Triangle area
                let e1 = [p1[0] - p0[0], p1[1] - p0[1], p1[2] - p0[2]];
                let e2 = [p2[0] - p0[0], p2[1] - p0[1], p2[2] - p0[2]];
                let cross = [
                    e1[1] * e2[2] - e1[2] * e2[1],
                    e1[2] * e2[0] - e1[0] * e2[2],
                    e1[0] * e2[1] - e1[1] * e2[0],
                ];
                let area = 0.5 * (cross[0] * cross[0] + cross[1] * cross[1] + cross[2] * cross[2]).sqrt();

                let normal_len = 2.0 * area;
                let normal = if normal_len > 1e-30 {
                    [cross[0] / normal_len, cross[1] / normal_len, cross[2] / normal_len]
                } else {
                    [0.0, 0.0, 1.0]
                };

                let center = [
                    (p0[0] + p1[0] + p2[0]) / 3.0,
                    (p0[1] + p1[1] + p2[1]) / 3.0,
                    (p0[2] + p1[2] + p2[2]) / 3.0,
                ];

                let adj = &face_tets[f];
                let (owner, neighbor);
                if adj.len() == 1 {
                    owner = adj[0];
                    neighbor = None;
                    boundary_face_ids.push(face_id);
                } else {
                    owner = adj[0];
                    neighbor = Some(adj[1]);
                }

                faces.push(Face::new(
                    face_id, face_nodes, owner, neighbor, area, normal, center,
                ));
            }
        }

        // 4. Boundary patches
        let mut boundary_patches = Vec::new();
        if !boundary_face_ids.is_empty() {
            boundary_patches.push(BoundaryPatch::new("boundary", boundary_face_ids));
        }

        // 5. Populate cell face lists
        for face in &faces {
            cells[face.owner_cell].faces.push(face.id);
            if let Some(nbr) = face.neighbor_cell {
                cells[nbr].faces.push(face.id);
            }
        }

        Ok(UnstructuredMesh::from_components(
            nodes,
            faces,
            cells,
            boundary_patches,
        ))
    }
}

/// Compute the 4 triangular faces of a tet, each sorted for consistent hashing.
fn tet_faces_sorted(tet: [usize; 4]) -> [[usize; 3]; 4] {
    let mut faces = [
        [tet[0], tet[1], tet[2]],
        [tet[0], tet[1], tet[3]],
        [tet[0], tet[2], tet[3]],
        [tet[1], tet[2], tet[3]],
    ];
    for f in &mut faces {
        f.sort();
    }
    faces
}

/// Compute the volume of a tetrahedron.
fn tet_volume(p0: &[f64; 3], p1: &[f64; 3], p2: &[f64; 3], p3: &[f64; 3]) -> f64 {
    let a = [p1[0] - p0[0], p1[1] - p0[1], p1[2] - p0[2]];
    let b = [p2[0] - p0[0], p2[1] - p0[1], p2[2] - p0[2]];
    let c = [p3[0] - p0[0], p3[1] - p0[1], p3[2] - p0[2]];
    let det = a[0] * (b[1] * c[2] - b[2] * c[1])
        - a[1] * (b[0] * c[2] - b[2] * c[0])
        + a[2] * (b[0] * c[1] - b[1] * c[0]);
    det.abs() / 6.0
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Helper: create a unit cube with 8 corner points.
    fn cube_points() -> Vec<[f64; 3]> {
        vec![
            [0.0, 0.0, 0.0],
            [1.0, 0.0, 0.0],
            [1.0, 1.0, 0.0],
            [0.0, 1.0, 0.0],
            [0.0, 0.0, 1.0],
            [1.0, 0.0, 1.0],
            [1.0, 1.0, 1.0],
            [0.0, 1.0, 1.0],
        ]
    }

    #[test]
    fn test_tetrahedralize_cube() {
        let mesher = TetMesher3D::new(cube_points());
        let tets = mesher.tetrahedralize().unwrap();
        // 8 points on a cube should produce tetrahedra that fill the cube
        assert!(
            tets.len() >= 5,
            "cube should have at least 5 tets, got {}",
            tets.len()
        );
    }

    #[test]
    fn test_build_cube_mesh() {
        let mesher = TetMesher3D::new(cube_points());
        let mesh = mesher.build().unwrap();
        assert_eq!(mesh.num_nodes(), 8);
        assert!(mesh.num_cells() >= 5);
    }

    #[test]
    fn test_total_volume_cube() {
        let mesher = TetMesher3D::new(cube_points());
        let mesh = mesher.build().unwrap();
        let total_vol: f64 = mesh.cells.iter().map(|c| c.volume).sum();
        assert!(
            (total_vol - 1.0).abs() < 1e-10,
            "cube volume {} expected 1.0",
            total_vol,
        );
    }

    #[test]
    fn test_all_cells_positive_volume() {
        let mesher = TetMesher3D::new(cube_points());
        let mesh = mesher.build().unwrap();
        for cell in &mesh.cells {
            assert!(
                cell.volume > 0.0,
                "cell {} has volume {}",
                cell.id,
                cell.volume,
            );
        }
    }

    #[test]
    fn test_delaunay_property_3d() {
        // Simple set of points and verify no point is inside any circumsphere
        let points = vec![
            [0.0, 0.0, 0.0],
            [2.0, 0.0, 0.0],
            [0.0, 2.0, 0.0],
            [0.0, 0.0, 2.0],
            [1.0, 1.0, 1.0],
        ];
        let mesher = TetMesher3D::new(points.clone());
        let tets = mesher.tetrahedralize().unwrap();

        for tet in &tets {
            let (cx, cy, cz, r2) = circumsphere(
                points[tet[0]],
                points[tet[1]],
                points[tet[2]],
                points[tet[3]],
            );
            for (pi, p) in points.iter().enumerate() {
                if pi == tet[0] || pi == tet[1] || pi == tet[2] || pi == tet[3] {
                    continue;
                }
                let d2 = (p[0] - cx).powi(2) + (p[1] - cy).powi(2) + (p[2] - cz).powi(2);
                assert!(
                    d2 >= r2 - 1e-8,
                    "point {} is inside circumsphere of tet {:?}",
                    pi,
                    tet,
                );
            }
        }
    }

    #[test]
    fn test_boundary_faces_exist() {
        let mesher = TetMesher3D::new(cube_points());
        let mesh = mesher.build().unwrap();
        let bp = mesh.boundary_patch("boundary");
        assert!(bp.is_some(), "should have a boundary patch");
        assert!(bp.unwrap().num_faces() > 0, "boundary should have faces");
    }

    #[test]
    fn test_too_few_points() {
        let mesher = TetMesher3D::new(vec![
            [0.0, 0.0, 0.0],
            [1.0, 0.0, 0.0],
            [0.0, 1.0, 0.0],
        ]);
        assert!(mesher.tetrahedralize().is_err());
    }

    #[test]
    fn test_cell_faces_populated() {
        let mesher = TetMesher3D::new(cube_points());
        let mesh = mesher.build().unwrap();
        for cell in &mesh.cells {
            assert!(
                !cell.faces.is_empty(),
                "cell {} has no faces",
                cell.id,
            );
            // Each tet should have exactly 4 faces
            assert_eq!(
                cell.faces.len(),
                4,
                "cell {} has {} faces, expected 4",
                cell.id,
                cell.faces.len(),
            );
        }
    }
}
