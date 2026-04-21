//! gfd-cad-geom — Parametric geometry primitives for the GFD CAD kernel.
//!
//! Layer 0 of the CAD crate graph. Provides curves (line/circle/bspline) and
//! surfaces (plane/cylinder/sphere/cone/torus) with evaluation, derivatives,
//! projection, and bounding boxes. No topology — that lives in `gfd-cad-topo`.

pub mod bbox;
pub mod curve;
pub mod point;
pub mod surface;
pub mod vec;

pub use bbox::BoundingBox;
pub use curve::{BSplineCurve, Circle, Curve, Line};
pub use point::Point3;
pub use surface::{BSplineSurface, Cone, Cylinder, Plane, Sphere, Surface, Torus};
pub use vec::{Direction3, Vector3};

/// Default angular tolerance for geometric predicates (radians).
pub const ANGLE_TOL: f64 = 1.0e-9;

/// Default linear tolerance (model units).
pub const LINEAR_TOL: f64 = 1.0e-7;

#[derive(Debug, thiserror::Error)]
pub enum GeomError {
    #[error("parameter {0} out of range")]
    OutOfRange(f64),
    #[error("degenerate geometry: {0}")]
    Degenerate(&'static str),
    #[error("projection failed")]
    ProjectionFailed,
}

pub type GeomResult<T> = Result<T, GeomError>;
