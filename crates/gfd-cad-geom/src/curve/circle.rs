use serde::{Deserialize, Serialize};
use std::f64::consts::TAU;

use crate::{Curve, Direction3, GeomResult, Point3, Vector3};

/// Circle in 3D. Parameter u is arc length, u ∈ [0, 2π·radius].
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Circle {
    pub center: Point3,
    pub normal: Direction3,
    pub x_axis: Direction3,
    pub radius: f64,
}

impl Circle {
    pub fn new(center: Point3, normal: Direction3, x_axis: Direction3, radius: f64) -> Self {
        Self { center, normal, x_axis, radius }
    }

    fn y_axis(&self) -> Vector3 {
        // normal × x_axis (assumed orthogonal on construction)
        self.normal.as_vec().cross(self.x_axis.as_vec())
    }

    /// Construct the unique circle passing through three non-colinear
    /// points `a`, `b`, `c`. Solves the perpendicular-bisector system in
    /// the plane of the three points. Returns `Err(Degenerate)` if the
    /// points are colinear or coincident.
    pub fn from_three_points(a: Point3, b: Point3, c: Point3) -> GeomResult<Self> {
        let ab = Vector3::new(b.x - a.x, b.y - a.y, b.z - a.z);
        let ac = Vector3::new(c.x - a.x, c.y - a.y, c.z - a.z);
        let n = ab.cross(ac);
        let nd = n.to_direction()?;
        // In the plane, solve for circumcenter in barycentric form:
        // center = a + (|ac|² · (ab · ac) · ab − |ab|² · (ab · ac) · ac
        //                + |ab|² · |ac|² · (1 − ... ))  —  standard formula:
        // d = 2 · |ab × ac|²
        // α = |ac|² · (ab · ab − ab · ac) / d (weight for b)
        // β = |ab|² · (ac · ac − ab · ac) / d (weight for c)
        // center = a + α · ab + β · ac   [equivalent barycentric form]
        let d = 2.0 * n.dot(n);
        if d.abs() < crate::LINEAR_TOL {
            return Err(crate::GeomError::Degenerate("colinear input points"));
        }
        let ab_ab = ab.dot(ab);
        let ac_ac = ac.dot(ac);
        let ab_ac = ab.dot(ac);
        let alpha = ac_ac * (ab_ab - ab_ac) / d;
        let beta  = ab_ab * (ac_ac - ab_ac) / d;
        let cx = a.x + alpha * ab.x + beta * ac.x;
        let cy = a.y + alpha * ab.y + beta * ac.y;
        let cz = a.z + alpha * ab.z + beta * ac.z;
        let center = Point3::new(cx, cy, cz);
        let radius = center.distance(a);
        let xd = Vector3::new(a.x - cx, a.y - cy, a.z - cz).to_direction()?;
        Ok(Self::new(center, nd, xd, radius))
    }
}

impl Curve for Circle {
    fn u_range(&self) -> (f64, f64) { (0.0, TAU * self.radius) }

    fn eval(&self, u: f64) -> GeomResult<Point3> {
        let theta = u / self.radius;
        let (s, c) = theta.sin_cos();
        let y = self.y_axis();
        Ok(Point3::new(
            self.center.x + self.radius * (c * self.x_axis.x + s * y.x),
            self.center.y + self.radius * (c * self.x_axis.y + s * y.y),
            self.center.z + self.radius * (c * self.x_axis.z + s * y.z),
        ))
    }

    fn tangent(&self, u: f64) -> GeomResult<Vector3> {
        let theta = u / self.radius;
        let (s, c) = theta.sin_cos();
        let y = self.y_axis();
        Ok(Vector3::new(
            -s * self.x_axis.x + c * y.x,
            -s * self.x_axis.y + c * y.y,
            -s * self.x_axis.z + c * y.z,
        ))
    }

    fn length(&self) -> f64 { TAU * self.radius }
}

#[cfg(test)]
mod tests {
    use super::*;
    use approx::assert_abs_diff_eq;

    #[test]
    fn unit_circle_quarter() {
        let c = Circle::new(Point3::ORIGIN, Direction3::Z, Direction3::X, 1.0);
        let p = c.eval(std::f64::consts::FRAC_PI_2).unwrap();
        assert_abs_diff_eq!(p.x, 0.0, epsilon = 1e-12);
        assert_abs_diff_eq!(p.y, 1.0, epsilon = 1e-12);
    }

    #[test]
    fn circle_from_three_axis_points() {
        // Three points on the unit circle in the XY plane:
        // (1,0,0), (0,1,0), (-1,0,0) → center = origin, radius = 1.
        let c = Circle::from_three_points(
            Point3::new(1.0, 0.0, 0.0),
            Point3::new(0.0, 1.0, 0.0),
            Point3::new(-1.0, 0.0, 0.0),
        ).unwrap();
        assert_abs_diff_eq!(c.center.x, 0.0, epsilon = 1e-10);
        assert_abs_diff_eq!(c.center.y, 0.0, epsilon = 1e-10);
        assert_abs_diff_eq!(c.center.z, 0.0, epsilon = 1e-10);
        assert_abs_diff_eq!(c.radius, 1.0, epsilon = 1e-10);
        // Normal should be ±Z.
        assert_abs_diff_eq!(c.normal.z.abs(), 1.0, epsilon = 1e-10);
    }

    #[test]
    fn circle_from_colinear_points_is_err() {
        let r = Circle::from_three_points(
            Point3::new(0.0, 0.0, 0.0),
            Point3::new(1.0, 0.0, 0.0),
            Point3::new(2.0, 0.0, 0.0),
        );
        assert!(r.is_err());
    }
}
