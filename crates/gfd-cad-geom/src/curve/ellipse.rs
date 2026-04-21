use serde::{Deserialize, Serialize};
use std::f64::consts::TAU;

use crate::{Curve, Direction3, GeomResult, Point3, Vector3};

/// Ellipse in 3D. Parameter u ∈ [0, 2π] is the angle, not arc length —
/// an ellipse has no closed-form arc-length parameterisation. For arc
/// length use [`Ellipse::length`] (Ramanujan II approximation).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Ellipse {
    pub center: Point3,
    pub normal: Direction3,
    pub x_axis: Direction3,
    pub major: f64,
    pub minor: f64,
}

impl Ellipse {
    pub fn new(center: Point3, normal: Direction3, x_axis: Direction3, major: f64, minor: f64) -> Self {
        Self { center, normal, x_axis, major, minor }
    }

    fn y_axis(&self) -> Vector3 {
        self.normal.as_vec().cross(self.x_axis.as_vec())
    }
}

impl Curve for Ellipse {
    fn u_range(&self) -> (f64, f64) { (0.0, TAU) }

    fn eval(&self, u: f64) -> GeomResult<Point3> {
        let (s, c) = u.sin_cos();
        let y = self.y_axis();
        Ok(Point3::new(
            self.center.x + self.major * c * self.x_axis.x + self.minor * s * y.x,
            self.center.y + self.major * c * self.x_axis.y + self.minor * s * y.y,
            self.center.z + self.major * c * self.x_axis.z + self.minor * s * y.z,
        ))
    }

    fn tangent(&self, u: f64) -> GeomResult<Vector3> {
        let (s, c) = u.sin_cos();
        let y = self.y_axis();
        Ok(Vector3::new(
            -self.major * s * self.x_axis.x + self.minor * c * y.x,
            -self.major * s * self.x_axis.y + self.minor * c * y.y,
            -self.major * s * self.x_axis.z + self.minor * c * y.z,
        ))
    }

    /// Ramanujan's second approximation:
    ///   L ≈ π (a + b) (1 + 3h/(10 + √(4 − 3h))), h = ((a−b)/(a+b))².
    /// Better than 1e-4 for eccentricity up to 0.99.
    fn length(&self) -> f64 {
        let a = self.major;
        let b = self.minor;
        if a + b == 0.0 { return 0.0; }
        let h = ((a - b) / (a + b)).powi(2);
        std::f64::consts::PI * (a + b) * (1.0 + 3.0 * h / (10.0 + (4.0 - 3.0 * h).sqrt()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use approx::assert_abs_diff_eq;

    #[test]
    fn ellipse_reduces_to_circle_when_equal_axes() {
        let e = Ellipse::new(Point3::ORIGIN, Direction3::Z, Direction3::X, 2.0, 2.0);
        assert_abs_diff_eq!(e.length(), TAU * 2.0, epsilon = 1e-6);
    }

    #[test]
    fn ellipse_evaluates_at_major_axis_endpoint() {
        let e = Ellipse::new(Point3::ORIGIN, Direction3::Z, Direction3::X, 3.0, 1.0);
        let p = e.eval(0.0).unwrap();
        assert_abs_diff_eq!(p.x, 3.0, epsilon = 1e-12);
        assert_abs_diff_eq!(p.y, 0.0, epsilon = 1e-12);
    }

    #[test]
    fn ellipse_quarter_is_minor_axis_endpoint() {
        let e = Ellipse::new(Point3::ORIGIN, Direction3::Z, Direction3::X, 3.0, 1.0);
        let p = e.eval(std::f64::consts::FRAC_PI_2).unwrap();
        assert_abs_diff_eq!(p.x, 0.0, epsilon = 1e-12);
        assert_abs_diff_eq!(p.y, 1.0, epsilon = 1e-12);
    }
}
