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
}
