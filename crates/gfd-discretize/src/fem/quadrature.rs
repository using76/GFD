//! Quadrature rules for numerical integration on finite elements.

use serde::{Deserialize, Serialize};

/// A single quadrature (integration) point.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuadraturePoint {
    /// Position in parent coordinates [xi, eta, zeta].
    pub position: [f64; 3],
    /// Integration weight.
    pub weight: f64,
}

/// A quadrature rule consisting of multiple integration points.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuadratureRule {
    /// The quadrature points and their weights.
    pub points: Vec<QuadraturePoint>,
}

/// Returns a 1-point Gauss quadrature rule for triangles.
///
/// This rule is exact for polynomials of degree 1 (linear).
/// Single point at the centroid (1/3, 1/3) with weight 1/2
/// (the area of the reference triangle).
pub fn gauss_triangle_1point() -> QuadratureRule {
    QuadratureRule {
        points: vec![QuadraturePoint {
            position: [1.0 / 3.0, 1.0 / 3.0, 0.0],
            weight: 0.5,
        }],
    }
}

/// Returns a 3-point Gauss quadrature rule for triangles.
///
/// This rule is exact for polynomials of degree 2 (quadratic).
/// Points at (1/6, 1/6), (2/3, 1/6), (1/6, 2/3) each with weight 1/6.
pub fn gauss_triangle_3point() -> QuadratureRule {
    let w = 1.0 / 6.0;
    QuadratureRule {
        points: vec![
            QuadraturePoint {
                position: [1.0 / 6.0, 1.0 / 6.0, 0.0],
                weight: w,
            },
            QuadraturePoint {
                position: [2.0 / 3.0, 1.0 / 6.0, 0.0],
                weight: w,
            },
            QuadraturePoint {
                position: [1.0 / 6.0, 2.0 / 3.0, 0.0],
                weight: w,
            },
        ],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn one_point_weight_sum() {
        let rule = gauss_triangle_1point();
        let total: f64 = rule.points.iter().map(|p| p.weight).sum();
        // Reference triangle area = 0.5
        assert!((total - 0.5).abs() < 1e-12);
    }

    #[test]
    fn three_point_weight_sum() {
        let rule = gauss_triangle_3point();
        let total: f64 = rule.points.iter().map(|p| p.weight).sum();
        // Reference triangle area = 0.5
        assert!((total - 0.5).abs() < 1e-12);
    }

    #[test]
    fn three_point_count() {
        let rule = gauss_triangle_3point();
        assert_eq!(rule.points.len(), 3);
    }
}
