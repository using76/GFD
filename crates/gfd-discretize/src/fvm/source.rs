//! Source term linearization for FVM.

use serde::{Deserialize, Serialize};

/// A linearized source term: S(phi) = Sc + Sp * phi.
///
/// * `sc` is the constant part, added to the RHS: b_P += Sc * V
/// * `sp` is the coefficient of phi, added to the diagonal: a_P -= Sp * V
///   (negative sign because Sp should be <= 0 for stability)
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct LinearizedSource {
    /// Constant part of the source.
    pub sc: f64,
    /// Coefficient of phi in the source (should be <= 0 for stability).
    pub sp: f64,
}

/// Linearize a source term given its total value and an implicit coefficient.
///
/// Given S_total (the total source evaluated at the current state) and
/// an implicit coefficient, decomposes into:
///   Sc = S_total - implicit_coeff * phi  (but without phi, approximated)
///   Sp = implicit_coeff
///
/// In the simplest form:
///   Sc = total_source
///   Sp = implicit_coeff (should be negative for stability)
///
/// # Arguments
/// * `total_source` - Total source term value S_total.
/// * `implicit_coeff` - The implicit (linearized) part Sp.
pub fn linearize_source(total_source: f64, implicit_coeff: f64) -> LinearizedSource {
    LinearizedSource {
        sc: total_source,
        sp: implicit_coeff,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn linearize_basic() {
        let ls = linearize_source(100.0, -5.0);
        assert!((ls.sc - 100.0).abs() < 1e-12);
        assert!((ls.sp - (-5.0)).abs() < 1e-12);
    }
}
