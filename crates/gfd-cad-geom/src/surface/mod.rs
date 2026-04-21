//! Parametric surfaces.

use crate::{Point3, Vector3, GeomResult};

pub mod bspline;
pub mod cone;
pub mod cylinder;
pub mod plane;
pub mod sphere;
pub mod torus;

pub use bspline::BSplineSurface;
pub use cone::Cone;
pub use cylinder::Cylinder;
pub use plane::Plane;
pub use sphere::Sphere;
pub use torus::Torus;

/// Parametric surface S(u, v) : [u0,u1] × [v0,v1] → R^3.
pub trait Surface {
    fn u_range(&self) -> (f64, f64);
    fn v_range(&self) -> (f64, f64);
    fn eval(&self, u: f64, v: f64) -> GeomResult<Point3>;
    fn normal(&self, u: f64, v: f64) -> GeomResult<Vector3>;
}
