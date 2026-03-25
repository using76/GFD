//! Time-dependent boundary condition profiles.

use serde::{Deserialize, Serialize};

/// A time-dependent profile for modulating boundary condition values.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TimeProfile {
    /// Constant value (time-independent).
    Constant(f64),

    /// Linear ramp from `start` to `end` over the time interval [t_start, t_end].
    Ramp {
        /// Value at t_start.
        start: f64,
        /// Value at t_end.
        end: f64,
        /// Ramp start time.
        t_start: f64,
        /// Ramp end time.
        t_end: f64,
    },

    /// Sinusoidal profile: amplitude * sin(2*pi*frequency*t + phase) + offset.
    Sinusoidal {
        /// Peak amplitude.
        amplitude: f64,
        /// Frequency [Hz].
        frequency: f64,
        /// Phase offset [radians].
        phase: f64,
        /// Baseline offset.
        offset: f64,
    },

    /// Piecewise-linear table of (time, value) pairs.
    Table(Vec<(f64, f64)>),

    /// Expression-based profile (evaluated via gfd-expression engine).
    Expression(String),
}

impl TimeProfile {
    /// Evaluates the profile at time `t`.
    pub fn evaluate(&self, t: f64) -> f64 {
        match self {
            TimeProfile::Constant(v) => *v,

            TimeProfile::Ramp { start, end, t_start, t_end } => {
                if t <= *t_start {
                    *start
                } else if t >= *t_end {
                    *end
                } else {
                    let frac = (t - t_start) / (t_end - t_start);
                    start + frac * (end - start)
                }
            }

            TimeProfile::Sinusoidal { amplitude, frequency, phase, offset } => {
                let omega = 2.0 * std::f64::consts::PI * frequency;
                offset + amplitude * (omega * t + phase).sin()
            }

            TimeProfile::Table(table) => {
                if table.is_empty() {
                    return 0.0;
                }
                if table.len() == 1 {
                    return table[0].1;
                }
                // Before first point: hold constant.
                if t <= table[0].0 {
                    return table[0].1;
                }
                // After last point: hold constant.
                if t >= table[table.len() - 1].0 {
                    return table[table.len() - 1].1;
                }
                // Find the bracketing interval and interpolate linearly.
                for i in 0..table.len() - 1 {
                    let (t0, v0) = table[i];
                    let (t1, v1) = table[i + 1];
                    if t >= t0 && t <= t1 {
                        let frac = (t - t0) / (t1 - t0);
                        return v0 + frac * (v1 - v0);
                    }
                }
                // Fallback (should not be reached).
                table[table.len() - 1].1
            }

            TimeProfile::Expression(_expr) => {
                // Full implementation would parse and evaluate via gfd-expression.
                // Placeholder returns 0.
                0.0
            }
        }
    }
}
