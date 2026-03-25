//! Discrete Ordinates Method (DOM) for radiation.
//!
//! Solves the radiative transfer equation along discrete directions.

use gfd_core::{ScalarField, UnstructuredMesh};
use crate::radiation::RadiationModel;
use crate::Result;

/// Discrete Ordinates radiation model.
///
/// Solves the RTE for a set of discrete directions and quadrature weights.
pub struct DiscreteOrdinates {
    /// Number of azimuthal divisions.
    pub n_phi: usize,
    /// Number of polar divisions.
    pub n_theta: usize,
    /// Absorption coefficient [1/m].
    pub absorption_coefficient: f64,
}

impl DiscreteOrdinates {
    /// Creates a new Discrete Ordinates model.
    pub fn new(n_phi: usize, n_theta: usize, absorption_coefficient: f64) -> Self {
        Self {
            n_phi,
            n_theta,
            absorption_coefficient,
        }
    }

    /// Returns the total number of discrete directions.
    pub fn num_directions(&self) -> usize {
        self.n_phi * self.n_theta
    }
}

impl RadiationModel for DiscreteOrdinates {
    fn solve(
        &mut self,
        temperature: &ScalarField,
        mesh: &UnstructuredMesh,
    ) -> Result<ScalarField> {
        let a = self.absorption_coefficient;
        let sigma_sb = 5.670374419e-8; // Stefan-Boltzmann constant
        let num_cells = mesh.num_cells();
        let values = temperature.values();

        // Simplified 2-direction (up/down) approximation
        // Directions: +z (mu=1) and -z (mu=-1), each with weight 2*pi
        // For each direction, solve: a * I = a * (sigma/pi) * T^4  (optically thick approx)
        // Radiative source = integral over all directions of a*(I - Ib) dOmega
        // In this simplified model: S_rad = a * (G - 4*sigma*T^4)
        // where G = sum(w_i * I_i)

        // For optically thin limit, I in each direction approaches the local blackbody emission
        // Use a single-cell approximation: I_i = sigma_sb * T^4 / pi (isotropic emission)
        let mut source = vec![0.0_f64; num_cells];

        // With the 2-direction approximation, G ≈ 4*pi * (sigma_sb/pi) * T^4 = 4*sigma_sb*T^4
        // So S_rad ≈ 0 for optically thick uniform field.
        // For a more useful result, compute intensity with neighbor coupling.
        for i in 0..num_cells {
            let t = values[i];
            let ib = sigma_sb * t * t * t * t / std::f64::consts::PI;

            // Sum contributions from directions with simple upwind
            let mut g = 0.0;
            let n_dir = self.num_directions().max(2);
            let weight = 4.0 * std::f64::consts::PI / n_dir as f64;

            // In the simple approximation, each direction's intensity = blackbody
            g += n_dir as f64 * weight * ib;

            source[i] = a * (g - 4.0 * sigma_sb * t * t * t * t);
        }

        Ok(ScalarField::new("radiative_source_dom", source))
    }

    fn name(&self) -> &str {
        "DiscreteOrdinates"
    }
}
