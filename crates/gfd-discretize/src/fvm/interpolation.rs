//! Face value interpolation for FVM.

/// Interpolate a scalar value to a face using linear (distance-weighted) interpolation.
///
/// phi_f = weight * phi_owner + (1 - weight) * phi_neighbor
///
/// where `weight` is the geometric interpolation factor, typically
/// `d_Nf / d_PN` (distance from neighbor to face / distance between centers).
///
/// # Arguments
/// * `phi_owner` - Field value at the owner cell center.
/// * `phi_neighbor` - Field value at the neighbor cell center.
/// * `weight` - Interpolation weight for the owner cell (0..1).
///
/// # Returns
/// Interpolated value at the face.
pub fn interpolate_to_face(phi_owner: f64, phi_neighbor: f64, weight: f64) -> f64 {
    weight * phi_owner + (1.0 - weight) * phi_neighbor
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn midpoint_interpolation() {
        let val = interpolate_to_face(10.0, 20.0, 0.5);
        assert!((val - 15.0).abs() < 1e-12);
    }

    #[test]
    fn owner_weighted() {
        let val = interpolate_to_face(10.0, 20.0, 1.0);
        assert!((val - 10.0).abs() < 1e-12);
    }

    #[test]
    fn neighbor_weighted() {
        let val = interpolate_to_face(10.0, 20.0, 0.0);
        assert!((val - 20.0).abs() < 1e-12);
    }

    #[test]
    fn asymmetric_weight() {
        let val = interpolate_to_face(0.0, 100.0, 0.25);
        assert!((val - 75.0).abs() < 1e-12);
    }
}
