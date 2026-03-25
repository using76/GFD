//! Mesh data structures and utilities.

pub mod structured;
pub mod unstructured;
pub mod cell;
pub mod face;
pub mod node;
pub mod partition;

use serde::{Deserialize, Serialize};

/// Top-level mesh enum that can hold either a structured or unstructured mesh.
#[derive(Debug, Clone)]
pub enum Mesh {
    Structured(structured::StructuredMesh),
    Unstructured(unstructured::UnstructuredMesh),
}

impl Mesh {
    /// Returns summary information about this mesh.
    pub fn info(&self) -> MeshInfo {
        match self {
            Mesh::Structured(m) => MeshInfo {
                num_cells: m.num_cells(),
                num_faces: m.num_faces(),
                num_nodes: m.num_nodes(),
                dimensions: m.dimensions(),
            },
            Mesh::Unstructured(m) => MeshInfo {
                num_cells: m.num_cells(),
                num_faces: m.num_faces(),
                num_nodes: m.num_nodes(),
                dimensions: 3,
            },
        }
    }
}

/// Summary information about a mesh.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct MeshInfo {
    pub num_cells: usize,
    pub num_faces: usize,
    pub num_nodes: usize,
    pub dimensions: usize,
}
