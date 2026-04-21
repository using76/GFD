use serde::{Deserialize, Serialize};
use std::f64::consts::{PI, TAU};

use crate::{GeomResult, Point3, Surface, Vector3};

/// Sphere centered at `center` with radius `radius`. Parameterization:
/// u = longitude [0,2π], v = latitude [-π/2, π/2].
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Sphere {
    pub center: Point3,
    pub radius: f64,
}

impl Sphere {
    pub fn new(center: Point3, radius: f64) -> Self {
        Self { center, radius }
    }
}

impl Surface for Sphere {
    fn u_range(&self) -> (f64, f64) { (0.0, TAU) }
    fn v_range(&self) -> (f64, f64) { (-PI / 2.0, PI / 2.0) }

    fn eval(&self, u: f64, v: f64) -> GeomResult<Point3> {
        let (su, cu) = u.sin_cos();
        let (sv, cv) = v.sin_cos();
        Ok(Point3::new(
            self.center.x + self.radius * cv * cu,
            self.center.y + self.radius * cv * su,
            self.center.z + self.radius * sv,
        ))
    }

    fn normal(&self, u: f64, v: f64) -> GeomResult<Vector3> {
        let (su, cu) = u.sin_cos();
        let (sv, cv) = v.sin_cos();
        Ok(Vector3::new(cv * cu, cv * su, sv))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use approx::assert_abs_diff_eq;

    #[test]
    fn unit_sphere_equator() {
        let s = Sphere::new(Point3::ORIGIN, 1.0);
        let p = s.eval(0.0, 0.0).unwrap();
        assert_abs_diff_eq!(p.x, 1.0, epsilon = 1e-12);
        assert_abs_diff_eq!(p.y, 0.0, epsilon = 1e-12);
        assert_abs_diff_eq!(p.z, 0.0, epsilon = 1e-12);
    }
}
