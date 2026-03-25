//! Convection-diffusion energy equation solver.
//!
//! Solves: rho*cp*dT/dt + div(rho*cp*U*T) = div(k*grad(T)) + S

use std::collections::HashMap;

use gfd_core::{UnstructuredMesh, VectorField};
use gfd_discretize::fvm::diffusion::compute_diffusive_coefficient;
use gfd_linalg::traits::LinearSolverTrait;

use crate::{ThermalError, ThermalState, Result};

/// Convection-diffusion solver for the energy equation.
///
/// Requires a velocity field as input (from the fluid solver).
pub struct ConvectionDiffusionSolver {
    /// Maximum number of iterations.
    pub max_iterations: usize,
    /// Convergence tolerance.
    pub tolerance: f64,
    /// Under-relaxation factor.
    pub under_relaxation: f64,
    /// Density [kg/m^3].
    pub density: f64,
    /// Specific heat capacity [J/(kg*K)].
    pub specific_heat: f64,
    /// Thermal conductivity [W/(m*K)].
    pub conductivity: f64,
    /// Volumetric heat source [W/m^3].
    pub source: f64,
    /// Boundary temperatures: patch name -> temperature [K].
    pub boundary_temps: HashMap<String, f64>,
}

impl ConvectionDiffusionSolver {
    /// Creates a new convection-diffusion solver.
    pub fn new() -> Self {
        Self {
            max_iterations: 100,
            tolerance: 1e-6,
            under_relaxation: 0.8,
            density: 1.0,
            specific_heat: 1000.0,
            conductivity: 1.0,
            source: 0.0,
            boundary_temps: HashMap::new(),
        }
    }

    /// Creates a solver with specified material properties.
    pub fn with_properties(
        density: f64,
        specific_heat: f64,
        conductivity: f64,
        source: f64,
    ) -> Self {
        Self {
            max_iterations: 100,
            tolerance: 1e-6,
            under_relaxation: 0.8,
            density,
            specific_heat,
            conductivity,
            source,
            boundary_temps: HashMap::new(),
        }
    }

    /// Sets boundary temperature conditions.
    pub fn set_boundary_temps(&mut self, boundary_temps: HashMap<String, f64>) {
        self.boundary_temps = boundary_temps;
    }

    /// Solves the energy equation for one time step.
    ///
    /// rho*cp*dT/dt + div(rho*cp*U*T) = div(k*grad(T)) + S
    ///
    /// Uses implicit Euler for time discretization, first-order upwind for
    /// convection, and central differencing for diffusion.
    pub fn solve_step(
        &self,
        state: &mut ThermalState,
        velocity: &VectorField,
        mesh: &UnstructuredMesh,
        dt: f64,
    ) -> Result<f64> {
        let n = mesh.num_cells();
        let rho_cp = self.density * self.specific_heat;

        // Build face-to-patch lookup.
        let mut face_to_patch: HashMap<usize, &str> = HashMap::new();
        for patch in &mesh.boundary_patches {
            for &fid in &patch.face_ids {
                face_to_patch.insert(fid, &patch.name);
            }
        }

        // Save old temperature for temporal term.
        let t_old: Vec<f64> = (0..n)
            .map(|i| state.temperature.get(i).unwrap_or(0.0))
            .collect();

        // Coefficient arrays.
        let mut a_p = vec![0.0_f64; n];
        let mut neighbors_list: Vec<Vec<(usize, f64)>> = vec![Vec::new(); n];
        let mut sources = vec![0.0_f64; n];

        // Loop over all faces: assemble convection + diffusion.
        for face in &mesh.faces {
            let owner = face.owner_cell;

            if let Some(neighbor) = face.neighbor_cell {
                // --- Internal face ---

                // Diffusion coefficient.
                let c_o = &mesh.cells[owner].center;
                let c_n = &mesh.cells[neighbor].center;
                let dist = ((c_o[0] - c_n[0]).powi(2)
                    + (c_o[1] - c_n[1]).powi(2)
                    + (c_o[2] - c_n[2]).powi(2))
                .sqrt();
                let d_coeff = compute_diffusive_coefficient(self.conductivity, face.area, dist);

                // Convective mass flux: F = rho * u_f . n_f * A_f
                let vel_o = velocity.values()[owner];
                let vel_n = velocity.values()[neighbor];
                let u_f = [
                    0.5 * (vel_o[0] + vel_n[0]),
                    0.5 * (vel_o[1] + vel_n[1]),
                    0.5 * (vel_o[2] + vel_n[2]),
                ];
                let f_flux = rho_cp
                    * (u_f[0] * face.normal[0]
                        + u_f[1] * face.normal[1]
                        + u_f[2] * face.normal[2])
                    * face.area;

                // First-order upwind: if flux > 0, use owner value; else use neighbor.
                let f_pos = f64::max(f_flux, 0.0);
                let f_neg = f64::max(-f_flux, 0.0);

                // Owner equation contributions.
                a_p[owner] += d_coeff + f_pos;
                neighbors_list[owner].push((neighbor, d_coeff + f_neg));

                // Neighbor equation contributions.
                a_p[neighbor] += d_coeff + f_neg;
                neighbors_list[neighbor].push((owner, d_coeff + f_pos));
            } else {
                // --- Boundary face ---
                if let Some(&patch_name) = face_to_patch.get(&face.id) {
                    if let Some(&t_bc) = self.boundary_temps.get(patch_name) {
                        // Dirichlet BC.
                        let c_o = &mesh.cells[owner].center;
                        let fc = &face.center;
                        let dist = ((c_o[0] - fc[0]).powi(2)
                            + (c_o[1] - fc[1]).powi(2)
                            + (c_o[2] - fc[2]).powi(2))
                        .sqrt()
                        .max(1e-30);

                        let d_coeff = compute_diffusive_coefficient(self.conductivity, face.area, dist);

                        // Convective flux from boundary.
                        let vel_o = velocity.values()[owner];
                        let f_flux = rho_cp
                            * (vel_o[0] * face.normal[0]
                                + vel_o[1] * face.normal[1]
                                + vel_o[2] * face.normal[2])
                            * face.area;

                        let f_pos = f64::max(f_flux, 0.0);
                        let f_neg = f64::max(-f_flux, 0.0);

                        a_p[owner] += d_coeff + f_pos;
                        sources[owner] += (d_coeff + f_neg) * t_bc;
                    }
                    // else: zero-gradient (Neumann) BC - no contribution.
                }
            }
        }

        // Add temporal term (implicit Euler): rho*cp*V/dt.
        for i in 0..n {
            let temporal_coeff = rho_cp * mesh.cells[i].volume / dt;
            a_p[i] += temporal_coeff;
            sources[i] += temporal_coeff * t_old[i];
        }

        // Add volumetric source term.
        for i in 0..n {
            sources[i] += self.source * mesh.cells[i].volume;
        }

        // Under-relaxation.
        for i in 0..n {
            let a_p_orig = a_p[i];
            a_p[i] /= self.under_relaxation;
            sources[i] += (1.0 - self.under_relaxation) / self.under_relaxation * a_p_orig * t_old[i];
        }

        // Assemble the linear system.
        let mut assembler = gfd_matrix::assembler::Assembler::new(n);
        for i in 0..n {
            assembler.add_cell_equation(i, a_p[i], &neighbors_list[i], sources[i]);
        }
        let mut system = assembler
            .finalize()
            .map_err(|e| ThermalError::CoreError(gfd_core::CoreError::SparseMatrixError(e.to_string())))?;

        // Use old temperature as initial guess.
        for i in 0..n {
            system.x[i] = t_old[i];
        }

        // Solve the linear system using BiCGSTAB (non-symmetric due to convection).
        let mut solver = gfd_linalg::iterative::bicgstab::BiCGSTAB::new(
            self.tolerance,
            self.max_iterations,
        );
        let stats = solver
            .solve(&system.a, &system.b, &mut system.x)
            .map_err(|e| ThermalError::CoreError(gfd_core::CoreError::SparseMatrixError(e.to_string())))?;

        // Copy solution into thermal state.
        for i in 0..n {
            state.temperature.set(i, system.x[i])?;
        }

        Ok(stats.final_residual)
    }
}

impl Default for ConvectionDiffusionSolver {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gfd_core::mesh::cell::Cell;
    use gfd_core::mesh::face::Face;
    use gfd_core::mesh::unstructured::{BoundaryPatch, UnstructuredMesh};

    /// Creates a 1D mesh of `nx` cells along x in [0, length], each cell 1x1x1 cross-section.
    fn make_1d_mesh(nx: usize, length: f64) -> UnstructuredMesh {
        let dx = length / nx as f64;
        let cross_area = 1.0;

        let mut cells = Vec::with_capacity(nx);
        for i in 0..nx {
            let cx = (i as f64 + 0.5) * dx;
            cells.push(Cell::new(
                i,
                vec![],
                vec![],
                dx * 1.0 * 1.0,
                [cx, 0.5, 0.5],
            ));
        }

        let mut faces: Vec<Face> = Vec::new();
        let mut face_id = 0usize;

        // Left boundary face.
        let left_face_id = face_id;
        faces.push(Face::new(face_id, vec![], 0, None, cross_area, [-1.0, 0.0, 0.0], [0.0, 0.5, 0.5]));
        face_id += 1;

        // Internal faces.
        for i in 0..nx - 1 {
            let fx = (i as f64 + 1.0) * dx;
            faces.push(Face::new(face_id, vec![], i, Some(i + 1), cross_area, [1.0, 0.0, 0.0], [fx, 0.5, 0.5]));
            face_id += 1;
        }

        // Right boundary face.
        let right_face_id = face_id;
        faces.push(Face::new(face_id, vec![], nx - 1, None, cross_area, [1.0, 0.0, 0.0], [length, 0.5, 0.5]));

        let boundary_patches = vec![
            BoundaryPatch::new("left", vec![left_face_id]),
            BoundaryPatch::new("right", vec![right_face_id]),
        ];

        UnstructuredMesh::from_components(vec![], faces, cells, boundary_patches)
    }

    #[test]
    fn convection_diffusion_approaches_steady() {
        // 1D convection-diffusion with constant velocity, should approach
        // exponential profile (or linear for Peclet ~ 0).
        let nx = 20;
        let length = 1.0;
        let mesh = make_1d_mesh(nx, length);

        // Low velocity -> diffusion-dominated -> nearly linear profile.
        let velocity = VectorField::new("velocity", vec![[0.01, 0.0, 0.0]; nx]);

        let mut solver = ConvectionDiffusionSolver::with_properties(
            1.0,    // density
            1.0,    // specific heat
            1.0,    // conductivity
            0.0,    // source
        );
        solver.set_boundary_temps(HashMap::from([
            ("left".to_string(), 100.0),
            ("right".to_string(), 200.0),
        ]));

        let mut state = ThermalState::new(nx, 150.0);
        let dt = 0.01;

        for _ in 0..1000 {
            solver.solve_step(&mut state, &velocity, &mesh, dt).unwrap();
        }

        // Temperature should be monotonically increasing from ~100 to ~200.
        let mut prev = 0.0;
        for i in 0..nx {
            let t = state.temperature.get(i).unwrap();
            assert!(t >= prev - 1e-6, "Temperature should be monotonic");
            assert!(t > 90.0 && t < 210.0, "Temperature out of expected range: {}", t);
            prev = t;
        }
    }
}
