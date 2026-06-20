//! `gfd-geo` — geospatial I/O for the flood-simulation track (see
//! `docs/FLOOD_PLAN.md`). Layer 1: terrain (DEM), CRS/georeferencing, and a
//! native IFC reader land here.
//!
//! Phase 1 (this module): DEM I/O — the ESRI ASCII grid (`.asc`) reader and a
//! [`Dem`] grid with a bilinear sampler. GeoTIFF / LAS, CRS, and IFC arrive in
//! later phases.
//!
//! Precision note (FLOOD_PLAN §3): all geo math here is `f64`. Projected
//! coordinates (UTM, ≥5e5 m) must be origin-shifted to a local frame BEFORE
//! being lowered into a `f32` `TriMesh`/STL, or sub-decimeter resolution is lost.

pub mod dem;
pub mod geotiff;
pub mod ifc;
pub mod sdf;

pub use dem::Dem;
pub use ifc::{IfcElement, IfcModel};

use thiserror::Error;

#[derive(Debug, Error)]
pub enum GeoError {
    #[error("io error: {0}")]
    Io(String),
    #[error("parse error: {0}")]
    Parse(String),
    #[error("unsupported: {0}")]
    Unsupported(String),
}

pub type GeoResult<T> = Result<T, GeoError>;

impl From<std::io::Error> for GeoError {
    fn from(e: std::io::Error) -> Self {
        GeoError::Io(e.to_string())
    }
}
