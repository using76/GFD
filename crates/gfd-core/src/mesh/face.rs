//! Mesh face definition.

use serde::{Deserialize, Serialize};

/// Classification of a face within the mesh.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FaceType {
    /// An internal face shared between two cells.
    Internal,
    /// A face on the domain boundary.
    Boundary,
    /// A face at a partition interface (for parallel decomposition).
    Interface,
}

/// A face in the mesh, connecting cells and defined by nodes.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Face {
    /// Unique identifier for this face.
    pub id: usize,
    /// Indices of the nodes that form this face.
    pub nodes: Vec<usize>,
    /// Index of the cell that owns this face.
    pub owner_cell: usize,
    /// Index of the neighboring cell (None for boundary faces).
    pub neighbor_cell: Option<usize>,
    /// Area of the face.
    pub area: f64,
    /// Outward-pointing unit normal vector of the face.
    pub normal: [f64; 3],
    /// Centroid of the face.
    pub center: [f64; 3],
}

impl Face {
    /// Creates a new face.
    pub fn new(
        id: usize,
        nodes: Vec<usize>,
        owner_cell: usize,
        neighbor_cell: Option<usize>,
        area: f64,
        normal: [f64; 3],
        center: [f64; 3],
    ) -> Self {
        Self {
            id,
            nodes,
            owner_cell,
            neighbor_cell,
            area,
            normal,
            center,
        }
    }

    /// Returns the type of this face.
    pub fn face_type(&self) -> FaceType {
        if self.neighbor_cell.is_some() {
            FaceType::Internal
        } else {
            FaceType::Boundary
        }
    }

    /// Returns true if this face is on the boundary.
    pub fn is_boundary(&self) -> bool {
        self.neighbor_cell.is_none()
    }

    /// Returns the number of nodes defining this face.
    pub fn num_nodes(&self) -> usize {
        self.nodes.len()
    }
}
