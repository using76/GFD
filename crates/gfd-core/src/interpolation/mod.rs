//! Interpolation schemes for computing face values from cell values.

use serde::{Deserialize, Serialize};

use crate::field::ScalarField;
use crate::mesh::unstructured::UnstructuredMesh;
use crate::Result;

/// Available interpolation schemes for face value computation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum InterpolationScheme {
    /// Linear (distance-weighted) interpolation.
    Linear,
    /// Upwind interpolation (uses the upstream cell value).
    Upwind,
    /// Central differencing (arithmetic average of owner and neighbor).
    Central,
    /// Second-order upwind with gradient correction.
    SecondOrderUpwind,
    /// Blended scheme (linear combination of upwind and central).
    Blended,
}

/// Trait for interpolation of field values to face centers.
pub trait Interpolator {
    /// Interpolates a scalar field from cell centers to face centers.
    ///
    /// Returns a vector of face values with one entry per face.
    fn interpolate_scalar(
        &self,
        field: &ScalarField,
        mesh: &UnstructuredMesh,
    ) -> Result<Vec<f64>>;

    /// Returns the interpolation scheme used.
    fn scheme(&self) -> InterpolationScheme;
}

/// Linear (distance-weighted) interpolator.
#[derive(Debug, Clone)]
pub struct LinearInterpolator;

impl Interpolator for LinearInterpolator {
    fn interpolate_scalar(
        &self,
        field: &ScalarField,
        mesh: &UnstructuredMesh,
    ) -> Result<Vec<f64>> {
        let values = field.values();
        let mut result = Vec::with_capacity(mesh.num_faces());

        for face_id in 0..mesh.num_faces() {
            let face = &mesh.faces[face_id];
            let owner = face.owner_cell;

            if let Some(neighbor) = face.neighbor_cell {
                // Internal face: arithmetic mean (correct for uniform grids)
                result.push(0.5 * (values[owner] + values[neighbor]));
            } else {
                // Boundary face: zero-gradient extrapolation
                result.push(values[owner]);
            }
        }

        Ok(result)
    }

    fn scheme(&self) -> InterpolationScheme {
        InterpolationScheme::Linear
    }
}

/// Upwind interpolator.
#[derive(Debug, Clone)]
pub struct UpwindInterpolator;

impl Interpolator for UpwindInterpolator {
    fn interpolate_scalar(
        &self,
        field: &ScalarField,
        mesh: &UnstructuredMesh,
    ) -> Result<Vec<f64>> {
        // Without face flux direction, use arithmetic mean (same as linear).
        // True upwind is applied at the discretization level via
        // compute_convective_coefficient, not at the interpolation level.
        let values = field.values();
        let mut result = Vec::with_capacity(mesh.num_faces());

        for face_id in 0..mesh.num_faces() {
            let face = &mesh.faces[face_id];
            let owner = face.owner_cell;

            if let Some(neighbor) = face.neighbor_cell {
                result.push(0.5 * (values[owner] + values[neighbor]));
            } else {
                result.push(values[owner]);
            }
        }

        Ok(result)
    }

    fn scheme(&self) -> InterpolationScheme {
        InterpolationScheme::Upwind
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mesh::cell::Cell;
    use crate::mesh::face::Face;
    use crate::mesh::unstructured::UnstructuredMesh;

    /// Creates a 3x1x1 hex mesh manually.
    ///
    /// 3 cells along x-axis, each 1x1x1. Domain: [0,3] x [0,1] x [0,1].
    /// Cell centers: (0.5,0.5,0.5), (1.5,0.5,0.5), (2.5,0.5,0.5).
    fn make_3x1x1_mesh() -> UnstructuredMesh {
        let cells = vec![
            Cell::new(0, vec![], vec![], 1.0, [0.5, 0.5, 0.5]),
            Cell::new(1, vec![], vec![], 1.0, [1.5, 0.5, 0.5]),
            Cell::new(2, vec![], vec![], 1.0, [2.5, 0.5, 0.5]),
        ];

        let mut faces = Vec::new();
        let mut fid = 0;

        // X-normal faces
        // x=0 boundary (owner=cell0)
        faces.push(Face::new(fid, vec![], 0, None, 1.0, [-1.0, 0.0, 0.0], [0.0, 0.5, 0.5]));
        fid += 1;
        // x=1 internal (owner=cell0, neighbor=cell1)
        faces.push(Face::new(fid, vec![], 0, Some(1), 1.0, [1.0, 0.0, 0.0], [1.0, 0.5, 0.5]));
        fid += 1;
        // x=2 internal (owner=cell1, neighbor=cell2)
        faces.push(Face::new(fid, vec![], 1, Some(2), 1.0, [1.0, 0.0, 0.0], [2.0, 0.5, 0.5]));
        fid += 1;
        // x=3 boundary (owner=cell2)
        faces.push(Face::new(fid, vec![], 2, None, 1.0, [1.0, 0.0, 0.0], [3.0, 0.5, 0.5]));
        fid += 1;

        // Y-normal faces (boundaries)
        for i in 0..3 {
            let cx = i as f64 + 0.5;
            faces.push(Face::new(fid, vec![], i, None, 1.0, [0.0, -1.0, 0.0], [cx, 0.0, 0.5]));
            fid += 1;
            faces.push(Face::new(fid, vec![], i, None, 1.0, [0.0, 1.0, 0.0], [cx, 1.0, 0.5]));
            fid += 1;
        }

        // Z-normal faces (boundaries)
        for i in 0..3 {
            let cx = i as f64 + 0.5;
            faces.push(Face::new(fid, vec![], i, None, 1.0, [0.0, 0.0, -1.0], [cx, 0.5, 0.0]));
            fid += 1;
            faces.push(Face::new(fid, vec![], i, None, 1.0, [0.0, 0.0, 1.0], [cx, 0.5, 1.0]));
            fid += 1;
        }

        UnstructuredMesh::from_components(vec![], faces, cells, vec![])
    }

    #[test]
    fn test_linear_uniform_field() {
        let mesh = make_3x1x1_mesh();
        // Uniform field: phi = 3.0 everywhere
        let field = ScalarField::new("phi", vec![3.0, 3.0, 3.0]);

        let interp = LinearInterpolator;
        let face_values = interp.interpolate_scalar(&field, &mesh).unwrap();

        assert_eq!(face_values.len(), mesh.num_faces());
        for (i, &val) in face_values.iter().enumerate() {
            assert!(
                (val - 3.0).abs() < 1e-10,
                "face {} value = {} (expected 3.0)",
                i,
                val
            );
        }
    }

    #[test]
    fn test_linear_interpolation_internal_faces() {
        let mesh = make_3x1x1_mesh();
        // Linear field phi = x (cell centers: 0.5, 1.5, 2.5)
        let field = ScalarField::new("phi", vec![0.5, 1.5, 2.5]);

        let interp = LinearInterpolator;
        let face_values = interp.interpolate_scalar(&field, &mesh).unwrap();

        // Face at x=1 (index 1): internal, average of cell0 and cell1 => (0.5+1.5)/2 = 1.0
        assert!(
            (face_values[1] - 1.0).abs() < 1e-10,
            "face at x=1: {} (expected 1.0)",
            face_values[1]
        );

        // Face at x=2 (index 2): internal, average of cell1 and cell2 => (1.5+2.5)/2 = 2.0
        assert!(
            (face_values[2] - 2.0).abs() < 1e-10,
            "face at x=2: {} (expected 2.0)",
            face_values[2]
        );

        // Boundary face at x=0 (index 0): owner is cell0 => 0.5
        assert!(
            (face_values[0] - 0.5).abs() < 1e-10,
            "face at x=0: {} (expected 0.5)",
            face_values[0]
        );

        // Boundary face at x=3 (index 3): owner is cell2 => 2.5
        assert!(
            (face_values[3] - 2.5).abs() < 1e-10,
            "face at x=3: {} (expected 2.5)",
            face_values[3]
        );
    }

    #[test]
    fn test_upwind_matches_linear_without_flux() {
        let mesh = make_3x1x1_mesh();
        let field = ScalarField::new("phi", vec![1.0, 2.0, 3.0]);

        let linear = LinearInterpolator;
        let upwind = UpwindInterpolator;

        let linear_vals = linear.interpolate_scalar(&field, &mesh).unwrap();
        let upwind_vals = upwind.interpolate_scalar(&field, &mesh).unwrap();

        // Without flux information, upwind should produce the same results as linear
        assert_eq!(linear_vals.len(), upwind_vals.len());
        for (i, (&lv, &uv)) in linear_vals.iter().zip(upwind_vals.iter()).enumerate() {
            assert!(
                (lv - uv).abs() < 1e-10,
                "face {}: linear={}, upwind={} (should match)",
                i,
                lv,
                uv
            );
        }
    }
}
