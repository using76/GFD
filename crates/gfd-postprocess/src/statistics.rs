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

/// Running time-averaged statistics accumulator.
///
/// Computes running mean and variance using Welford's online algorithm,
/// which is numerically stable even for long time series. This is useful
/// for computing time-averaged fields in unsteady simulations (e.g., LES/DES).
///
/// After N samples:
/// - `mean()` returns the arithmetic mean
/// - `variance()` returns the population variance
/// - `std_dev()` returns the population standard deviation
pub struct RunningStatistics {
    /// Number of samples accumulated.
    count: usize,
    /// Running mean for each cell.
    mean_values: Vec<f64>,
    /// Running M2 (sum of squared deviations from mean) for each cell.
    m2_values: Vec<f64>,
    /// Running minimum for each cell.
    min_values: Vec<f64>,
    /// Running maximum for each cell.
    max_values: Vec<f64>,
}

impl RunningStatistics {
    /// Creates a new running statistics accumulator for the given number of cells.
    pub fn new(num_cells: usize) -> Self {
        Self {
            count: 0,
            mean_values: vec![0.0; num_cells],
            m2_values: vec![0.0; num_cells],
            min_values: vec![f64::INFINITY; num_cells],
            max_values: vec![f64::NEG_INFINITY; num_cells],
        }
    }

    /// Returns the number of samples accumulated so far.
    pub fn count(&self) -> usize {
        self.count
    }

    /// Returns the number of cells being tracked.
    pub fn num_cells(&self) -> usize {
        self.mean_values.len()
    }

    /// Adds a new sample (one time step's field values) to the running statistics.
    ///
    /// Uses Welford's online algorithm for numerically stable mean/variance:
    ///   delta = x - mean_old
    ///   mean_new = mean_old + delta / n
    ///   delta2 = x - mean_new
    ///   M2_new = M2_old + delta * delta2
    pub fn add_sample(&mut self, field: &ScalarField) {
        let values = field.values();
        let n = values.len().min(self.mean_values.len());
        self.count += 1;
        let count = self.count as f64;

        for i in 0..n {
            let x = values[i];

            // Welford's algorithm
            let delta = x - self.mean_values[i];
            self.mean_values[i] += delta / count;
            let delta2 = x - self.mean_values[i];
            self.m2_values[i] += delta * delta2;

            // Update min/max
            if x < self.min_values[i] {
                self.min_values[i] = x;
            }
            if x > self.max_values[i] {
                self.max_values[i] = x;
            }
        }
    }

    /// Returns the current running mean as a ScalarField.
    pub fn mean(&self) -> ScalarField {
        ScalarField::new("time_mean", self.mean_values.clone())
    }

    /// Returns the mean values as a slice.
    pub fn mean_values(&self) -> &[f64] {
        &self.mean_values
    }

    /// Returns the current running population variance as a ScalarField.
    ///
    /// Variance = M2 / N
    pub fn variance(&self) -> ScalarField {
        if self.count == 0 {
            return ScalarField::new(
                "time_variance",
                vec![0.0; self.mean_values.len()],
            );
        }
        let n = self.count as f64;
        let var: Vec<f64> = self.m2_values.iter().map(|m2| m2 / n).collect();
        ScalarField::new("time_variance", var)
    }

    /// Returns the variance values as a slice (computed on the fly).
    pub fn variance_values(&self) -> Vec<f64> {
        if self.count == 0 {
            return vec![0.0; self.mean_values.len()];
        }
        let n = self.count as f64;
        self.m2_values.iter().map(|m2| m2 / n).collect()
    }

    /// Returns the current running standard deviation as a ScalarField.
    ///
    /// StdDev = sqrt(Variance) = sqrt(M2 / N)
    pub fn std_dev(&self) -> ScalarField {
        if self.count == 0 {
            return ScalarField::new(
                "time_std_dev",
                vec![0.0; self.mean_values.len()],
            );
        }
        let n = self.count as f64;
        let std: Vec<f64> = self
            .m2_values
            .iter()
            .map(|m2| (m2 / n).sqrt())
            .collect();
        ScalarField::new("time_std_dev", std)
    }

    /// Returns the running minimum as a ScalarField.
    pub fn min(&self) -> ScalarField {
        ScalarField::new("time_min", self.min_values.clone())
    }

    /// Returns the running maximum as a ScalarField.
    pub fn max(&self) -> ScalarField {
        ScalarField::new("time_max", self.max_values.clone())
    }

    /// Resets all accumulated statistics.
    pub fn reset(&mut self) {
        self.count = 0;
        self.mean_values.fill(0.0);
        self.m2_values.fill(0.0);
        self.min_values.fill(f64::INFINITY);
        self.max_values.fill(f64::NEG_INFINITY);
    }
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

    #[test]
    fn test_running_statistics_constant() {
        // A constant field should have zero variance
        let mut stats = RunningStatistics::new(3);
        for _ in 0..10 {
            let field = ScalarField::new("T", vec![42.0, 42.0, 42.0]);
            stats.add_sample(&field);
        }
        assert_eq!(stats.count(), 10);
        let mean = stats.mean();
        for v in mean.values() {
            assert!((v - 42.0).abs() < 1e-12);
        }
        let var = stats.variance();
        for v in var.values() {
            assert!(v.abs() < 1e-12, "Variance of constant should be 0, got {}", v);
        }
    }

    #[test]
    fn test_running_statistics_known_sequence() {
        // Feed values [1, 2, 3, 4, 5] to a single cell
        let mut stats = RunningStatistics::new(1);
        for val in [1.0, 2.0, 3.0, 4.0, 5.0] {
            let field = ScalarField::new("x", vec![val]);
            stats.add_sample(&field);
        }
        assert_eq!(stats.count(), 5);

        // Mean = 3.0
        let mean = stats.mean();
        assert!((mean.values()[0] - 3.0).abs() < 1e-12);

        // Population variance = ((1-3)^2 + (2-3)^2 + (3-3)^2 + (4-3)^2 + (5-3)^2) / 5
        // = (4 + 1 + 0 + 1 + 4) / 5 = 2.0
        let var = stats.variance();
        assert!(
            (var.values()[0] - 2.0).abs() < 1e-12,
            "Expected variance 2.0, got {}",
            var.values()[0]
        );

        // Std dev = sqrt(2) ~ 1.414
        let std = stats.std_dev();
        assert!((std.values()[0] - (2.0_f64).sqrt()).abs() < 1e-12);

        // Min/max
        let mn = stats.min();
        assert!((mn.values()[0] - 1.0).abs() < 1e-12);
        let mx = stats.max();
        assert!((mx.values()[0] - 5.0).abs() < 1e-12);
    }

    #[test]
    fn test_running_statistics_multiple_cells() {
        let mut stats = RunningStatistics::new(2);

        stats.add_sample(&ScalarField::new("f", vec![1.0, 10.0]));
        stats.add_sample(&ScalarField::new("f", vec![3.0, 20.0]));

        let mean = stats.mean();
        assert!((mean.values()[0] - 2.0).abs() < 1e-12);
        assert!((mean.values()[1] - 15.0).abs() < 1e-12);

        // Variance cell 0: ((1-2)^2 + (3-2)^2) / 2 = 1.0
        // Variance cell 1: ((10-15)^2 + (20-15)^2) / 2 = 25.0
        let var = stats.variance();
        assert!((var.values()[0] - 1.0).abs() < 1e-12);
        assert!((var.values()[1] - 25.0).abs() < 1e-12);
    }

    #[test]
    fn test_running_statistics_reset() {
        let mut stats = RunningStatistics::new(2);
        stats.add_sample(&ScalarField::new("f", vec![5.0, 10.0]));
        assert_eq!(stats.count(), 1);

        stats.reset();
        assert_eq!(stats.count(), 0);

        let mean = stats.mean();
        for v in mean.values() {
            assert!(v.abs() < 1e-12, "Mean should be 0 after reset");
        }
    }

    #[test]
    fn test_running_statistics_empty() {
        let stats = RunningStatistics::new(3);
        assert_eq!(stats.count(), 0);
        let var = stats.variance();
        for v in var.values() {
            assert!(v.abs() < 1e-12);
        }
    }
}
