//! Discrete Phase Model (DPM) — Lagrangian Particle Tracking.
//!
//! Tracks discrete particles through the Eulerian flow field by solving
//! the particle equation of motion:
//!
//!   m_p * du_p/dt = F_drag + F_gravity + F_pressure_gradient
//!
//! Drag is modelled with the Schiller-Naumann correlation. Particles
//! are advanced with a first-order explicit Euler scheme.

use gfd_core::{UnstructuredMesh, VectorField};
use crate::Result;

/// Drag model for particle force computation.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum DragModel {
    /// Schiller-Naumann correlation (default, valid for spherical particles).
    /// C_D = 24/Re_p * (1 + 0.15*Re_p^0.687) for Re_p < 1000, else 0.44.
    SchillerNaumann,
    /// Stokes drag (valid for Re_p << 1): C_D = 24/Re_p.
    Stokes,
    /// Non-spherical drag with a shape factor (Haider-Levenspiel).
    NonSpherical {
        /// Sphericity factor (1.0 = perfect sphere).
        sphericity: f64,
    },
}

/// A single Lagrangian particle.
#[derive(Debug, Clone)]
pub struct Particle {
    /// Position in Cartesian space [m].
    pub position: [f64; 3],
    /// Velocity [m/s].
    pub velocity: [f64; 3],
    /// Diameter [m].
    pub diameter: f64,
    /// Material density [kg/m^3].
    pub density: f64,
    /// Temperature [K].
    pub temperature: f64,
    /// Mass [kg].
    pub mass: f64,
    /// Index of the mesh cell that currently contains this particle.
    pub cell_id: usize,
    /// Whether this particle is still active (not escaped or removed).
    pub active: bool,
}

impl Particle {
    /// Creates a new spherical particle from diameter and density.
    ///
    /// Mass is computed automatically assuming a sphere: m = rho * pi/6 * d^3.
    pub fn new(
        position: [f64; 3],
        velocity: [f64; 3],
        diameter: f64,
        density: f64,
        temperature: f64,
        cell_id: usize,
    ) -> Self {
        let mass = density * std::f64::consts::PI / 6.0 * diameter.powi(3);
        Self {
            position,
            velocity,
            diameter,
            density,
            temperature,
            mass,
            cell_id,
            active: true,
        }
    }

    /// Returns the projected area of the particle (pi/4 * d^2).
    pub fn projected_area(&self) -> f64 {
        std::f64::consts::PI / 4.0 * self.diameter * self.diameter
    }
}

/// Discrete Phase Model solver.
///
/// Manages a collection of Lagrangian particles and advances them
/// through the Eulerian flow field each time step.
#[derive(Debug)]
pub struct DpmSolver {
    /// Collection of tracked particles.
    pub particles: Vec<Particle>,
    /// Drag model applied to all particles.
    pub drag_model: DragModel,
    /// Gravity vector [m/s^2].
    pub gravity: [f64; 3],
    /// Fluid density [kg/m^3] (constant for now).
    pub fluid_density: f64,
    /// Fluid dynamic viscosity [Pa*s].
    pub fluid_viscosity: f64,
}

impl DpmSolver {
    /// Creates a new DPM solver with no particles.
    pub fn new(
        drag_model: DragModel,
        gravity: [f64; 3],
        fluid_density: f64,
        fluid_viscosity: f64,
    ) -> Self {
        Self {
            particles: Vec::new(),
            drag_model,
            gravity,
            fluid_density,
            fluid_viscosity,
        }
    }

    /// Injects particles along a boundary face.
    ///
    /// Creates `count` particles uniformly distributed across the given
    /// boundary face indices. Each particle is placed at the face centre
    /// with the supplied initial velocity.
    pub fn inject_particles(
        &mut self,
        mesh: &UnstructuredMesh,
        face_ids: &[usize],
        velocity: [f64; 3],
        diameter: f64,
        density: f64,
        temperature: f64,
        count_per_face: usize,
    ) {
        for &fid in face_ids {
            if fid >= mesh.faces.len() {
                continue;
            }
            let face = &mesh.faces[fid];
            let cell_id = face.owner_cell;
            for _ in 0..count_per_face {
                let p = Particle::new(
                    face.center,
                    velocity,
                    diameter,
                    density,
                    temperature,
                    cell_id,
                );
                self.particles.push(p);
            }
        }
    }

    /// Computes drag coefficient from a given drag model (static helper to avoid borrow issues).
    fn compute_drag(model: DragModel, re_p: f64) -> f64 {
        match model {
            DragModel::SchillerNaumann => {
                if re_p < 1e-10 { 0.0 }
                else if re_p < 1000.0 { 24.0 / re_p * (1.0 + 0.15 * re_p.powf(0.687)) }
                else { 0.44 }
            }
            DragModel::Stokes => {
                if re_p < 1e-10 { 0.0 } else { 24.0 / re_p }
            }
            DragModel::NonSpherical { sphericity } => {
                let cd_sphere = if re_p < 1e-10 { 0.0 }
                    else if re_p < 1000.0 { 24.0 / re_p * (1.0 + 0.15 * re_p.powf(0.687)) }
                    else { 0.44 };
                cd_sphere / sphericity.max(0.01)
            }
        }
    }

    /// Computes the drag coefficient C_D for a particle Reynolds number.
    pub fn drag_coefficient(&self, re_p: f64) -> f64 {
        match self.drag_model {
            DragModel::SchillerNaumann => {
                if re_p < 1e-10 {
                    0.0
                } else if re_p < 1000.0 {
                    24.0 / re_p * (1.0 + 0.15 * re_p.powf(0.687))
                } else {
                    0.44
                }
            }
            DragModel::Stokes => {
                if re_p < 1e-10 {
                    0.0
                } else {
                    24.0 / re_p
                }
            }
            DragModel::NonSpherical { sphericity } => {
                // Haider-Levenspiel simplified for non-spherical particles.
                // Falls back to Schiller-Naumann multiplied by a shape correction.
                let cd_sphere = if re_p < 1e-10 {
                    0.0
                } else if re_p < 1000.0 {
                    24.0 / re_p * (1.0 + 0.15 * re_p.powf(0.687))
                } else {
                    0.44
                };
                // Shape correction: C_D increases as sphericity decreases from 1.
                let correction = 1.0 / (sphericity.max(0.01));
                cd_sphere * correction
            }
        }
    }

    /// Interpolates the fluid velocity at a particle position.
    ///
    /// Uses a simple nearest-cell (cell-centroid) value. This is first-order
    /// accurate and avoids the cost of inverse-distance weighting.
    pub fn interpolate_velocity(
        &self,
        particle: &Particle,
        fluid_velocity: &VectorField,
    ) -> [f64; 3] {
        if particle.cell_id < fluid_velocity.values().len() {
            fluid_velocity.values()[particle.cell_id]
        } else {
            [0.0; 3]
        }
    }

    /// Locates the host cell for a particle by checking the current cell
    /// and its face-neighbors (stencil walk).
    ///
    /// Returns `Some(cell_id)` if found, `None` if the particle has left
    /// the domain.
    pub fn find_host_cell(
        &self,
        position: [f64; 3],
        current_cell: usize,
        mesh: &UnstructuredMesh,
    ) -> Option<usize> {
        // First check if particle is still in the current cell.
        if Self::point_in_cell(position, current_cell, mesh) {
            return Some(current_cell);
        }

        // Walk to neighboring cells through shared faces.
        let cell = &mesh.cells[current_cell];
        for &face_id in &cell.faces {
            let face = &mesh.faces[face_id];
            if let Some(neighbor) = face.neighbor_cell {
                let candidate = if neighbor == current_cell {
                    face.owner_cell
                } else {
                    neighbor
                };
                if Self::point_in_cell(position, candidate, mesh) {
                    return Some(candidate);
                }
            }
        }

        // Brute-force fallback for large jumps (rare).
        for cid in 0..mesh.num_cells() {
            if Self::point_in_cell(position, cid, mesh) {
                return Some(cid);
            }
        }

        // Particle has left the domain.
        None
    }

    /// Rough check whether a point lies inside a convex cell by verifying
    /// that the point is on the inward side of every bounding face.
    fn point_in_cell(
        position: [f64; 3],
        cell_id: usize,
        mesh: &UnstructuredMesh,
    ) -> bool {
        let cell = &mesh.cells[cell_id];
        for &face_id in &cell.faces {
            let face = &mesh.faces[face_id];
            // Vector from face centre to point.
            let dx = position[0] - face.center[0];
            let dy = position[1] - face.center[1];
            let dz = position[2] - face.center[2];

            // Face normal points outward from the owner cell.
            let dot = dx * face.normal[0] + dy * face.normal[1] + dz * face.normal[2];

            // If this cell is the owner, the point should be on the negative side
            // of the outward normal.  If the cell is the neighbor, the opposite.
            let is_owner = face.owner_cell == cell_id;
            if is_owner && dot > 1e-12 {
                return false;
            }
            if !is_owner && dot < -1e-12 {
                return false;
            }
        }
        true
    }

    /// Advances all active particles by one time step.
    ///
    /// For each particle the equation of motion is integrated with
    /// explicit Euler:
    ///
    ///   u_p^{n+1} = u_p^n + dt/m_p * (F_drag + F_gravity + F_pressure)
    ///   x_p^{n+1} = x_p^n + dt * u_p^{n+1}
    pub fn advance_particles(
        &mut self,
        dt: f64,
        fluid_velocity: &VectorField,
        mesh: &UnstructuredMesh,
    ) -> Result<()> {
        let rho_f = self.fluid_density;
        let mu = self.fluid_viscosity;
        let g = self.gravity;
        let drag_model = self.drag_model;

        for p in self.particles.iter_mut() {
            if !p.active {
                continue;
            }

            // 1. Interpolate fluid velocity at particle location.
            let u_f = if p.cell_id < fluid_velocity.values().len() {
                fluid_velocity.values()[p.cell_id]
            } else {
                [0.0; 3]
            };

            // Relative velocity.
            let u_rel = [
                u_f[0] - p.velocity[0],
                u_f[1] - p.velocity[1],
                u_f[2] - p.velocity[2],
            ];
            let u_rel_mag =
                (u_rel[0] * u_rel[0] + u_rel[1] * u_rel[1] + u_rel[2] * u_rel[2]).sqrt();

            // 2. Particle Reynolds number.
            let re_p = rho_f * u_rel_mag * p.diameter / (mu + 1e-30);

            // 3. Drag force: F_drag = 0.5 * C_D * rho_f * A_p * |u_rel| * u_rel_dir.
            let cd = Self::compute_drag(drag_model, re_p);
            let a_p = p.projected_area();
            let f_drag = [
                0.5 * cd * rho_f * a_p * u_rel_mag * u_rel[0],
                0.5 * cd * rho_f * a_p * u_rel_mag * u_rel[1],
                0.5 * cd * rho_f * a_p * u_rel_mag * u_rel[2],
            ];

            // 4. Buoyancy-corrected gravity: F_grav = m_p * g * (1 - rho_f/rho_p).
            let buoyancy_factor = 1.0 - rho_f / p.density;
            let f_grav = [
                p.mass * g[0] * buoyancy_factor,
                p.mass * g[1] * buoyancy_factor,
                p.mass * g[2] * buoyancy_factor,
            ];

            // 5. Pressure gradient force: F_pg = m_p * (rho_f / rho_p) * Du_f/Dt.
            //    Approximated simply as m_p * (rho_f / rho_p) * 0 for now (steady flow).
            //    A proper implementation would need the fluid acceleration.
            let f_pg = [0.0; 3];

            // 6. Integrate velocity: u_p^{n+1} = u_p^n + dt/m_p * sum(F).
            if p.mass > 0.0 {
                let inv_mass = 1.0 / p.mass;
                for dim in 0..3 {
                    p.velocity[dim] +=
                        dt * inv_mass * (f_drag[dim] + f_grav[dim] + f_pg[dim]);
                }
            }

            // 7. Integrate position: x_p^{n+1} = x_p^n + dt * u_p^{n+1}.
            for dim in 0..3 {
                p.position[dim] += dt * p.velocity[dim];
            }

            // 8. Find new host cell.
            match Self::find_host_cell_static(p.position, p.cell_id, mesh) {
                Some(new_cell) => p.cell_id = new_cell,
                None => p.active = false, // escaped domain
            }
        }

        Ok(())
    }

    /// Static version of find_host_cell (avoids borrow conflict in advance loop).
    fn find_host_cell_static(
        position: [f64; 3],
        current_cell: usize,
        mesh: &UnstructuredMesh,
    ) -> Option<usize> {
        // Check current cell first.
        if Self::point_in_cell(position, current_cell, mesh) {
            return Some(current_cell);
        }

        // Check neighbors.
        let cell = &mesh.cells[current_cell];
        for &face_id in &cell.faces {
            let face = &mesh.faces[face_id];
            if let Some(neighbor) = face.neighbor_cell {
                let candidate = if neighbor == current_cell {
                    face.owner_cell
                } else {
                    neighbor
                };
                if Self::point_in_cell(position, candidate, mesh) {
                    return Some(candidate);
                }
            }
        }

        // Brute-force fallback.
        for cid in 0..mesh.num_cells() {
            if Self::point_in_cell(position, cid, mesh) {
                return Some(cid);
            }
        }

        None
    }

    /// Returns the number of active particles.
    pub fn num_active(&self) -> usize {
        self.particles.iter().filter(|p| p.active).count()
    }

    /// Removes all inactive particles from the list.
    pub fn remove_inactive(&mut self) {
        self.particles.retain(|p| p.active);
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use gfd_core::mesh::structured::StructuredMesh;

    fn make_test_mesh(nx: usize, ny: usize) -> UnstructuredMesh {
        let sm = StructuredMesh::uniform(nx, ny, 0, 1.0, 1.0, 0.0);
        sm.to_unstructured()
    }

    #[test]
    fn test_particle_creation() {
        let p = Particle::new([0.0; 3], [1.0, 0.0, 0.0], 1e-3, 2500.0, 300.0, 0);
        assert!(p.active);
        assert!(p.mass > 0.0);
        // mass = rho * pi/6 * d^3
        let expected_mass = 2500.0 * std::f64::consts::PI / 6.0 * 1e-9;
        assert!((p.mass - expected_mass).abs() < 1e-18);
        assert!((p.projected_area() - std::f64::consts::PI / 4.0 * 1e-6).abs() < 1e-18);
    }

    #[test]
    fn test_drag_coefficient_stokes() {
        let solver = DpmSolver::new(
            DragModel::Stokes,
            [0.0, -9.81, 0.0],
            1.0,
            1e-3,
        );
        // Stokes: C_D = 24/Re
        let cd = solver.drag_coefficient(10.0);
        assert!((cd - 2.4).abs() < 1e-12);
    }

    #[test]
    fn test_drag_coefficient_schiller_naumann() {
        let solver = DpmSolver::new(
            DragModel::SchillerNaumann,
            [0.0, -9.81, 0.0],
            1.0,
            1e-3,
        );
        // Re < 1000: C_D = 24/Re * (1 + 0.15*Re^0.687)
        let re: f64 = 100.0;
        let expected = 24.0 / re * (1.0 + 0.15 * re.powf(0.687));
        let cd = solver.drag_coefficient(re);
        assert!((cd - expected).abs() < 1e-12);

        // Re >= 1000: C_D = 0.44
        let cd_high = solver.drag_coefficient(5000.0);
        assert!((cd_high - 0.44).abs() < 1e-12);
    }

    #[test]
    fn test_drag_coefficient_zero_re() {
        let solver = DpmSolver::new(
            DragModel::SchillerNaumann,
            [0.0, -9.81, 0.0],
            1.0,
            1e-3,
        );
        let cd = solver.drag_coefficient(0.0);
        assert!(cd.abs() < 1e-12);
    }

    #[test]
    fn test_inject_particles() {
        let mesh = make_test_mesh(5, 5);
        let mut solver = DpmSolver::new(
            DragModel::SchillerNaumann,
            [0.0, -9.81, 0.0],
            1.225,
            1.8e-5,
        );

        // Find boundary faces (faces with no neighbor).
        let boundary_faces: Vec<usize> = mesh
            .faces
            .iter()
            .enumerate()
            .filter(|(_, f)| f.neighbor_cell.is_none())
            .map(|(i, _)| i)
            .take(3)
            .collect();

        solver.inject_particles(
            &mesh,
            &boundary_faces,
            [1.0, 0.0, 0.0],
            1e-3,
            2500.0,
            300.0,
            2,
        );

        assert_eq!(solver.particles.len(), boundary_faces.len() * 2);
        assert_eq!(solver.num_active(), solver.particles.len());
    }

    #[test]
    fn test_particle_stationary_in_zero_flow() {
        // A neutrally buoyant particle in zero flow should stay put.
        let mesh = make_test_mesh(3, 3);
        let n = mesh.num_cells();
        let fluid_vel = VectorField::zeros("velocity", n);

        let mut solver = DpmSolver::new(
            DragModel::SchillerNaumann,
            [0.0, 0.0, 0.0], // no gravity
            1000.0,
            1e-3,
        );

        // Place particle at center of cell 4.
        let center = mesh.cells[4].center;
        let p = Particle::new(center, [0.0; 3], 1e-3, 1000.0, 300.0, 4);
        solver.particles.push(p);

        solver.advance_particles(0.01, &fluid_vel, &mesh).unwrap();

        let p = &solver.particles[0];
        assert!(p.active);
        for dim in 0..3 {
            assert!(
                (p.position[dim] - center[dim]).abs() < 1e-14,
                "Particle should not move in zero flow"
            );
        }
    }

    #[test]
    fn test_particle_drag_accelerates_toward_fluid() {
        // A stationary particle in a uniform flow field should accelerate
        // in the flow direction due to drag.
        let mesh = make_test_mesh(5, 5);
        let n = mesh.num_cells();
        let fluid_vel = VectorField::new(
            "velocity",
            vec![[1.0, 0.0, 0.0]; n],
        );

        let mut solver = DpmSolver::new(
            DragModel::SchillerNaumann,
            [0.0, 0.0, 0.0],
            1.225,
            1.8e-5,
        );

        let center = mesh.cells[12].center;
        let p = Particle::new(center, [0.0; 3], 1e-4, 2500.0, 300.0, 12);
        solver.particles.push(p);

        solver.advance_particles(0.001, &fluid_vel, &mesh).unwrap();

        let p = &solver.particles[0];
        // Particle should have gained positive x-velocity from drag.
        assert!(
            p.velocity[0] > 0.0,
            "Particle should accelerate toward fluid velocity, got vx={}",
            p.velocity[0]
        );
    }

    #[test]
    fn test_particle_gravity_settling() {
        // A heavy particle in stagnant fluid should fall under gravity.
        let mesh = make_test_mesh(3, 3);
        let n = mesh.num_cells();
        let fluid_vel = VectorField::zeros("velocity", n);

        let mut solver = DpmSolver::new(
            DragModel::Stokes,
            [0.0, -9.81, 0.0],
            1.0,  // air-like
            1e-5,
        );

        let center = mesh.cells[4].center;
        let p = Particle::new(center, [0.0; 3], 1e-4, 2500.0, 300.0, 4);
        solver.particles.push(p);

        solver.advance_particles(0.001, &fluid_vel, &mesh).unwrap();

        let p = &solver.particles[0];
        // Buoyancy factor ~ 1 - 1/2500 ≈ 1, so net gravity is downward.
        assert!(
            p.velocity[1] < 0.0,
            "Heavy particle should settle, got vy={}",
            p.velocity[1]
        );
    }

    #[test]
    fn test_remove_inactive() {
        let mut solver = DpmSolver::new(
            DragModel::SchillerNaumann,
            [0.0; 3],
            1.0,
            1e-3,
        );
        let p1 = Particle::new([0.0; 3], [0.0; 3], 1e-3, 1000.0, 300.0, 0);
        let mut p2 = Particle::new([0.0; 3], [0.0; 3], 1e-3, 1000.0, 300.0, 0);
        p2.active = false;
        solver.particles.push(p1);
        solver.particles.push(p2);

        assert_eq!(solver.num_active(), 1);
        solver.remove_inactive();
        assert_eq!(solver.particles.len(), 1);
        assert!(solver.particles[0].active);
    }

    #[test]
    fn test_interpolate_velocity() {
        let mesh = make_test_mesh(3, 3);
        let n = mesh.num_cells();
        let mut vel_data = vec![[0.0; 3]; n];
        vel_data[4] = [2.5, -1.0, 0.0];
        let fluid_vel = VectorField::new("velocity", vel_data);

        let solver = DpmSolver::new(
            DragModel::SchillerNaumann,
            [0.0; 3],
            1.0,
            1e-3,
        );

        let p = Particle::new(mesh.cells[4].center, [0.0; 3], 1e-3, 1000.0, 300.0, 4);
        let v = solver.interpolate_velocity(&p, &fluid_vel);
        assert!((v[0] - 2.5).abs() < 1e-15);
        assert!((v[1] - (-1.0)).abs() < 1e-15);
    }

    #[test]
    fn test_non_spherical_drag_higher_than_sphere() {
        let solver_sphere = DpmSolver::new(
            DragModel::SchillerNaumann,
            [0.0; 3],
            1.0,
            1e-3,
        );
        let solver_nonsp = DpmSolver::new(
            DragModel::NonSpherical { sphericity: 0.5 },
            [0.0; 3],
            1.0,
            1e-3,
        );

        let re = 50.0;
        let cd_sphere = solver_sphere.drag_coefficient(re);
        let cd_nonsp = solver_nonsp.drag_coefficient(re);
        assert!(
            cd_nonsp > cd_sphere,
            "Non-spherical drag should be higher: {} vs {}",
            cd_nonsp,
            cd_sphere
        );
    }
}
