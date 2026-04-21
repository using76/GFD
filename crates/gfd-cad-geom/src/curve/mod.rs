//! Parametric curves.

use crate::{Point3, Vector3, GeomResult};

pub mod bspline;
pub mod circle;
pub mod ellipse;
pub mod line;
pub mod polyline;

pub use bspline::BSplineCurve;
pub use circle::Circle;
pub use ellipse::Ellipse;
pub use line::Line;
pub use polyline::Polyline;

/// A parametric curve C(u) : [u_min, u_max] → R^3.
pub trait Curve {
    fn u_range(&self) -> (f64, f64);
    fn eval(&self, u: f64) -> GeomResult<Point3>;
    fn tangent(&self, u: f64) -> GeomResult<Vector3>;
    fn length(&self) -> f64;

    /// Closest-point projection on the curve using golden-section search.
    /// Returns `(u, distance)`. Adequate for GUI measurement; Phase 2 topology
    /// will swap in Newton-iteration.
    fn closest_point(&self, p: Point3) -> GeomResult<(f64, f64)> {
        let (lo, hi) = self.u_range();
        let phi = (1.0 + 5f64.sqrt()) / 2.0;
        let inv_phi = 1.0 / phi;
        let mut a = lo;
        let mut b = hi;
        let dist = |u: f64| -> f64 {
            self.eval(u).map(|q| q.distance(p)).unwrap_or(f64::INFINITY)
        };
        let mut c = b - (b - a) * inv_phi;
        let mut d = a + (b - a) * inv_phi;
        for _ in 0..80 {
            if dist(c) < dist(d) { b = d; } else { a = c; }
            c = b - (b - a) * inv_phi;
            d = a + (b - a) * inv_phi;
        }
        let u = 0.5 * (a + b);
        Ok((u, dist(u)))
    }
}
