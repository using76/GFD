//! Mesh file readers.

pub mod gmsh;
pub mod stl;
pub mod cgns;

use gfd_core::UnstructuredMesh;
use crate::Result;

/// Trait for reading mesh files into an UnstructuredMesh.
pub trait MeshReader {
    /// Reads a mesh file and returns an UnstructuredMesh.
    fn read(&self, path: &str) -> Result<UnstructuredMesh>;
}
