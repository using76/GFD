//! Voronoi dual mesh generator.
//!
//! Given a 2D Delaunay triangulation, computes the Voronoi dual mesh where:
//! - Each Voronoi vertex is the circumcenter of a Delaunay triangle
//! - Each Voronoi cell is the polygon surrounding a Delaunay point
//! - This creates polyhedral cells suitable for FVM (Fluent-style poly mesh)

use std::collections::HashMap;

use gfd_core::mesh::cell::Cell;
use gfd_core::mesh::face::Face;
use gfd_core::mesh::node::Node;
use gfd_core::mesh::unstructured::{BoundaryPatch, UnstructuredMesh};

use super::delaunay::{circumcenter, DelaunayMesher2D};
use crate::{MeshError, MeshGenerator, Result};

/// Builder for a Voronoi dual mesh from a set of 2D seed points.
///
/// Internally performs a Delaunay triangulation, then computes the dual.
pub struct VoronoiMeshBuilder {
    /// Seed points for the Voronoi diagram.
    points: Vec<[f64; 2]>,
    /// Depth in z for the single-layer 3D mesh.
    depth: f64,
}

impl VoronoiMeshBuilder {
    /// Create a new Voronoi mesh builder.
    ///
    /// # Arguments
    /// * `points` - 2D seed points (Voronoi cell centers)
    /// * `depth` - z-thickness for the single hex layer
    pub fn new(points: Vec<[f64; 2]>, depth: f64) -> Self {
        Self { points, depth }
    }
}

impl MeshGenerator for VoronoiMeshBuilder {
    fn build(&self) -> Result<UnstructuredMesh> {
        if self.points.len() < 3 {
            return Err(MeshError::InvalidParameters(
                "Need at least 3 seed points for Voronoi mesh".to_string(),
            ));
        }
        if self.depth <= 0.0 {
            return Err(MeshError::InvalidParameters(
                "depth must be > 0".to_string(),
            ));
        }

        let dz = self.depth;
        let n_seeds = self.points.len();

        // 1. Delaunay triangulation
        let mesher = DelaunayMesher2D::new(self.points.clone());
        let triangles = mesher.triangulate()?;
        let n_tris = triangles.len();

        // 2. Compute circumcenters (Voronoi vertices)
        let voronoi_vertices: Vec<[f64; 2]> = triangles
            .iter()
            .map(|tri| {
                circumcenter(
                    self.points[tri[0]],
                    self.points[tri[1]],
                    self.points[tri[2]],
                )
            })
            .collect();

        // 3. Build adjacency: for each seed point, find which triangles use it,
        //    then order the corresponding circumcenters around the seed point.
        let mut point_triangles: Vec<Vec<usize>> = vec![Vec::new(); n_seeds];
        for (ti, tri) in triangles.iter().enumerate() {
            point_triangles[tri[0]].push(ti);
            point_triangles[tri[1]].push(ti);
            point_triangles[tri[2]].push(ti);
        }

        // Edge-to-triangle adjacency
        let mut edge_tris: HashMap<[usize; 2], Vec<usize>> = HashMap::new();
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

        // For each seed point, order the Voronoi vertices (circumcenters of
        // adjacent triangles) in angular order around the seed point.
        let mut voronoi_cells: Vec<Vec<usize>> = Vec::with_capacity(n_seeds);
        let mut is_boundary_cell = vec![false; n_seeds];

        for pi in 0..n_seeds {
            let adj_tris = &point_triangles[pi];
            if adj_tris.is_empty() {
                voronoi_cells.push(Vec::new());
                is_boundary_cell[pi] = true;
                continue;
            }

            // Check if this point is on the convex hull boundary
            // A point is on the boundary if any of its edges is a boundary edge
            // (shared by only one triangle)
            let mut on_boundary = false;
            for &ti in adj_tris {
                let tri = &triangles[ti];
                let edges = [
                    sorted_edge(tri[0], tri[1]),
                    sorted_edge(tri[1], tri[2]),
                    sorted_edge(tri[2], tri[0]),
                ];
                for e in &edges {
                    if (e[0] == pi || e[1] == pi) && edge_tris[e].len() == 1 {
                        on_boundary = true;
                        break;
                    }
                }
                if on_boundary {
                    break;
                }
            }
            is_boundary_cell[pi] = on_boundary;

            // Sort triangles by angle of their circumcenter relative to the seed point
            let seed = self.points[pi];
            let mut sorted_tris = adj_tris.clone();
            sorted_tris.sort_by(|&a, &b| {
                let va = voronoi_vertices[a];
                let vb = voronoi_vertices[b];
                let angle_a = (va[1] - seed[1]).atan2(va[0] - seed[0]);
                let angle_b = (vb[1] - seed[1]).atan2(vb[0] - seed[0]);
                angle_a.partial_cmp(&angle_b).unwrap()
            });

            voronoi_cells.push(sorted_tris);
        }

        // 4. Build mesh nodes: Voronoi vertices at z=0 and z=dz
        let mut nodes = Vec::with_capacity(2 * n_tris);
        for (i, v) in voronoi_vertices.iter().enumerate() {
            nodes.push(Node::new(i, [v[0], v[1], 0.0]));
        }
        for (i, v) in voronoi_vertices.iter().enumerate() {
            nodes.push(Node::new(n_tris + i, [v[0], v[1], dz]));
        }

        // 5. Build cells (only for interior Voronoi cells)
        // Map from seed point index to cell index (skip boundary cells)
        let mut seed_to_cell: Vec<Option<usize>> = vec![None; n_seeds];
        let mut cells = Vec::new();

        for pi in 0..n_seeds {
            if is_boundary_cell[pi] || voronoi_cells[pi].len() < 3 {
                continue;
            }

            let cell_id = cells.len();
            seed_to_cell[pi] = Some(cell_id);

            let vor_verts = &voronoi_cells[pi];
            let nv = vor_verts.len();

            // Cell nodes: bottom polygon + top polygon
            let mut cell_nodes = Vec::with_capacity(2 * nv);
            for &ti in vor_verts {
                cell_nodes.push(ti); // bottom
            }
            for &ti in vor_verts {
                cell_nodes.push(n_tris + ti); // top
            }

            // Cell center is the seed point
            let center = [self.points[pi][0], self.points[pi][1], dz / 2.0];

            // Volume: polygon area * dz
            let poly_area = polygon_area_2d(&voronoi_vertices, vor_verts);
            let vol = poly_area * dz;

            cells.push(Cell::new(cell_id, cell_nodes, Vec::new(), vol, center));
        }

        // 6. Build faces
        let mut faces: Vec<Face> = Vec::new();
        let mut boundary_lateral_faces = Vec::new();
        let mut zmin_faces = Vec::new();
        let mut zmax_faces = Vec::new();

        // Lateral faces: one per Delaunay edge (shared by two Voronoi cells)
        let mut created_edges: std::collections::HashSet<[usize; 2]> =
            std::collections::HashSet::new();

        for (edge, adj) in &edge_tris {
            if created_edges.contains(edge) {
                continue;
            }
            created_edges.insert(*edge);

            let pi_a = edge[0];
            let pi_b = edge[1];
            let cell_a = seed_to_cell[pi_a];
            let cell_b = seed_to_cell[pi_b];

            // Only create face if at least one side has a valid cell
            if cell_a.is_none() && cell_b.is_none() {
                continue;
            }

            if adj.len() == 2 {
                // Internal Delaunay edge -> Voronoi face between two circumcenters
                let v0 = adj[0]; // Voronoi vertex index (= triangle index)
                let v1 = adj[1];

                let face_id = faces.len();
                let face_nodes = vec![v0, v1, n_tris + v1, n_tris + v0];

                // Face area: distance between circumcenters * dz
                let dx = voronoi_vertices[v1][0] - voronoi_vertices[v0][0];
                let dy = voronoi_vertices[v1][1] - voronoi_vertices[v0][1];
                let edge_len = (dx * dx + dy * dy).sqrt();
                let area = edge_len * dz;

                // Skip degenerate faces (coincident circumcenters, e.g. on regular grids)
                if edge_len < 1e-14 {
                    continue;
                }

                // Normal: direction from seed_a to seed_b
                let sdx = self.points[pi_b][0] - self.points[pi_a][0];
                let sdy = self.points[pi_b][1] - self.points[pi_a][1];
                let slen = (sdx * sdx + sdy * sdy).sqrt();
                let normal = if slen > 1e-30 {
                    [sdx / slen, sdy / slen, 0.0]
                } else {
                    [1.0, 0.0, 0.0]
                };

                let center = [
                    (voronoi_vertices[v0][0] + voronoi_vertices[v1][0]) / 2.0,
                    (voronoi_vertices[v0][1] + voronoi_vertices[v1][1]) / 2.0,
                    dz / 2.0,
                ];

                match (cell_a, cell_b) {
                    (Some(ca), Some(cb)) => {
                        faces.push(Face::new(
                            face_id, face_nodes, ca, Some(cb), area, normal, center,
                        ));
                    }
                    (Some(ca), None) => {
                        boundary_lateral_faces.push(face_id);
                        faces.push(Face::new(
                            face_id, face_nodes, ca, None, area, normal, center,
                        ));
                    }
                    (None, Some(cb)) => {
                        boundary_lateral_faces.push(face_id);
                        let neg_normal = [-normal[0], -normal[1], -normal[2]];
                        faces.push(Face::new(
                            face_id, face_nodes, cb, None, area, neg_normal, center,
                        ));
                    }
                    _ => {}
                }
            } else if adj.len() == 1 {
                // Boundary Delaunay edge -> boundary Voronoi face
                // This edge is on the convex hull; skip or create boundary face
                // We skip these as the adjacent Voronoi cells are boundary (infinite)
            }
        }

        // Top and bottom faces for each Voronoi cell
        for pi in 0..n_seeds {
            if let Some(cell_id) = seed_to_cell[pi] {
                let vor_verts = &voronoi_cells[pi];

                // Bottom face (z=0)
                {
                    let face_id = faces.len();
                    let face_nodes: Vec<usize> = vor_verts.iter().copied().collect();
                    let area = polygon_area_2d(&voronoi_vertices, vor_verts);
                    let center = [self.points[pi][0], self.points[pi][1], 0.0];

                    zmin_faces.push(face_id);
                    faces.push(Face::new(
                        face_id,
                        face_nodes,
                        cell_id,
                        None,
                        area,
                        [0.0, 0.0, -1.0],
                        center,
                    ));
                }

                // Top face (z=dz)
                {
                    let face_id = faces.len();
                    let face_nodes: Vec<usize> =
                        vor_verts.iter().map(|&v| n_tris + v).collect();
                    let area = polygon_area_2d(&voronoi_vertices, vor_verts);
                    let center = [self.points[pi][0], self.points[pi][1], dz];

                    zmax_faces.push(face_id);
                    faces.push(Face::new(
                        face_id,
                        face_nodes,
                        cell_id,
                        None,
                        area,
                        [0.0, 0.0, 1.0],
                        center,
                    ));
                }
            }
        }

        // Boundary patches
        let mut boundary_patches = Vec::new();
        if !boundary_lateral_faces.is_empty() {
            boundary_patches.push(BoundaryPatch::new("boundary", boundary_lateral_faces));
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

fn sorted_edge(a: usize, b: usize) -> [usize; 2] {
    if a <= b { [a, b] } else { [b, a] }
}

/// Compute the area of a 2D polygon given vertex indices into a vertex array.
fn polygon_area_2d(vertices: &[[f64; 2]], indices: &[usize]) -> f64 {
    let n = indices.len();
    if n < 3 {
        return 0.0;
    }
    let mut area = 0.0;
    for i in 0..n {
        let cur = vertices[indices[i]];
        let next = vertices[indices[(i + 1) % n]];
        area += cur[0] * next[1] - next[0] * cur[1];
    }
    area.abs() / 2.0
}

#[cfg(test)]
mod tests {
    use super::*;

    fn grid_points(nx: usize, ny: usize, spacing: f64) -> Vec<[f64; 2]> {
        let mut pts = Vec::new();
        for j in 0..ny {
            for i in 0..nx {
                pts.push([i as f64 * spacing, j as f64 * spacing]);
            }
        }
        pts
    }

    #[test]
    fn test_voronoi_basic() {
        // 4x4 grid of points
        let points = grid_points(4, 4, 1.0);
        let builder = VoronoiMeshBuilder::new(points, 1.0);
        let mesh = builder.build().unwrap();

        // Should produce some interior Voronoi cells
        assert!(
            mesh.num_cells() > 0,
            "should have at least one Voronoi cell"
        );

        // All cells should have positive volume
        for cell in &mesh.cells {
            assert!(cell.volume > 0.0, "cell {} vol={}", cell.id, cell.volume);
        }
    }

    #[test]
    fn test_voronoi_has_faces() {
        let points = grid_points(3, 3, 1.0);
        let builder = VoronoiMeshBuilder::new(points, 0.5);
        let mesh = builder.build().unwrap();

        // Every cell should have at least 3 faces (bottom, top, and lateral)
        for cell in &mesh.cells {
            assert!(
                cell.faces.len() >= 3,
                "cell {} has only {} faces",
                cell.id,
                cell.faces.len()
            );
        }
    }

    #[test]
    fn test_voronoi_boundary_patches() {
        let points = grid_points(4, 4, 1.0);
        let builder = VoronoiMeshBuilder::new(points, 1.0);
        let mesh = builder.build().unwrap();

        // Should have zmin and zmax patches
        assert!(mesh.boundary_patch("zmin").is_some());
        assert!(mesh.boundary_patch("zmax").is_some());
    }

    #[test]
    fn test_voronoi_too_few_points() {
        let builder = VoronoiMeshBuilder::new(vec![[0.0, 0.0], [1.0, 0.0]], 1.0);
        assert!(builder.build().is_err());
    }

    #[test]
    fn test_voronoi_all_faces_positive_area() {
        let points = grid_points(5, 5, 0.5);
        let builder = VoronoiMeshBuilder::new(points, 1.0);
        let mesh = builder.build().unwrap();

        for face in &mesh.faces {
            assert!(
                face.area > 0.0,
                "face {} has area {}",
                face.id,
                face.area
            );
        }
    }

    #[test]
    fn test_polygon_area_2d_unit_square() {
        let vertices = vec![[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]];
        let indices = vec![0, 1, 2, 3];
        let area = polygon_area_2d(&vertices, &indices);
        assert!((area - 1.0).abs() < 1e-12);
    }

    #[test]
    fn test_polygon_area_2d_triangle() {
        let vertices = vec![[0.0, 0.0], [2.0, 0.0], [0.0, 3.0]];
        let indices = vec![0, 1, 2];
        let area = polygon_area_2d(&vertices, &indices);
        assert!((area - 3.0).abs() < 1e-12);
    }
}
