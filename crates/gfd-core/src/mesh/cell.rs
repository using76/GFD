//! Mesh cell (control volume) definition.

use serde::{Deserialize, Serialize};

/// Classification of cell topology.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CellType {
    Tetrahedron,
    Hexahedron,
    Wedge,
    Pyramid,
    Polyhedron,
}

/// A cell (control volume) in the mesh.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Cell {
    /// Unique identifier for this cell.
    pub id: usize,
    /// Indices of the nodes that form this cell.
    pub nodes: Vec<usize>,
    /// Indices of the faces bounding this cell.
    pub faces: Vec<usize>,
    /// Volume of the cell.
    pub volume: f64,
    /// Centroid of the cell.
    pub center: [f64; 3],
}

impl Cell {
    /// Creates a new cell.
    pub fn new(
        id: usize,
        nodes: Vec<usize>,
        faces: Vec<usize>,
        volume: f64,
        center: [f64; 3],
    ) -> Self {
        Self {
            id,
            nodes,
            faces,
            volume,
            center,
        }
    }

    /// Returns the number of nodes in this cell.
    pub fn num_nodes(&self) -> usize {
        self.nodes.len()
    }

    /// Returns the number of faces bounding this cell.
    pub fn num_faces(&self) -> usize {
        self.faces.len()
    }

    /// Infers the cell type from the number of nodes.
    pub fn cell_type(&self) -> CellType {
        match self.nodes.len() {
            4 => CellType::Tetrahedron,
            5 => CellType::Pyramid,
            6 => CellType::Wedge,
            8 => CellType::Hexahedron,
            _ => CellType::Polyhedron,
        }
    }
}
