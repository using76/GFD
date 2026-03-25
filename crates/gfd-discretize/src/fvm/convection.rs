//! Convective flux discretization for FVM.

use gfd_core::ConvectionScheme;

/// Computes the convective flux coefficients for a face.
///
/// Given the mass flux `face_flux` (F = rho * u_f * A_f) through a face and the
/// chosen convection scheme, returns `(owner_coeff, neighbor_coeff)` that
/// contribute to the matrix equation:
///
///   a_P += owner_coeff
///   a_N += neighbor_coeff
///
/// # Arguments
/// * `face_flux` - The mass flux through the face (positive from owner to neighbor).
/// * `scheme` - The convection discretization scheme.
///
/// # Returns
/// `(owner_coeff, neighbor_coeff)` pair.
pub fn compute_convective_coefficient(
    face_flux: f64,
    scheme: &ConvectionScheme,
) -> (f64, f64) {
    match scheme {
        ConvectionScheme::FirstOrderUpwind => {
            // First-order upwind:
            //   If F > 0: flux = F * phi_P  =>  a_P += F, a_N += 0
            //   If F < 0: flux = F * phi_N  =>  a_P += 0, a_N += -F (because F is negative)
            //   If F == 0: no contribution
            if face_flux > 0.0 {
                (face_flux, 0.0)
            } else {
                (0.0, -face_flux)
            }
        }
        ConvectionScheme::Central => {
            // Central differencing:
            //   flux = F * 0.5 * (phi_P + phi_N)
            //   a_P += F/2,  a_N += F/2  (but neighbor coefficient sign convention
            //   means a_N contribution to the owner equation is -F/2).
            let half_f = 0.5 * face_flux;
            (half_f, -half_f)
        }
        // For schemes not yet fully implemented, fall back to first-order upwind.
        _ => {
            if face_flux > 0.0 {
                (face_flux, 0.0)
            } else {
                (0.0, -face_flux)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn upwind_positive_flux() {
        let (ap, an) = compute_convective_coefficient(2.5, &ConvectionScheme::FirstOrderUpwind);
        assert!((ap - 2.5).abs() < 1e-12);
        assert!((an - 0.0).abs() < 1e-12);
    }

    #[test]
    fn upwind_negative_flux() {
        let (ap, an) = compute_convective_coefficient(-3.0, &ConvectionScheme::FirstOrderUpwind);
        assert!((ap - 0.0).abs() < 1e-12);
        assert!((an - 3.0).abs() < 1e-12);
    }

    #[test]
    fn upwind_zero_flux() {
        let (ap, an) = compute_convective_coefficient(0.0, &ConvectionScheme::FirstOrderUpwind);
        assert!((ap - 0.0).abs() < 1e-12);
        assert!((an - 0.0).abs() < 1e-12);
    }

    #[test]
    fn central_positive_flux() {
        let (ap, an) = compute_convective_coefficient(4.0, &ConvectionScheme::Central);
        assert!((ap - 2.0).abs() < 1e-12);
        assert!((an - (-2.0)).abs() < 1e-12);
    }
}
