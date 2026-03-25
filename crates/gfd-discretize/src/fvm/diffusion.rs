//! Diffusive flux discretization for FVM.

/// Computes the diffusive flux coefficient for a face.
///
/// The diffusive contribution across a face is:
///   D = gamma * A_f / d_PN
///
/// where:
/// * `gamma` - diffusion coefficient (e.g. thermal conductivity, viscosity)
/// * `area`  - face area A_f
/// * `distance` - distance between owner and neighbor cell centers d_PN
///
/// This coefficient contributes:
///   a_P += D  (owner diagonal)
///   a_N  = D  (off-diagonal, to be subtracted from owner equation)
///   flux = D * (phi_N - phi_P)
///
/// # Panics
/// Panics if `distance` is zero.
pub fn compute_diffusive_coefficient(gamma: f64, area: f64, distance: f64) -> f64 {
    assert!(
        distance > 0.0,
        "Distance between cell centers must be positive, got {}",
        distance
    );
    gamma * area / distance
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn basic_diffusion_coefficient() {
        let d = compute_diffusive_coefficient(1.0, 2.0, 0.5);
        assert!((d - 4.0).abs() < 1e-12);
    }

    #[test]
    fn scaled_diffusion() {
        let d = compute_diffusive_coefficient(0.01, 1.0, 0.1);
        assert!((d - 0.1).abs() < 1e-12);
    }

    #[test]
    #[should_panic]
    fn zero_distance_panics() {
        compute_diffusive_coefficient(1.0, 1.0, 0.0);
    }
}
