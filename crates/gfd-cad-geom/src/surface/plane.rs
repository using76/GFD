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

    fn y_axis(&self) -> Vector3 {
        self.normal.as_vec().cross(self.x_axis.as_vec())
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
