//! Cartesian mesh builder with support for grading (cell stretching) on each face.

use std::collections::HashMap;

use gfd_core::mesh::cell::Cell;
use gfd_core::mesh::face::Face;
use gfd_core::mesh::node::Node;
use gfd_core::mesh::unstructured::{BoundaryPatch, UnstructuredMesh};

use crate::{MeshError, MeshGenerator, Result};

use super::grading::{
    bigeometric_distribution, geometric_distribution, tanh_distribution, uniform_distribution,
};

/// Specification for how cells are graded (stretched) near a boundary.
#[derive(Debug, Clone)]
pub enum GradingSpec {
    /// No grading -- uniform spacing.
    Uniform,
    /// Geometric expansion: first cell height, expansion ratio, number of graded layers.
    Geometric {
        first_height: f64,
        ratio: f64,
        layers: usize,
    },
    /// Hyperbolic-tangent clustering at both ends. `delta` controls strength.
    Tanh { delta: f64 },
    /// Bi-geometric: prescribed first and last cell heights with smooth transition.
    BiGeometric {
        first: f64,
        last: f64,
        ratio: f64,
    },
}

/// Builder for Cartesian meshes with optional grading on each of the 6 faces.
///
/// # Example
/// ```
/// use gfd_mesh::structured::cartesian::{CartesianMeshBuilder, GradingSpec};
/// use gfd_mesh::MeshGenerator;
///
/// let mesh = CartesianMeshBuilder::new(10, 10, 1, 1.0, 1.0, 0.1)
///     .grading("ymin", GradingSpec::Geometric { first_height: 0.01, ratio: 1.2, layers: 10 })
///     .build()
///     .unwrap();
/// assert_eq!(mesh.num_cells(), 100);
/// ```
pub struct CartesianMeshBuilder {
    nx: usize,
    ny: usize,
    nz: usize,
    lx: f64,
    ly: f64,
    lz: f64,
    gradings: HashMap<String, GradingSpec>,
}

impl CartesianMeshBuilder {
    /// Create a new builder for an `nx * ny * nz` mesh over domain `[0,lx] x [0,ly] x [0,lz]`.
    pub fn new(nx: usize, ny: usize, nz: usize, lx: f64, ly: f64, lz: f64) -> Self {
        Self {
            nx,
            ny,
            nz,
            lx,
            ly,
            lz,
            gradings: HashMap::new(),
        }
    }

    /// Add grading specification for a boundary face.
    ///
    /// Valid face names: `"xmin"`, `"xmax"`, `"ymin"`, `"ymax"`, `"zmin"`, `"zmax"`.
    pub fn grading(mut self, face_name: &str, spec: GradingSpec) -> Self {
        self.gradings.insert(face_name.to_string(), spec);
        self
    }

    /// Compute 1D node positions along an axis, merging gradings from the
    /// "min" and "max" faces of that axis.
    fn compute_axis_positions(
        &self,
        n: usize,
        length: f64,
        min_face: &str,
        max_face: &str,
    ) -> Result<Vec<f64>> {
        let min_spec = self.gradings.get(min_face);
        let max_spec = self.gradings.get(max_face);

        match (min_spec, max_spec) {
            (None, None) | (Some(GradingSpec::Uniform), Some(GradingSpec::Uniform)) => {
                Ok(uniform_distribution(n, length))
            }
            (Some(GradingSpec::Uniform), None) | (None, Some(GradingSpec::Uniform)) => {
                Ok(uniform_distribution(n, length))
            }
            // Tanh clusters both ends, so it applies regardless of which face specified it.
            (Some(GradingSpec::Tanh { delta }), _) | (_, Some(GradingSpec::Tanh { delta })) => {
                Ok(tanh_distribution(n, length, *delta))
            }
            // Geometric on min face: cluster near 0
            (
                Some(GradingSpec::Geometric {
                    first_height,
                    ratio,
                    layers,
                }),
                None,
            )
            | (
                Some(GradingSpec::Geometric {
                    first_height,
                    ratio,
                    layers,
                }),
                Some(GradingSpec::Uniform),
            ) => {
                let graded_n = (*layers).min(n);
                let remaining = n - graded_n;

                if remaining == 0 {
                    Ok(geometric_distribution(n, length, *first_height, *ratio))
                } else {
                    // Compute how much length the graded part uses
                    let graded_raw = geometric_distribution(graded_n, 1.0, *first_height, *ratio);
                    let graded_length_raw = graded_raw[graded_n];
                    // Scale so graded part uses proportional length
                    let graded_length = (graded_length_raw / (graded_length_raw + remaining as f64 * (*first_height * ratio.powi(graded_n as i32 - 1)))) * length;
                    let graded_length = graded_length.min(length * 0.9); // safety cap

                    let mut positions = geometric_distribution(graded_n, graded_length, *first_height, *ratio);
                    let uniform_part = uniform_distribution(remaining, length - graded_length);
                    for i in 1..=remaining {
                        positions.push(graded_length + uniform_part[i]);
                    }
                    *positions.last_mut().unwrap() = length;
                    Ok(positions)
                }
            }
            // Geometric on max face: cluster near length (mirror)
            (None, Some(GradingSpec::Geometric { first_height, ratio, layers }))
            | (Some(GradingSpec::Uniform), Some(GradingSpec::Geometric { first_height, ratio, layers })) => {
                let graded_n = (*layers).min(n);
                let remaining = n - graded_n;

                if remaining == 0 {
                    let mut pos = geometric_distribution(n, length, *first_height, *ratio);
                    // Mirror: positions measured from the max end
                    pos.reverse();
                    for p in &mut pos {
                        *p = length - *p;
                    }
                    Ok(pos)
                } else {
                    let graded_raw = geometric_distribution(graded_n, 1.0, *first_height, *ratio);
                    let graded_length_raw = graded_raw[graded_n];
                    let graded_length = (graded_length_raw / (graded_length_raw + remaining as f64 * (*first_height * ratio.powi(graded_n as i32 - 1)))) * length;
                    let graded_length = graded_length.min(length * 0.9);

                    let mut graded = geometric_distribution(graded_n, graded_length, *first_height, *ratio);
                    // Mirror the graded part
                    graded.reverse();
                    for p in &mut graded {
                        *p = graded_length - *p;
                    }

                    let uniform_part = uniform_distribution(remaining, length - graded_length);
                    let mut positions = uniform_part;
                    for i in 1..=graded_n {
                        positions.push((length - graded_length) + graded[i]);
                    }
                    *positions.last_mut().unwrap() = length;
                    Ok(positions)
                }
            }
            // Geometric on both faces
            (
                Some(GradingSpec::Geometric {
                    first_height: fh_min,
                    ..
                }),
                Some(GradingSpec::Geometric {
                    first_height: fh_max,
                    ..
                }),
            ) => {
                // Use bigeometric with the two first heights
                Ok(bigeometric_distribution(n, length, *fh_min, *fh_max))
            }
            // BiGeometric
            (Some(GradingSpec::BiGeometric { first, last, .. }), _)
            | (_, Some(GradingSpec::BiGeometric { first, last, .. })) => {
                Ok(bigeometric_distribution(n, length, *first, *last))
            }
        }
    }
}

impl MeshGenerator for CartesianMeshBuilder {
    fn build(&self) -> Result<UnstructuredMesh> {
        if self.nx == 0 || self.ny == 0 {
            return Err(MeshError::InvalidParameters(
                "nx and ny must be > 0".to_string(),
            ));
        }
        if self.lx <= 0.0 || self.ly <= 0.0 {
            return Err(MeshError::InvalidParameters(
                "lx and ly must be > 0".to_string(),
            ));
        }

        let effective_nz = if self.nz == 0 { 1 } else { self.nz };
        let lz_eff = if self.nz == 0 { 1.0 } else { self.lz };

        // Compute 1D node positions along each axis
        let x_pos = self.compute_axis_positions(self.nx, self.lx, "xmin", "xmax")?;
        let y_pos = self.compute_axis_positions(self.ny, self.ly, "ymin", "ymax")?;
        let z_pos = if self.nz == 0 {
            vec![0.0, lz_eff]
        } else {
            self.compute_axis_positions(self.nz, self.lz, "zmin", "zmax")?
        };

        // Node indexing helper
        let node_idx = |i: usize, j: usize, k: usize| -> usize {
            k * (self.ny + 1) * (self.nx + 1) + j * (self.nx + 1) + i
        };

        // 1. Build nodes
        let num_nodes = (self.nx + 1) * (self.ny + 1) * (effective_nz + 1);
        let mut nodes = Vec::with_capacity(num_nodes);
        for k in 0..=effective_nz {
            for j in 0..=self.ny {
                for i in 0..=self.nx {
                    let id = node_idx(i, j, k);
                    nodes.push(Node::new(id, [x_pos[i], y_pos[j], z_pos[k]]));
                }
            }
        }

        // 2. Build cells
        let cell_flat =
            |i: usize, j: usize, k: usize| -> usize { k * self.nx * self.ny + j * self.nx + i };

        let num_cells = self.nx * self.ny * effective_nz;
        let mut cells = Vec::with_capacity(num_cells);
        for k in 0..effective_nz {
            for j in 0..self.ny {
                for i in 0..self.nx {
                    let cell_id = cell_flat(i, j, k);
                    let n0 = node_idx(i, j, k);
                    let n1 = node_idx(i + 1, j, k);
                    let n2 = node_idx(i + 1, j + 1, k);
                    let n3 = node_idx(i, j + 1, k);
                    let n4 = node_idx(i, j, k + 1);
                    let n5 = node_idx(i + 1, j, k + 1);
                    let n6 = node_idx(i + 1, j + 1, k + 1);
                    let n7 = node_idx(i, j + 1, k + 1);

                    let cx = (x_pos[i] + x_pos[i + 1]) / 2.0;
                    let cy = (y_pos[j] + y_pos[j + 1]) / 2.0;
                    let cz = (z_pos[k] + z_pos[k + 1]) / 2.0;
                    let vol = (x_pos[i + 1] - x_pos[i])
                        * (y_pos[j + 1] - y_pos[j])
                        * (z_pos[k + 1] - z_pos[k]);

                    cells.push(Cell::new(
                        cell_id,
                        vec![n0, n1, n2, n3, n4, n5, n6, n7],
                        Vec::new(),
                        vol,
                        [cx, cy, cz],
                    ));
                }
            }
        }

        // 3. Build faces
        let mut faces: Vec<Face> = Vec::new();
        let mut xmin_faces = Vec::new();
        let mut xmax_faces = Vec::new();
        let mut ymin_faces = Vec::new();
        let mut ymax_faces = Vec::new();
        let mut zmin_faces = Vec::new();
        let mut zmax_faces = Vec::new();

        // X-direction faces
        for k in 0..effective_nz {
            for j in 0..self.ny {
                for i in 0..=self.nx {
                    let face_id = faces.len();
                    let fn0 = node_idx(i, j, k);
                    let fn1 = node_idx(i, j + 1, k);
                    let fn2 = node_idx(i, j + 1, k + 1);
                    let fn3 = node_idx(i, j, k + 1);
                    let face_nodes = vec![fn0, fn1, fn2, fn3];

                    let area = (y_pos[j + 1] - y_pos[j]) * (z_pos[k + 1] - z_pos[k]);
                    let center = [
                        x_pos[i],
                        (y_pos[j] + y_pos[j + 1]) / 2.0,
                        (z_pos[k] + z_pos[k + 1]) / 2.0,
                    ];

                    let (owner, neighbor, normal);
                    if i == 0 {
                        owner = cell_flat(0, j, k);
                        neighbor = None;
                        normal = [-1.0, 0.0, 0.0];
                        xmin_faces.push(face_id);
                    } else if i == self.nx {
                        owner = cell_flat(self.nx - 1, j, k);
                        neighbor = None;
                        normal = [1.0, 0.0, 0.0];
                        xmax_faces.push(face_id);
                    } else {
                        owner = cell_flat(i - 1, j, k);
                        neighbor = Some(cell_flat(i, j, k));
                        normal = [1.0, 0.0, 0.0];
                    }

                    faces.push(Face::new(
                        face_id, face_nodes, owner, neighbor, area, normal, center,
                    ));
                }
            }
        }

        // Y-direction faces
        for k in 0..effective_nz {
            for j in 0..=self.ny {
                for i in 0..self.nx {
                    let face_id = faces.len();
                    let fn0 = node_idx(i, j, k);
                    let fn1 = node_idx(i + 1, j, k);
                    let fn2 = node_idx(i + 1, j, k + 1);
                    let fn3 = node_idx(i, j, k + 1);
                    let face_nodes = vec![fn0, fn1, fn2, fn3];

                    let area = (x_pos[i + 1] - x_pos[i]) * (z_pos[k + 1] - z_pos[k]);
                    let center = [
                        (x_pos[i] + x_pos[i + 1]) / 2.0,
                        y_pos[j],
                        (z_pos[k] + z_pos[k + 1]) / 2.0,
                    ];

                    let (owner, neighbor, normal);
                    if j == 0 {
                        owner = cell_flat(i, 0, k);
                        neighbor = None;
                        normal = [0.0, -1.0, 0.0];
                        ymin_faces.push(face_id);
                    } else if j == self.ny {
                        owner = cell_flat(i, self.ny - 1, k);
                        neighbor = None;
                        normal = [0.0, 1.0, 0.0];
                        ymax_faces.push(face_id);
                    } else {
                        owner = cell_flat(i, j - 1, k);
                        neighbor = Some(cell_flat(i, j, k));
                        normal = [0.0, 1.0, 0.0];
                    }

                    faces.push(Face::new(
                        face_id, face_nodes, owner, neighbor, area, normal, center,
                    ));
                }
            }
        }

        // Z-direction faces
        for k in 0..=effective_nz {
            for j in 0..self.ny {
                for i in 0..self.nx {
                    let face_id = faces.len();
                    let fn0 = node_idx(i, j, k);
                    let fn1 = node_idx(i + 1, j, k);
                    let fn2 = node_idx(i + 1, j + 1, k);
                    let fn3 = node_idx(i, j + 1, k);
                    let face_nodes = vec![fn0, fn1, fn2, fn3];

                    let area = (x_pos[i + 1] - x_pos[i]) * (y_pos[j + 1] - y_pos[j]);
                    let center = [
                        (x_pos[i] + x_pos[i + 1]) / 2.0,
                        (y_pos[j] + y_pos[j + 1]) / 2.0,
                        z_pos[k],
                    ];

                    let (owner, neighbor, normal);
                    if k == 0 {
                        owner = cell_flat(i, j, 0);
                        neighbor = None;
                        normal = [0.0, 0.0, -1.0];
                        zmin_faces.push(face_id);
                    } else if k == effective_nz {
                        owner = cell_flat(i, j, effective_nz - 1);
                        neighbor = None;
                        normal = [0.0, 0.0, 1.0];
                        zmax_faces.push(face_id);
                    } else {
                        owner = cell_flat(i, j, k - 1);
                        neighbor = Some(cell_flat(i, j, k));
                        normal = [0.0, 0.0, 1.0];
                    }

                    faces.push(Face::new(
                        face_id, face_nodes, owner, neighbor, area, normal, center,
                    ));
                }
            }
        }

        // 4. Boundary patches
        let mut boundary_patches = Vec::new();
        if !xmin_faces.is_empty() {
            boundary_patches.push(BoundaryPatch::new("xmin", xmin_faces));
        }
        if !xmax_faces.is_empty() {
            boundary_patches.push(BoundaryPatch::new("xmax", xmax_faces));
        }
        if !ymin_faces.is_empty() {
            boundary_patches.push(BoundaryPatch::new("ymin", ymin_faces));
        }
        if !ymax_faces.is_empty() {
            boundary_patches.push(BoundaryPatch::new("ymax", ymax_faces));
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

    #[test]
    fn test_uniform_cartesian() {
        let builder = CartesianMeshBuilder::new(3, 4, 2, 3.0, 4.0, 2.0);
        let mesh = builder.build().unwrap();
        assert_eq!(mesh.num_cells(), 3 * 4 * 2);
        assert_eq!(mesh.num_nodes(), 4 * 5 * 3);
        // x-faces + y-faces + z-faces
        let expected_faces = (3 + 1) * 4 * 2 + 3 * (4 + 1) * 2 + 3 * 4 * (2 + 1);
        assert_eq!(mesh.num_faces(), expected_faces);
    }

    #[test]
    fn test_2d_mode() {
        let builder = CartesianMeshBuilder::new(5, 5, 0, 1.0, 1.0, 0.0);
        let mesh = builder.build().unwrap();
        assert_eq!(mesh.num_cells(), 25); // 5*5*1 effective
    }

    #[test]
    fn test_grading_ymin_geometric() {
        let builder = CartesianMeshBuilder::new(2, 10, 1, 1.0, 1.0, 1.0).grading(
            "ymin",
            GradingSpec::Geometric {
                first_height: 0.01,
                ratio: 1.3,
                layers: 10,
            },
        );
        let mesh = builder.build().unwrap();
        assert_eq!(mesh.num_cells(), 2 * 10 * 1);

        // Check that cells near y=0 are smaller than cells near y=1
        // Cell at j=0 vs cell at j=9
        let cell_bottom = &mesh.cells[0]; // i=0, j=0, k=0
        let cell_top = &mesh.cells[2 * 9]; // i=0, j=9, k=0
        assert!(
            cell_bottom.volume < cell_top.volume,
            "bottom cell vol {} should be < top cell vol {}",
            cell_bottom.volume,
            cell_top.volume
        );
    }

    #[test]
    fn test_grading_tanh() {
        let builder = CartesianMeshBuilder::new(2, 20, 1, 1.0, 1.0, 1.0)
            .grading("ymin", GradingSpec::Tanh { delta: 2.0 });
        let mesh = builder.build().unwrap();
        assert_eq!(mesh.num_cells(), 2 * 20 * 1);
        // Cells near boundaries should be smaller than cells in the middle
        let cell_bottom = &mesh.cells[0]; // j=0
        let cell_mid = &mesh.cells[2 * 10]; // j=10
        assert!(
            cell_bottom.volume < cell_mid.volume,
            "bottom {} < mid {}",
            cell_bottom.volume,
            cell_mid.volume
        );
    }

    #[test]
    fn test_total_volume_conserved() {
        let lx = 2.0;
        let ly = 3.0;
        let lz = 1.5;
        let builder = CartesianMeshBuilder::new(5, 8, 3, lx, ly, lz).grading(
            "ymin",
            GradingSpec::Geometric {
                first_height: 0.05,
                ratio: 1.2,
                layers: 8,
            },
        );
        let mesh = builder.build().unwrap();
        let total_vol: f64 = mesh.cells.iter().map(|c| c.volume).sum();
        let expected = lx * ly * lz;
        assert!(
            (total_vol - expected).abs() < 1e-10,
            "total volume {} != expected {}",
            total_vol,
            expected
        );
    }

    #[test]
    fn test_all_faces_positive_area() {
        let builder = CartesianMeshBuilder::new(3, 3, 2, 1.0, 1.0, 1.0)
            .grading("xmin", GradingSpec::Tanh { delta: 1.5 });
        let mesh = builder.build().unwrap();
        for face in &mesh.faces {
            assert!(face.area > 0.0, "face {} has non-positive area", face.id);
        }
    }

    #[test]
    fn test_boundary_patch_counts() {
        let builder = CartesianMeshBuilder::new(4, 3, 2, 1.0, 1.0, 1.0);
        let mesh = builder.build().unwrap();

        let xmin = mesh.boundary_patch("xmin").unwrap();
        let xmax = mesh.boundary_patch("xmax").unwrap();
        let ymin = mesh.boundary_patch("ymin").unwrap();
        let ymax = mesh.boundary_patch("ymax").unwrap();
        let zmin = mesh.boundary_patch("zmin").unwrap();
        let zmax = mesh.boundary_patch("zmax").unwrap();

        assert_eq!(xmin.num_faces(), 3 * 2); // ny * nz
        assert_eq!(xmax.num_faces(), 3 * 2);
        assert_eq!(ymin.num_faces(), 4 * 2); // nx * nz
        assert_eq!(ymax.num_faces(), 4 * 2);
        assert_eq!(zmin.num_faces(), 4 * 3); // nx * ny
        assert_eq!(zmax.num_faces(), 4 * 3);
    }

    #[test]
    fn test_invalid_parameters() {
        let result = CartesianMeshBuilder::new(0, 5, 1, 1.0, 1.0, 1.0).build();
        assert!(result.is_err());

        let result = CartesianMeshBuilder::new(5, 5, 1, -1.0, 1.0, 1.0).build();
        assert!(result.is_err());
    }

    #[test]
    fn test_cell_faces_populated() {
        let builder = CartesianMeshBuilder::new(2, 2, 1, 1.0, 1.0, 1.0);
        let mesh = builder.build().unwrap();
        for cell in &mesh.cells {
            // Each hex cell in a structured grid should have 6 faces
            assert_eq!(cell.faces.len(), 6, "cell {} has {} faces", cell.id, cell.faces.len());
        }
    }
}
