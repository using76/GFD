//! Mortar method for conservative field mapping.
//!
//! Implements a weighted-area mortar mapping for non-matching fluid-solid
//! interfaces. The mapping computes overlap areas between source and target
//! face pairs and uses area-weighted interpolation for conservative field
//! transfer.

use gfd_core::ScalarField;
use crate::Result;
use super::FieldMapper;

/// A surface face descriptor used for mortar overlap computation.
#[derive(Debug, Clone)]
pub struct SurfaceFace {
    /// Center of the face.
    pub center: [f64; 3],
    /// Outward normal vector (unit).
    pub normal: [f64; 3],
    /// Face area.
    pub area: f64,
    /// Half-extents of the face bounding box (for overlap estimation).
    /// For a quadrilateral face, these approximate the half-widths along
    /// the two tangential directions.
    pub half_extents: [f64; 2],
}

/// Maps field values using the mortar element method.
///
/// Provides conservative and consistent interpolation by computing
/// overlap integrals between source and target surface elements.
/// The mapping satisfies:
///
/// T_target[i] = sum_j(w_ij * T_source[j])
///
/// where w_ij = overlap_area(i, j) / target_face_area(i), ensuring
/// that a uniform source field maps exactly to the same uniform value
/// on the target.
pub struct MortarMapper {
    /// Precomputed mortar integration weights.
    /// weights[target_idx] = [(source_idx, weight), ...]
    pub weights: Vec<Vec<(usize, f64)>>,
}

impl MortarMapper {
    /// Creates a new mortar mapper with pre-computed overlap integrals.
    pub fn new(weights: Vec<Vec<(usize, f64)>>) -> Self {
        Self { weights }
    }

    /// Builds a mortar mapper from source and target face descriptions.
    ///
    /// For each target face, finds overlapping source faces and computes
    /// area-weighted mapping coefficients. Two faces are considered to
    /// overlap if their projections onto the interface plane intersect.
    ///
    /// The overlap area is approximated by computing the intersection of
    /// axis-aligned bounding boxes projected onto the interface plane,
    /// which is efficient and sufficiently accurate for typical FSI meshes.
    pub fn from_faces(
        source_faces: &[SurfaceFace],
        target_faces: &[SurfaceFace],
    ) -> Self {
        let mut weights = Vec::with_capacity(target_faces.len());

        for target in target_faces {
            let mut face_weights = Vec::new();
            let mut total_overlap = 0.0_f64;

            for (src_idx, source) in source_faces.iter().enumerate() {
                let overlap = compute_overlap_area(source, target);
                if overlap > 1e-30 {
                    face_weights.push((src_idx, overlap));
                    total_overlap += overlap;
                }
            }

            // Normalize weights: w_i = overlap_i / total_overlap_area
            // This ensures conservation: if source is uniform T, target gets T
            if total_overlap > 1e-30 {
                for entry in face_weights.iter_mut() {
                    entry.1 /= total_overlap;
                }
            }

            weights.push(face_weights);
        }

        Self { weights }
    }

    /// Returns the number of target faces in this mapping.
    pub fn num_targets(&self) -> usize {
        self.weights.len()
    }

    /// Returns the maximum number of source faces contributing to any target.
    pub fn max_stencil_size(&self) -> usize {
        self.weights.iter().map(|w| w.len()).max().unwrap_or(0)
    }

    /// Checks that all weights sum to approximately 1.0 for each target,
    /// which ensures conservation.
    pub fn check_conservation(&self) -> f64 {
        let mut max_error = 0.0_f64;
        for w in &self.weights {
            if w.is_empty() {
                continue;
            }
            let sum: f64 = w.iter().map(|(_, wt)| *wt).sum();
            max_error = max_error.max((sum - 1.0).abs());
        }
        max_error
    }
}

/// Computes the approximate overlap area between two surface faces.
///
/// Projects both faces onto their average normal plane and computes
/// the intersection area of their bounding rectangles. This is a
/// simplified approach that works well for nearly co-planar faces
/// on FSI interfaces.
fn compute_overlap_area(source: &SurfaceFace, target: &SurfaceFace) -> f64 {
    // Check if faces are roughly co-planar (normals should be roughly aligned)
    let dot_n = source.normal[0] * target.normal[0]
        + source.normal[1] * target.normal[1]
        + source.normal[2] * target.normal[2];

    // If normals are not roughly aligned (anti-parallel is OK for interfaces),
    // these faces are unlikely to overlap
    if dot_n.abs() < 0.3 {
        return 0.0;
    }

    // Find the primary normal direction (largest component of average normal)
    let avg_normal = [
        0.5 * (source.normal[0] + target.normal[0]),
        0.5 * (source.normal[1] + target.normal[1]),
        0.5 * (source.normal[2] + target.normal[2]),
    ];

    let abs_n = [avg_normal[0].abs(), avg_normal[1].abs(), avg_normal[2].abs()];
    let primary_axis = if abs_n[0] >= abs_n[1] && abs_n[0] >= abs_n[2] {
        0 // x-normal: project onto yz plane
    } else if abs_n[1] >= abs_n[2] {
        1 // y-normal: project onto xz plane
    } else {
        2 // z-normal: project onto xy plane
    };

    // Get the two tangential axes
    let (tan_a, tan_b) = match primary_axis {
        0 => (1, 2),
        1 => (0, 2),
        _ => (0, 1),
    };

    // Check normal distance: if faces are too far apart along the normal, no overlap
    let normal_dist = (source.center[primary_axis] - target.center[primary_axis]).abs();
    let thickness_tol = 0.1 * (source.area.sqrt() + target.area.sqrt());
    if normal_dist > thickness_tol {
        return 0.0;
    }

    // Compute bounding box overlap in the tangential plane
    let src_half_a = source.half_extents[0];
    let src_half_b = source.half_extents[1];
    let tgt_half_a = target.half_extents[0];
    let tgt_half_b = target.half_extents[1];

    let overlap_a = compute_1d_overlap(
        source.center[tan_a],
        src_half_a,
        target.center[tan_a],
        tgt_half_a,
    );
    let overlap_b = compute_1d_overlap(
        source.center[tan_b],
        src_half_b,
        target.center[tan_b],
        tgt_half_b,
    );

    overlap_a * overlap_b
}

/// Computes the overlap length of two 1D intervals.
///
/// Interval A: [center_a - half_a, center_a + half_a]
/// Interval B: [center_b - half_b, center_b + half_b]
fn compute_1d_overlap(center_a: f64, half_a: f64, center_b: f64, half_b: f64) -> f64 {
    let lo_a = center_a - half_a;
    let hi_a = center_a + half_a;
    let lo_b = center_b - half_b;
    let hi_b = center_b + half_b;

    let overlap = f64::min(hi_a, hi_b) - f64::max(lo_a, lo_b);
    overlap.max(0.0)
}

impl FieldMapper for MortarMapper {
    fn map_field(&self, from: &ScalarField, to: &mut ScalarField) -> Result<()> {
        let from_values = from.values();
        let to_values = to.values_mut();

        for (target_idx, weight_pairs) in self.weights.iter().enumerate() {
            if target_idx >= to_values.len() {
                break;
            }
            let mut val = 0.0_f64;
            for &(src_idx, weight) in weight_pairs {
                if src_idx < from_values.len() {
                    val += weight * from_values[src_idx];
                }
            }
            to_values[target_idx] = val;
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gfd_core::ScalarField;

    #[test]
    fn test_precomputed_weights_mapping() {
        // Simple mapping: 3 target faces, 4 source faces
        let weights = vec![
            vec![(0, 0.5), (1, 0.5)],   // target 0 from sources 0 and 1
            vec![(1, 0.3), (2, 0.7)],   // target 1 from sources 1 and 2
            vec![(3, 1.0)],              // target 2 from source 3 only
        ];

        let mapper = MortarMapper::new(weights);
        let source = ScalarField::new("T_src", vec![100.0, 200.0, 300.0, 400.0]);
        let mut target = ScalarField::new("T_tgt", vec![0.0; 3]);

        mapper.map_field(&source, &mut target).unwrap();

        assert!((target.values()[0] - 150.0).abs() < 1e-10); // 0.5*100 + 0.5*200
        assert!((target.values()[1] - 270.0).abs() < 1e-10); // 0.3*200 + 0.7*300
        assert!((target.values()[2] - 400.0).abs() < 1e-10); // 1.0*400
    }

    #[test]
    fn test_from_faces_matching_mesh() {
        // Two identical face grids: mapping should be close to identity
        let faces: Vec<SurfaceFace> = (0..4)
            .map(|i| {
                let x = (i % 2) as f64 * 0.5 + 0.25;
                let y = (i / 2) as f64 * 0.5 + 0.25;
                SurfaceFace {
                    center: [x, y, 0.0],
                    normal: [0.0, 0.0, 1.0],
                    area: 0.25,
                    half_extents: [0.25, 0.25],
                }
            })
            .collect();

        let mapper = MortarMapper::from_faces(&faces, &faces);
        assert_eq!(mapper.num_targets(), 4);

        // Each target should map mainly from its corresponding source
        let source = ScalarField::new("T", vec![1.0, 2.0, 3.0, 4.0]);
        let mut target = ScalarField::new("T", vec![0.0; 4]);
        mapper.map_field(&source, &mut target).unwrap();

        // With matching meshes, the result should be close to the source
        for i in 0..4 {
            assert!(
                (target.values()[i] - source.values()[i]).abs() < 0.5,
                "Matching mesh mapping error too large at face {}: got {}, expected {}",
                i,
                target.values()[i],
                source.values()[i],
            );
        }
    }

    #[test]
    fn test_from_faces_uniform_field_conservation() {
        // A uniform field should map exactly regardless of mesh mismatch
        let source_faces: Vec<SurfaceFace> = (0..3)
            .map(|i| SurfaceFace {
                center: [i as f64 * 0.5 + 0.25, 0.5, 0.0],
                normal: [0.0, 0.0, 1.0],
                area: 0.5,
                half_extents: [0.25, 0.5],
            })
            .collect();

        let target_faces: Vec<SurfaceFace> = (0..4)
            .map(|i| SurfaceFace {
                center: [i as f64 * 0.375 + 0.1875, 0.5, 0.0],
                normal: [0.0, 0.0, 1.0],
                area: 0.375,
                half_extents: [0.1875, 0.5],
            })
            .collect();

        let mapper = MortarMapper::from_faces(&source_faces, &target_faces);

        // Uniform field of 42.0 should map to 42.0 everywhere
        let source = ScalarField::new("T", vec![42.0; 3]);
        let mut target = ScalarField::new("T", vec![0.0; 4]);
        mapper.map_field(&source, &mut target).unwrap();

        for i in 0..4 {
            assert!(
                (target.values()[i] - 42.0).abs() < 1e-10,
                "Uniform field should be preserved: target[{}] = {}",
                i,
                target.values()[i],
            );
        }
    }

    #[test]
    fn test_conservation_check() {
        let weights = vec![
            vec![(0, 0.4), (1, 0.6)],
            vec![(1, 1.0)],
        ];
        let mapper = MortarMapper::new(weights);
        assert!(mapper.check_conservation() < 1e-10);
    }

    #[test]
    fn test_overlap_area_no_overlap() {
        let face_a = SurfaceFace {
            center: [0.0, 0.0, 0.0],
            normal: [0.0, 0.0, 1.0],
            area: 1.0,
            half_extents: [0.5, 0.5],
        };
        let face_b = SurfaceFace {
            center: [10.0, 10.0, 0.0],
            normal: [0.0, 0.0, 1.0],
            area: 1.0,
            half_extents: [0.5, 0.5],
        };
        assert!(compute_overlap_area(&face_a, &face_b) < 1e-30);
    }

    #[test]
    fn test_overlap_area_full_overlap() {
        let face = SurfaceFace {
            center: [0.5, 0.5, 0.0],
            normal: [0.0, 0.0, 1.0],
            area: 1.0,
            half_extents: [0.5, 0.5],
        };
        let overlap = compute_overlap_area(&face, &face);
        assert!(
            (overlap - 1.0).abs() < 1e-10,
            "Full overlap should equal face area: got {}",
            overlap
        );
    }

    #[test]
    fn test_overlap_area_partial() {
        let face_a = SurfaceFace {
            center: [0.0, 0.0, 0.0],
            normal: [0.0, 0.0, 1.0],
            area: 4.0,
            half_extents: [1.0, 1.0],
        };
        let face_b = SurfaceFace {
            center: [1.0, 0.0, 0.0],
            normal: [0.0, 0.0, 1.0],
            area: 4.0,
            half_extents: [1.0, 1.0],
        };
        // Overlap in x: max(0, min(1,2) - max(-1,0)) = min(1,2) - 0 = 1
        // Overlap in y: max(0, min(1,1) - max(-1,-1)) = 1-(-1) = 2
        // Area = 1 * 2 = 2
        let overlap = compute_overlap_area(&face_a, &face_b);
        assert!(
            (overlap - 2.0).abs() < 1e-10,
            "Partial overlap should be 2.0: got {}",
            overlap
        );
    }

    #[test]
    fn test_perpendicular_faces_no_overlap() {
        let face_a = SurfaceFace {
            center: [0.0, 0.0, 0.0],
            normal: [0.0, 0.0, 1.0],
            area: 1.0,
            half_extents: [0.5, 0.5],
        };
        let face_b = SurfaceFace {
            center: [0.0, 0.0, 0.0],
            normal: [1.0, 0.0, 0.0],
            area: 1.0,
            half_extents: [0.5, 0.5],
        };
        // Perpendicular normals: dot = 0 < 0.3, so no overlap
        assert!(compute_overlap_area(&face_a, &face_b) < 1e-30);
    }
}
