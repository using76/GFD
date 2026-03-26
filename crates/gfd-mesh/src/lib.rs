//! GFD Mesh Generation Library
//!
//! Provides structured, unstructured, hybrid, adaptive, and dynamic mesh
//! generation capabilities comparable to Fluent Meshing / snappyHexMesh.

pub mod structured;
pub mod unstructured;
pub mod hybrid;
pub mod adaptation;
pub mod motion;
pub mod quality;
pub mod geometry;
pub mod io;

use gfd_core::mesh::unstructured::UnstructuredMesh;

/// Error type for mesh generation operations.
#[derive(Debug, thiserror::Error)]
pub enum MeshError {
    #[error("Invalid mesh parameters: {0}")]
    InvalidParameters(String),
    #[error("Mesh generation failed: {0}")]
    GenerationFailed(String),
    #[error("Quality check failed: {0}")]
    QualityFailed(String),
    #[error("Geometry error: {0}")]
    GeometryError(String),
}

pub type Result<T> = std::result::Result<T, MeshError>;

/// Trait that all mesh generators implement.
pub trait MeshGenerator {
    /// Build the mesh and return as UnstructuredMesh.
    fn build(&self) -> Result<UnstructuredMesh>;
}
