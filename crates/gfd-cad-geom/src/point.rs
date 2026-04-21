use nalgebra::Point3 as NaPoint3;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Point3 {
    pub x: f64,
    pub y: f64,
    pub z: f64,
}

impl Point3 {
    pub const ORIGIN: Self = Self { x: 0.0, y: 0.0, z: 0.0 };

    #[inline]
    pub const fn new(x: f64, y: f64, z: f64) -> Self {
        Self { x, y, z }
    }

    #[inline]
    pub fn to_na(self) -> NaPoint3<f64> {
        NaPoint3::new(self.x, self.y, self.z)
    }

    #[inline]
    pub fn from_na(p: NaPoint3<f64>) -> Self {
        Self { x: p.x, y: p.y, z: p.z }
    }

    #[inline]
    pub fn distance(self, other: Self) -> f64 {
        let dx = self.x - other.x;
        let dy = self.y - other.y;
        let dz = self.z - other.z;
        (dx * dx + dy * dy + dz * dz).sqrt()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use approx::assert_abs_diff_eq;

    #[test]
    fn distance_axis_aligned() {
        let a = Point3::new(0.0, 0.0, 0.0);
        let b = Point3::new(3.0, 4.0, 0.0);
        assert_abs_diff_eq!(a.distance(b), 5.0, epsilon = 1e-12);
    }
}
