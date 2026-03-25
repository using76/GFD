//! Shape function definitions for finite elements.

/// Trait for element shape functions.
///
/// Implementations provide shape function values and their gradients
/// evaluated at a point in the parent (reference) coordinate system.
pub trait ShapeFunction {
    /// Evaluate shape functions at the given parent coordinates.
    ///
    /// # Arguments
    /// * `xi` - Parent coordinates [xi, eta, zeta] (unused components may be ignored).
    ///
    /// # Returns
    /// Vector of shape function values, one per node.
    fn evaluate(&self, xi: &[f64; 3]) -> Vec<f64>;

    /// Evaluate shape function gradients at the given parent coordinates.
    ///
    /// # Arguments
    /// * `xi` - Parent coordinates [xi, eta, zeta].
    ///
    /// # Returns
    /// Vector of gradient vectors [dN/dxi, dN/deta, dN/dzeta], one per node.
    fn gradient(&self, xi: &[f64; 3]) -> Vec<[f64; 3]>;
}

/// 3-node triangular element shape functions (linear, 2D).
///
/// Parent coordinates: xi in [0,1], eta in [0,1], xi + eta <= 1.
///   N1 = 1 - xi - eta
///   N2 = xi
///   N3 = eta
#[derive(Debug, Clone)]
pub struct Tri3ShapeFn;

impl ShapeFunction for Tri3ShapeFn {
    fn evaluate(&self, xi: &[f64; 3]) -> Vec<f64> {
        let (xi_val, eta_val) = (xi[0], xi[1]);
        vec![1.0 - xi_val - eta_val, xi_val, eta_val]
    }

    fn gradient(&self, _xi: &[f64; 3]) -> Vec<[f64; 3]> {
        // Gradients are constant for a linear triangle:
        //   dN1/dxi = -1,  dN1/deta = -1,  dN1/dzeta = 0
        //   dN2/dxi =  1,  dN2/deta =  0,  dN2/dzeta = 0
        //   dN3/dxi =  0,  dN3/deta =  1,  dN3/dzeta = 0
        vec![
            [-1.0, -1.0, 0.0],
            [1.0, 0.0, 0.0],
            [0.0, 1.0, 0.0],
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tri3_at_origin() {
        let tri = Tri3ShapeFn;
        let n = tri.evaluate(&[0.0, 0.0, 0.0]);
        assert!((n[0] - 1.0).abs() < 1e-12); // N1 = 1
        assert!((n[1] - 0.0).abs() < 1e-12); // N2 = 0
        assert!((n[2] - 0.0).abs() < 1e-12); // N3 = 0
    }

    #[test]
    fn tri3_at_node2() {
        let tri = Tri3ShapeFn;
        let n = tri.evaluate(&[1.0, 0.0, 0.0]);
        assert!((n[0] - 0.0).abs() < 1e-12);
        assert!((n[1] - 1.0).abs() < 1e-12);
        assert!((n[2] - 0.0).abs() < 1e-12);
    }

    #[test]
    fn tri3_at_node3() {
        let tri = Tri3ShapeFn;
        let n = tri.evaluate(&[0.0, 1.0, 0.0]);
        assert!((n[0] - 0.0).abs() < 1e-12);
        assert!((n[1] - 0.0).abs() < 1e-12);
        assert!((n[2] - 1.0).abs() < 1e-12);
    }

    #[test]
    fn tri3_partition_of_unity() {
        let tri = Tri3ShapeFn;
        let n = tri.evaluate(&[0.3, 0.2, 0.0]);
        let sum: f64 = n.iter().sum();
        assert!((sum - 1.0).abs() < 1e-12);
    }

    #[test]
    fn tri3_gradients_constant() {
        let tri = Tri3ShapeFn;
        let g1 = tri.gradient(&[0.0, 0.0, 0.0]);
        let g2 = tri.gradient(&[0.3, 0.2, 0.0]);
        for i in 0..3 {
            for j in 0..3 {
                assert!((g1[i][j] - g2[i][j]).abs() < 1e-12);
            }
        }
    }
}
