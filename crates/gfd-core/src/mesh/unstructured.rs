//! Unstructured mesh representation.

use serde::{Deserialize, Serialize};

use super::cell::Cell;
use super::face::Face;
use super::node::Node;
use crate::{CoreError, Result};

/// A named group of boundary faces.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BoundaryPatch {
    /// Name of the boundary patch (e.g., "inlet", "wall", "outlet").
    pub name: String,
    /// Indices of the faces belonging to this patch.
    pub face_ids: Vec<usize>,
}

impl BoundaryPatch {
    /// Creates a new boundary patch.
    pub fn new(name: impl Into<String>, face_ids: Vec<usize>) -> Self {
        Self {
            name: name.into(),
            face_ids,
        }
    }

    /// Returns the number of faces in this patch.
    pub fn num_faces(&self) -> usize {
        self.face_ids.len()
    }
}

/// An unstructured mesh composed of arbitrary polyhedral cells.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UnstructuredMesh {
    /// All nodes in the mesh.
    pub nodes: Vec<Node>,
    /// All faces in the mesh.
    pub faces: Vec<Face>,
    /// All cells in the mesh.
    pub cells: Vec<Cell>,
    /// Named boundary patches.
    pub boundary_patches: Vec<BoundaryPatch>,
}

impl UnstructuredMesh {
    /// Creates a new empty unstructured mesh.
    pub fn new() -> Self {
        Self {
            nodes: Vec::new(),
            faces: Vec::new(),
            cells: Vec::new(),
            boundary_patches: Vec::new(),
        }
    }

    /// Creates an unstructured mesh from the given components.
    pub fn from_components(
        nodes: Vec<Node>,
        faces: Vec<Face>,
        cells: Vec<Cell>,
        boundary_patches: Vec<BoundaryPatch>,
    ) -> Self {
        Self {
            nodes,
            faces,
            cells,
            boundary_patches,
        }
    }

    /// Returns the number of cells in the mesh.
    pub fn num_cells(&self) -> usize {
        self.cells.len()
    }

    /// Returns the number of faces in the mesh.
    pub fn num_faces(&self) -> usize {
        self.faces.len()
    }

    /// Returns the number of nodes in the mesh.
    pub fn num_nodes(&self) -> usize {
        self.nodes.len()
    }

    /// Returns a reference to the cell at the given index.
    pub fn cell(&self, id: usize) -> Result<&Cell> {
        self.cells.get(id).ok_or(CoreError::IndexOutOfBounds {
            index: id,
            size: self.cells.len(),
        })
    }

    /// Returns a reference to the face at the given index.
    pub fn face(&self, id: usize) -> Result<&Face> {
        self.faces.get(id).ok_or(CoreError::IndexOutOfBounds {
            index: id,
            size: self.faces.len(),
        })
    }

    /// Returns a reference to the node at the given index.
    pub fn node(&self, id: usize) -> Result<&Node> {
        self.nodes.get(id).ok_or(CoreError::IndexOutOfBounds {
            index: id,
            size: self.nodes.len(),
        })
    }

    /// Returns a reference to the boundary patch with the given name.
    pub fn boundary_patch(&self, name: &str) -> Option<&BoundaryPatch> {
        self.boundary_patches.iter().find(|p| p.name == name)
    }

    /// Returns the names of all boundary patches.
    pub fn boundary_patch_names(&self) -> Vec<&str> {
        self.boundary_patches.iter().map(|p| p.name.as_str()).collect()
    }
}

impl Default for UnstructuredMesh {
    fn default() -> Self {
        Self::new()
    }
}
