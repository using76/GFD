use serde::{Deserialize, Serialize};

use crate::{Curve, Direction3, GeomError, GeomResult, Point3, Vector3};

/// Straight line segment between two endpoints (u = 0 .. length).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Line {
    pub origin: Point3,
    pub direction: Direction3,
    pub length: f64,
}

impl Line {
    pub fn from_points(a: Point3, b: Point3) -> GeomResult<Self> {
        let v = Vector3::new(b.x - a.x, b.y - a.y, b.z - a.z);
        let length = v.norm();
        if length < crate::LINEAR_TOL {
            return Err(GeomError::Degenerate("coincident endpoints"));
        }
        Ok(Self { origin: a, direction: v.to_direction()?, length })
    }
}

impl Curve for Line {
    fn u_range(&self) -> (f64, f64) { (0.0, self.length) }

    fn eval(&self, u: f64) -> GeomResult<Point3> {
        if u < -crate::LINEAR_TOL || u > self.length + crate::LINEAR_TOL {
            return Err(GeomError::OutOfRange(u));
        }
        Ok(Point3::new(
            self.origin.x + self.direction.x * u,
            self.origin.y + self.direction.y * u,
            self.origin.z + self.direction.z * u,
        ))
    }

    fn tangent(&self, _u: f64) -> GeomResult<Vector3> {
        Ok(self.direction.as_vec())
    }

    fn length(&self) -> f64 { self.length }
}

#[cfg(test)]
mod tests {
    use super::*;
    use approx::assert_abs_diff_eq;

    #[test]
    fn line_eval_midpoint() {
        let l = Line::from_points(Point3::ORIGIN, Point3::new(2.0, 0.0, 0.0)).unwrap();
        let p = l.eval(1.0).unwrap();
        assert_abs_diff_eq!(p.x, 1.0, epsilon = 1e-12);
    }
}
