//! O-grid mesh generator for cylindrical geometry.
//!
//! Creates an annular mesh between an inner circle of radius `r_inner` and an
//! outer circle of radius `r_outer`, with `n_radial` cells in the radial
//! direction and `n_circ` cells in the circumferential direction.

use std::f64::consts::PI;

use gfd_core::mesh::cell::Cell;
use gfd_core::mesh::face::Face;
use gfd_core::mesh::node::Node;
use gfd_core::mesh::unstructured::{BoundaryPatch, UnstructuredMesh};

use crate::{MeshError, MeshGenerator, Result};

/// Builder for an O-grid (annular / cylindrical) mesh.
///
/// The mesh is a single layer in z (quasi-2D) with hex cells arranged in
/// concentric rings between the inner and outer radii.
pub struct OGridBuilder {
    /// Inner circle radius.
    r_inner: f64,
    /// Outer circle radius.
    r_outer: f64,
    /// Number of cells in the radial direction.
    n_radial: usize,
    /// Number of cells in the circumferential direction.
    n_circ: usize,
    /// Depth in z-direction (single layer).
    depth: f64,
}

impl OGridBuilder {
    /// Create a new O-grid builder.
    ///
    /// # Arguments
    /// * `r_inner` - Inner radius (> 0)
    /// * `r_outer` - Outer radius (> r_inner)
    /// * `n_radial` - Number of cells in the radial direction
    /// * `n_circ` - Number of cells in the circumferential direction (must be >= 3)
    /// * `depth` - Thickness in z for the single hex layer
    pub fn new(
        r_inner: f64,
        r_outer: f64,
        n_radial: usize,
        n_circ: usize,
        depth: f64,
    ) -> Self {
        Self {
            r_inner,
            r_outer,
            n_radial,
            n_circ,
            depth,
        }
    }
}

impl MeshGenerator for OGridBuilder {
    fn build(&self) -> Result<UnstructuredMesh> {
        if self.r_inner <= 0.0 || self.r_outer <= self.r_inner {
            return Err(MeshError::InvalidParameters(
                "Need 0 < r_inner < r_outer".to_string(),
            ));
        }
        if self.n_radial == 0 || self.n_circ < 3 {
            return Err(MeshError::InvalidParameters(
                "n_radial > 0 and n_circ >= 3 required".to_string(),
            ));
        }
        if self.depth <= 0.0 {
            return Err(MeshError::InvalidParameters(
                "depth must be > 0".to_string(),
            ));
        }

        let nr = self.n_radial;
        let nc = self.n_circ;
        let dz = self.depth;

        // Radial positions: uniform spacing between r_inner and r_outer
        let dr = (self.r_outer - self.r_inner) / nr as f64;
        let radii: Vec<f64> = (0..=nr).map(|i| self.r_inner + i as f64 * dr).collect();

        // Circumferential positions: uniform angles [0, 2*pi)
        let dtheta = 2.0 * PI / nc as f64;
        let angles: Vec<f64> = (0..nc).map(|j| j as f64 * dtheta).collect();

        // Node indexing: (radial_ring, circ_index, z_layer) -> flat index
        // Rings: 0..=nr, circ: 0..nc, z: 0 or 1
        let node_idx = |ring: usize, circ: usize, z_layer: usize| -> usize {
            z_layer * (nr + 1) * nc + ring * nc + circ
        };

        // 1. Nodes
        let num_nodes = 2 * (nr + 1) * nc;
        let mut nodes = Vec::with_capacity(num_nodes);
        for z_layer in 0..2_usize {
            let z = z_layer as f64 * dz;
            for ring in 0..=nr {
                let r = radii[ring];
                for c in 0..nc {
                    let theta = angles[c];
                    let id = node_idx(ring, c, z_layer);
                    nodes.push(Node::new(id, [r * theta.cos(), r * theta.sin(), z]));
                }
            }
        }

        // Cell indexing: (radial, circ) -> flat
        let cell_flat = |ring: usize, circ: usize| -> usize { ring * nc + circ };

        // 2. Cells
        let num_cells = nr * nc;
        let mut cells = Vec::with_capacity(num_cells);
        for ring in 0..nr {
            for c in 0..nc {
                let c_next = (c + 1) % nc;
                let cell_id = cell_flat(ring, c);

                let n0 = node_idx(ring, c, 0);
                let n1 = node_idx(ring, c_next, 0);
                let n2 = node_idx(ring + 1, c_next, 0);
                let n3 = node_idx(ring + 1, c, 0);
                let n4 = node_idx(ring, c, 1);
                let n5 = node_idx(ring, c_next, 1);
                let n6 = node_idx(ring + 1, c_next, 1);
                let n7 = node_idx(ring + 1, c, 1);

                // Cell volume: annular sector
                // V = 0.5 * (r_outer^2 - r_inner^2) * dtheta * dz
                let r_in = radii[ring];
                let r_out = radii[ring + 1];
                let vol = 0.5 * (r_out * r_out - r_in * r_in) * dtheta * dz;

                // Cell center: mid-radius, mid-angle
                let r_mid = (r_in + r_out) / 2.0;
                let theta_mid = angles[c] + dtheta / 2.0;
                let center = [
                    r_mid * theta_mid.cos(),
                    r_mid * theta_mid.sin(),
                    dz / 2.0,
                ];

                cells.push(Cell::new(
                    cell_id,
                    vec![n0, n1, n2, n3, n4, n5, n6, n7],
                    Vec::new(),
                    vol,
                    center,
                ));
            }
        }

        // 3. Faces
        let mut faces: Vec<Face> = Vec::new();
        let mut inner_wall_faces = Vec::new();
        let mut outer_wall_faces = Vec::new();
        let mut zmin_faces = Vec::new();
        let mut zmax_faces = Vec::new();

        // Radial faces (between rings, plus inner/outer boundaries)
        for ring in 0..=nr {
            for c in 0..nc {
                let c_next = (c + 1) % nc;
                let face_id = faces.len();

                let fn0 = node_idx(ring, c, 0);
                let fn1 = node_idx(ring, c_next, 0);
                let fn2 = node_idx(ring, c_next, 1);
                let fn3 = node_idx(ring, c, 1);
                let face_nodes = vec![fn0, fn1, fn2, fn3];

                // Face is on a cylindrical surface at radius r
                let r = radii[ring];
                // Arc length approximation for area
                let arc_len = r * dtheta;
                let area = arc_len * dz;

                // Normal points radially outward
                let theta_mid = angles[c] + dtheta / 2.0;
                let normal_dir = if ring == 0 { -1.0 } else { 1.0 };
                let normal = [
                    normal_dir * theta_mid.cos(),
                    normal_dir * theta_mid.sin(),
                    0.0,
                ];
                let center = [
                    r * theta_mid.cos(),
                    r * theta_mid.sin(),
                    dz / 2.0,
                ];

                let (owner, neighbor);
                if ring == 0 {
                    // Inner boundary
                    owner = cell_flat(0, c);
                    neighbor = None;
                    inner_wall_faces.push(face_id);
                } else if ring == nr {
                    // Outer boundary
                    owner = cell_flat(nr - 1, c);
                    neighbor = None;
                    outer_wall_faces.push(face_id);
                } else {
                    // Internal face between ring-1 and ring
                    owner = cell_flat(ring - 1, c);
                    neighbor = Some(cell_flat(ring, c));
                }

                faces.push(Face::new(face_id, face_nodes, owner, neighbor, area, normal, center));
            }
        }

        // Circumferential faces (between adjacent angular sectors)
        // In a full O-grid these are all internal (periodic connectivity)
        for ring in 0..nr {
            for c in 0..nc {
                let c_next = (c + 1) % nc;
                let face_id = faces.len();

                let fn0 = node_idx(ring, c_next, 0);
                let fn1 = node_idx(ring + 1, c_next, 0);
                let fn2 = node_idx(ring + 1, c_next, 1);
                let fn3 = node_idx(ring, c_next, 1);
                let face_nodes = vec![fn0, fn1, fn2, fn3];

                let r_in = radii[ring];
                let r_out = radii[ring + 1];
                let area = (r_out - r_in) * dz;

                // Normal in circumferential direction at this angle
                let theta = angles[c_next];
                // tangential direction at angle theta is (-sin, cos)
                let normal = [-theta.sin(), theta.cos(), 0.0];
                let r_mid = (r_in + r_out) / 2.0;
                let center = [r_mid * theta.cos(), r_mid * theta.sin(), dz / 2.0];

                // Always internal: connects cell (ring, c) to cell (ring, c_next)
                let owner = cell_flat(ring, c);
                let neighbor = Some(cell_flat(ring, c_next));

                faces.push(Face::new(face_id, face_nodes, owner, neighbor, area, normal, center));
            }
        }

        // Z-direction faces (top and bottom -- all boundary for single layer)
        for ring in 0..nr {
            for c in 0..nc {
                let c_next = (c + 1) % nc;

                // Bottom face (z=0)
                {
                    let face_id = faces.len();
                    let fn0 = node_idx(ring, c, 0);
                    let fn1 = node_idx(ring, c_next, 0);
                    let fn2 = node_idx(ring + 1, c_next, 0);
                    let fn3 = node_idx(ring + 1, c, 0);
                    let face_nodes = vec![fn0, fn1, fn2, fn3];

                    let r_in = radii[ring];
                    let r_out = radii[ring + 1];
                    let area = 0.5 * (r_out * r_out - r_in * r_in) * dtheta;

                    let r_mid = (r_in + r_out) / 2.0;
                    let theta_mid = angles[c] + dtheta / 2.0;
                    let center = [r_mid * theta_mid.cos(), r_mid * theta_mid.sin(), 0.0];

                    let owner = cell_flat(ring, c);
                    zmin_faces.push(face_id);

                    faces.push(Face::new(
                        face_id,
                        face_nodes,
                        owner,
                        None,
                        area,
                        [0.0, 0.0, -1.0],
                        center,
                    ));
                }

                // Top face (z=dz)
                {
                    let face_id = faces.len();
                    let fn0 = node_idx(ring, c, 1);
                    let fn1 = node_idx(ring, c_next, 1);
                    let fn2 = node_idx(ring + 1, c_next, 1);
                    let fn3 = node_idx(ring + 1, c, 1);
                    let face_nodes = vec![fn0, fn1, fn2, fn3];

                    let r_in = radii[ring];
                    let r_out = radii[ring + 1];
                    let area = 0.5 * (r_out * r_out - r_in * r_in) * dtheta;

                    let r_mid = (r_in + r_out) / 2.0;
                    let theta_mid = angles[c] + dtheta / 2.0;
                    let center = [r_mid * theta_mid.cos(), r_mid * theta_mid.sin(), dz];

                    let owner = cell_flat(ring, c);
                    zmax_faces.push(face_id);

                    faces.push(Face::new(
                        face_id,
                        face_nodes,
                        owner,
                        None,
                        area,
                        [0.0, 0.0, 1.0],
                        center,
                    ));
                }
            }
        }

        // 4. Boundary patches
        let mut boundary_patches = Vec::new();
        if !inner_wall_faces.is_empty() {
            boundary_patches.push(BoundaryPatch::new("inner_wall", inner_wall_faces));
        }
        if !outer_wall_faces.is_empty() {
            boundary_patches.push(BoundaryPatch::new("outer_wall", outer_wall_faces));
        }
        if !zmin_faces.is_empty() {
            boundary_patches.push(BoundaryPatch::new("zmin", zmin_faces));
        }
        if !zmax_faces.is_empty() {
            boundary_patches.push(BoundaryPatch::new("zmax", zmax_faces));
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::f64::consts::PI;

    #[test]
    fn test_basic_o_grid() {
        let builder = OGridBuilder::new(1.0, 2.0, 4, 8, 0.5);
        let mesh = builder.build().unwrap();
        assert_eq!(mesh.num_cells(), 4 * 8);
        // Nodes: 2 z-layers * (nr+1) rings * nc points = 2 * 5 * 8 = 80
        assert_eq!(mesh.num_nodes(), 2 * 5 * 8);
    }

    #[test]
    fn test_boundary_patches() {
        let nr = 3;
        let nc = 6;
        let builder = OGridBuilder::new(0.5, 1.5, nr, nc, 1.0);
        let mesh = builder.build().unwrap();

        let inner = mesh.boundary_patch("inner_wall").unwrap();
        let outer = mesh.boundary_patch("outer_wall").unwrap();
        let zmin = mesh.boundary_patch("zmin").unwrap();
        let zmax = mesh.boundary_patch("zmax").unwrap();

        assert_eq!(inner.num_faces(), nc);
        assert_eq!(outer.num_faces(), nc);
        assert_eq!(zmin.num_faces(), nr * nc);
        assert_eq!(zmax.num_faces(), nr * nc);
    }

    #[test]
    fn test_total_volume() {
        let r_in = 1.0;
        let r_out = 3.0;
        let dz = 2.0;
        let builder = OGridBuilder::new(r_in, r_out, 5, 12, dz);
        let mesh = builder.build().unwrap();

        let total_vol: f64 = mesh.cells.iter().map(|c| c.volume).sum();
        let expected = PI * (r_out * r_out - r_in * r_in) * dz;
        assert!(
            (total_vol - expected).abs() < 1e-10,
            "total vol {} vs expected {}",
            total_vol,
            expected
        );
    }

    #[test]
    fn test_all_faces_positive_area() {
        let builder = OGridBuilder::new(0.5, 2.0, 3, 8, 1.0);
        let mesh = builder.build().unwrap();
        for face in &mesh.faces {
            assert!(face.area > 0.0, "face {} has area {}", face.id, face.area);
        }
    }

    #[test]
    fn test_invalid_radii() {
        assert!(OGridBuilder::new(0.0, 1.0, 3, 6, 1.0).build().is_err());
        assert!(OGridBuilder::new(2.0, 1.0, 3, 6, 1.0).build().is_err());
        assert!(OGridBuilder::new(1.0, 1.0, 3, 6, 1.0).build().is_err());
    }

    #[test]
    fn test_invalid_counts() {
        assert!(OGridBuilder::new(1.0, 2.0, 0, 6, 1.0).build().is_err());
        assert!(OGridBuilder::new(1.0, 2.0, 3, 2, 1.0).build().is_err());
    }

    #[test]
    fn test_cells_have_faces() {
        let builder = OGridBuilder::new(1.0, 2.0, 2, 6, 1.0);
        let mesh = builder.build().unwrap();
        for cell in &mesh.cells {
            // Each cell should have faces assigned
            assert!(
                !cell.faces.is_empty(),
                "cell {} has no faces",
                cell.id
            );
        }
    }
}
