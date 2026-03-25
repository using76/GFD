//! VDB tree data structure.
//!
//! The VDB tree is a hierarchical structure with multiple levels of nodes:
//! - InternalNode (large branching factor, e.g. 32^3 or 16^3)
//! - InternalNode (smaller branching factor, e.g. 16^3 or 8^3)
//! - LeafNode (stores actual voxel data, typically 8^3)

/// A VDB tree node.
#[derive(Debug, Clone)]
pub enum TreeNode {
    /// Internal node with child pointers.
    Internal {
        /// Log2 of the branching factor dimension.
        log2dim: u32,
        /// Child mask indicating which children are allocated.
        child_mask: Vec<bool>,
        /// Child nodes (sparse).
        children: Vec<Option<Box<TreeNode>>>,
    },
    /// Leaf node storing voxel values.
    Leaf {
        /// Log2 of the leaf dimension.
        log2dim: u32,
        /// Active voxel mask.
        value_mask: Vec<bool>,
        /// Voxel values.
        values: Vec<f64>,
        /// Background value for inactive voxels.
        background: f64,
    },
}

impl TreeNode {
    /// Creates a new empty leaf node.
    pub fn new_leaf(log2dim: u32, background: f64) -> Self {
        let size = 1 << (3 * log2dim);
        TreeNode::Leaf {
            log2dim,
            value_mask: vec![false; size],
            values: vec![background; size],
            background,
        }
    }
}
