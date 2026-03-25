//! Field statistics: min, max, mean, RMS, standard deviation.

use gfd_core::ScalarField;
use gfd_core::field::Field;

/// Returns the minimum value of the scalar field.
///
/// Returns `f64::INFINITY` if the field is empty.
pub fn field_min(field: &ScalarField) -> f64 {
    field
        .values()
        .iter()
        .copied()
        .fold(f64::INFINITY, f64::min)
}

/// Returns the maximum value of the scalar field.
///
/// Returns `f64::NEG_INFINITY` if the field is empty.
pub fn field_max(field: &ScalarField) -> f64 {
    field
        .values()
        .iter()
        .copied()
        .fold(f64::NEG_INFINITY, f64::max)
}

/// Returns the arithmetic mean of the scalar field.
///
/// Returns 0.0 if the field is empty.
pub fn field_mean(field: &ScalarField) -> f64 {
    let n = field.len();
    if n == 0 {
        return 0.0;
    }
    let sum: f64 = field.values().iter().sum();
    sum / n as f64
}

/// Returns the root mean square (RMS) of the scalar field.
///
/// RMS = sqrt(mean(x^2))
///
/// Returns 0.0 if the field is empty.
pub fn field_rms(field: &ScalarField) -> f64 {
    let n = field.len();
    if n == 0 {
        return 0.0;
    }
    let sum_sq: f64 = field.values().iter().map(|v| v * v).sum();
    (sum_sq / n as f64).sqrt()
}

/// Returns the standard deviation of the scalar field.
///
/// Uses the population standard deviation: sqrt(mean((x - mean)^2))
///
/// Returns 0.0 if the field is empty.
pub fn field_std_dev(field: &ScalarField) -> f64 {
    let n = field.len();
    if n == 0 {
        return 0.0;
    }
    let mean = field_mean(field);
    let variance: f64 = field
        .values()
        .iter()
        .map(|v| {
            let diff = v - mean;
            diff * diff
        })
        .sum::<f64>()
        / n as f64;
    variance.sqrt()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_statistics_basic() {
        let field = ScalarField::new("test", vec![1.0, 2.0, 3.0, 4.0, 5.0]);

        assert_eq!(field_min(&field), 1.0);
        assert_eq!(field_max(&field), 5.0);
        assert_eq!(field_mean(&field), 3.0);

        let rms = field_rms(&field);
        let expected_rms = ((1.0 + 4.0 + 9.0 + 16.0 + 25.0) / 5.0_f64).sqrt();
        assert!((rms - expected_rms).abs() < 1e-12);

        let std = field_std_dev(&field);
        let expected_std = (2.0_f64).sqrt(); // variance = 2.0
        assert!((std - expected_std).abs() < 1e-12);
    }

    #[test]
    fn test_statistics_empty() {
        let field = ScalarField::new("empty", vec![]);
        assert_eq!(field_mean(&field), 0.0);
        assert_eq!(field_rms(&field), 0.0);
        assert_eq!(field_std_dev(&field), 0.0);
    }
}
