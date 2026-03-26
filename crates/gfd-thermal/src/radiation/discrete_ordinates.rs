//! Discrete Ordinates Method (DOM) for radiation.
//!
//! Solves the radiative transfer equation along discrete directions
//! using the S4 level-symmetric quadrature set.

use gfd_core::{ScalarField, UnstructuredMesh};
use crate::radiation::RadiationModel;
use crate::Result;

/// A single discrete direction with its weight.
#[derive(Debug, Clone, Copy)]
struct Ordinate {
    /// Direction cosines (mu, eta, xi) - unit direction vector.
    direction: [f64; 3],
    /// Quadrature weight (sum of all weights = 4*pi).
    weight: f64,
}

/// Discrete Ordinates radiation model.
///
/// Solves the RTE for a set of discrete directions and quadrature weights.
/// Supports S2 (8 directions) and S4 (24 directions) quadrature sets.
pub struct DiscreteOrdinates {
    /// Number of azimuthal divisions.
    pub n_phi: usize,
    /// Number of polar divisions.
    pub n_theta: usize,
    /// Absorption coefficient [1/m].
    pub absorption_coefficient: f64,
    /// Precomputed ordinates (directions + weights).
    ordinates: Vec<Ordinate>,
}

impl DiscreteOrdinates {
    /// Creates a new Discrete Ordinates model.
    pub fn new(n_phi: usize, n_theta: usize, absorption_coefficient: f64) -> Self {
        let ordinates = if n_phi >= 4 && n_theta >= 4 {
            Self::build_s4_quadrature()
        } else {
            Self::build_s2_quadrature()
        };
        Self {
            n_phi,
            n_theta,
            absorption_coefficient,
            ordinates,
        }
    }

    /// Creates an S4 model (24 directions, standard for 3D).
    pub fn new_s4(absorption_coefficient: f64) -> Self {
        Self {
            n_phi: 4,
            n_theta: 4,
            absorption_coefficient,
            ordinates: Self::build_s4_quadrature(),
        }
    }

    /// Returns the total number of discrete directions.
    pub fn num_directions(&self) -> usize {
        self.ordinates.len()
    }

    /// Builds the S2 level-symmetric quadrature set (8 directions in 3D).
    ///
    /// S2: one direction cosine value mu_1 = 1/sqrt(3), weight = 4*pi/8.
    /// All 8 combinations of (+/- mu, +/- mu, +/- mu).
    fn build_s2_quadrature() -> Vec<Ordinate> {
        let mu = 1.0 / 3.0_f64.sqrt();
        let w = 4.0 * std::f64::consts::PI / 8.0; // = pi/2

        let signs: [[f64; 3]; 8] = [
            [ 1.0,  1.0,  1.0],
            [ 1.0,  1.0, -1.0],
            [ 1.0, -1.0,  1.0],
            [ 1.0, -1.0, -1.0],
            [-1.0,  1.0,  1.0],
            [-1.0,  1.0, -1.0],
            [-1.0, -1.0,  1.0],
            [-1.0, -1.0, -1.0],
        ];

        signs.iter().map(|s| Ordinate {
            direction: [s[0] * mu, s[1] * mu, s[2] * mu],
            weight: w,
        }).collect()
    }

    /// Builds the S4 level-symmetric quadrature set (24 directions in 3D).
    ///
    /// S4 has two direction cosine values:
    ///   mu_1 = 0.2958759  (appears twice per octant direction triple)
    ///   mu_2 = 0.9082483  (appears once per octant direction triple)
    ///
    /// In each octant, there are 3 permutations of (mu_2, mu_1, mu_1),
    /// giving 3 directions * 8 octants = 24 total directions.
    ///
    /// Weights: w_1 = pi/6 for each direction (sum = 24 * pi/6 = 4*pi).
    fn build_s4_quadrature() -> Vec<Ordinate> {
        // S4 level-symmetric direction cosine values
        // From Fiveland (1988) and Modest "Radiative Heat Transfer"
        let mu_1: f64 = 0.2958759;
        let mu_2: f64 = 0.9082483;
        let w = std::f64::consts::PI / 6.0;

        let mut ordinates = Vec::with_capacity(24);

        // 8 octants
        for sx in &[1.0_f64, -1.0] {
            for sy in &[1.0_f64, -1.0] {
                for sz in &[1.0_f64, -1.0] {
                    // 3 permutations of (mu_2, mu_1, mu_1) per octant
                    ordinates.push(Ordinate {
                        direction: [sx * mu_2, sy * mu_1, sz * mu_1],
                        weight: w,
                    });
                    ordinates.push(Ordinate {
                        direction: [sx * mu_1, sy * mu_2, sz * mu_1],
                        weight: w,
                    });
                    ordinates.push(Ordinate {
                        direction: [sx * mu_1, sy * mu_1, sz * mu_2],
                        weight: w,
                    });
                }
            }
        }

        ordinates
    }

    /// Solves the RTE for a single ordinate direction using a cell-by-cell
    /// sweep with simple upwind differencing.
    ///
    /// For each cell, the RTE along direction s is:
    ///   dI/ds = kappa * (sigma*T^4/pi - I)
    ///
    /// Using an optically-thin cell approximation (cell optical thickness
    /// tau = kappa * L_cell << 1), the intensity leaving the cell is:
    ///   I_out = I_in * exp(-tau) + I_b * (1 - exp(-tau))
    ///
    /// where I_b = sigma * T^4 / pi is the blackbody intensity.
    fn solve_single_direction(
        &self,
        direction: &[f64; 3],
        temperature: &ScalarField,
        mesh: &UnstructuredMesh,
    ) -> Vec<f64> {
        let kappa = self.absorption_coefficient;
        let sigma_sb = 5.670374419e-8;
        let num_cells = mesh.num_cells();
        let values = temperature.values();

        // Intensity field for this direction
        let mut intensity = vec![0.0_f64; num_cells];

        // Initialize with blackbody emission (good initial guess)
        for i in 0..num_cells {
            let t = values[i];
            intensity[i] = sigma_sb * t * t * t * t / std::f64::consts::PI;
        }

        // Iterative sweep: repeat a few times for convergence
        for _sweep in 0..4 {
            let mut intensity_new = vec![0.0_f64; num_cells];

            for cell_id in 0..num_cells {
                let t = values[cell_id];
                let ib = sigma_sb * t * t * t * t / std::f64::consts::PI;

                // Characteristic cell length in the direction of propagation
                let vol = mesh.cells[cell_id].volume;
                let char_length = vol.cbrt();

                // Optical thickness of the cell
                let tau = kappa * char_length;

                // Compute incoming intensity from upwind neighbors
                let mut i_in = 0.0;
                let mut upwind_count = 0.0;

                for &face_id in &mesh.cells[cell_id].faces {
                    let face = &mesh.faces[face_id];
                    // Dot product of direction with outward normal
                    let dot = direction[0] * face.normal[0]
                        + direction[1] * face.normal[1]
                        + direction[2] * face.normal[2];

                    // Determine if owner or neighbor
                    let is_owner = face.owner_cell == cell_id;

                    // For the owner, the normal points outward.
                    // For the neighbor, the normal points inward (toward the owner).
                    let effective_dot = if is_owner { dot } else { -dot };

                    // Upwind face: radiation enters the cell (effective_dot < 0)
                    if effective_dot < 0.0 {
                        if let Some(neighbor) = face.neighbor_cell {
                            let other = if is_owner { neighbor } else { face.owner_cell };
                            i_in += intensity[other];
                            upwind_count += 1.0;
                        } else {
                            // Boundary: use blackbody emission of the cell as approximation
                            i_in += ib;
                            upwind_count += 1.0;
                        }
                    }
                }

                if upwind_count > 0.0 {
                    i_in /= upwind_count;
                } else {
                    i_in = ib;
                }

                // Exponential attenuation through cell
                if tau > 1e-10 {
                    let exp_neg_tau = (-tau).exp();
                    intensity_new[cell_id] = i_in * exp_neg_tau + ib * (1.0 - exp_neg_tau);
                } else {
                    // Optically thin: intensity is mostly the incoming intensity
                    // with a small emission correction
                    intensity_new[cell_id] = i_in + tau * (ib - i_in);
                }
            }

            intensity = intensity_new;
        }

        intensity
    }
}

impl RadiationModel for DiscreteOrdinates {
    fn solve(
        &mut self,
        temperature: &ScalarField,
        mesh: &UnstructuredMesh,
    ) -> Result<ScalarField> {
        let kappa = self.absorption_coefficient;
        let sigma_sb = 5.670374419e-8;
        let num_cells = mesh.num_cells();
        let values = temperature.values();

        // Solve the RTE for each discrete direction
        let mut g = vec![0.0_f64; num_cells]; // Incident radiation G = sum(w_i * I_i)

        for ordinate in &self.ordinates.clone() {
            let intensity = self.solve_single_direction(
                &ordinate.direction,
                temperature,
                mesh,
            );

            // Accumulate weighted intensity into incident radiation
            for i in 0..num_cells {
                g[i] += ordinate.weight * intensity[i];
            }
        }

        // Compute radiative source: -div(q_r) = kappa * (4*sigma*T^4 - G)
        // Note: this is the source term for the energy equation.
        // When emission > absorption (hot region), this is negative (cooling).
        // When absorption > emission (cold region), this is positive (heating).
        let mut source = vec![0.0_f64; num_cells];
        for i in 0..num_cells {
            let t = values[i];
            let emission = 4.0 * sigma_sb * t * t * t * t;
            // S_rad = kappa * (G - 4*sigma*T^4)
            // Positive G > emission means net absorption (heating)
            source[i] = kappa * (g[i] - emission);
        }

        Ok(ScalarField::new("radiative_source_dom", source))
    }

    fn name(&self) -> &str {
        "DiscreteOrdinates"
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gfd_core::mesh::cell::Cell;
    use gfd_core::mesh::face::Face;
    use gfd_core::mesh::unstructured::{BoundaryPatch, UnstructuredMesh};

    /// Creates a simple 1D mesh of `nx` cells for testing.
    fn make_1d_mesh(nx: usize, length: f64) -> UnstructuredMesh {
        let dx = length / nx as f64;
        let cross_area = 1.0;

        let mut cells = Vec::with_capacity(nx);
        for i in 0..nx {
            let cx = (i as f64 + 0.5) * dx;
            cells.push(Cell::new(i, vec![], vec![], dx, [cx, 0.5, 0.5]));
        }

        let mut faces: Vec<Face> = Vec::new();
        let mut face_id = 0usize;

        let left_face_id = face_id;
        faces.push(Face::new(face_id, vec![], 0, None, cross_area, [-1.0, 0.0, 0.0], [0.0, 0.5, 0.5]));
        face_id += 1;

        for i in 0..nx - 1 {
            let fx = (i as f64 + 1.0) * dx;
            faces.push(Face::new(face_id, vec![], i, Some(i + 1), cross_area, [1.0, 0.0, 0.0], [fx, 0.5, 0.5]));
            cells[i].faces.push(face_id);
            cells[i + 1].faces.push(face_id);
            face_id += 1;
        }

        let right_face_id = face_id;
        faces.push(Face::new(face_id, vec![], nx - 1, None, cross_area, [1.0, 0.0, 0.0], [length, 0.5, 0.5]));

        cells[0].faces.insert(0, left_face_id);
        cells[nx - 1].faces.push(right_face_id);

        let boundary_patches = vec![
            BoundaryPatch::new("left", vec![left_face_id]),
            BoundaryPatch::new("right", vec![right_face_id]),
        ];

        UnstructuredMesh::from_components(vec![], faces, cells, boundary_patches)
    }

    #[test]
    fn s2_has_8_directions() {
        let dom = DiscreteOrdinates::new(2, 2, 1.0);
        assert_eq!(dom.num_directions(), 8, "S2 should have 8 directions");
    }

    #[test]
    fn s4_has_24_directions() {
        let dom = DiscreteOrdinates::new_s4(1.0);
        assert_eq!(dom.num_directions(), 24, "S4 should have 24 directions");
    }

    #[test]
    fn s4_quadrature_weights_sum_to_4pi() {
        let ordinates = DiscreteOrdinates::build_s4_quadrature();
        let total_weight: f64 = ordinates.iter().map(|o| o.weight).sum();
        let expected = 4.0 * std::f64::consts::PI;
        assert!(
            (total_weight - expected).abs() / expected < 1e-10,
            "S4 weights should sum to 4*pi={}, got {}",
            expected,
            total_weight,
        );
    }

    #[test]
    fn s2_quadrature_weights_sum_to_4pi() {
        let ordinates = DiscreteOrdinates::build_s2_quadrature();
        let total_weight: f64 = ordinates.iter().map(|o| o.weight).sum();
        let expected = 4.0 * std::f64::consts::PI;
        assert!(
            (total_weight - expected).abs() / expected < 1e-10,
            "S2 weights should sum to 4*pi={}, got {}",
            expected,
            total_weight,
        );
    }

    #[test]
    fn s4_directions_are_unit_vectors() {
        let ordinates = DiscreteOrdinates::build_s4_quadrature();
        for (i, o) in ordinates.iter().enumerate() {
            let mag = (o.direction[0] * o.direction[0]
                + o.direction[1] * o.direction[1]
                + o.direction[2] * o.direction[2])
            .sqrt();
            assert!(
                (mag - 1.0).abs() < 1e-6,
                "Direction {} should be unit vector, magnitude = {}",
                i,
                mag,
            );
        }
    }

    #[test]
    fn s4_directions_symmetric() {
        // Check that directions are symmetric: for every (a,b,c) there's (-a,b,c) etc.
        let ordinates = DiscreteOrdinates::build_s4_quadrature();
        for o in &ordinates {
            let neg = [-o.direction[0], -o.direction[1], -o.direction[2]];
            let found = ordinates.iter().any(|other| {
                (other.direction[0] - neg[0]).abs() < 1e-10
                    && (other.direction[1] - neg[1]).abs() < 1e-10
                    && (other.direction[2] - neg[2]).abs() < 1e-10
            });
            assert!(
                found,
                "Direction [{:.4}, {:.4}, {:.4}] should have opposite direction",
                o.direction[0], o.direction[1], o.direction[2],
            );
        }
    }

    #[test]
    fn uniform_temperature_gives_near_zero_source() {
        // In a uniform temperature field, emission = absorption everywhere,
        // so the net radiative source should be approximately zero.
        let mesh = make_1d_mesh(5, 1.0);
        let temp = ScalarField::new("temperature", vec![1000.0; 5]);

        let mut dom = DiscreteOrdinates::new_s4(0.1);
        let source = dom.solve(&temp, &mesh).unwrap();

        for (i, &s) in source.values().iter().enumerate() {
            assert!(
                s.abs() < 1e-2,
                "Cell {}: uniform field source should be ~0, got {}",
                i,
                s,
            );
        }
    }

    #[test]
    fn hot_cell_loses_energy() {
        // A hot cell surrounded by cold cells should lose energy (negative source).
        let mesh = make_1d_mesh(5, 1.0);
        let mut temps = vec![300.0; 5];
        temps[2] = 1000.0; // hot cell in the middle

        let temp = ScalarField::new("temperature", temps);
        let mut dom = DiscreteOrdinates::new_s4(1.0);
        let source = dom.solve(&temp, &mesh).unwrap();

        let s_hot = source.values()[2];
        // The hot cell emits more than it absorbs -> net cooling (negative source)
        assert!(
            s_hot < 0.0,
            "Hot cell should have negative source (cooling), got {}",
            s_hot,
        );
    }

    #[test]
    fn cold_cell_gains_energy() {
        // A cold cell surrounded by hot cells should gain energy (positive source).
        let mesh = make_1d_mesh(5, 1.0);
        let mut temps = vec![1000.0; 5];
        temps[2] = 300.0; // cold cell in the middle

        let temp = ScalarField::new("temperature", temps);
        let mut dom = DiscreteOrdinates::new_s4(1.0);
        let source = dom.solve(&temp, &mesh).unwrap();

        let s_cold = source.values()[2];
        // The cold cell absorbs more than it emits -> net heating (positive source)
        assert!(
            s_cold > 0.0,
            "Cold cell should have positive source (heating), got {}",
            s_cold,
        );
    }

    #[test]
    fn higher_absorption_stronger_source() {
        // Higher absorption coefficient should produce stronger radiative sources.
        let mesh = make_1d_mesh(5, 1.0);
        let mut temps = vec![300.0; 5];
        temps[2] = 1000.0;
        let temp = ScalarField::new("temperature", temps);

        let mut dom_low = DiscreteOrdinates::new_s4(0.1);
        let source_low = dom_low.solve(&temp, &mesh).unwrap();

        let mut dom_high = DiscreteOrdinates::new_s4(1.0);
        let source_high = dom_high.solve(&temp, &mesh).unwrap();

        // Higher kappa should produce stronger magnitude source for the hot cell
        assert!(
            source_high.values()[2].abs() > source_low.values()[2].abs(),
            "Higher kappa source {} should be stronger than lower kappa source {}",
            source_high.values()[2],
            source_low.values()[2],
        );
    }

    #[test]
    fn s4_more_directions_than_default_s2() {
        let dom_s2 = DiscreteOrdinates::new(2, 2, 1.0);
        let dom_s4 = DiscreteOrdinates::new_s4(1.0);
        assert!(
            dom_s4.num_directions() > dom_s2.num_directions(),
            "S4 ({}) should have more directions than S2 ({})",
            dom_s4.num_directions(),
            dom_s2.num_directions(),
        );
    }
}
