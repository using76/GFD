//! Heat conduction solver.
//!
//! Solves the steady or transient heat conduction equation:
//! rho*cp*dT/dt = div(k*grad(T)) + S

use std::collections::HashMap;

use gfd_core::UnstructuredMesh;
use gfd_discretize::fvm::diffusion::compute_diffusive_coefficient;
use gfd_linalg::traits::LinearSolverTrait;

use crate::{ThermalError, ThermalState, Result};

/// Steady-state and transient heat conduction solver.
pub struct ConductionSolver {
    /// Maximum number of linear solver iterations.
    pub max_iterations: usize,
    /// Convergence tolerance for the linear solver.
    pub tolerance: f64,
    /// Under-relaxation factor.
    pub under_relaxation: f64,
}

impl ConductionSolver {
    /// Creates a new conduction solver with default parameters.
    pub fn new() -> Self {
        Self {
            max_iterations: 1000,
            tolerance: 1e-8,
            under_relaxation: 1.0,
        }
    }

    /// Solves the steady-state heat conduction equation.
    ///
    /// Discretization: laplacian(k, T) + S = 0
    ///
    /// Algorithm:
    /// 1. Assemble the diffusion matrix from conductivity and mesh geometry
    /// 2. Add source terms to the RHS
    /// 3. Apply boundary conditions (Dirichlet via linearization, Neumann as zero-gradient default)
    /// 4. Solve the resulting linear system A*T = b
    ///
    /// Returns the residual norm.
    pub fn solve_steady(
        &self,
        state: &mut ThermalState,
        mesh: &UnstructuredMesh,
        conductivity: f64,
        source: f64,
        boundary_temps: &HashMap<String, f64>,
    ) -> Result<f64> {
        let n = mesh.num_cells();

        // Pre-build a map from face_id -> patch_name for boundary faces.
        let mut face_to_patch: HashMap<usize, &str> = HashMap::new();
        for patch in &mesh.boundary_patches {
            for &fid in &patch.face_ids {
                face_to_patch.insert(fid, &patch.name);
            }
        }

        // Coefficient arrays.
        let mut a_p = vec![0.0_f64; n];
        let mut neighbors_list: Vec<Vec<(usize, f64)>> = vec![Vec::new(); n];
        let mut sources = vec![0.0_f64; n];

        // Loop over all faces.
        for face in &mesh.faces {
            let owner = face.owner_cell;

            if let Some(neighbor) = face.neighbor_cell {
                // Internal face.
                let c_o = &mesh.cells[owner].center;
                let c_n = &mesh.cells[neighbor].center;
                let dist = ((c_o[0] - c_n[0]).powi(2)
                    + (c_o[1] - c_n[1]).powi(2)
                    + (c_o[2] - c_n[2]).powi(2))
                .sqrt();

                let d = compute_diffusive_coefficient(conductivity, face.area, dist);

                a_p[owner] += d;
                a_p[neighbor] += d;
                neighbors_list[owner].push((neighbor, d));
                neighbors_list[neighbor].push((owner, d));
            } else {
                // Boundary face.
                if let Some(&patch_name) = face_to_patch.get(&face.id) {
                    if let Some(&t_bc) = boundary_temps.get(patch_name) {
                        // Dirichlet BC: linearize into coefficients.
                        let c_o = &mesh.cells[owner].center;
                        let fc = &face.center;
                        let dist = ((c_o[0] - fc[0]).powi(2)
                            + (c_o[1] - fc[1]).powi(2)
                            + (c_o[2] - fc[2]).powi(2))
                        .sqrt();

                        let d = compute_diffusive_coefficient(conductivity, face.area, dist);

                        a_p[owner] += d;
                        sources[owner] += d * t_bc;
                    }
                    // else: zero gradient (Neumann with zero flux) — do nothing.
                }
            }
        }

        // Add volumetric source term.
        for i in 0..n {
            sources[i] += source * mesh.cells[i].volume;
        }

        // Assemble the linear system.
        let mut assembler = gfd_matrix::assembler::Assembler::new(n);
        for i in 0..n {
            assembler.add_cell_equation(i, a_p[i], &neighbors_list[i], sources[i]);
        }
        let mut system = assembler
            .finalize()
            .map_err(|e| ThermalError::CoreError(gfd_core::CoreError::SparseMatrixError(e.to_string())))?;

        // Solve the linear system using CG.
        let mut solver = gfd_linalg::iterative::cg::CG::new(self.tolerance, self.max_iterations);
        let stats = solver
            .solve(&system.a, &system.b, &mut system.x)
            .map_err(|e| ThermalError::CoreError(gfd_core::CoreError::SparseMatrixError(e.to_string())))?;

        // Copy solution into thermal state.
        for i in 0..n {
            state.temperature.set(i, system.x[i])?;
        }

        Ok(stats.final_residual)
    }

    /// Solves one time step of the transient heat conduction equation.
    ///
    /// rho*cp*dT/dt = div(k*grad(T)) + S
    ///
    /// Uses implicit Euler time discretization.
    pub fn solve_transient_step(
        &self,
        state: &mut ThermalState,
        mesh: &UnstructuredMesh,
        conductivity: &[f64],
        rho_cp: &[f64],
        source: &[f64],
        dt: f64,
        boundary_temps: &HashMap<String, f64>,
    ) -> Result<f64> {
        let n = mesh.num_cells();

        // Pre-build a map from face_id -> patch_name for boundary faces.
        let mut face_to_patch: HashMap<usize, &str> = HashMap::new();
        for patch in &mesh.boundary_patches {
            for &fid in &patch.face_ids {
                face_to_patch.insert(fid, &patch.name);
            }
        }

        // Coefficient arrays.
        let mut a_p = vec![0.0_f64; n];
        let mut neighbors_list: Vec<Vec<(usize, f64)>> = vec![Vec::new(); n];
        let mut sources_vec = vec![0.0_f64; n];

        // Save old temperatures for the temporal term.
        let t_old: Vec<f64> = (0..n)
            .map(|i| state.temperature.get(i).unwrap_or(0.0))
            .collect();

        // Loop over all faces.
        for face in &mesh.faces {
            let owner = face.owner_cell;

            if let Some(neighbor) = face.neighbor_cell {
                // Internal face: use average conductivity.
                let k_face = 0.5 * (conductivity[owner] + conductivity[neighbor]);

                let c_o = &mesh.cells[owner].center;
                let c_n = &mesh.cells[neighbor].center;
                let dist = ((c_o[0] - c_n[0]).powi(2)
                    + (c_o[1] - c_n[1]).powi(2)
                    + (c_o[2] - c_n[2]).powi(2))
                .sqrt();

                let d = compute_diffusive_coefficient(k_face, face.area, dist);

                a_p[owner] += d;
                a_p[neighbor] += d;
                neighbors_list[owner].push((neighbor, d));
                neighbors_list[neighbor].push((owner, d));
            } else {
                // Boundary face.
                if let Some(&patch_name) = face_to_patch.get(&face.id) {
                    if let Some(&t_bc) = boundary_temps.get(patch_name) {
                        let c_o = &mesh.cells[owner].center;
                        let fc = &face.center;
                        let dist = ((c_o[0] - fc[0]).powi(2)
                            + (c_o[1] - fc[1]).powi(2)
                            + (c_o[2] - fc[2]).powi(2))
                        .sqrt();

                        let d = compute_diffusive_coefficient(conductivity[owner], face.area, dist);

                        a_p[owner] += d;
                        sources_vec[owner] += d * t_bc;
                    }
                }
            }
        }

        // Add temporal and source terms.
        for i in 0..n {
            let temporal_coeff = rho_cp[i] * mesh.cells[i].volume / dt;
            a_p[i] += temporal_coeff;
            sources_vec[i] += temporal_coeff * t_old[i];
            sources_vec[i] += source[i] * mesh.cells[i].volume;
        }

        // Assemble the linear system.
        let mut assembler = gfd_matrix::assembler::Assembler::new(n);
        for i in 0..n {
            assembler.add_cell_equation(i, a_p[i], &neighbors_list[i], sources_vec[i]);
        }
        let mut system = assembler
            .finalize()
            .map_err(|e| ThermalError::CoreError(gfd_core::CoreError::SparseMatrixError(e.to_string())))?;

        // Use old temperature as initial guess.
        for i in 0..n {
            system.x[i] = t_old[i];
        }

        // Solve the linear system using CG.
        let mut solver = gfd_linalg::iterative::cg::CG::new(self.tolerance, self.max_iterations);
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

impl Default for ConductionSolver {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gfd_core::mesh::cell::Cell;
    use gfd_core::mesh::face::Face;
    use gfd_core::mesh::node::Node;
    use gfd_core::mesh::unstructured::{BoundaryPatch, UnstructuredMesh};

    /// Creates a 1D mesh of `nx` cells along x in [0, length], each cell 1x1x1 cross-section.
    fn make_1d_mesh(nx: usize, length: f64) -> UnstructuredMesh {
        let dx = length / nx as f64;
        let cross_area = 1.0; // 1 m^2 cross-section

        // Nodes: we only need minimal info; the solver uses cell/face centers directly.
        let nodes: Vec<Node> = Vec::new();

        // Cells: nx cells, each with volume = dx * 1 * 1, center at ((i+0.5)*dx, 0.5, 0.5).
        let mut cells = Vec::with_capacity(nx);
        for i in 0..nx {
            let cx = (i as f64 + 0.5) * dx;
            cells.push(Cell::new(
                i,
                vec![],      // node indices not needed for this solver
                vec![],      // face indices filled below
                dx * 1.0 * 1.0, // volume
                [cx, 0.5, 0.5],
            ));
        }

        let mut faces: Vec<Face> = Vec::new();
        let mut face_id = 0usize;

        // Left boundary face (x = 0).
        let left_face_id = face_id;
        faces.push(Face::new(
            face_id,
            vec![],
            0,    // owner = cell 0
            None, // boundary
            cross_area,
            [-1.0, 0.0, 0.0],
            [0.0, 0.5, 0.5],
        ));
        face_id += 1;

        // Internal faces between consecutive cells.
        let mut internal_face_ids = Vec::new();
        for i in 0..nx - 1 {
            let fx = (i as f64 + 1.0) * dx;
            internal_face_ids.push(face_id);
            faces.push(Face::new(
                face_id,
                vec![],
                i,          // owner = cell i
                Some(i + 1), // neighbor = cell i+1
                cross_area,
                [1.0, 0.0, 0.0],
                [fx, 0.5, 0.5],
            ));
            face_id += 1;
        }

        // Right boundary face (x = length).
        let right_face_id = face_id;
        faces.push(Face::new(
            face_id,
            vec![],
            nx - 1, // owner = last cell
            None,    // boundary
            cross_area,
            [1.0, 0.0, 0.0],
            [length, 0.5, 0.5],
        ));

        // Update cell face lists.
        // Cell 0: left boundary face + internal face 0 (if exists).
        cells[0].faces.push(left_face_id);
        if nx > 1 {
            cells[0].faces.push(internal_face_ids[0]);
        }
        // Internal cells.
        for i in 1..nx - 1 {
            cells[i].faces.push(internal_face_ids[i - 1]);
            cells[i].faces.push(internal_face_ids[i]);
        }
        // Last cell: internal face (nx-2) + right boundary face.
        if nx > 1 {
            cells[nx - 1].faces.push(internal_face_ids[nx - 2]);
        }
        cells[nx - 1].faces.push(right_face_id);

        let boundary_patches = vec![
            BoundaryPatch::new("left", vec![left_face_id]),
            BoundaryPatch::new("right", vec![right_face_id]),
        ];

        UnstructuredMesh::from_components(nodes, faces, cells, boundary_patches)
    }

    #[test]
    fn steady_1d_conduction_linear_profile() {
        let nx = 10;
        let length = 1.0;
        let mesh = make_1d_mesh(nx, length);

        let conductivity = 1.0; // W/(m*K)
        let source = 0.0;       // No source
        let t_left = 100.0;
        let t_right = 200.0;

        let mut boundary_temps = HashMap::new();
        boundary_temps.insert("left".to_string(), t_left);
        boundary_temps.insert("right".to_string(), t_right);

        let mut state = ThermalState::new(nx, 150.0);
        let solver = ConductionSolver::new();

        let residual = solver
            .solve_steady(&mut state, &mesh, conductivity, source, &boundary_temps)
            .expect("Solver should succeed");

        assert!(
            residual < 1e-6,
            "Residual should be small, got {}",
            residual
        );

        // Check against analytical solution: T(x) = T_left + (T_right - T_left) * x / L
        let dx = length / nx as f64;
        for i in 0..nx {
            let x_center = (i as f64 + 0.5) * dx;
            let t_analytical = t_left + (t_right - t_left) * x_center / length;
            let t_computed = state.temperature.get(i).unwrap();
            let error = (t_computed - t_analytical).abs() / t_analytical;
            assert!(
                error < 0.01,
                "Cell {}: computed={:.4}, analytical={:.4}, error={:.4}%",
                i,
                t_computed,
                t_analytical,
                error * 100.0
            );
        }
    }

    #[test]
    fn steady_1d_conduction_with_source() {
        // Solve: d/dx(k dT/dx) + S = 0 on [0, L]
        // With T(0) = T(L) = 0 and uniform source S.
        // Analytical: T(x) = (S / (2k)) * x * (L - x)
        let nx = 50;
        let length = 1.0;
        let mesh = make_1d_mesh(nx, length);

        let conductivity = 1.0;
        let source_val = 100.0; // W/m^3

        let mut boundary_temps = HashMap::new();
        boundary_temps.insert("left".to_string(), 0.0);
        boundary_temps.insert("right".to_string(), 0.0);

        let mut state = ThermalState::new(nx, 0.0);
        let solver = ConductionSolver::new();

        let residual = solver
            .solve_steady(&mut state, &mesh, conductivity, source_val, &boundary_temps)
            .expect("Solver should succeed");

        assert!(
            residual < 1e-6,
            "Residual should be small, got {}",
            residual
        );

        // Analytical: T(x) = (S/(2k)) * x * (L - x)
        // Max temperature at x = L/2: T_max = S*L^2 / (8k) = 100 * 1 / 8 = 12.5
        let dx = length / nx as f64;
        for i in 0..nx {
            let x_center = (i as f64 + 0.5) * dx;
            let t_analytical = (source_val / (2.0 * conductivity)) * x_center * (length - x_center);
            let t_computed = state.temperature.get(i).unwrap();
            // Near-boundary cells have larger FVM discretization error due to
            // the half-cell distance to the boundary face vs cell center.
            // Use absolute error for small values, relative for larger.
            let abs_error = (t_computed - t_analytical).abs();
            let t_max_analytical = source_val * length * length / (8.0 * conductivity);
            let normalized_error = abs_error / t_max_analytical;
            assert!(
                normalized_error < 0.01,
                "Cell {}: computed={:.6}, analytical={:.6}, normalized_error={:.4}%",
                i,
                t_computed,
                t_analytical,
                normalized_error * 100.0
            );
        }
    }

    #[test]
    fn transient_1d_approaches_steady_state() {
        // Start at uniform 150 K, boundaries at 100 and 200.
        // After many time steps, should approach steady state: T(x) = 100 + 100*x.
        let nx = 10;
        let length = 1.0;
        let mesh = make_1d_mesh(nx, length);

        let conductivity = vec![1.0; nx];
        let rho_cp = vec![1.0; nx]; // rho * cp = 1
        let source = vec![0.0; nx];
        let dt = 0.01;

        let mut boundary_temps = HashMap::new();
        boundary_temps.insert("left".to_string(), 100.0);
        boundary_temps.insert("right".to_string(), 200.0);

        let mut state = ThermalState::new(nx, 150.0);
        let solver = ConductionSolver::new();

        // Run 500 time steps.
        for _ in 0..500 {
            solver
                .solve_transient_step(
                    &mut state,
                    &mesh,
                    &conductivity,
                    &rho_cp,
                    &source,
                    dt,
                    &boundary_temps,
                )
                .expect("Transient step should succeed");
        }

        // Check against steady-state analytical solution.
        let dx = length / nx as f64;
        for i in 0..nx {
            let x_center = (i as f64 + 0.5) * dx;
            let t_analytical = 100.0 + 100.0 * x_center / length;
            let t_computed = state.temperature.get(i).unwrap();
            let error = (t_computed - t_analytical).abs() / t_analytical;
            assert!(
                error < 0.01,
                "Cell {}: computed={:.4}, analytical={:.4}, error={:.4}%",
                i,
                t_computed,
                t_analytical,
                error * 100.0
            );
        }
    }
}
