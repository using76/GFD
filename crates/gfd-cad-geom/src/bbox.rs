use serde::{Deserialize, Serialize};

use crate::Point3;

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct BoundingBox {
    pub min: Point3,
    pub max: Point3,
}

impl BoundingBox {
    pub const EMPTY: Self = Self {
        min: Point3 { x: f64::INFINITY, y: f64::INFINITY, z: f64::INFINITY },
        max: Point3 { x: f64::NEG_INFINITY, y: f64::NEG_INFINITY, z: f64::NEG_INFINITY },
    };

    pub fn from_point(p: Point3) -> Self {
        Self { min: p, max: p }
    }

    pub fn expand(&mut self, p: Point3) {
        self.min.x = self.min.x.min(p.x);
        self.min.y = self.min.y.min(p.y);
        self.min.z = self.min.z.min(p.z);
        self.max.x = self.max.x.max(p.x);
        self.max.y = self.max.y.max(p.y);
        self.max.z = self.max.z.max(p.z);
    }

    pub fn diagonal(&self) -> f64 {
        self.min.distance(self.max)
    }

    pub fn is_empty(&self) -> bool {
        self.min.x > self.max.x || self.min.y > self.max.y || self.min.z > self.max.z
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn expand_single() {
        let mut b = BoundingBox::from_point(Point3::new(1.0, 2.0, 3.0));
        b.expand(Point3::new(-1.0, 5.0, 0.0));
        assert_eq!(b.min, Point3::new(-1.0, 2.0, 0.0));
        assert_eq!(b.max, Point3::new(1.0, 5.0, 3.0));
    }

    #[test]
    fn empty_reports_empty() {
        assert!(BoundingBox::EMPTY.is_empty());
    }
}
