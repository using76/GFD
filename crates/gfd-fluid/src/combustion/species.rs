//! Species transport solver for combustion simulations.
//!
//! Solves the transport equations for individual chemical species
//! mass fractions, including convection, diffusion, and reaction source terms.

use gfd_core::{ScalarField, VectorField, UnstructuredMesh};
use crate::Result;

/// Solver for species transport equations in reacting flows.
///
/// Solves the species mass fraction equation for each species i:
/// d(rho*Y_i)/dt + div(rho*U*Y_i) = div(rho*D_i*grad(Y_i)) + omega_i
///
/// where Y_i is the mass fraction, D_i the diffusion coefficient,
/// and omega_i the reaction source term for species i.
pub struct SpeciesTransportSolver {
    /// Mass fraction fields for each species.
    pub mass_fractions: Vec<ScalarField>,
    /// Species names.
    pub species_names: Vec<String>,
    /// Diffusion coefficients for each species [m^2/s].
    pub diffusivities: Vec<f64>,
    /// Schmidt number for turbulent diffusion (default 0.7).
    pub turbulent_schmidt: f64,
}

impl SpeciesTransportSolver {
    /// Creates a new species transport solver.
    pub fn new(
        species_names: Vec<String>,
        diffusivities: Vec<f64>,
        num_cells: usize,
    ) -> Self {
        assert_eq!(species_names.len(), diffusivities.len(),
            "Number of species names must match number of diffusivities");

        let mass_fractions = species_names
            .iter()
            .map(|name| ScalarField::zeros(&format!("Y_{}", name), num_cells))
            .collect();

        Self {
            mass_fractions,
            species_names,
            diffusivities,
            turbulent_schmidt: 0.7,
        }
    }

    /// Returns the number of species.
    pub fn num_species(&self) -> usize {
        self.species_names.len()
    }

    /// Solves the species transport equations for one time step.
    ///
    /// For each species i (except the last, which is computed from
    /// the constraint sum(Y_i) = 1):
    /// 1. Assemble the convection-diffusion equation
    /// 2. Add reaction source terms
    /// 3. Solve the linear system
    /// 4. Bound mass fractions to [0, 1]
    pub fn solve_species(
        &mut self,
        velocity: &VectorField,
        density: &ScalarField,
        mesh: &UnstructuredMesh,
        dt: f64,
        reaction_rates: &[ScalarField],
    ) -> Result<()> {
        let n = mesh.num_cells();
        let num_species = self.mass_fractions.len();
        if num_species == 0 {
            return Ok(());
        }

        // Solve for each species except the last (constraint: sum(Y_i) = 1)
        let solve_count = if num_species > 1 { num_species - 1 } else { num_species };

        for sp in 0..solve_count {
            let y_old = self.mass_fractions[sp].values().to_vec();
            let mut y_new = y_old.clone();
            let diff = self.diffusivities[sp];

            // Accumulate net flux per cell
            let mut net_flux = vec![0.0; n];

            for face in &mesh.faces {
                let owner = face.owner_cell;

                if let Some(neighbor) = face.neighbor_cell {
                    // Face velocity and density (interpolated)
                    let vel_o = velocity.values()[owner];
                    let vel_n = velocity.values()[neighbor];
                    let rho_o = density.values()[owner];
                    let rho_n = density.values()[neighbor];
                    let rho_f = 0.5 * (rho_o + rho_n);
                    let u_f = [
                        0.5 * (vel_o[0] + vel_n[0]),
                        0.5 * (vel_o[1] + vel_n[1]),
                        0.5 * (vel_o[2] + vel_n[2]),
                    ];

                    // Convective mass flux
                    let mass_flux = rho_f
                        * (u_f[0] * face.normal[0]
                            + u_f[1] * face.normal[1]
                            + u_f[2] * face.normal[2])
                        * face.area;

                    // Upwind for convective flux
                    let y_f = if mass_flux >= 0.0 {
                        y_old[owner]
                    } else {
                        y_old[neighbor]
                    };
                    let conv_flux = mass_flux * y_f;

                    // Diffusive flux: rho * D * (Y_n - Y_o) / dist * area
                    let co = mesh.cells[owner].center;
                    let cn = mesh.cells[neighbor].center;
                    let dist = ((co[0] - cn[0]).powi(2)
                        + (co[1] - cn[1]).powi(2)
                        + (co[2] - cn[2]).powi(2))
                    .sqrt()
                    .max(1e-30);
                    let diff_flux =
                        rho_f * diff * (y_old[neighbor] - y_old[owner]) / dist * face.area;

                    // Total flux: convective (out of owner) - diffusive (into owner)
                    net_flux[owner] += conv_flux - diff_flux;
                    net_flux[neighbor] -= conv_flux - diff_flux;
                } else {
                    // Boundary: zero-gradient (no diffusive flux, convective uses owner value)
                    let vel_o = velocity.values()[owner];
                    let rho_o = density.values()[owner];
                    let mass_flux = rho_o
                        * (vel_o[0] * face.normal[0]
                            + vel_o[1] * face.normal[1]
                            + vel_o[2] * face.normal[2])
                        * face.area;
                    let conv_flux = mass_flux * y_old[owner];
                    net_flux[owner] += conv_flux;
                }
            }

            // Time integration: explicit Euler
            // d(rho*Y)/dt = -net_flux/V + omega
            for i in 0..n {
                let vol = mesh.cells[i].volume;
                let rho_i = density.values()[i];
                if vol > 0.0 && rho_i > 0.0 {
                    let source = if sp < reaction_rates.len() {
                        reaction_rates[sp].values()[i]
                    } else {
                        0.0
                    };
                    y_new[i] = y_old[i] + dt / (rho_i * vol) * (-net_flux[i] + source * vol);
                }
                y_new[i] = y_new[i].clamp(0.0, 1.0);
            }

            self.mass_fractions[sp].values_mut().copy_from_slice(&y_new);
        }

        // Compute last species from constraint: Y_N = 1 - sum(Y_i, i=0..N-2)
        if num_species > 1 {
            for cell in 0..n {
                let sum: f64 = (0..num_species - 1)
                    .map(|sp| self.mass_fractions[sp].values()[cell])
                    .sum();
                let _ = self.mass_fractions[num_species - 1].set(cell, (1.0 - sum).clamp(0.0, 1.0));
            }
        }

        // Renormalize
        self.normalize_mass_fractions();

        Ok(())
    }

    /// Ensures that mass fractions sum to 1 and are bounded in [0, 1].
    pub fn normalize_mass_fractions(&mut self) {
        let num_cells = if let Some(first) = self.mass_fractions.first() {
            first.values().len()
        } else {
            return;
        };

        for cell in 0..num_cells {
            // Clamp all to [0, 1]
            for mf in self.mass_fractions.iter_mut() {
                let val = mf.values_mut();
                val[cell] = val[cell].clamp(0.0, 1.0);
            }

            // Normalize so sum = 1
            let sum: f64 = self.mass_fractions.iter().map(|mf| mf.values()[cell]).sum();
            if sum > 0.0 {
                for mf in self.mass_fractions.iter_mut() {
                    mf.values_mut()[cell] /= sum;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gfd_core::mesh::cell::Cell;
    use gfd_core::mesh::face::Face;
    use gfd_core::mesh::unstructured::{BoundaryPatch, UnstructuredMesh};

    /// Creates a simple 1D mesh with 5 cells aligned along x-axis.
    /// Each cell is 1x1x1, domain is [0,5]x[0,1]x[0,1].
    fn make_1d_mesh(n: usize) -> UnstructuredMesh {
        let dx = 1.0;
        let dy = 1.0;
        let dz = 1.0;

        let mut cells = Vec::new();
        for i in 0..n {
            let cx = (i as f64 + 0.5) * dx;
            cells.push(Cell::new(i, vec![], vec![], dx * dy * dz, [cx, 0.5 * dy, 0.5 * dz]));
        }

        let mut faces = Vec::new();
        let mut fid = 0;
        let mut left_faces = Vec::new();
        let mut right_faces = Vec::new();

        // Internal x-normal faces
        for i in 0..(n - 1) {
            let fx = (i as f64 + 1.0) * dx;
            faces.push(Face::new(
                fid, vec![], i, Some(i + 1), dy * dz,
                [1.0, 0.0, 0.0], [fx, 0.5 * dy, 0.5 * dz],
            ));
            cells[i].faces.push(fid);
            cells[i + 1].faces.push(fid);
            fid += 1;
        }

        // Left boundary (x=0)
        faces.push(Face::new(
            fid, vec![], 0, None, dy * dz,
            [-1.0, 0.0, 0.0], [0.0, 0.5 * dy, 0.5 * dz],
        ));
        cells[0].faces.push(fid);
        left_faces.push(fid);
        fid += 1;

        // Right boundary (x=n*dx)
        faces.push(Face::new(
            fid, vec![], n - 1, None, dy * dz,
            [1.0, 0.0, 0.0], [n as f64 * dx, 0.5 * dy, 0.5 * dz],
        ));
        cells[n - 1].faces.push(fid);
        right_faces.push(fid);

        let boundary_patches = vec![
            BoundaryPatch::new("left", left_faces),
            BoundaryPatch::new("right", right_faces),
        ];

        UnstructuredMesh::from_components(vec![], faces, cells, boundary_patches)
    }

    #[test]
    fn test_species_transport_solver_creation() {
        let names = vec!["CH4".to_string(), "O2".to_string(), "N2".to_string()];
        let diffs = vec![1e-5, 2e-5, 1.5e-5];
        let solver = SpeciesTransportSolver::new(names.clone(), diffs.clone(), 10);

        assert_eq!(solver.num_species(), 3);
        assert_eq!(solver.species_names, names);
        assert_eq!(solver.mass_fractions.len(), 3);
        for mf in &solver.mass_fractions {
            assert_eq!(mf.values().len(), 10);
        }
    }

    #[test]
    fn test_species_diffusion_only() {
        // Pure diffusion test: no velocity, no reactions.
        // Initial step function Y_A: left cells=1, right cells=0.
        // Diffusion should smooth out the profile.
        let n = 5;
        let mesh = make_1d_mesh(n);

        let names = vec!["A".to_string(), "B".to_string()];
        let diffs = vec![0.1, 0.1]; // high diffusivity for visible effect
        let mut solver = SpeciesTransportSolver::new(names, diffs, n);

        // Initialize: Y_A = 1 for left 2 cells, 0 for right 3
        for i in 0..2 {
            let _ = solver.mass_fractions[0].set(i, 0.8);
            let _ = solver.mass_fractions[1].set(i, 0.2);
        }
        for i in 2..n {
            let _ = solver.mass_fractions[0].set(i, 0.0);
            let _ = solver.mass_fractions[1].set(i, 1.0);
        }

        let velocity = VectorField::zeros("velocity", n); // zero velocity
        let density = ScalarField::new("rho", vec![1.0; n]);
        let reaction_rates: Vec<ScalarField> = Vec::new(); // no reactions

        let dt = 0.1;
        let y_a_before = solver.mass_fractions[0].values().to_vec();

        solver.solve_species(&velocity, &density, &mesh, dt, &reaction_rates).unwrap();

        let y_a_after = solver.mass_fractions[0].values().to_vec();

        // Cell 2 (boundary of step) should have gained some Y_A from diffusion
        assert!(y_a_after[2] > y_a_before[2],
            "Cell 2 should gain Y_A from diffusion: before={}, after={}",
            y_a_before[2], y_a_after[2]);
        // Cell 1 (left of boundary) should have lost some Y_A
        assert!(y_a_after[1] < y_a_before[1],
            "Cell 1 should lose Y_A due to diffusion: before={}, after={}",
            y_a_before[1], y_a_after[1]);

        // Mass fractions should sum to 1 for each cell
        for cell in 0..n {
            let sum: f64 = solver.mass_fractions.iter().map(|mf| mf.values()[cell]).sum();
            assert!((sum - 1.0).abs() < 1e-10,
                "Mass fractions should sum to 1 at cell {}: sum={}", cell, sum);
        }
    }

    #[test]
    fn test_species_with_reaction_source() {
        // Test that reaction source terms modify species mass fractions.
        let n = 3;
        let mesh = make_1d_mesh(n);

        let names = vec!["fuel".to_string(), "oxidizer".to_string(), "product".to_string()];
        let diffs = vec![1e-5, 1e-5, 1e-5];
        let mut solver = SpeciesTransportSolver::new(names, diffs, n);

        // Initialize: fuel=0.2, oxidizer=0.3, product=0.5
        for i in 0..n {
            let _ = solver.mass_fractions[0].set(i, 0.2);
            let _ = solver.mass_fractions[1].set(i, 0.3);
            let _ = solver.mass_fractions[2].set(i, 0.5);
        }

        let velocity = VectorField::zeros("velocity", n);
        let density = ScalarField::new("rho", vec![1.0; n]);

        // Reaction source: consume fuel and oxidizer, produce product
        let mut omega_fuel = ScalarField::zeros("omega_fuel", n);
        let mut omega_ox = ScalarField::zeros("omega_ox", n);
        let omega_prod = ScalarField::zeros("omega_prod", n); // last species computed from constraint
        for i in 0..n {
            let _ = omega_fuel.set(i, -0.5); // consume fuel: -0.5 kg/(m^3*s)
            let _ = omega_ox.set(i, -1.0);   // consume oxidizer: -1.0 kg/(m^3*s)
        }
        let reaction_rates = vec![omega_fuel, omega_ox, omega_prod];

        let dt = 0.01; // small time step for stability
        let y_fuel_before = solver.mass_fractions[0].values()[0];

        solver.solve_species(&velocity, &density, &mesh, dt, &reaction_rates).unwrap();

        let y_fuel_after = solver.mass_fractions[0].values()[0];

        // Fuel should decrease due to negative source
        assert!(y_fuel_after < y_fuel_before,
            "Fuel should decrease: before={}, after={}", y_fuel_before, y_fuel_after);

        // All mass fractions should be bounded [0, 1] and sum to 1
        for cell in 0..n {
            let sum: f64 = solver.mass_fractions.iter().map(|mf| mf.values()[cell]).sum();
            assert!((sum - 1.0).abs() < 1e-10,
                "Mass fractions should sum to 1 at cell {}: sum={}", cell, sum);
            for mf in &solver.mass_fractions {
                assert!(mf.values()[cell] >= 0.0 && mf.values()[cell] <= 1.0,
                    "Mass fraction out of bounds at cell {}: {}", cell, mf.values()[cell]);
            }
        }
    }

    #[test]
    fn test_species_normalization() {
        let names = vec!["A".to_string(), "B".to_string()];
        let diffs = vec![1e-5, 1e-5];
        let mut solver = SpeciesTransportSolver::new(names, diffs, 3);

        // Set unnormalized values
        let _ = solver.mass_fractions[0].set(0, 0.3);
        let _ = solver.mass_fractions[1].set(0, 0.9); // sum = 1.2 (> 1)
        let _ = solver.mass_fractions[0].set(1, 0.0);
        let _ = solver.mass_fractions[1].set(1, 0.0); // sum = 0 (edge case)
        let _ = solver.mass_fractions[0].set(2, 0.5);
        let _ = solver.mass_fractions[1].set(2, 0.5); // sum = 1 (already normal)

        solver.normalize_mass_fractions();

        // Cell 0: should be renormalized to sum = 1
        let sum_0: f64 = solver.mass_fractions.iter().map(|mf| mf.values()[0]).sum();
        assert!((sum_0 - 1.0).abs() < 1e-10, "Cell 0 should sum to 1: {}", sum_0);

        // Cell 2: should remain unchanged
        assert!((solver.mass_fractions[0].values()[2] - 0.5).abs() < 1e-10);
        assert!((solver.mass_fractions[1].values()[2] - 0.5).abs() < 1e-10);
    }

    #[test]
    fn test_species_convection_transport() {
        // Test that a uniform velocity field advects species correctly.
        let n = 5;
        let mesh = make_1d_mesh(n);

        let names = vec!["A".to_string(), "B".to_string()];
        let diffs = vec![0.0, 0.0]; // zero diffusion to isolate convection
        let mut solver = SpeciesTransportSolver::new(names, diffs, n);

        // Initialize: Y_A = 1 only in cell 0
        let _ = solver.mass_fractions[0].set(0, 1.0);
        let _ = solver.mass_fractions[1].set(0, 0.0);
        for i in 1..n {
            let _ = solver.mass_fractions[0].set(i, 0.0);
            let _ = solver.mass_fractions[1].set(i, 1.0);
        }

        // Uniform velocity in +x direction
        let mut velocity = VectorField::zeros("velocity", n);
        for i in 0..n {
            velocity.values_mut()[i] = [1.0, 0.0, 0.0];
        }
        let density = ScalarField::new("rho", vec![1.0; n]);
        let reaction_rates: Vec<ScalarField> = Vec::new();

        let dt = 0.1;

        solver.solve_species(&velocity, &density, &mesh, dt, &reaction_rates).unwrap();

        // Cell 1 should have gained some Y_A from convection (upwind from cell 0)
        assert!(solver.mass_fractions[0].values()[1] > 0.0,
            "Cell 1 should gain Y_A from convection: {}",
            solver.mass_fractions[0].values()[1]);

        // All should still be bounded and sum to 1
        for cell in 0..n {
            let sum: f64 = solver.mass_fractions.iter().map(|mf| mf.values()[cell]).sum();
            assert!((sum - 1.0).abs() < 1e-10,
                "Mass fractions should sum to 1 at cell {}: sum={}", cell, sum);
        }
    }
}
