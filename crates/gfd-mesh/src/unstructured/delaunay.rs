//! 2D Delaunay triangulation using the Bowyer-Watson algorithm.
//!
//! Produces a triangulated 2D mesh (stored as single-layer hex/wedge cells in
//! an `UnstructuredMesh`) suitable for FVM or as input for Voronoi dual meshing.

use gfd_core::mesh::cell::Cell;
use gfd_core::mesh::face::Face;
use gfd_core::mesh::node::Node;
use gfd_core::mesh::unstructured::{BoundaryPatch, UnstructuredMesh};

use crate::{MeshError, MeshGenerator, Result};

/// A triangle represented by three point indices.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct Triangle {
    v: [usize; 3],
}

impl Triangle {
    fn new(a: usize, b: usize, c: usize) -> Self {
        Self { v: [a, b, c] }
    }

    /// Returns true if this triangle contains the given vertex index.
    fn contains_vertex(&self, v: usize) -> bool {
        self.v[0] == v || self.v[1] == v || self.v[2] == v
    }

    /// Returns the three edges as sorted pairs (for comparison).
    fn edges(&self) -> [[usize; 2]; 3] {
        let mut edges = [
            [self.v[0], self.v[1]],
            [self.v[1], self.v[2]],
            [self.v[2], self.v[0]],
        ];
        for e in &mut edges {
            if e[0] > e[1] {
                e.swap(0, 1);
            }
        }
        edges
    }
}

/// Compute circumcircle center and radius squared for a triangle.
fn circumcircle(
    ax: f64,
    ay: f64,
    bx: f64,
    by: f64,
    cx: f64,
    cy: f64,
) -> (f64, f64, f64) {
    let d = 2.0 * (ax * (by - cy) + bx * (cy - ay) + cx * (ay - by));
    if d.abs() < 1e-30 {
        // Degenerate (collinear) -- return huge radius
        return (0.0, 0.0, f64::MAX);
    }
    let ux = ((ax * ax + ay * ay) * (by - cy)
        + (bx * bx + by * by) * (cy - ay)
        + (cx * cx + cy * cy) * (ay - by))
        / d;
    let uy = ((ax * ax + ay * ay) * (cx - bx)
        + (bx * bx + by * by) * (ax - cx)
        + (cx * cx + cy * cy) * (bx - ax))
        / d;
    let r2 = (ax - ux) * (ax - ux) + (ay - uy) * (ay - uy);
    (ux, uy, r2)
}

/// 2D Delaunay triangulator using the Bowyer-Watson algorithm.
pub struct DelaunayMesher2D {
    /// Input points (x, y).
    points: Vec<[f64; 2]>,
    /// Maximum triangle area constraint (0 = no constraint).
    max_area: f64,
}

impl DelaunayMesher2D {
    /// Create a new Delaunay mesher with the given points.
    pub fn new(points: Vec<[f64; 2]>) -> Self {
        Self {
            points,
            max_area: 0.0,
        }
    }

    /// Set maximum allowed triangle area for refinement.
    pub fn with_max_area(mut self, area: f64) -> Self {
        self.max_area = area;
        self
    }

    /// Run the Bowyer-Watson algorithm and return triangles as index triples.
    ///
    /// The returned indices refer to `self.points` (super-triangle vertices excluded).
    pub fn triangulate(&self) -> Result<Vec<[usize; 3]>> {
        if self.points.len() < 3 {
            return Err(MeshError::InvalidParameters(
                "Need at least 3 points for Delaunay triangulation".to_string(),
            ));
        }

        // Find bounding box
        let (mut xmin, mut xmax, mut ymin, mut ymax) =
            (f64::MAX, f64::MIN, f64::MAX, f64::MIN);
        for p in &self.points {
            xmin = xmin.min(p[0]);
            xmax = xmax.max(p[0]);
            ymin = ymin.min(p[1]);
            ymax = ymax.max(p[1]);
        }
        let dx = xmax - xmin;
        let dy = ymax - ymin;
        let dmax = dx.max(dy);
        let mid_x = (xmin + xmax) / 2.0;
        let mid_y = (ymin + ymax) / 2.0;

        // Super-triangle: large enough to contain all points with margin
        let margin = 10.0 * dmax;
        let n = self.points.len();
        // Super-triangle vertices stored at indices n, n+1, n+2
        let mut all_points: Vec<[f64; 2]> = self.points.clone();
        all_points.push([mid_x - margin, mid_y - margin]);
        all_points.push([mid_x + margin, mid_y - margin]);
        all_points.push([mid_x, mid_y + margin]);

        let super_tri = Triangle::new(n, n + 1, n + 2);
        let mut triangles = vec![super_tri];

        // Insert points one by one
        for pt_idx in 0..n {
            let px = all_points[pt_idx][0];
            let py = all_points[pt_idx][1];

            // Find all triangles whose circumcircle contains the point
            let mut bad_triangles = Vec::new();
            for (i, tri) in triangles.iter().enumerate() {
                let (cx, cy, r2) = circumcircle(
                    all_points[tri.v[0]][0],
                    all_points[tri.v[0]][1],
                    all_points[tri.v[1]][0],
                    all_points[tri.v[1]][1],
                    all_points[tri.v[2]][0],
                    all_points[tri.v[2]][1],
                );
                let dist2 = (px - cx) * (px - cx) + (py - cy) * (py - cy);
                if dist2 < r2 + 1e-10 {
                    bad_triangles.push(i);
                }
            }

            // Find the boundary of the polygonal hole (edges shared by exactly one bad triangle)
            let mut edge_count: std::collections::HashMap<[usize; 2], usize> =
                std::collections::HashMap::new();
            // Also track which bad triangle each edge came from (for winding order)
            let mut edge_opposite: std::collections::HashMap<[usize; 2], usize> =
                std::collections::HashMap::new();

            for &bi in &bad_triangles {
                let tri = &triangles[bi];
                for edge in tri.edges() {
                    *edge_count.entry(edge).or_insert(0) += 1;
                    // Store the vertex of this triangle that is opposite the edge
                    let opp = tri.v.iter().find(|&&v| v != edge[0] && v != edge[1]).unwrap();
                    edge_opposite.insert(edge, *opp);
                }
            }

            let boundary_edges: Vec<[usize; 2]> = edge_count
                .iter()
                .filter(|(_, &count)| count == 1)
                .map(|(e, _)| *e)
                .collect();

            // Remove bad triangles (in reverse order to preserve indices)
            let mut bad_sorted = bad_triangles.clone();
            bad_sorted.sort_unstable_by(|a, b| b.cmp(a));
            for bi in bad_sorted {
                triangles.swap_remove(bi);
            }

            // Create new triangles connecting the point to each boundary edge
            for edge in &boundary_edges {
                triangles.push(Triangle::new(pt_idx, edge[0], edge[1]));
            }
        }

        // Remove triangles that share vertices with the super-triangle
        triangles.retain(|tri| {
            !tri.contains_vertex(n) && !tri.contains_vertex(n + 1) && !tri.contains_vertex(n + 2)
        });

        // Ensure consistent orientation (counter-clockwise)
        let result: Vec<[usize; 3]> = triangles
            .iter()
            .map(|tri| {
                let a = &self.points[tri.v[0]];
                let b = &self.points[tri.v[1]];
                let c = &self.points[tri.v[2]];
                // Cross product to check orientation
                let cross = (b[0] - a[0]) * (c[1] - a[1]) - (b[1] - a[1]) * (c[0] - a[0]);
                if cross >= 0.0 {
                    [tri.v[0], tri.v[1], tri.v[2]]
                } else {
                    [tri.v[0], tri.v[2], tri.v[1]]
                }
            })
            .collect();

        Ok(result)
    }
}

impl MeshGenerator for DelaunayMesher2D {
    /// Build a 2D triangular mesh as an UnstructuredMesh.
    ///
    /// Each triangle becomes a wedge cell (6 nodes, prism) with a single z-layer.
    fn build(&self) -> Result<UnstructuredMesh> {
        let triangles = self.triangulate()?;
        let n_pts = self.points.len();
        let n_tris = triangles.len();
        let dz = 1.0; // unit depth for 2D

        // 1. Nodes: bottom layer (z=0) + top layer (z=dz)
        let mut nodes = Vec::with_capacity(2 * n_pts);
        for (i, p) in self.points.iter().enumerate() {
            nodes.push(Node::new(i, [p[0], p[1], 0.0]));
        }
        for (i, p) in self.points.iter().enumerate() {
            nodes.push(Node::new(n_pts + i, [p[0], p[1], dz]));
        }

        // Build edge-to-triangle adjacency for face connectivity
        let mut edge_tris: std::collections::HashMap<[usize; 2], Vec<usize>> =
            std::collections::HashMap::new();
        for (ti, tri) in triangles.iter().enumerate() {
            let edges = [
                sorted_edge(tri[0], tri[1]),
                sorted_edge(tri[1], tri[2]),
                sorted_edge(tri[2], tri[0]),
            ];
            for e in &edges {
                edge_tris.entry(*e).or_default().push(ti);
            }
        }

        // 2. Faces + 3. Cells (build together)
        let mut faces: Vec<Face> = Vec::new();
        let mut cells: Vec<Cell> = Vec::with_capacity(n_tris);
        let mut boundary_face_ids = Vec::new();

        // Initialize cells
        for (ti, tri) in triangles.iter().enumerate() {
            let v0 = tri[0];
            let v1 = tri[1];
            let v2 = tri[2];

            // Wedge nodes: bottom triangle + top triangle
            let cell_nodes = vec![v0, v1, v2, n_pts + v0, n_pts + v1, n_pts + v2];

            // Cell center
            let cx = (self.points[v0][0] + self.points[v1][0] + self.points[v2][0]) / 3.0;
            let cy = (self.points[v0][1] + self.points[v1][1] + self.points[v2][1]) / 3.0;
            let cz = dz / 2.0;

            // Triangle area (cross product / 2)
            let ax = self.points[v1][0] - self.points[v0][0];
            let ay = self.points[v1][1] - self.points[v0][1];
            let bx = self.points[v2][0] - self.points[v0][0];
            let by = self.points[v2][1] - self.points[v0][1];
            let tri_area = (ax * by - ay * bx).abs() / 2.0;
            let vol = tri_area * dz;

            cells.push(Cell::new(ti, cell_nodes, Vec::new(), vol, [cx, cy, cz]));
        }

        // Create lateral faces (one per edge of each triangle, but shared edges only once)
        let mut created_edges: std::collections::HashSet<[usize; 2]> =
            std::collections::HashSet::new();

        for tri in &triangles {
            let tri_edges = [
                sorted_edge(tri[0], tri[1]),
                sorted_edge(tri[1], tri[2]),
                sorted_edge(tri[2], tri[0]),
            ];

            for edge in &tri_edges {
                if created_edges.contains(edge) {
                    continue;
                }
                created_edges.insert(*edge);

                let face_id = faces.len();
                let e0 = edge[0];
                let e1 = edge[1];
                let face_nodes = vec![e0, e1, n_pts + e1, n_pts + e0];

                // Edge length
                let dx = self.points[e1][0] - self.points[e0][0];
                let dy = self.points[e1][1] - self.points[e0][1];
                let edge_len = (dx * dx + dy * dy).sqrt();
                let area = edge_len * dz;

                // Normal: rotate edge direction 90 degrees (outward from owner)
                let nx = dy / edge_len;
                let ny = -dx / edge_len;

                let center = [
                    (self.points[e0][0] + self.points[e1][0]) / 2.0,
                    (self.points[e0][1] + self.points[e1][1]) / 2.0,
                    dz / 2.0,
                ];

                let adj = &edge_tris[edge];
                let (owner, neighbor);
                if adj.len() == 1 {
                    // Boundary face
                    owner = adj[0];
                    neighbor = None;
                    boundary_face_ids.push(face_id);
                } else {
                    // Internal face
                    owner = adj[0];
                    neighbor = Some(adj[1]);
                }

                faces.push(Face::new(
                    face_id,
                    face_nodes,
                    owner,
                    neighbor,
                    area,
                    [nx, ny, 0.0],
                    center,
                ));
            }
        }

        // Bottom (z=0) and top (z=dz) faces for each cell
        let mut zmin_faces = Vec::new();
        let mut zmax_faces = Vec::new();

        for (ti, tri) in triangles.iter().enumerate() {
            // Bottom face
            {
                let face_id = faces.len();
                let face_nodes = vec![tri[0], tri[1], tri[2]];

                let ax = self.points[tri[1]][0] - self.points[tri[0]][0];
                let ay = self.points[tri[1]][1] - self.points[tri[0]][1];
                let bx = self.points[tri[2]][0] - self.points[tri[0]][0];
                let by = self.points[tri[2]][1] - self.points[tri[0]][1];
                let area = (ax * by - ay * bx).abs() / 2.0;

                let center = [
                    (self.points[tri[0]][0] + self.points[tri[1]][0] + self.points[tri[2]][0])
                        / 3.0,
                    (self.points[tri[0]][1] + self.points[tri[1]][1] + self.points[tri[2]][1])
                        / 3.0,
                    0.0,
                ];

                zmin_faces.push(face_id);
                faces.push(Face::new(
                    face_id,
                    face_nodes,
                    ti,
                    None,
                    area,
                    [0.0, 0.0, -1.0],
                    center,
                ));
            }

            // Top face
            {
                let face_id = faces.len();
                let face_nodes = vec![n_pts + tri[0], n_pts + tri[1], n_pts + tri[2]];

                let ax = self.points[tri[1]][0] - self.points[tri[0]][0];
                let ay = self.points[tri[1]][1] - self.points[tri[0]][1];
                let bx = self.points[tri[2]][0] - self.points[tri[0]][0];
                let by = self.points[tri[2]][1] - self.points[tri[0]][1];
                let area = (ax * by - ay * bx).abs() / 2.0;

                let center = [
                    (self.points[tri[0]][0] + self.points[tri[1]][0] + self.points[tri[2]][0])
                        / 3.0,
                    (self.points[tri[0]][1] + self.points[tri[1]][1] + self.points[tri[2]][1])
                        / 3.0,
                    dz,
                ];

                zmax_faces.push(face_id);
                faces.push(Face::new(
                    face_id,
                    face_nodes,
                    ti,
                    None,
                    area,
                    [0.0, 0.0, 1.0],
                    center,
                ));
            }
        }

        // Boundary patches
        let mut boundary_patches = Vec::new();
        if !boundary_face_ids.is_empty() {
            boundary_patches.push(BoundaryPatch::new("boundary", boundary_face_ids));
        }
        if !zmin_faces.is_empty() {
            boundary_patches.push(BoundaryPatch::new("zmin", zmin_faces));
        }
        if !zmax_faces.is_empty() {
            boundary_patches.push(BoundaryPatch::new("zmax", zmax_faces));
        }

        // Populate cell face lists
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

/// Return a sorted edge pair for consistent hashing.
fn sorted_edge(a: usize, b: usize) -> [usize; 2] {
    if a <= b {
        [a, b]
    } else {
        [b, a]
    }
}

/// Returns the circumcenter (x, y) of a triangle. Used by the Voronoi dual.
pub fn circumcenter(p0: [f64; 2], p1: [f64; 2], p2: [f64; 2]) -> [f64; 2] {
    let (cx, cy, _) = circumcircle(p0[0], p0[1], p1[0], p1[1], p2[0], p2[1]);
    [cx, cy]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn square_points() -> Vec<[f64; 2]> {
        vec![
            [0.0, 0.0],
            [1.0, 0.0],
            [1.0, 1.0],
            [0.0, 1.0],
        ]
    }

    #[test]
    fn test_triangulate_square() {
        let mesher = DelaunayMesher2D::new(square_points());
        let tris = mesher.triangulate().unwrap();
        // 4 points in convex position => exactly 2 triangles
        assert_eq!(tris.len(), 2, "expected 2 triangles, got {}", tris.len());
    }

    #[test]
    fn test_build_square_mesh() {
        let mesher = DelaunayMesher2D::new(square_points());
        let mesh = mesher.build().unwrap();
        assert_eq!(mesh.num_cells(), 2);
        // 4 points * 2 layers = 8 nodes
        assert_eq!(mesh.num_nodes(), 8);
    }

    #[test]
    fn test_delaunay_property() {
        // Generate a grid of points and verify Delaunay property
        let mut points = Vec::new();
        for i in 0..5 {
            for j in 0..5 {
                points.push([i as f64, j as f64]);
            }
        }
        let mesher = DelaunayMesher2D::new(points.clone());
        let tris = mesher.triangulate().unwrap();

        // Verify: no point lies inside the circumcircle of any triangle
        for tri in &tris {
            let p0 = points[tri[0]];
            let p1 = points[tri[1]];
            let p2 = points[tri[2]];
            let (cx, cy, r2) = circumcircle(p0[0], p0[1], p1[0], p1[1], p2[0], p2[1]);

            for (pi, p) in points.iter().enumerate() {
                if pi == tri[0] || pi == tri[1] || pi == tri[2] {
                    continue;
                }
                let d2 = (p[0] - cx) * (p[0] - cx) + (p[1] - cy) * (p[1] - cy);
                // Allow small tolerance for points on the circumcircle
                assert!(
                    d2 >= r2 - 1e-8,
                    "point {} is inside circumcircle of tri {:?} (d2={}, r2={})",
                    pi,
                    tri,
                    d2,
                    r2
                );
            }
        }
    }

    #[test]
    fn test_triangulate_many_points() {
        // Euler formula: V - E + F = 2 => F = 2n - h - 2 for convex hull
        // where h = hull vertices. For interior points, triangles = 2*n - 2 - h
        let mut points = Vec::new();
        for i in 0..4 {
            for j in 0..4 {
                points.push([i as f64 * 0.3, j as f64 * 0.3]);
            }
        }
        let mesher = DelaunayMesher2D::new(points);
        let tris = mesher.triangulate().unwrap();
        // 16 points in a 4x4 grid => should produce valid triangulation
        assert!(tris.len() >= 10, "got {} triangles", tris.len());
    }

    #[test]
    fn test_all_cells_positive_volume() {
        let points = vec![
            [0.0, 0.0],
            [2.0, 0.0],
            [2.0, 2.0],
            [0.0, 2.0],
            [1.0, 1.0],
        ];
        let mesher = DelaunayMesher2D::new(points);
        let mesh = mesher.build().unwrap();
        for cell in &mesh.cells {
            assert!(cell.volume > 0.0, "cell {} has volume {}", cell.id, cell.volume);
        }
    }

    #[test]
    fn test_too_few_points() {
        let mesher = DelaunayMesher2D::new(vec![[0.0, 0.0], [1.0, 0.0]]);
        assert!(mesher.triangulate().is_err());
    }

    #[test]
    fn test_circumcenter_equilateral() {
        let p0 = [0.0, 0.0];
        let p1 = [1.0, 0.0];
        let p2 = [0.5, (3.0_f64).sqrt() / 2.0];
        let cc = circumcenter(p0, p1, p2);
        assert!((cc[0] - 0.5).abs() < 1e-12);
        assert!((cc[1] - (3.0_f64).sqrt() / 6.0).abs() < 1e-12);
    }
}
