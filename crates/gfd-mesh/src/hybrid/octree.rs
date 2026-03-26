//! Octree-based adaptive mesh refinement.
//!
//! Provides an octree data structure that can be selectively refined and
//! converted to an `UnstructuredMesh`.

use gfd_core::mesh::cell::Cell;
use gfd_core::mesh::face::Face;
use gfd_core::mesh::node::Node;
use gfd_core::mesh::unstructured::{BoundaryPatch, UnstructuredMesh};

use crate::{MeshError, Result};

/// A node in the octree.
#[derive(Debug, Clone)]
pub struct OctreeNode {
    /// Refinement level (0 = root).
    pub level: usize,
    /// Center of this cell.
    pub center: [f64; 3],
    /// Half the side length of this cell.
    pub half_size: f64,
    /// Children, if this cell has been refined.
    pub children: Option<Box<[OctreeNode; 8]>>,
    /// Cell id in the final mesh (only set for leaf cells).
    pub cell_id: Option<usize>,
}

impl OctreeNode {
    /// Creates a new leaf octree node.
    fn new(level: usize, center: [f64; 3], half_size: f64) -> Self {
        Self {
            level,
            center,
            half_size,
            children: None,
            cell_id: None,
        }
    }

    /// Returns true if this is a leaf node (not refined).
    pub fn is_leaf(&self) -> bool {
        self.children.is_none()
    }

    /// Refines this cell into 8 children.
    fn refine(&mut self) {
        if self.children.is_some() {
            return;
        }
        let hs = self.half_size * 0.5;
        let cx = self.center[0];
        let cy = self.center[1];
        let cz = self.center[2];

        let children = [
            OctreeNode::new(self.level + 1, [cx - hs, cy - hs, cz - hs], hs),
            OctreeNode::new(self.level + 1, [cx + hs, cy - hs, cz - hs], hs),
            OctreeNode::new(self.level + 1, [cx - hs, cy + hs, cz - hs], hs),
            OctreeNode::new(self.level + 1, [cx + hs, cy + hs, cz - hs], hs),
            OctreeNode::new(self.level + 1, [cx - hs, cy - hs, cz + hs], hs),
            OctreeNode::new(self.level + 1, [cx + hs, cy - hs, cz + hs], hs),
            OctreeNode::new(self.level + 1, [cx - hs, cy + hs, cz + hs], hs),
            OctreeNode::new(self.level + 1, [cx + hs, cy + hs, cz + hs], hs),
        ];
        self.children = Some(Box::new(children));
    }

    /// Refine current leaves where the predicate is true, up to max_level.
    /// Only refines leaves that existed before this call (does not recurse into
    /// newly created children).
    fn refine_where(&mut self, predicate: &dyn Fn([f64; 3]) -> bool, max_level: usize) {
        if self.level >= max_level {
            return;
        }
        if self.is_leaf() {
            if predicate(self.center) {
                self.refine();
            }
            // Do NOT recurse into newly created children.
            return;
        }
        if let Some(ref mut children) = self.children {
            for child in children.iter_mut() {
                child.refine_where(predicate, max_level);
            }
        }
    }

    /// Collects all leaf nodes into a flat list, assigning cell IDs.
    fn collect_leaves(&mut self, leaves: &mut Vec<LeafInfo>) {
        if self.is_leaf() {
            let id = leaves.len();
            self.cell_id = Some(id);
            leaves.push(LeafInfo {
                center: self.center,
                half_size: self.half_size,
                level: self.level,
            });
        } else if let Some(ref mut children) = self.children {
            for child in children.iter_mut() {
                child.collect_leaves(leaves);
            }
        }
    }

    /// Enforces the 2:1 balance constraint so that no two adjacent leaves differ
    /// by more than one level of refinement.
    fn balance_21(&mut self, max_level: usize) {
        // Multi-pass approach: iterate until no changes.
        let mut changed = true;
        while changed {
            changed = false;
            self.balance_pass(&mut changed, max_level);
        }
    }

    fn balance_pass(&mut self, changed: &mut bool, max_level: usize) {
        if self.is_leaf() {
            return;
        }
        if let Some(ref mut children) = self.children {
            for child in children.iter_mut() {
                child.balance_pass(changed, max_level);
            }

            // Check if any child has children while a sibling is a leaf at a level
            // that is 2+ levels coarser than the children's children.
            let max_child_depth = children.iter().map(|c| c.max_depth()).max().unwrap_or(0);
            let min_child_depth = children.iter().map(|c| c.max_depth()).min().unwrap_or(0);

            if max_child_depth > min_child_depth + 1 {
                // Need to refine the coarsest children
                for child in children.iter_mut() {
                    if child.is_leaf() && child.level < max_level && child.max_depth() < max_child_depth - 1 {
                        child.refine();
                        *changed = true;
                    }
                }
            }
        }
    }

    /// Returns the maximum depth below this node.
    fn max_depth(&self) -> usize {
        if self.is_leaf() {
            self.level
        } else if let Some(ref children) = self.children {
            children.iter().map(|c| c.max_depth()).max().unwrap_or(self.level)
        } else {
            self.level
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct LeafInfo {
    center: [f64; 3],
    half_size: f64,
    #[allow(dead_code)]
    level: usize,
}

/// An octree-based mesh that supports adaptive refinement.
pub struct OctreeMesh {
    /// Root node of the octree.
    pub root: OctreeNode,
    /// Maximum allowed refinement level.
    pub max_level: usize,
    /// Domain extents: [xmin, xmax, ymin, ymax, zmin, zmax].
    pub domain: [f64; 6],
}

impl OctreeMesh {
    /// Creates a new octree mesh covering the given domain.
    pub fn new(domain: [f64; 6], max_level: usize) -> Self {
        let cx = (domain[0] + domain[1]) * 0.5;
        let cy = (domain[2] + domain[3]) * 0.5;
        let cz = (domain[4] + domain[5]) * 0.5;
        let hx = (domain[1] - domain[0]) * 0.5;
        let hy = (domain[3] - domain[2]) * 0.5;
        let hz = (domain[5] - domain[4]) * 0.5;
        let half_size = hx.max(hy).max(hz);

        Self {
            root: OctreeNode::new(0, [cx, cy, cz], half_size),
            max_level,
            domain,
        }
    }

    /// Refine all leaves where the predicate returns true.
    pub fn refine_at(&mut self, predicate: impl Fn([f64; 3]) -> bool) {
        self.root.refine_where(&predicate, self.max_level);
    }

    /// Enforces the 2:1 balance constraint.
    pub fn balance(&mut self) {
        self.root.balance_21(self.max_level);
    }

    /// Converts the octree leaves into an `UnstructuredMesh`.
    ///
    /// Each leaf becomes a hexahedral cell. Faces are created between adjacent cells
    /// and on the domain boundary.
    pub fn to_unstructured(&self) -> Result<UnstructuredMesh> {
        // Collect all leaves
        let mut root_clone = self.root.clone();
        let mut leaves = Vec::new();
        root_clone.collect_leaves(&mut leaves);

        let n_cells = leaves.len();
        if n_cells == 0 {
            return Err(MeshError::GenerationFailed(
                "Octree has no leaf cells".into(),
            ));
        }

        // Build nodes and cells for each leaf (each leaf = 1 hex cell with 8 unique nodes)
        let mut nodes: Vec<Node> = Vec::new();
        let mut cells: Vec<Cell> = Vec::with_capacity(n_cells);

        for (ci, leaf) in leaves.iter().enumerate() {
            let hs = leaf.half_size;
            let cx = leaf.center[0];
            let cy = leaf.center[1];
            let cz = leaf.center[2];

            let base_nid = nodes.len();

            // 8 corners of the hex
            let corners = [
                [cx - hs, cy - hs, cz - hs], // 0
                [cx + hs, cy - hs, cz - hs], // 1
                [cx + hs, cy + hs, cz - hs], // 2
                [cx - hs, cy + hs, cz - hs], // 3
                [cx - hs, cy - hs, cz + hs], // 4
                [cx + hs, cy - hs, cz + hs], // 5
                [cx + hs, cy + hs, cz + hs], // 6
                [cx - hs, cy + hs, cz + hs], // 7
            ];

            for (i, &pos) in corners.iter().enumerate() {
                nodes.push(Node::new(base_nid + i, pos));
            }

            let cell_nodes: Vec<usize> = (base_nid..base_nid + 8).collect();
            let vol = (2.0 * hs).powi(3);

            cells.push(Cell::new(ci, cell_nodes, Vec::new(), vol, leaf.center));
        }

        // Build faces: for each pair of adjacent leaf cells, create an internal face.
        // Also create boundary faces on the domain boundary.
        let mut faces: Vec<Face> = Vec::new();
        let mut boundary_face_ids: Vec<usize> = Vec::new();

        // Use a spatial lookup to find neighbors: for each cell, check in 6 directions
        // We build a simple hash map from cell center to cell index.
        let eps = 1e-10;

        // Map from quantized center to cell index (for finding neighbors)
        // We need to handle multi-resolution, so we check in each direction at the
        // cell's own half_size distance.
        let cell_centers: Vec<([f64; 3], f64)> = leaves
            .iter()
            .map(|l| (l.center, l.half_size))
            .collect();

        // For simplicity, check face adjacency using AABB overlap.
        // Two cells share a face if they are adjacent along exactly one axis.
        let directions: [[f64; 3]; 6] = [
            [1.0, 0.0, 0.0],
            [-1.0, 0.0, 0.0],
            [0.0, 1.0, 0.0],
            [0.0, -1.0, 0.0],
            [0.0, 0.0, 1.0],
            [0.0, 0.0, -1.0],
        ];

        // Track which face pairs we've already created
        let mut face_pairs: std::collections::HashSet<(usize, usize)> = std::collections::HashSet::new();

        for ci in 0..n_cells {
            let (cc, hs) = cell_centers[ci];
            let side = 2.0 * hs;

            for (di, dir) in directions.iter().enumerate() {
                // Check if any cell is adjacent in this direction
                let _probe = [
                    cc[0] + dir[0] * side,
                    cc[1] + dir[1] * side,
                    cc[2] + dir[2] * side,
                ];

                // Find a neighbor cell whose center is near the probe point
                // (same size), or a cell at a different level whose face overlaps.
                let mut found_neighbor = false;

                for ni in 0..n_cells {
                    if ni == ci {
                        continue;
                    }
                    let (nc, nhs) = cell_centers[ni];

                    // Check if cells share a face: their centers must be separated
                    // by exactly (hs + nhs) along one axis and overlap on the other two.
                    let dx = (nc[0] - cc[0]).abs();
                    let dy = (nc[1] - cc[1]).abs();
                    let dz = (nc[2] - cc[2]).abs();

                    let axis = di / 2; // 0=x, 1=y, 2=z
                    let sep = [dx, dy, dz];
                    let expected_sep = hs + nhs;

                    // Check that the separation along the face axis is correct
                    if (sep[axis] - expected_sep).abs() > eps * expected_sep.max(1.0) {
                        continue;
                    }

                    // Check overlap on the other two axes
                    let mut overlaps = true;
                    for a in 0..3 {
                        if a == axis {
                            continue;
                        }
                        // Overlap condition: |center_diff| < hs + nhs on this axis
                        // But for same-size cells, centers should match on other axes.
                        // For different-size cells, the smaller cell's center should be
                        // within the larger cell's face extent.
                        if sep[a] >= hs + nhs - eps {
                            overlaps = false;
                            break;
                        }
                    }

                    if !overlaps {
                        continue;
                    }

                    // Check direction sign
                    let diff = [nc[0] - cc[0], nc[1] - cc[1], nc[2] - cc[2]];
                    if diff[axis] * dir[axis] < 0.0 {
                        continue;
                    }

                    found_neighbor = true;

                    // Create face if not already done
                    let pair = if ci < ni { (ci, ni) } else { (ni, ci) };
                    if face_pairs.insert(pair) {
                        let fid = faces.len();
                        let face_area = (2.0 * hs.min(nhs)).powi(2);
                        let face_center = [
                            (cc[0] + nc[0]) * 0.5,
                            (cc[1] + nc[1]) * 0.5,
                            (cc[2] + nc[2]) * 0.5,
                        ];
                        let normal = *dir;

                        faces.push(Face::new(
                            fid,
                            vec![], // Skip node connectivity for octree faces
                            ci,
                            Some(ni),
                            face_area,
                            normal,
                            face_center,
                        ));
                        cells[ci].faces.push(fid);
                        cells[ni].faces.push(fid);
                    }
                    break;
                }

                if !found_neighbor {
                    // Boundary face
                    let fid = faces.len();
                    let face_area = (2.0 * hs).powi(2);
                    let face_center = [
                        cc[0] + dir[0] * hs,
                        cc[1] + dir[1] * hs,
                        cc[2] + dir[2] * hs,
                    ];

                    faces.push(Face::new(
                        fid,
                        vec![],
                        ci,
                        None,
                        face_area,
                        *dir,
                        face_center,
                    ));
                    cells[ci].faces.push(fid);
                    boundary_face_ids.push(fid);
                }
            }
        }

        let mut boundary_patches = Vec::new();
        if !boundary_face_ids.is_empty() {
            boundary_patches.push(BoundaryPatch::new("boundary", boundary_face_ids));
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
    fn test_octree_single_cell() {
        let oct = OctreeMesh::new([0.0, 1.0, 0.0, 1.0, 0.0, 1.0], 3);
        let mesh = oct.to_unstructured().unwrap();
        assert_eq!(mesh.cells.len(), 1, "Unrefined octree should have 1 cell");
    }

    #[test]
    fn test_octree_one_refinement() {
        let mut oct = OctreeMesh::new([0.0, 1.0, 0.0, 1.0, 0.0, 1.0], 3);
        oct.refine_at(|_| true); // Refine everything once
        let mesh = oct.to_unstructured().unwrap();
        assert_eq!(mesh.cells.len(), 8, "One full refinement should give 8 cells");
    }

    #[test]
    fn test_octree_selective_refinement() {
        let mut oct = OctreeMesh::new([0.0, 2.0, 0.0, 2.0, 0.0, 2.0], 3);
        // Refine only cells in the positive octant
        oct.refine_at(|c| c[0] > 1.0 && c[1] > 1.0 && c[2] > 1.0);
        let _mesh = oct.to_unstructured().unwrap();
        // Root refines into 8. One of those 8 (the +++ octant) refines into 8.
        // But only if the root-level cell center satisfies the predicate.
        // Root center = [1,1,1], which is NOT > 1 in all components.
        // So the root itself doesn't refine.
        // Let's first do a full refine, then selectively refine.
        let mut oct2 = OctreeMesh::new([0.0, 2.0, 0.0, 2.0, 0.0, 2.0], 3);
        oct2.refine_at(|_| true); // Level 0 -> 8 cells at level 1
        oct2.refine_at(|c| c[0] > 1.0 && c[1] > 1.0 && c[2] > 1.0);
        let mesh2 = oct2.to_unstructured().unwrap();
        // 7 unrefined + 8 refined = 15 cells
        assert_eq!(mesh2.cells.len(), 15, "Selective refinement: 7+8=15 cells");
    }

    #[test]
    fn test_octree_max_level_limit() {
        let mut oct = OctreeMesh::new([0.0, 1.0, 0.0, 1.0, 0.0, 1.0], 1);
        oct.refine_at(|_| true); // Level 0 -> 8
        oct.refine_at(|_| true); // Should not refine further (max_level=1)
        let mesh = oct.to_unstructured().unwrap();
        assert_eq!(mesh.cells.len(), 8, "Should not exceed max_level=1");
    }

    #[test]
    fn test_octree_boundary_faces() {
        let oct = OctreeMesh::new([0.0, 1.0, 0.0, 1.0, 0.0, 1.0], 3);
        let mesh = oct.to_unstructured().unwrap();
        // Single cell should have 6 boundary faces
        let boundary_count: usize = mesh
            .boundary_patches
            .iter()
            .map(|p| p.num_faces())
            .sum();
        assert_eq!(boundary_count, 6, "Single cell should have 6 boundary faces");
    }
}
