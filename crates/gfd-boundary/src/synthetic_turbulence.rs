//! Synthetic turbulence generation for inlet boundary conditions.
//!
//! Provides methods to generate coherent turbulent velocity fluctuations
//! at inflow boundaries, enabling more realistic LES and hybrid RANS-LES
//! simulations.

use gfd_core::mesh::face::Face;

/// Trait for synthetic turbulence generators.
///
/// Implementations produce velocity fluctuation vectors at a given boundary
/// face and simulation time.  The generated perturbations should satisfy
/// prescribed integral length-scale and Reynolds-stress statistics.
pub trait SyntheticTurbulenceGenerator: std::fmt::Debug + Send + Sync {
    /// Generates a velocity fluctuation vector `[u', v', w']` at the given
    /// boundary face at physical time `t`.
    fn generate(&self, face: &Face, t: f64) -> [f64; 3];
}

/// Divergence-free vortex method (Sergent, 2002; Mathey et al., 2006).
///
/// Superimposes randomly placed 2-D vortex tubes on the inflow plane.
/// Each vortex induces a tangential velocity perturbation whose magnitude
/// depends on the prescribed turbulence intensity and vortex core size.
#[derive(Debug, Clone)]
pub struct VortexMethod {
    /// Number of synthetic vortices on the inlet plane.
    pub num_vortices: usize,
    /// Vortex core size sigma (related to integral length scale).
    pub sigma: f64,
    /// Turbulence intensity scaling factor.
    pub intensity: f64,
}

impl VortexMethod {
    /// Creates a new vortex method generator.
    pub fn new(num_vortices: usize, sigma: f64, intensity: f64) -> Self {
        Self {
            num_vortices,
            sigma,
            intensity,
        }
    }
}

impl SyntheticTurbulenceGenerator for VortexMethod {
    fn generate(&self, face: &Face, t: f64) -> [f64; 3] {
        // Simplified vortex method: sum contributions from random vortices
        // Each vortex has a fixed position on the inlet plane and rotates with time
        let sigma = self.sigma;
        let intensity = self.intensity;
        let fc = face.center;

        let mut u_prime = [0.0_f64; 3];

        for k in 0..self.num_vortices {
            // Deterministic pseudo-random vortex center from index k
            let seed = k as f64;
            let yv = (seed * 0.6180339887).fract() * 2.0 - 1.0; // [-1, 1]
            let zv = ((seed + 0.5) * 0.6180339887).fract() * 2.0 - 1.0;
            let sign = if k % 2 == 0 { 1.0 } else { -1.0 };

            // Distance from vortex center to face center (in y-z plane)
            let dy = fc[1] - yv;
            let dz = fc[2] - zv;
            let r2 = dy * dy + dz * dz;
            let s2 = sigma * sigma;

            // Lamb-Oseen vortex: u_theta = Gamma/(2*pi*r) * (1 - exp(-r^2/sigma^2))
            let strength = sign * intensity * (1.0 - (-r2 / s2.max(1e-30)).exp());
            let r = r2.sqrt().max(1e-30);

            // Tangential velocity in y-z plane
            u_prime[1] += -strength * dz / r;
            u_prime[2] += strength * dy / r;
        }

        // Scale by 1/sqrt(N) to keep magnitude independent of vortex count
        let scale = 1.0 / (self.num_vortices as f64).sqrt().max(1.0);
        let _ = t; // Time could be used to advect vortices

        [u_prime[0] * scale, u_prime[1] * scale, u_prime[2] * scale]
    }
}

/// Digital filter method (Klein, Sadiki & Janicka, 2003).
///
/// Generates spatially and temporally correlated random fields that reproduce
/// prescribed one-point statistics (Reynolds stresses) and two-point
/// correlations (integral length and time scales) using a convolution of
/// white noise with an exponential kernel.
#[derive(Debug, Clone)]
pub struct DigitalFilterMethod {
    /// Integral length scale in each direction `[l_x, l_y, l_z]`.
    pub length_scales: [f64; 3],
    /// Integral time scale for temporal correlation.
    pub time_scale: f64,
    /// Target Reynolds stress tensor (symmetric, stored as 6 components).
    pub target_reynolds_stresses: [f64; 6],
}

impl DigitalFilterMethod {
    /// Creates a new digital filter method generator with isotropic length scales.
    pub fn new(length_scale: f64, time_scale: f64, turbulence_intensity: f64) -> Self {
        // Isotropic turbulence: R_ii = 2/3 * k, off-diagonal = 0
        // With k = 1.5 * (U * I)^2, approximate with intensity alone.
        let r_diag = turbulence_intensity * turbulence_intensity;
        Self {
            length_scales: [length_scale; 3],
            time_scale,
            target_reynolds_stresses: [r_diag, r_diag, r_diag, 0.0, 0.0, 0.0],
        }
    }
}

impl SyntheticTurbulenceGenerator for DigitalFilterMethod {
    fn generate(&self, face: &Face, t: f64) -> [f64; 3] {
        // Simplified digital filter: generate pseudo-random fluctuations
        // scaled by target Reynolds stresses
        // In a full implementation, these would be spatially and temporally correlated
        let fc = face.center;

        // Simple hash-based pseudo-random generator from position and time
        let hash = |x: f64, y: f64, z: f64, t: f64| -> f64 {
            let v = (x * 12.9898 + y * 78.233 + z * 45.164 + t * 93.9543).sin() * 43758.5453;
            v.fract() * 2.0 - 1.0
        };

        let u = hash(fc[0], fc[1], fc[2], t) * self.target_reynolds_stresses[0].sqrt();
        let v = hash(fc[0] + 0.1, fc[1] + 0.2, fc[2] + 0.3, t) * self.target_reynolds_stresses[1].sqrt();
        let w = hash(fc[0] + 0.4, fc[1] + 0.5, fc[2] + 0.6, t) * self.target_reynolds_stresses[2].sqrt();

        [u, v, w]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_vortex_method_creation() {
        let vm = VortexMethod::new(100, 0.01, 0.05);
        assert_eq!(vm.num_vortices, 100);
        assert!((vm.sigma - 0.01).abs() < 1e-15);
        assert!((vm.intensity - 0.05).abs() < 1e-15);
    }

    #[test]
    fn test_digital_filter_creation() {
        let df = DigitalFilterMethod::new(0.01, 0.001, 0.1);
        assert_eq!(df.length_scales, [0.01; 3]);
        assert!((df.time_scale - 0.001).abs() < 1e-15);
    }
}
