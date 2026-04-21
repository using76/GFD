use serde::{Deserialize, Serialize};

use crate::{Direction3, GeomResult, Point3, Surface, Vector3};

/// Infinite plane defined by origin and two orthonormal axes.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Plane {
    pub origin: Point3,
    pub normal: Direction3,
    pub x_axis: Direction3,
}

impl Plane {
    pub fn new(origin: Point3, normal: Direction3, x_axis: Direction3) -> Self {
        Self { origin, normal, x_axis }
    }

    pub fn xy() -> Self {
        Self::new(Point3::ORIGIN, Direction3::Z, Direction3::X)
    }

    /// Construct the plane through three non-colinear points `a`, `b`, `c`.
    /// The plane normal is (b-a) × (c-a) normalised; the `x_axis` is aligned
    /// with (b-a). Returns `Err(Degenerate)` if the three points are
    /// colinear or coincident within `LINEAR_TOL`.
    pub fn from_three_points(a: Point3, b: Point3, c: Point3) -> GeomResult<Self> {
        let ab = Vector3::new(b.x - a.x, b.y - a.y, b.z - a.z);
        let ac = Vector3::new(c.x - a.x, c.y - a.y, c.z - a.z);
        let n = ab.cross(ac);
        let nd = n.to_direction()?;
        let xd = ab.to_direction()?;
        Ok(Self::new(a, nd, xd))
    }

    fn y_axis(&self) -> Vector3 {
        self.normal.as_vec().cross(self.x_axis.as_vec())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use approx::assert_abs_diff_eq;

    #[test]
    fn plane_from_three_points_z0() {
        let p = Plane::from_three_points(
            Point3::new(0.0, 0.0, 0.0),
            Point3::new(1.0, 0.0, 0.0),
            Point3::new(0.0, 1.0, 0.0),
        ).unwrap();
        assert_abs_diff_eq!(p.normal.z.abs(), 1.0, epsilon = 1e-10);
        assert_abs_diff_eq!(p.x_axis.x, 1.0, epsilon = 1e-10);
    }

    #[test]
    fn plane_from_colinear_points_err() {
        let r = Plane::from_three_points(
            Point3::new(0.0, 0.0, 0.0),
            Point3::new(1.0, 0.0, 0.0),
            Point3::new(2.0, 0.0, 0.0),
        );
        assert!(r.is_err());
    }
}

impl Surface for Plane {
    fn u_range(&self) -> (f64, f64) { (f64::NEG_INFINITY, f64::INFINITY) }
    fn v_range(&self) -> (f64, f64) { (f64::NEG_INFINITY, f64::INFINITY) }

    fn eval(&self, u: f64, v: f64) -> GeomResult<Point3> {
        let y = self.y_axis();
        Ok(Point3::new(
            self.origin.x + u * self.x_axis.x + v * y.x,
            self.origin.y + u * self.x_axis.y + v * y.y,
            self.origin.z + u * self.x_axis.z + v * y.z,
        ))
    }

    fn normal(&self, _u: f64, _v: f64) -> GeomResult<Vector3> {
        Ok(self.normal.as_vec())
    }
}
