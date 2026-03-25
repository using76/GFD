//! Mesh node (vertex) definition.

use serde::{Deserialize, Serialize};

/// A node (vertex) in the mesh.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct Node {
    /// Unique identifier for this node.
    pub id: usize,
    /// 3D position of the node.
    pub position: [f64; 3],
}

impl Node {
    /// Creates a new node with the given id and position.
    pub fn new(id: usize, position: [f64; 3]) -> Self {
        Self { id, position }
    }

    /// Returns the x-coordinate.
    pub fn x(&self) -> f64 {
        self.position[0]
    }

    /// Returns the y-coordinate.
    pub fn y(&self) -> f64 {
        self.position[1]
    }

    /// Returns the z-coordinate.
    pub fn z(&self) -> f64 {
        self.position[2]
    }
}
