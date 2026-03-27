//! Geometry module.
//!
//! Provides SDF primitives, distance field computation from triangle meshes,
//! CAD defeaturing, surface operations, CFD preparation, and marching cubes.

pub mod analysis;
pub mod boolean_ops;
pub mod cfd_prep;
pub mod defeaturing;
pub mod distance_field;
pub mod extrude;
pub mod marching_cubes;
pub mod primitives;
pub mod sketch;
pub mod stl_reader;
pub mod surface_ops;
pub mod transform;
