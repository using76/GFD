//! # gfd-occt
//!
//! OpenCASCADE (OCCT) backend for **professional B-Rep CAD**: shape healing
//! (`ShapeFix`), real boolean (cut/fuse/common → enclosure fluid extraction),
//! general fillet/chamfer, STEP/STL I/O, and `BRepMesh` tessellation for the
//! viewport.
//!
//! ## Feature flags (mirrors `gfd-gpu`'s optional `cuda`)
//!
//! * `occt` — links OpenCASCADE via a cxx bridge (`build.rs`, added in the OCCT
//!   integration phase). Requires a C++ toolchain + OCCT (vcpkg / prebuilt).
//!
//! Without the `occt` feature the crate compiles as a thin stub so the workspace
//! builds on machines with no C++/OCCT toolchain; every operation returns
//! [`OcctError::NotBuilt`] and callers fall back to the pure-Rust `gfd-cad`
//! kernel. This keeps `cargo build`/`cargo test --workspace` green by default.

use thiserror::Error;

/// Errors from the OCCT backend.
#[derive(Debug, Error)]
pub enum OcctError {
    /// The crate was compiled without the `occt` feature (OpenCASCADE not linked).
    #[error("gfd-occt was built without the `occt` feature — OpenCASCADE is not linked")]
    NotBuilt,
    /// An OCCT operation failed at runtime.
    #[error("OCCT operation failed: {0}")]
    Failed(String),
}

/// `true` when this crate was compiled with the `occt` feature (OpenCASCADE linked).
pub const fn is_available() -> bool {
    cfg!(feature = "occt")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn availability_matches_feature() {
        assert_eq!(is_available(), cfg!(feature = "occt"));
    }

    #[test]
    fn not_built_error_renders() {
        assert!(OcctError::NotBuilt.to_string().contains("occt"));
    }
}
