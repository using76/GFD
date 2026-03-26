//! Wall prism layer extrusion.
//!
//! Given a surface mesh (boundary faces) and normal information, extrude
//! prism/wedge layers outward from the wall for boundary-layer resolution.

use std::collections::{HashMap, HashSet};

use gfd_core::mesh::cell::Cell;
use gfd_core::mesh::face::Face;
use gfd_core::mesh::node::Node;
use gfd_core::mesh::unstructured::{BoundaryPatch, UnstructuredMesh};

use crate::{MeshError, MeshGenerator, Result};

/// Builder for wall-normal prism layers.
///
/// Takes a base surface mesh (a set of triangular or quad faces defined by
/// 2D points) and extrudes layers of prism cells in the wall-normal direction.
pub struct PrismLayerBuilder {
    /// First layer height.
    first_height: f64,
    /// Growth ratio between successive layers.
    growth_ratio: f64,
    /// Number of prism layers.
    num_layers: usize,
    /// Wall surface nodes: (x, y, z).
    wall_nodes: Vec<[f64; 3]>,
    /// Wall surface faces: each is a list of node indices.
    wall_faces: Vec<Vec<usize>>,
    /// Outward-pointing unit normals at each wall node.
    wall_normals: Vec<[f64; 3]>,
}

impl PrismLayerBuilder {
    /// Create a new prism layer builder.
    ///
    /// # Arguments
    /// * `first_height` - Height of the first prism layer
    /// * `growth_ratio` - Ratio by which each layer height grows
    /// * `num_layers` - Number of prism layers to extrude
    /// * `wall_nodes` - 3D positions of wall surface nodes
    /// * `wall_faces` - Surface face connectivity (indices into `wall_nodes`)
    /// * `wall_normals` - Unit outward normal at each wall node
    pub fn new(
        first_height: f64,
        growth_ratio: f64,
        num_layers: usize,
        wall_nodes: Vec<[f64; 3]>,
        wall_faces: Vec<Vec<usize>>,
        wall_normals: Vec<[f64; 3]>,
    ) -> Self {
        Self {
            first_height,
            growth_ratio,
            num_layers,
            wall_nodes,
            wall_faces,
            wall_normals,
        }
    }

    /// Compute cumulative layer heights: [0, h1, h1+h2, ...].
    fn cumulative_heights(&self) -> Vec<f64> {
        let mut heights = Vec::with_capacity(self.num_layers + 1);
        heights.push(0.0);
        let mut h = self.first_height;
        let mut cum = 0.0;
        for _ in 0..self.num_layers {
            cum += h;
            heights.push(cum);
            h *= self.growth_ratio;
        }
        heights
    }
}

impl MeshGenerator for PrismLayerBuilder {
    fn build(&self) -> Result<UnstructuredMesh> {
        if self.first_height <= 0.0 {
            return Err(MeshError::InvalidParameters(
                "first_height must be > 0".to_string(),
            ));
        }
        if self.growth_ratio <= 0.0 {
            return Err(MeshError::InvalidParameters(
                "growth_ratio must be > 0".to_string(),
            ));
        }
        if self.num_layers == 0 {
            return Err(MeshError::InvalidParameters(
                "num_layers must be > 0".to_string(),
            ));
        }
        if self.wall_nodes.is_empty() || self.wall_faces.is_empty() {
            return Err(MeshError::InvalidParameters(
                "wall_nodes and wall_faces must be non-empty".to_string(),
            ));
        }
        if self.wall_normals.len() != self.wall_nodes.len() {
            return Err(MeshError::InvalidParameters(
                "wall_normals must have same length as wall_nodes".to_string(),
            ));
        }

        let n_wall = self.wall_nodes.len();
        let n_faces = self.wall_faces.len();
        let nl = self.num_layers;
        let cum_h = self.cumulative_heights();

        // 1. Create nodes: (num_layers + 1) layers of wall nodes
        let n_total_nodes = n_wall * (nl + 1);
        let mut nodes = Vec::with_capacity(n_total_nodes);

        let node_idx = |wall_node: usize, layer: usize| -> usize { layer * n_wall + wall_node };

        for layer in 0..=nl {
            let h = cum_h[layer];
            for wn in 0..n_wall {
                let id = node_idx(wn, layer);
                let base = &self.wall_nodes[wn];
                let normal = &self.wall_normals[wn];
                nodes.push(Node::new(id, [
                    base[0] + h * normal[0],
                    base[1] + h * normal[1],
                    base[2] + h * normal[2],
                ]));
            }
        }

        // Cell indexing: (face_index, layer) -> cell_id
        let cell_idx = |face: usize, layer: usize| -> usize { layer * n_faces + face };

        // 2. Create cells (prisms/hexahedra depending on face vertex count)
        let n_total_cells = n_faces * nl;
        let mut cells = Vec::with_capacity(n_total_cells);

        for layer in 0..nl {
            for (fi, wall_face) in self.wall_faces.iter().enumerate() {
                let cell_id = cell_idx(fi, layer);
                let nv = wall_face.len();

                // Bottom nodes (at layer) + top nodes (at layer+1)
                let mut cell_nodes = Vec::with_capacity(2 * nv);
                for &wn in wall_face {
                    cell_nodes.push(node_idx(wn, layer));
                }
                for &wn in wall_face {
                    cell_nodes.push(node_idx(wn, layer + 1));
                }

                // Compute cell volume and center
                let layer_h = cum_h[layer + 1] - cum_h[layer];

                // Approximate face area using the wall face at mid-layer
                let mid_h = (cum_h[layer] + cum_h[layer + 1]) / 2.0;
                let mut cx = 0.0;
                let mut cy = 0.0;
                let mut cz = 0.0;
                for &wn in wall_face {
                    let n = &self.wall_normals[wn];
                    let b = &self.wall_nodes[wn];
                    cx += b[0] + mid_h * n[0];
                    cy += b[1] + mid_h * n[1];
                    cz += b[2] + mid_h * n[2];
                }
                cx /= nv as f64;
                cy /= nv as f64;
                cz /= nv as f64;

                // Compute face area from wall face (approximate for prism volume)
                let face_area = compute_polygon_area(&self.wall_nodes, wall_face);
                let vol = face_area * layer_h;

                cells.push(Cell::new(cell_id, cell_nodes, Vec::new(), vol, [cx, cy, cz]));
            }
        }

        // 3. Build faces
        let mut faces: Vec<Face> = Vec::new();
        let mut wall_boundary_faces = Vec::new();
        let mut outer_boundary_faces = Vec::new();

        // Build edge-to-face adjacency for lateral face identification
        let mut edge_faces: HashMap<[usize; 2], Vec<usize>> = HashMap::new();
        for (fi, face) in self.wall_faces.iter().enumerate() {
            let nv = face.len();
            for k in 0..nv {
                let e = sorted_edge(face[k], face[(k + 1) % nv]);
                edge_faces.entry(e).or_default().push(fi);
            }
        }

        // Bottom faces (wall boundary, layer=0)
        for (fi, wall_face) in self.wall_faces.iter().enumerate() {
            let face_id = faces.len();
            let face_nodes: Vec<usize> = wall_face.iter().map(|&wn| node_idx(wn, 0)).collect();

            let area = compute_polygon_area(&self.wall_nodes, wall_face);

            // Normal pointing inward (into the wall)
            let avg_normal = average_normal(&self.wall_normals, wall_face);
            let normal = [-avg_normal[0], -avg_normal[1], -avg_normal[2]];

            let center = face_center_at_layer(&self.wall_nodes, &self.wall_normals, wall_face, 0.0);

            wall_boundary_faces.push(face_id);
            faces.push(Face::new(face_id, face_nodes, cell_idx(fi, 0), None, area, normal, center));
        }

        // Top faces (outer boundary, layer=nl)
        for (fi, wall_face) in self.wall_faces.iter().enumerate() {
            let face_id = faces.len();
            let face_nodes: Vec<usize> = wall_face.iter().map(|&wn| node_idx(wn, nl)).collect();

            let area = compute_polygon_area(&self.wall_nodes, wall_face);
            let avg_normal = average_normal(&self.wall_normals, wall_face);
            let center = face_center_at_layer(
                &self.wall_nodes,
                &self.wall_normals,
                wall_face,
                cum_h[nl],
            );

            outer_boundary_faces.push(face_id);
            faces.push(Face::new(
                face_id,
                face_nodes,
                cell_idx(fi, nl - 1),
                None,
                area,
                avg_normal,
                center,
            ));
        }

        // Inter-layer faces (between layer and layer+1 for the same face)
        for layer in 1..nl {
            for (fi, wall_face) in self.wall_faces.iter().enumerate() {
                let face_id = faces.len();
                let face_nodes: Vec<usize> =
                    wall_face.iter().map(|&wn| node_idx(wn, layer)).collect();

                let area = compute_polygon_area(&self.wall_nodes, wall_face);
                let avg_normal = average_normal(&self.wall_normals, wall_face);
                let center = face_center_at_layer(
                    &self.wall_nodes,
                    &self.wall_normals,
                    wall_face,
                    cum_h[layer],
                );

                let owner = cell_idx(fi, layer - 1);
                let neighbor = Some(cell_idx(fi, layer));

                faces.push(Face::new(face_id, face_nodes, owner, neighbor, area, avg_normal, center));
            }
        }

        // Lateral faces (between adjacent surface faces sharing an edge)
        // and lateral boundary faces (edges on the surface boundary)
        let mut lateral_boundary_faces = Vec::new();

        for layer in 0..nl {
            let mut created_edges: HashSet<[usize; 2]> = HashSet::new();

            for wall_face in &self.wall_faces {
                let nv = wall_face.len();
                for k in 0..nv {
                    let e = sorted_edge(wall_face[k], wall_face[(k + 1) % nv]);
                    if created_edges.contains(&e) {
                        continue;
                    }
                    created_edges.insert(e);

                    let face_id = faces.len();
                    let e0 = e[0];
                    let e1 = e[1];
                    let face_nodes = vec![
                        node_idx(e0, layer),
                        node_idx(e1, layer),
                        node_idx(e1, layer + 1),
                        node_idx(e0, layer + 1),
                    ];

                    // Area: edge_length * layer_height
                    let dx = self.wall_nodes[e1][0] - self.wall_nodes[e0][0];
                    let dy = self.wall_nodes[e1][1] - self.wall_nodes[e0][1];
                    let dz_edge = self.wall_nodes[e1][2] - self.wall_nodes[e0][2];
                    let edge_len = (dx * dx + dy * dy + dz_edge * dz_edge).sqrt();
                    let layer_h = cum_h[layer + 1] - cum_h[layer];
                    let area = edge_len * layer_h;

                    // Normal: cross product of edge direction and extrusion direction
                    let avg_n0 = &self.wall_normals[e0];
                    let avg_n1 = &self.wall_normals[e1];
                    let extrude = [
                        (avg_n0[0] + avg_n1[0]) / 2.0,
                        (avg_n0[1] + avg_n1[1]) / 2.0,
                        (avg_n0[2] + avg_n1[2]) / 2.0,
                    ];
                    let normal = cross_normalize(
                        [dx, dy, dz_edge],
                        extrude,
                    );

                    let mid_h = (cum_h[layer] + cum_h[layer + 1]) / 2.0;
                    let center = [
                        (self.wall_nodes[e0][0] + self.wall_nodes[e1][0]) / 2.0
                            + mid_h * (avg_n0[0] + avg_n1[0]) / 2.0,
                        (self.wall_nodes[e0][1] + self.wall_nodes[e1][1]) / 2.0
                            + mid_h * (avg_n0[1] + avg_n1[1]) / 2.0,
                        (self.wall_nodes[e0][2] + self.wall_nodes[e1][2]) / 2.0
                            + mid_h * (avg_n0[2] + avg_n1[2]) / 2.0,
                    ];

                    let adj = &edge_faces[&e];
                    if adj.len() == 1 {
                        // Boundary lateral face
                        let owner = cell_idx(adj[0], layer);
                        lateral_boundary_faces.push(face_id);
                        faces.push(Face::new(
                            face_id, face_nodes, owner, None, area, normal, center,
                        ));
                    } else {
                        // Internal lateral face
                        let owner = cell_idx(adj[0], layer);
                        let neighbor = Some(cell_idx(adj[1], layer));
                        faces.push(Face::new(
                            face_id, face_nodes, owner, neighbor, area, normal, center,
                        ));
                    }
                }
            }
        }

        // Boundary patches
        let mut boundary_patches = Vec::new();
        if !wall_boundary_faces.is_empty() {
            boundary_patches.push(BoundaryPatch::new("wall", wall_boundary_faces));
        }
        if !outer_boundary_faces.is_empty() {
            boundary_patches.push(BoundaryPatch::new("outer", outer_boundary_faces));
        }
        if !lateral_boundary_faces.is_empty() {
            boundary_patches.push(BoundaryPatch::new("lateral", lateral_boundary_faces));
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

/// Sort an edge pair for consistent hashing.
fn sorted_edge(a: usize, b: usize) -> [usize; 2] {
    if a <= b { [a, b] } else { [b, a] }
}

/// Compute the area of a planar polygon given its vertices.
fn compute_polygon_area(all_nodes: &[[f64; 3]], face_indices: &[usize]) -> f64 {
    if face_indices.len() < 3 {
        return 0.0;
    }
    // Use the shoelace formula generalised to 3D (Newell's method)
    let mut nx = 0.0;
    let mut ny = 0.0;
    let mut nz = 0.0;
    let n = face_indices.len();
    for i in 0..n {
        let cur = &all_nodes[face_indices[i]];
        let next = &all_nodes[face_indices[(i + 1) % n]];
        nx += (cur[1] - next[1]) * (cur[2] + next[2]);
        ny += (cur[2] - next[2]) * (cur[0] + next[0]);
        nz += (cur[0] - next[0]) * (cur[1] + next[1]);
    }
    0.5 * (nx * nx + ny * ny + nz * nz).sqrt()
}

/// Average normal of a face from its vertex normals.
fn average_normal(normals: &[[f64; 3]], face_indices: &[usize]) -> [f64; 3] {
    let mut n = [0.0, 0.0, 0.0];
    for &idx in face_indices {
        n[0] += normals[idx][0];
        n[1] += normals[idx][1];
        n[2] += normals[idx][2];
    }
    let len = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt();
    if len > 1e-30 {
        [n[0] / len, n[1] / len, n[2] / len]
    } else {
        [0.0, 0.0, 1.0]
    }
}

/// Compute face center at a given extrusion height.
fn face_center_at_layer(
    wall_nodes: &[[f64; 3]],
    normals: &[[f64; 3]],
    face_indices: &[usize],
    h: f64,
) -> [f64; 3] {
    let n = face_indices.len() as f64;
    let mut c = [0.0, 0.0, 0.0];
    for &idx in face_indices {
        c[0] += wall_nodes[idx][0] + h * normals[idx][0];
        c[1] += wall_nodes[idx][1] + h * normals[idx][1];
        c[2] += wall_nodes[idx][2] + h * normals[idx][2];
    }
    [c[0] / n, c[1] / n, c[2] / n]
}

/// Cross product of two vectors, normalised.
fn cross_normalize(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    let cx = a[1] * b[2] - a[2] * b[1];
    let cy = a[2] * b[0] - a[0] * b[2];
    let cz = a[0] * b[1] - a[1] * b[0];
    let len = (cx * cx + cy * cy + cz * cz).sqrt();
    if len > 1e-30 {
        [cx / len, cy / len, cz / len]
    } else {
        [0.0, 0.0, 1.0]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Helper: create a simple flat square surface for testing.
    fn flat_square_surface() -> (Vec<[f64; 3]>, Vec<Vec<usize>>, Vec<[f64; 3]>) {
        let nodes = vec![
            [0.0, 0.0, 0.0],
            [1.0, 0.0, 0.0],
            [1.0, 1.0, 0.0],
            [0.0, 1.0, 0.0],
        ];
        // Two triangles making a square
        let faces = vec![vec![0, 1, 2], vec![0, 2, 3]];
        // All normals pointing in +z
        let normals = vec![[0.0, 0.0, 1.0]; 4];
        (nodes, faces, normals)
    }

    #[test]
    fn test_basic_prism_layer() {
        let (nodes, faces, normals) = flat_square_surface();
        let builder = PrismLayerBuilder::new(0.1, 1.2, 3, nodes, faces, normals);
        let mesh = builder.build().unwrap();

        // 2 surface faces * 3 layers = 6 cells
        assert_eq!(mesh.num_cells(), 6);
        // 4 wall nodes * 4 layers = 16 nodes
        assert_eq!(mesh.num_nodes(), 16);
    }

    #[test]
    fn test_layer_heights() {
        let (nodes, faces, normals) = flat_square_surface();
        let first_h = 0.1;
        let ratio = 1.5;
        let nl = 4;
        let builder = PrismLayerBuilder::new(first_h, ratio, nl, nodes, faces, normals);
        let mesh = builder.build().unwrap();

        // Total height: 0.1 + 0.15 + 0.225 + 0.3375 = 0.8125
        let total_h = first_h * (1.0 - ratio.powi(nl as i32)) / (1.0 - ratio);

        // Check that outer nodes are at the expected height in z
        let n_wall = 4;
        for wn in 0..n_wall {
            let outer_node = &mesh.nodes[nl * n_wall + wn];
            assert!(
                (outer_node.position[2] - total_h).abs() < 1e-12,
                "node {} z={} expected {}",
                outer_node.id,
                outer_node.position[2],
                total_h
            );
        }
    }

    #[test]
    fn test_all_cells_positive_volume() {
        let (nodes, faces, normals) = flat_square_surface();
        let builder = PrismLayerBuilder::new(0.05, 1.3, 5, nodes, faces, normals);
        let mesh = builder.build().unwrap();
        for cell in &mesh.cells {
            assert!(cell.volume > 0.0, "cell {} vol={}", cell.id, cell.volume);
        }
    }

    #[test]
    fn test_boundary_patches_exist() {
        let (nodes, faces, normals) = flat_square_surface();
        let builder = PrismLayerBuilder::new(0.1, 1.0, 2, nodes, faces, normals);
        let mesh = builder.build().unwrap();

        assert!(mesh.boundary_patch("wall").is_some());
        assert!(mesh.boundary_patch("outer").is_some());
    }

    #[test]
    fn test_single_layer() {
        let (nodes, faces, normals) = flat_square_surface();
        let builder = PrismLayerBuilder::new(0.5, 1.0, 1, nodes, faces, normals);
        let mesh = builder.build().unwrap();
        assert_eq!(mesh.num_cells(), 2);
    }

    #[test]
    fn test_invalid_params() {
        let (nodes, faces, normals) = flat_square_surface();
        assert!(PrismLayerBuilder::new(0.0, 1.0, 1, nodes.clone(), faces.clone(), normals.clone())
            .build()
            .is_err());
        assert!(PrismLayerBuilder::new(0.1, 1.0, 0, nodes.clone(), faces.clone(), normals.clone())
            .build()
            .is_err());
        assert!(PrismLayerBuilder::new(0.1, 1.0, 1, vec![], faces.clone(), normals.clone())
            .build()
            .is_err());
    }

    #[test]
    fn test_growth_ratio() {
        let (nodes, faces, normals) = flat_square_surface();
        let first_h = 0.1;
        let ratio = 2.0;
        let builder = PrismLayerBuilder::new(first_h, ratio, 3, nodes, faces, normals);
        let mesh = builder.build().unwrap();

        // Check that cells in successive layers have increasing volume
        // Layer 0, face 0: cell 0; Layer 1, face 0: cell 2; Layer 2, face 0: cell 4
        let n_faces = 2;
        let v0 = mesh.cells[0 * n_faces].volume;
        let v1 = mesh.cells[1 * n_faces].volume;
        let v2 = mesh.cells[2 * n_faces].volume;
        assert!(v1 > v0, "layer 1 vol {} should be > layer 0 vol {}", v1, v0);
        assert!(v2 > v1, "layer 2 vol {} should be > layer 1 vol {}", v2, v1);
    }
}
