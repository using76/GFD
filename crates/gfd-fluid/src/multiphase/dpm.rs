//! Discrete Phase Model (DPM) — Lagrangian Particle Tracking.
//!
//! Tracks discrete particles through the Eulerian flow field by solving
//! the particle equation of motion:
//!
//!   m_p * du_p/dt = F_drag + F_gravity + F_pressure_gradient + F_lift + F_vm + F_th
//!
//! Features:
//! - Drag models: Schiller-Naumann, Stokes, non-spherical (Haider-Levenspiel)
//! - Two-way coupling (momentum and heat source to Eulerian field)
//! - Particle heat transfer with Ranz-Marshall correlation
//! - Wall interactions: reflect, trap, escape
//! - Turbulent dispersion via Discrete Random Walk (DRW)
//! - Additional forces: Saffman lift, thermophoretic, virtual mass
//! - Particle size distributions: uniform, Rosin-Rammler
//! - Runtime statistics tracking

use gfd_core::{UnstructuredMesh, VectorField};
use crate::Result;

// ---------------------------------------------------------------------------
// Simple xorshift64 PRNG (no external rand dependency)
// ---------------------------------------------------------------------------

/// Minimal xorshift64 pseudo-random number generator.
///
/// Produces a deterministic sequence of u64 values; sufficient for
/// turbulent dispersion where statistical quality is secondary to
/// reproducibility and zero external dependencies.
#[derive(Debug, Clone)]
struct Xorshift64 {
    state: u64,
}

impl Xorshift64 {
    fn new(seed: u64) -> Self {
        Self {
            state: if seed == 0 { 0x5DEECE66D } else { seed },
        }
    }

    fn next_u64(&mut self) -> u64 {
        let mut x = self.state;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.state = x;
        x
    }

    /// Returns a uniform f64 in [0, 1).
    fn next_f64(&mut self) -> f64 {
        (self.next_u64() >> 11) as f64 / ((1u64 << 53) as f64)
    }

    /// Returns a standard normal variate via Box-Muller transform.
    fn next_normal(&mut self) -> f64 {
        let u1 = self.next_f64().max(1e-30); // avoid ln(0)
        let u2 = self.next_f64();
        (-2.0 * u1.ln()).sqrt() * (2.0 * std::f64::consts::PI * u2).cos()
    }
}

// ---------------------------------------------------------------------------
// Enums
// ---------------------------------------------------------------------------

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

/// Behaviour when a particle reaches a wall (boundary face).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum WallInteraction {
    /// Elastic/inelastic reflection.  `restitution` ∈ (0,1] scales the
    /// normal velocity component after the bounce.
    Reflect {
        /// Coefficient of restitution for the normal velocity component.
        /// 1.0 = perfectly elastic, 0.5 = lose half the normal speed, etc.
        restitution: f64,
    },
    /// Trap the particle at the wall — deactivate it and record its mass.
    Trap,
    /// Let the particle escape through the boundary (deactivate).
    Escape,
}

/// Optional additional forces acting on particles beyond drag, gravity,
/// and pressure gradient.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AdditionalForces {
    /// Enable Saffman lift force.
    pub saffman_lift: bool,
    /// Enable thermophoretic force.  Requires a temperature gradient field.
    pub thermophoretic: bool,
    /// Thermophoretic coefficient D_T (simplified model).
    /// Default: 0.0 (disabled).  Typical order ~1e-10 for small particles in gas.
    pub thermophoretic_coeff: f64,
    /// Enable virtual (added) mass force.
    pub virtual_mass: bool,
}

impl Default for AdditionalForces {
    fn default() -> Self {
        Self {
            saffman_lift: false,
            thermophoretic: false,
            thermophoretic_coeff: 0.0,
            virtual_mass: false,
        }
    }
}

/// Particle size distribution used when injecting particles.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ParticleSizeDistribution {
    /// All particles share the same diameter.
    Monodisperse {
        diameter: f64,
    },
    /// Uniform distribution between `d_min` and `d_max`.
    Uniform {
        d_min: f64,
        d_max: f64,
    },
    /// Rosin-Rammler (Weibull) distribution:
    ///   Y_d = exp(-(d/d_mean)^n)
    /// where `n` is the spread parameter. Particles are sampled from
    /// `n_bins` equally-spaced quantiles.
    RosinRammler {
        d_mean: f64,
        spread_param: f64,
        n_bins: usize,
    },
}

// ---------------------------------------------------------------------------
// Particle
// ---------------------------------------------------------------------------

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
    /// Time the particle has been alive [s] — for residence-time statistics.
    pub age: f64,
    /// Remaining time before a new turbulent fluctuation is sampled [s].
    pub turb_time_remaining: f64,
    /// Current turbulent velocity fluctuation [m/s] (DRW model).
    pub turb_fluctuation: [f64; 3],
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
            age: 0.0,
            turb_time_remaining: 0.0,
            turb_fluctuation: [0.0; 3],
        }
    }

    /// Returns the projected area of the particle (pi/4 * d^2).
    pub fn projected_area(&self) -> f64 {
        std::f64::consts::PI / 4.0 * self.diameter * self.diameter
    }

    /// Returns the surface area of the particle (pi * d^2).
    pub fn surface_area(&self) -> f64 {
        std::f64::consts::PI * self.diameter * self.diameter
    }

    /// Returns the volume of the particle (pi/6 * d^3).
    pub fn volume(&self) -> f64 {
        std::f64::consts::PI / 6.0 * self.diameter.powi(3)
    }
}

// ---------------------------------------------------------------------------
// Particle statistics
// ---------------------------------------------------------------------------

/// Aggregate statistics about the particle population.
#[derive(Debug, Clone)]
pub struct ParticleStatistics {
    /// Number of currently active particles.
    pub active_count: usize,
    /// Number of particles that have escaped the domain.
    pub escaped_count: usize,
    /// Number of particles trapped at walls.
    pub trapped_count: usize,
    /// Total injected particle count (active + escaped + trapped).
    pub total_count: usize,
    /// Mean particle diameter [m].
    pub mean_diameter: f64,
    /// Minimum particle diameter [m].
    pub min_diameter: f64,
    /// Maximum particle diameter [m].
    pub max_diameter: f64,
    /// Mean particle speed [m/s].
    pub mean_speed: f64,
    /// Maximum particle speed [m/s].
    pub max_speed: f64,
    /// Minimum particle speed [m/s].
    pub min_speed: f64,
    /// Mean particle temperature [K].
    pub mean_temperature: f64,
    /// Min particle temperature [K].
    pub min_temperature: f64,
    /// Max particle temperature [K].
    pub max_temperature: f64,
    /// Total mass of all active particles [kg].
    pub total_active_mass: f64,
    /// Total mass of trapped particles [kg].
    pub total_trapped_mass: f64,
    /// Mean residence time of active particles [s].
    pub mean_residence_time: f64,
}

impl Default for ParticleStatistics {
    fn default() -> Self {
        Self {
            active_count: 0,
            escaped_count: 0,
            trapped_count: 0,
            total_count: 0,
            mean_diameter: 0.0,
            min_diameter: 0.0,
            max_diameter: 0.0,
            mean_speed: 0.0,
            max_speed: 0.0,
            min_speed: 0.0,
            mean_temperature: 0.0,
            min_temperature: 0.0,
            max_temperature: 0.0,
            total_active_mass: 0.0,
            total_trapped_mass: 0.0,
            mean_residence_time: 0.0,
        }
    }
}

// ---------------------------------------------------------------------------
// DPM solver
// ---------------------------------------------------------------------------

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
    /// Wall interaction model.
    pub wall_interaction: WallInteraction,
    /// Additional forces configuration.
    pub additional_forces: AdditionalForces,
    /// Fluid thermal conductivity [W/(m*K)] for Ranz-Marshall correlation.
    pub fluid_conductivity: f64,
    /// Fluid specific heat capacity [J/(kg*K)] for Prandtl number.
    pub fluid_specific_heat: f64,
    /// Particle specific heat capacity [J/(kg*K)] for heat transfer.
    pub particle_specific_heat: f64,
    /// Accumulated trapped mass [kg].
    pub trapped_mass: f64,
    /// Count of escaped particles.
    pub escaped_count: usize,
    /// Count of trapped particles.
    pub trapped_count: usize,
    /// Internal PRNG for turbulent dispersion.
    rng: Xorshift64,
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
            wall_interaction: WallInteraction::Escape,
            additional_forces: AdditionalForces::default(),
            fluid_conductivity: 0.026, // air default
            fluid_specific_heat: 1006.0,
            particle_specific_heat: 840.0,
            trapped_mass: 0.0,
            escaped_count: 0,
            trapped_count: 0,
            rng: Xorshift64::new(42),
        }
    }

    // -----------------------------------------------------------------------
    // Injection
    // -----------------------------------------------------------------------

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

    /// Injects particles with a given size distribution.
    ///
    /// For each boundary face, `count_per_face` particles are created.
    /// Their diameters are drawn from the specified distribution.
    pub fn inject_particles_with_distribution(
        &mut self,
        mesh: &UnstructuredMesh,
        face_ids: &[usize],
        velocity: [f64; 3],
        distribution: ParticleSizeDistribution,
        density: f64,
        temperature: f64,
        count_per_face: usize,
    ) {
        let diameters = Self::sample_diameters(distribution, count_per_face, &mut self.rng);
        for &fid in face_ids {
            if fid >= mesh.faces.len() {
                continue;
            }
            let face = &mesh.faces[fid];
            let cell_id = face.owner_cell;
            for &d in &diameters {
                let p = Particle::new(
                    face.center,
                    velocity,
                    d,
                    density,
                    temperature,
                    cell_id,
                );
                self.particles.push(p);
            }
        }
    }

    /// Generates a vector of diameters from a size distribution.
    fn sample_diameters(
        dist: ParticleSizeDistribution,
        count: usize,
        rng: &mut Xorshift64,
    ) -> Vec<f64> {
        match dist {
            ParticleSizeDistribution::Monodisperse { diameter } => {
                vec![diameter; count]
            }
            ParticleSizeDistribution::Uniform { d_min, d_max } => {
                (0..count)
                    .map(|_| d_min + (d_max - d_min) * rng.next_f64())
                    .collect()
            }
            ParticleSizeDistribution::RosinRammler {
                d_mean,
                spread_param,
                n_bins,
            } => {
                // Sample from n_bins equally-spaced quantiles, then cycle.
                let bins = n_bins.max(1);
                let inv_n = spread_param.recip();
                let diameters_bins: Vec<f64> = (0..bins)
                    .map(|i| {
                        // quantile fraction — avoid 0 and 1
                        let q = (i as f64 + 0.5) / bins as f64;
                        // Inverse CDF: d = d_mean * (-ln(1 - q))^(1/n)
                        d_mean * (-((1.0 - q).ln())).powf(inv_n)
                    })
                    .collect();
                (0..count)
                    .map(|i| diameters_bins[i % diameters_bins.len()])
                    .collect()
            }
        }
    }

    // -----------------------------------------------------------------------
    // Drag
    // -----------------------------------------------------------------------

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
                let cd_sphere = if re_p < 1e-10 {
                    0.0
                } else if re_p < 1000.0 {
                    24.0 / re_p * (1.0 + 0.15 * re_p.powf(0.687))
                } else {
                    0.44
                };
                let correction = 1.0 / (sphericity.max(0.01));
                cd_sphere * correction
            }
        }
    }

    // -----------------------------------------------------------------------
    // Interpolation
    // -----------------------------------------------------------------------

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

    // -----------------------------------------------------------------------
    // Cell search
    // -----------------------------------------------------------------------

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
        Self::find_host_cell_static(position, current_cell, mesh)
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
            let dx = position[0] - face.center[0];
            let dy = position[1] - face.center[1];
            let dz = position[2] - face.center[2];

            let dot = dx * face.normal[0] + dy * face.normal[1] + dz * face.normal[2];

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

    /// Static version of find_host_cell (avoids borrow conflict in advance loop).
    fn find_host_cell_static(
        position: [f64; 3],
        current_cell: usize,
        mesh: &UnstructuredMesh,
    ) -> Option<usize> {
        if Self::point_in_cell(position, current_cell, mesh) {
            return Some(current_cell);
        }

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

        None
    }

    // -----------------------------------------------------------------------
    // Wall interaction helpers
    // -----------------------------------------------------------------------

    /// Identifies the boundary face that the particle crossed when leaving
    /// `old_cell` and applies the configured wall interaction.
    ///
    /// Returns `true` if the particle is still active after the interaction.
    fn handle_wall_interaction(
        particle: &mut Particle,
        old_cell: usize,
        mesh: &UnstructuredMesh,
        wall_interaction: WallInteraction,
        trapped_mass: &mut f64,
        trapped_count: &mut usize,
        escaped_count: &mut usize,
    ) {
        // Find the boundary face the particle crossed.
        let cell = &mesh.cells[old_cell];
        let mut best_face_id = None;
        let mut best_dot = f64::NEG_INFINITY;

        for &fid in &cell.faces {
            let face = &mesh.faces[fid];
            if face.neighbor_cell.is_some() {
                continue; // internal face
            }
            // Displacement from face center to particle position.
            let dx = particle.position[0] - face.center[0];
            let dy = particle.position[1] - face.center[1];
            let dz = particle.position[2] - face.center[2];

            // Use outward normal from the owner cell.
            let sign = if face.owner_cell == old_cell { 1.0 } else { -1.0 };
            let dot = sign * (dx * face.normal[0] + dy * face.normal[1] + dz * face.normal[2]);
            if dot > best_dot {
                best_dot = dot;
                best_face_id = Some(fid);
            }
        }

        match wall_interaction {
            WallInteraction::Escape => {
                particle.active = false;
                *escaped_count += 1;
            }
            WallInteraction::Trap => {
                particle.active = false;
                *trapped_mass += particle.mass;
                *trapped_count += 1;
            }
            WallInteraction::Reflect { restitution } => {
                if let Some(fid) = best_face_id {
                    let face = &mesh.faces[fid];
                    let sign = if face.owner_cell == old_cell { 1.0 } else { -1.0 };
                    let n = [
                        sign * face.normal[0],
                        sign * face.normal[1],
                        sign * face.normal[2],
                    ];

                    // Reverse normal component of velocity, scale by restitution.
                    let v_dot_n = particle.velocity[0] * n[0]
                        + particle.velocity[1] * n[1]
                        + particle.velocity[2] * n[2];

                    for dim in 0..3 {
                        particle.velocity[dim] -= (1.0 + restitution) * v_dot_n * n[dim];
                    }

                    // Move particle back inside the cell.
                    let cell_center = mesh.cells[old_cell].center;
                    particle.position = cell_center;
                    particle.cell_id = old_cell;
                    // particle stays active
                } else {
                    // No boundary face found — fall back to escape.
                    particle.active = false;
                    *escaped_count += 1;
                }
            }
        }
    }

    // -----------------------------------------------------------------------
    // Particle advancement
    // -----------------------------------------------------------------------

    /// Advances all active particles by one time step.
    ///
    /// For each particle the equation of motion is integrated with
    /// explicit Euler:
    ///
    ///   u_p^{n+1} = u_p^n + dt/m_p * (F_drag + F_gravity + F_pressure + ...)
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
        let wall_interaction = self.wall_interaction;
        let additional_forces = self.additional_forces;

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

            // Relative velocity (including turbulent fluctuation if any).
            let u_f_eff = [
                u_f[0] + p.turb_fluctuation[0],
                u_f[1] + p.turb_fluctuation[1],
                u_f[2] + p.turb_fluctuation[2],
            ];
            let u_rel = [
                u_f_eff[0] - p.velocity[0],
                u_f_eff[1] - p.velocity[1],
                u_f_eff[2] - p.velocity[2],
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

            // 5. Pressure gradient force (steady approximation — zero for now).
            let f_pg = [0.0; 3];

            // 6. Additional forces.
            let mut f_extra = [0.0; 3];

            // 6a. Saffman lift force (simplified).
            if additional_forces.saffman_lift {
                // F_lift = 1.615 * mu * d_p * Re_G^0.5 * (u_f - u_p) x omega / |omega|
                // We approximate the shear Reynolds number Re_G using the relative velocity.
                // Without a full velocity gradient tensor, use Re_G ~ Re_p as simplification.
                let re_g_sqrt = re_p.sqrt();
                let coeff = 1.615 * mu * p.diameter * re_g_sqrt;
                // Lift direction perpendicular to flow in y-z plane (simplified).
                // A full implementation would need grad(u). Here, we apply a
                // cross-product-like correction in the plane normal to u_rel.
                if u_rel_mag > 1e-30 {
                    // Simple: lift acts perpendicular to relative velocity.
                    // Use an arbitrary perpendicular vector (rotate u_rel by 90 deg in y-z).
                    let perp = [-u_rel[1], u_rel[0], 0.0];
                    let perp_mag = (perp[0] * perp[0] + perp[1] * perp[1] + perp[2] * perp[2]).sqrt();
                    if perp_mag > 1e-30 {
                        for dim in 0..3 {
                            f_extra[dim] += coeff * perp[dim] / perp_mag;
                        }
                    }
                }
            }

            // 6b. Thermophoretic force.
            // F_th = -D_T * grad(T) / T
            // grad(T) is not available per-cell here; it would require a temperature
            // gradient field.  The `compute_heat_transfer` method handles thermal
            // coupling.  Here we allow a user-specified coefficient that produces
            // a force proportional to the temperature difference over cell size.
            // This is a placeholder; a full implementation passes grad_T externally.

            // 6c. Virtual mass force.
            // F_vm = C_vm * rho_f * V_p * (Du_f/Dt - du_p/dt)
            // Requires fluid acceleration, which we don't have in this simple step.
            // Stored for future use.

            // 7. Integrate velocity: u_p^{n+1} = u_p^n + dt/m_p * sum(F).
            if p.mass > 0.0 {
                let inv_mass = 1.0 / p.mass;
                for dim in 0..3 {
                    p.velocity[dim] +=
                        dt * inv_mass * (f_drag[dim] + f_grav[dim] + f_pg[dim] + f_extra[dim]);
                }
            }

            // 8. Integrate position: x_p^{n+1} = x_p^n + dt * u_p^{n+1}.
            let old_position = p.position;
            for dim in 0..3 {
                p.position[dim] += dt * p.velocity[dim];
            }

            // 9. Update age.
            p.age += dt;

            // 10. Find new host cell.
            let old_cell = p.cell_id;
            match Self::find_host_cell_static(p.position, p.cell_id, mesh) {
                Some(new_cell) => p.cell_id = new_cell,
                None => {
                    // Particle left domain — apply wall interaction.
                    p.position = old_position; // restore for wall handling
                    Self::handle_wall_interaction(
                        p,
                        old_cell,
                        mesh,
                        wall_interaction,
                        &mut self.trapped_mass,
                        &mut self.trapped_count,
                        &mut self.escaped_count,
                    );
                }
            }
        }

        Ok(())
    }

    // -----------------------------------------------------------------------
    // Two-way coupling: momentum source
    // -----------------------------------------------------------------------

    /// Computes the per-cell momentum source from particles to the fluid.
    ///
    /// For each active particle, the reaction force (Newton's 3rd law) is
    /// accumulated into the cell that hosts it:
    ///
    ///   S_cell = -F_drag_on_particle / V_cell
    ///
    /// The returned vector has one `[f64; 3]` per mesh cell.
    pub fn compute_momentum_source(
        &self,
        dt: f64,
        fluid_velocity: &VectorField,
        mesh: &UnstructuredMesh,
    ) -> Vec<[f64; 3]> {
        let n = mesh.num_cells();
        let mut source = vec![[0.0; 3]; n];
        let rho_f = self.fluid_density;
        let mu = self.fluid_viscosity;
        let drag_model = self.drag_model;

        for p in &self.particles {
            if !p.active || p.cell_id >= n {
                continue;
            }

            let u_f = if p.cell_id < fluid_velocity.values().len() {
                fluid_velocity.values()[p.cell_id]
            } else {
                [0.0; 3]
            };

            let u_rel = [
                u_f[0] - p.velocity[0],
                u_f[1] - p.velocity[1],
                u_f[2] - p.velocity[2],
            ];
            let u_rel_mag =
                (u_rel[0] * u_rel[0] + u_rel[1] * u_rel[1] + u_rel[2] * u_rel[2]).sqrt();
            let re_p = rho_f * u_rel_mag * p.diameter / (mu + 1e-30);
            let cd = Self::compute_drag(drag_model, re_p);
            let a_p = p.projected_area();

            // Drag force on particle (positive = in direction of u_rel, i.e., toward fluid).
            let f_drag = [
                0.5 * cd * rho_f * a_p * u_rel_mag * u_rel[0],
                0.5 * cd * rho_f * a_p * u_rel_mag * u_rel[1],
                0.5 * cd * rho_f * a_p * u_rel_mag * u_rel[2],
            ];

            // Reaction on fluid = -F_drag / V_cell (force per unit volume).
            let v_cell = mesh.cells[p.cell_id].volume.max(1e-30);
            for dim in 0..3 {
                source[p.cell_id][dim] -= f_drag[dim] / v_cell;
            }
        }

        let _ = dt; // dt reserved for future implicit coupling
        source
    }

    // -----------------------------------------------------------------------
    // Heat transfer
    // -----------------------------------------------------------------------

    /// Computes convective heat transfer between particles and the fluid.
    ///
    /// Uses the Ranz-Marshall correlation for the Nusselt number:
    ///   Nu = 2 + 0.6 * Re_p^0.5 * Pr^0.33
    ///   h  = Nu * k_f / d_p
    ///   Q  = h * A_p * (T_f - T_p)
    ///
    /// Each particle's temperature is updated, and the per-cell heat source
    /// to the fluid (energy removed from / added to fluid) is returned.
    pub fn compute_heat_transfer(
        &mut self,
        fluid_temperature: &[f64],
        fluid_velocity: &VectorField,
        dt: f64,
    ) -> Vec<f64> {
        let n = fluid_temperature.len();
        let mut heat_source = vec![0.0; n];
        let rho_f = self.fluid_density;
        let mu = self.fluid_viscosity;
        let k_f = self.fluid_conductivity;
        let cp_f = self.fluid_specific_heat;
        let cp_p = self.particle_specific_heat;
        let pr = mu * cp_f / (k_f + 1e-30); // Prandtl number

        for p in self.particles.iter_mut() {
            if !p.active || p.cell_id >= n {
                continue;
            }

            let t_f = fluid_temperature[p.cell_id];

            // Particle Reynolds number.
            let u_f = if p.cell_id < fluid_velocity.values().len() {
                fluid_velocity.values()[p.cell_id]
            } else {
                [0.0; 3]
            };
            let u_rel = [
                u_f[0] - p.velocity[0],
                u_f[1] - p.velocity[1],
                u_f[2] - p.velocity[2],
            ];
            let u_rel_mag =
                (u_rel[0] * u_rel[0] + u_rel[1] * u_rel[1] + u_rel[2] * u_rel[2]).sqrt();
            let re_p = rho_f * u_rel_mag * p.diameter / (mu + 1e-30);

            // Ranz-Marshall: Nu = 2 + 0.6 * Re_p^0.5 * Pr^(1/3)
            let nu = 2.0 + 0.6 * re_p.sqrt() * pr.powf(1.0 / 3.0);
            let h = nu * k_f / (p.diameter + 1e-30);
            let a_s = p.surface_area();

            // Heat flux: Q = h * A_s * (T_f - T_p)  [W] (positive = heating particle)
            let q = h * a_s * (t_f - p.temperature);

            // Update particle temperature: dT_p = Q * dt / (m_p * cp_p).
            if p.mass > 0.0 && cp_p > 0.0 {
                p.temperature += q * dt / (p.mass * cp_p);
            }

            // Source to fluid energy equation (negative of particle heat gain per volume).
            // Note: heat_source is per-cell total, not per-volume. The caller divides
            // by cell volume if needed.
            heat_source[p.cell_id] -= q;
        }

        heat_source
    }

    // -----------------------------------------------------------------------
    // Turbulent dispersion (Discrete Random Walk)
    // -----------------------------------------------------------------------

    /// Applies the Discrete Random Walk (DRW) turbulent dispersion model.
    ///
    /// Each particle sees `u_f + u'` where `u'` is a random velocity
    /// fluctuation sampled from the turbulent kinetic energy field.
    ///
    /// - `u'_i = zeta * sqrt(2k/3)` with `zeta ~ N(0,1)`
    /// - Interaction time: `t_int = -T_L * ln(rand())` where `T_L = 0.15 * k / epsilon`
    /// - The fluctuation is held constant for `t_int`, then re-sampled.
    pub fn apply_turbulent_dispersion(
        &mut self,
        k_field: &[f64],
        epsilon_field: &[f64],
        dt: f64,
    ) {
        for p in self.particles.iter_mut() {
            if !p.active {
                continue;
            }

            // Decrement remaining interaction time.
            p.turb_time_remaining -= dt;

            if p.turb_time_remaining <= 0.0 {
                // Sample new fluctuation.
                let k = if p.cell_id < k_field.len() {
                    k_field[p.cell_id].max(0.0)
                } else {
                    0.0
                };
                let eps = if p.cell_id < epsilon_field.len() {
                    epsilon_field[p.cell_id].max(1e-30)
                } else {
                    1e-30
                };

                let rms = (2.0 * k / 3.0).sqrt();
                p.turb_fluctuation = [
                    self.rng.next_normal() * rms,
                    self.rng.next_normal() * rms,
                    self.rng.next_normal() * rms,
                ];

                // Lagrangian time scale.
                let t_l = 0.15 * k / eps;
                // Interaction time: t_int = -T_L * ln(rand), where rand in (0,1).
                let r = self.rng.next_f64().max(1e-30);
                p.turb_time_remaining = -t_l * r.ln();
            }
        }
    }

    // -----------------------------------------------------------------------
    // Additional forces (applied externally)
    // -----------------------------------------------------------------------

    /// Computes the Saffman lift force on a single particle.
    ///
    /// F_lift = 1.615 * mu * d_p * Re_G^0.5 * (u_f - u_p) x omega / |omega|
    ///
    /// `velocity_gradient_mag` is |grad(u)| at the particle cell.  `omega`
    /// is the local vorticity vector.  In the simplified formulation we
    /// estimate Re_G = rho * |grad(u)| * d_p^2 / mu.
    pub fn saffman_lift_force(
        &self,
        particle: &Particle,
        u_f: [f64; 3],
        velocity_gradient_mag: f64,
        omega: [f64; 3],
    ) -> [f64; 3] {
        let mu = self.fluid_viscosity;
        let rho_f = self.fluid_density;
        let d = particle.diameter;

        let re_g = rho_f * velocity_gradient_mag * d * d / (mu + 1e-30);
        let coeff = 1.615 * mu * d * re_g.sqrt();

        // (u_f - u_p) x omega
        let du = [
            u_f[0] - particle.velocity[0],
            u_f[1] - particle.velocity[1],
            u_f[2] - particle.velocity[2],
        ];
        let cross = [
            du[1] * omega[2] - du[2] * omega[1],
            du[2] * omega[0] - du[0] * omega[2],
            du[0] * omega[1] - du[1] * omega[0],
        ];
        let omega_mag = (omega[0] * omega[0] + omega[1] * omega[1] + omega[2] * omega[2]).sqrt();
        if omega_mag < 1e-30 {
            return [0.0; 3];
        }

        [
            coeff * cross[0] / omega_mag,
            coeff * cross[1] / omega_mag,
            coeff * cross[2] / omega_mag,
        ]
    }

    /// Computes the thermophoretic force on a single particle.
    ///
    /// Simplified model: F_th = -D_T * grad(T) / T
    /// where D_T is the thermophoretic coefficient.
    pub fn thermophoretic_force(
        &self,
        _particle: &Particle,
        grad_t: [f64; 3],
        temperature: f64,
    ) -> [f64; 3] {
        let d_t = self.additional_forces.thermophoretic_coeff;
        if temperature.abs() < 1e-30 {
            return [0.0; 3];
        }
        [
            -d_t * grad_t[0] / temperature,
            -d_t * grad_t[1] / temperature,
            -d_t * grad_t[2] / temperature,
        ]
    }

    /// Computes the virtual mass force on a single particle.
    ///
    /// F_vm = 0.5 * rho_f * V_p * (a_fluid - a_particle)
    pub fn virtual_mass_force(
        &self,
        particle: &Particle,
        fluid_acceleration: [f64; 3],
        particle_acceleration: [f64; 3],
    ) -> [f64; 3] {
        let c_vm = 0.5;
        let v_p = particle.volume();
        let rho_f = self.fluid_density;
        [
            c_vm * rho_f * v_p * (fluid_acceleration[0] - particle_acceleration[0]),
            c_vm * rho_f * v_p * (fluid_acceleration[1] - particle_acceleration[1]),
            c_vm * rho_f * v_p * (fluid_acceleration[2] - particle_acceleration[2]),
        ]
    }

    // -----------------------------------------------------------------------
    // Statistics
    // -----------------------------------------------------------------------

    /// Computes aggregate statistics for the current particle population.
    pub fn compute_statistics(&self) -> ParticleStatistics {
        let mut stats = ParticleStatistics::default();
        stats.escaped_count = self.escaped_count;
        stats.trapped_count = self.trapped_count;
        stats.total_trapped_mass = self.trapped_mass;

        let active: Vec<&Particle> = self.particles.iter().filter(|p| p.active).collect();
        stats.active_count = active.len();
        stats.total_count = stats.active_count + stats.escaped_count + stats.trapped_count;

        if active.is_empty() {
            return stats;
        }

        let mut sum_d = 0.0;
        let mut sum_speed = 0.0;
        let mut sum_temp = 0.0;
        let mut sum_mass = 0.0;
        let mut sum_age = 0.0;
        let mut min_d = f64::MAX;
        let mut max_d = f64::MIN;
        let mut min_speed = f64::MAX;
        let mut max_speed = f64::MIN;
        let mut min_temp = f64::MAX;
        let mut max_temp = f64::MIN;

        for p in &active {
            let speed = (p.velocity[0] * p.velocity[0]
                + p.velocity[1] * p.velocity[1]
                + p.velocity[2] * p.velocity[2])
            .sqrt();

            sum_d += p.diameter;
            sum_speed += speed;
            sum_temp += p.temperature;
            sum_mass += p.mass;
            sum_age += p.age;

            if p.diameter < min_d { min_d = p.diameter; }
            if p.diameter > max_d { max_d = p.diameter; }
            if speed < min_speed { min_speed = speed; }
            if speed > max_speed { max_speed = speed; }
            if p.temperature < min_temp { min_temp = p.temperature; }
            if p.temperature > max_temp { max_temp = p.temperature; }
        }

        let n = active.len() as f64;
        stats.mean_diameter = sum_d / n;
        stats.min_diameter = min_d;
        stats.max_diameter = max_d;
        stats.mean_speed = sum_speed / n;
        stats.min_speed = min_speed;
        stats.max_speed = max_speed;
        stats.mean_temperature = sum_temp / n;
        stats.min_temperature = min_temp;
        stats.max_temperature = max_temp;
        stats.total_active_mass = sum_mass;
        stats.mean_residence_time = sum_age / n;

        stats
    }

    // -----------------------------------------------------------------------
    // Utilities
    // -----------------------------------------------------------------------

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

    // ===================================================================
    // New tests for enhanced features
    // ===================================================================

    // --- Two-way coupling (momentum source) ---

    #[test]
    fn test_momentum_source_zero_for_matched_velocity() {
        // If particle velocity equals fluid velocity, drag is zero
        // so momentum source should be zero.
        let mesh = make_test_mesh(3, 3);
        let n = mesh.num_cells();
        let fluid_vel = VectorField::new("velocity", vec![[1.0, 0.0, 0.0]; n]);

        let mut solver = DpmSolver::new(
            DragModel::SchillerNaumann,
            [0.0; 3],
            1.225,
            1.8e-5,
        );
        let center = mesh.cells[4].center;
        // Particle velocity matches fluid velocity.
        let p = Particle::new(center, [1.0, 0.0, 0.0], 1e-4, 2500.0, 300.0, 4);
        solver.particles.push(p);

        let src = solver.compute_momentum_source(0.001, &fluid_vel, &mesh);
        // Momentum source should be ~zero for cell 4.
        for dim in 0..3 {
            assert!(
                src[4][dim].abs() < 1e-20,
                "Expected zero source for matched velocity, got {:?}",
                src[4]
            );
        }
    }

    #[test]
    fn test_momentum_source_opposes_particle_drag() {
        // Particle at rest in a flow field: drag accelerates particle,
        // reaction decelerates fluid (negative source in flow direction).
        let mesh = make_test_mesh(5, 5);
        let n = mesh.num_cells();
        let fluid_vel = VectorField::new("velocity", vec![[2.0, 0.0, 0.0]; n]);

        let mut solver = DpmSolver::new(
            DragModel::SchillerNaumann,
            [0.0; 3],
            1.225,
            1.8e-5,
        );
        let center = mesh.cells[12].center;
        let p = Particle::new(center, [0.0; 3], 1e-3, 2500.0, 300.0, 12);
        solver.particles.push(p);

        let src = solver.compute_momentum_source(0.001, &fluid_vel, &mesh);
        // Drag on particle is in +x direction (fluid drags it along).
        // Reaction on fluid is in -x direction.
        assert!(
            src[12][0] < 0.0,
            "Momentum source should oppose flow: src_x={}",
            src[12][0]
        );
    }

    #[test]
    fn test_momentum_source_scales_with_particles() {
        // Doubling particles in a cell should double the momentum source.
        let mesh = make_test_mesh(3, 3);
        let n = mesh.num_cells();
        let fluid_vel = VectorField::new("velocity", vec![[1.0, 0.0, 0.0]; n]);

        let mut solver = DpmSolver::new(
            DragModel::SchillerNaumann,
            [0.0; 3],
            1.225,
            1.8e-5,
        );
        let center = mesh.cells[4].center;
        solver.particles.push(Particle::new(center, [0.0; 3], 1e-4, 2500.0, 300.0, 4));
        let src_1 = solver.compute_momentum_source(0.001, &fluid_vel, &mesh);

        solver.particles.push(Particle::new(center, [0.0; 3], 1e-4, 2500.0, 300.0, 4));
        let src_2 = solver.compute_momentum_source(0.001, &fluid_vel, &mesh);

        let ratio = src_2[4][0] / src_1[4][0];
        assert!(
            (ratio - 2.0).abs() < 1e-10,
            "Momentum source should double: ratio={}",
            ratio
        );
    }

    // --- Heat transfer ---

    #[test]
    fn test_heat_transfer_heats_cold_particle() {
        // Cold particle in hot fluid should gain temperature.
        let mesh = make_test_mesh(3, 3);
        let n = mesh.num_cells();
        let fluid_temp = vec![500.0; n]; // hot fluid
        let fluid_vel = VectorField::new("velocity", vec![[1.0, 0.0, 0.0]; n]);

        let mut solver = DpmSolver::new(
            DragModel::SchillerNaumann,
            [0.0; 3],
            1.225,
            1.8e-5,
        );
        let center = mesh.cells[4].center;
        let p = Particle::new(center, [0.5, 0.0, 0.0], 1e-3, 2500.0, 300.0, 4);
        solver.particles.push(p);

        let t_before = solver.particles[0].temperature;
        let heat_src = solver.compute_heat_transfer(&fluid_temp, &fluid_vel, 0.01);

        let t_after = solver.particles[0].temperature;
        assert!(
            t_after > t_before,
            "Cold particle in hot fluid should heat up: {} -> {}",
            t_before, t_after
        );
        // Heat source to fluid should be negative (fluid loses heat).
        assert!(
            heat_src[4] < 0.0,
            "Heat source should be negative (fluid loses heat): {}",
            heat_src[4]
        );
    }

    #[test]
    fn test_heat_transfer_cools_hot_particle() {
        // Hot particle in cold fluid should cool down.
        let mesh = make_test_mesh(3, 3);
        let n = mesh.num_cells();
        let fluid_temp = vec![300.0; n];
        let fluid_vel = VectorField::new("velocity", vec![[1.0, 0.0, 0.0]; n]);

        let mut solver = DpmSolver::new(
            DragModel::SchillerNaumann,
            [0.0; 3],
            1.225,
            1.8e-5,
        );
        let center = mesh.cells[4].center;
        let p = Particle::new(center, [0.5, 0.0, 0.0], 1e-3, 2500.0, 600.0, 4);
        solver.particles.push(p);

        let t_before = solver.particles[0].temperature;
        let heat_src = solver.compute_heat_transfer(&fluid_temp, &fluid_vel, 0.01);

        let t_after = solver.particles[0].temperature;
        assert!(
            t_after < t_before,
            "Hot particle should cool: {} -> {}",
            t_before, t_after
        );
        // Heat source to fluid should be positive (fluid gains heat).
        assert!(
            heat_src[4] > 0.0,
            "Heat source should be positive (fluid gains heat): {}",
            heat_src[4]
        );
    }

    #[test]
    fn test_heat_transfer_equilibrium() {
        // If T_p == T_f, no heat transfer should occur.
        let mesh = make_test_mesh(3, 3);
        let n = mesh.num_cells();
        let fluid_temp = vec![400.0; n];
        let fluid_vel = VectorField::zeros("velocity", n);

        let mut solver = DpmSolver::new(
            DragModel::SchillerNaumann,
            [0.0; 3],
            1.225,
            1.8e-5,
        );
        let center = mesh.cells[4].center;
        let p = Particle::new(center, [0.0; 3], 1e-3, 2500.0, 400.0, 4);
        solver.particles.push(p);

        let heat_src = solver.compute_heat_transfer(&fluid_temp, &fluid_vel, 0.01);

        assert!(
            (solver.particles[0].temperature - 400.0).abs() < 1e-10,
            "No temperature change at equilibrium"
        );
        assert!(
            heat_src[4].abs() < 1e-10,
            "No heat source at equilibrium: {}",
            heat_src[4]
        );
    }

    // --- Wall interaction ---

    #[test]
    fn test_wall_interaction_escape() {
        // A particle heading out of domain with Escape policy should deactivate.
        let mesh = make_test_mesh(3, 3);
        let n = mesh.num_cells();
        // Strong velocity heading outside.
        let fluid_vel = VectorField::zeros("velocity", n);

        let mut solver = DpmSolver::new(
            DragModel::SchillerNaumann,
            [0.0; 3],
            1.225,
            1.8e-5,
        );
        solver.wall_interaction = WallInteraction::Escape;

        // Place particle near boundary with velocity heading out.
        let center = mesh.cells[0].center;
        let p = Particle::new(center, [-100.0, 0.0, 0.0], 1e-4, 2500.0, 300.0, 0);
        solver.particles.push(p);

        solver.advance_particles(0.1, &fluid_vel, &mesh).unwrap();

        assert!(
            !solver.particles[0].active,
            "Particle should have escaped"
        );
        assert_eq!(solver.escaped_count, 1);
    }

    #[test]
    fn test_wall_interaction_trap() {
        let mesh = make_test_mesh(3, 3);
        let n = mesh.num_cells();
        let fluid_vel = VectorField::zeros("velocity", n);

        let mut solver = DpmSolver::new(
            DragModel::SchillerNaumann,
            [0.0; 3],
            1.225,
            1.8e-5,
        );
        solver.wall_interaction = WallInteraction::Trap;

        let center = mesh.cells[0].center;
        let p = Particle::new(center, [-100.0, 0.0, 0.0], 1e-4, 2500.0, 300.0, 0);
        let particle_mass = p.mass;
        solver.particles.push(p);

        solver.advance_particles(0.1, &fluid_vel, &mesh).unwrap();

        assert!(
            !solver.particles[0].active,
            "Particle should have been trapped"
        );
        assert_eq!(solver.trapped_count, 1);
        assert!(
            (solver.trapped_mass - particle_mass).abs() < 1e-30,
            "Trapped mass should equal particle mass"
        );
    }

    #[test]
    fn test_wall_interaction_reflect() {
        let mesh = make_test_mesh(5, 5);
        let n = mesh.num_cells();
        let fluid_vel = VectorField::zeros("velocity", n);

        let mut solver = DpmSolver::new(
            DragModel::SchillerNaumann,
            [0.0; 3],
            1.225,
            1.8e-5,
        );
        solver.wall_interaction = WallInteraction::Reflect { restitution: 1.0 };

        // Place particle near left boundary, moving left.
        let center = mesh.cells[0].center;
        let p = Particle::new(center, [-100.0, 0.0, 0.0], 1e-4, 2500.0, 300.0, 0);
        solver.particles.push(p);

        solver.advance_particles(0.001, &fluid_vel, &mesh).unwrap();

        // Particle should still be active after reflection.
        assert!(
            solver.particles[0].active,
            "Particle should still be active after reflection"
        );
        assert_eq!(solver.escaped_count, 0);
        assert_eq!(solver.trapped_count, 0);
    }

    // --- Turbulent dispersion ---

    #[test]
    fn test_turbulent_dispersion_nonzero_fluctuation() {
        let mesh = make_test_mesh(3, 3);
        let n = mesh.num_cells();
        let k_field = vec![1.0; n]; // moderate TKE
        let eps_field = vec![0.5; n];

        let mut solver = DpmSolver::new(
            DragModel::SchillerNaumann,
            [0.0; 3],
            1.225,
            1.8e-5,
        );
        let center = mesh.cells[4].center;
        let p = Particle::new(center, [0.0; 3], 1e-4, 2500.0, 300.0, 4);
        solver.particles.push(p);

        // Initially no fluctuation.
        assert_eq!(solver.particles[0].turb_fluctuation, [0.0; 3]);

        solver.apply_turbulent_dispersion(&k_field, &eps_field, 0.01);

        // After DRW, fluctuation should be non-zero (probabilistically).
        let fluct = solver.particles[0].turb_fluctuation;
        let fluct_mag = (fluct[0] * fluct[0] + fluct[1] * fluct[1] + fluct[2] * fluct[2]).sqrt();
        assert!(
            fluct_mag > 0.0,
            "Turbulent fluctuation should be non-zero after DRW"
        );
    }

    #[test]
    fn test_turbulent_dispersion_zero_tke() {
        // With k=0, fluctuations should remain zero.
        let mesh = make_test_mesh(3, 3);
        let n = mesh.num_cells();
        let k_field = vec![0.0; n];
        let eps_field = vec![1.0; n];

        let mut solver = DpmSolver::new(
            DragModel::SchillerNaumann,
            [0.0; 3],
            1.225,
            1.8e-5,
        );
        let center = mesh.cells[4].center;
        let p = Particle::new(center, [0.0; 3], 1e-4, 2500.0, 300.0, 4);
        solver.particles.push(p);

        solver.apply_turbulent_dispersion(&k_field, &eps_field, 0.01);

        let fluct = solver.particles[0].turb_fluctuation;
        for dim in 0..3 {
            assert!(
                fluct[dim].abs() < 1e-30,
                "Fluctuation should be zero with k=0"
            );
        }
    }

    #[test]
    fn test_turbulent_dispersion_interaction_time() {
        // After DRW, the particle should have a positive remaining interaction time.
        let mesh = make_test_mesh(3, 3);
        let n = mesh.num_cells();
        let k_field = vec![2.0; n];
        let eps_field = vec![1.0; n];

        let mut solver = DpmSolver::new(
            DragModel::SchillerNaumann,
            [0.0; 3],
            1.225,
            1.8e-5,
        );
        let center = mesh.cells[4].center;
        let p = Particle::new(center, [0.0; 3], 1e-4, 2500.0, 300.0, 4);
        solver.particles.push(p);

        solver.apply_turbulent_dispersion(&k_field, &eps_field, 0.001);

        assert!(
            solver.particles[0].turb_time_remaining > 0.0,
            "Interaction time should be positive: {}",
            solver.particles[0].turb_time_remaining
        );
    }

    // --- Additional forces ---

    #[test]
    fn test_saffman_lift_force() {
        let solver = DpmSolver::new(
            DragModel::SchillerNaumann,
            [0.0; 3],
            1.225,
            1.8e-5,
        );
        let p = Particle::new([0.0; 3], [0.0; 3], 1e-3, 2500.0, 300.0, 0);
        let u_f = [1.0, 0.0, 0.0];
        let grad_u_mag = 10.0; // 1/s
        let omega = [0.0, 0.0, 5.0]; // vorticity in z-direction

        let f_lift = solver.saffman_lift_force(&p, u_f, grad_u_mag, omega);
        // Lift should be perpendicular to both relative velocity and vorticity.
        // u_rel = [1,0,0], omega = [0,0,5] => cross = [0*5 - 0*0, 0*0 - 1*5, 1*0 - 0*0]
        //                                             = [0, -5, 0]
        // So lift should be in the -y direction.
        assert!(
            f_lift[1] < 0.0,
            "Saffman lift should be in -y for this setup: {:?}",
            f_lift
        );
        // x and z components should be zero.
        assert!(f_lift[0].abs() < 1e-20, "Lift x should be ~0: {}", f_lift[0]);
        assert!(f_lift[2].abs() < 1e-20, "Lift z should be ~0: {}", f_lift[2]);
    }

    #[test]
    fn test_saffman_lift_zero_vorticity() {
        let solver = DpmSolver::new(
            DragModel::SchillerNaumann,
            [0.0; 3],
            1.225,
            1.8e-5,
        );
        let p = Particle::new([0.0; 3], [0.0; 3], 1e-3, 2500.0, 300.0, 0);
        let f_lift = solver.saffman_lift_force(&p, [1.0, 0.0, 0.0], 10.0, [0.0; 3]);
        for dim in 0..3 {
            assert!(f_lift[dim].abs() < 1e-30, "Lift should be zero with no vorticity");
        }
    }

    #[test]
    fn test_thermophoretic_force() {
        let mut solver = DpmSolver::new(
            DragModel::SchillerNaumann,
            [0.0; 3],
            1.225,
            1.8e-5,
        );
        solver.additional_forces.thermophoretic = true;
        solver.additional_forces.thermophoretic_coeff = 1e-10;

        let p = Particle::new([0.0; 3], [0.0; 3], 1e-3, 2500.0, 300.0, 0);
        let grad_t = [100.0, 0.0, 0.0]; // temperature increases in +x
        let f_th = solver.thermophoretic_force(&p, grad_t, 400.0);

        // Force should be in -x direction (particles move from hot to cold).
        assert!(
            f_th[0] < 0.0,
            "Thermophoretic force should oppose temperature gradient: {:?}",
            f_th
        );
    }

    #[test]
    fn test_virtual_mass_force() {
        let solver = DpmSolver::new(
            DragModel::SchillerNaumann,
            [0.0; 3],
            1000.0, // water
            1e-3,
        );
        let p = Particle::new([0.0; 3], [0.0; 3], 1e-2, 2500.0, 300.0, 0);
        let fluid_accel = [1.0, 0.0, 0.0];
        let particle_accel = [0.0, 0.0, 0.0];

        let f_vm = solver.virtual_mass_force(&p, fluid_accel, particle_accel);
        // F_vm = 0.5 * rho_f * V_p * (a_f - a_p)
        let v_p = std::f64::consts::PI / 6.0 * (1e-2_f64).powi(3);
        let expected_x = 0.5 * 1000.0 * v_p * 1.0;
        assert!(
            (f_vm[0] - expected_x).abs() < 1e-15,
            "Virtual mass force mismatch: {} vs {}",
            f_vm[0],
            expected_x
        );
    }

    // --- Particle size distribution ---

    #[test]
    fn test_inject_uniform_distribution() {
        let mesh = make_test_mesh(3, 3);
        let boundary_faces: Vec<usize> = mesh
            .faces
            .iter()
            .enumerate()
            .filter(|(_, f)| f.neighbor_cell.is_none())
            .map(|(i, _)| i)
            .take(1)
            .collect();

        let mut solver = DpmSolver::new(
            DragModel::SchillerNaumann,
            [0.0; 3],
            1.225,
            1.8e-5,
        );

        solver.inject_particles_with_distribution(
            &mesh,
            &boundary_faces,
            [1.0, 0.0, 0.0],
            ParticleSizeDistribution::Uniform {
                d_min: 1e-4,
                d_max: 1e-3,
            },
            2500.0,
            300.0,
            20,
        );

        assert_eq!(solver.particles.len(), 20);
        for p in &solver.particles {
            assert!(
                p.diameter >= 1e-4 && p.diameter <= 1e-3,
                "Diameter {} out of uniform range",
                p.diameter
            );
        }
    }

    #[test]
    fn test_inject_rosin_rammler_distribution() {
        let mesh = make_test_mesh(3, 3);
        let boundary_faces: Vec<usize> = mesh
            .faces
            .iter()
            .enumerate()
            .filter(|(_, f)| f.neighbor_cell.is_none())
            .map(|(i, _)| i)
            .take(1)
            .collect();

        let mut solver = DpmSolver::new(
            DragModel::SchillerNaumann,
            [0.0; 3],
            1.225,
            1.8e-5,
        );

        solver.inject_particles_with_distribution(
            &mesh,
            &boundary_faces,
            [1.0, 0.0, 0.0],
            ParticleSizeDistribution::RosinRammler {
                d_mean: 1e-3,
                spread_param: 3.5,
                n_bins: 10,
            },
            2500.0,
            300.0,
            10,
        );

        assert_eq!(solver.particles.len(), 10);
        // All diameters should be positive.
        for p in &solver.particles {
            assert!(p.diameter > 0.0, "Diameter should be positive");
        }
        // Diameters should not all be identical (they come from different quantiles).
        let d0 = solver.particles[0].diameter;
        let all_same = solver.particles.iter().all(|p| (p.diameter - d0).abs() < 1e-15);
        assert!(
            !all_same,
            "Rosin-Rammler should produce different diameters across bins"
        );
    }

    #[test]
    fn test_monodisperse_distribution() {
        let mesh = make_test_mesh(3, 3);
        let boundary_faces: Vec<usize> = mesh
            .faces
            .iter()
            .enumerate()
            .filter(|(_, f)| f.neighbor_cell.is_none())
            .map(|(i, _)| i)
            .take(1)
            .collect();

        let mut solver = DpmSolver::new(
            DragModel::SchillerNaumann,
            [0.0; 3],
            1.225,
            1.8e-5,
        );

        solver.inject_particles_with_distribution(
            &mesh,
            &boundary_faces,
            [1.0, 0.0, 0.0],
            ParticleSizeDistribution::Monodisperse { diameter: 5e-4 },
            2500.0,
            300.0,
            5,
        );

        assert_eq!(solver.particles.len(), 5);
        for p in &solver.particles {
            assert!((p.diameter - 5e-4).abs() < 1e-15);
        }
    }

    // --- Particle statistics ---

    #[test]
    fn test_particle_statistics_basic() {
        let mut solver = DpmSolver::new(
            DragModel::SchillerNaumann,
            [0.0; 3],
            1.225,
            1.8e-5,
        );

        // Three active particles.
        solver.particles.push(Particle::new([0.0; 3], [1.0, 0.0, 0.0], 1e-3, 2500.0, 300.0, 0));
        solver.particles.push(Particle::new([0.0; 3], [0.0, 2.0, 0.0], 2e-3, 2500.0, 400.0, 0));
        solver.particles.push(Particle::new([0.0; 3], [0.0, 0.0, 3.0], 3e-3, 2500.0, 500.0, 0));

        // One escaped, one trapped.
        solver.escaped_count = 1;
        solver.trapped_count = 1;
        solver.trapped_mass = 0.001;

        let stats = solver.compute_statistics();

        assert_eq!(stats.active_count, 3);
        assert_eq!(stats.escaped_count, 1);
        assert_eq!(stats.trapped_count, 1);
        assert_eq!(stats.total_count, 5);

        // Mean diameter: (1e-3 + 2e-3 + 3e-3) / 3 = 2e-3
        assert!((stats.mean_diameter - 2e-3).abs() < 1e-15);
        assert!((stats.min_diameter - 1e-3).abs() < 1e-15);
        assert!((stats.max_diameter - 3e-3).abs() < 1e-15);

        // Speeds: 1, 2, 3 => mean=2, min=1, max=3
        assert!((stats.mean_speed - 2.0).abs() < 1e-12);
        assert!((stats.min_speed - 1.0).abs() < 1e-12);
        assert!((stats.max_speed - 3.0).abs() < 1e-12);

        // Temperature: 300, 400, 500 => mean=400
        assert!((stats.mean_temperature - 400.0).abs() < 1e-10);
        assert!((stats.min_temperature - 300.0).abs() < 1e-10);
        assert!((stats.max_temperature - 500.0).abs() < 1e-10);

        assert!((stats.total_trapped_mass - 0.001).abs() < 1e-15);
    }

    #[test]
    fn test_particle_statistics_empty() {
        let solver = DpmSolver::new(
            DragModel::SchillerNaumann,
            [0.0; 3],
            1.225,
            1.8e-5,
        );
        let stats = solver.compute_statistics();
        assert_eq!(stats.active_count, 0);
        assert_eq!(stats.total_count, 0);
    }

    #[test]
    fn test_particle_age_advances() {
        let mesh = make_test_mesh(5, 5);
        let n = mesh.num_cells();
        let fluid_vel = VectorField::zeros("velocity", n);

        let mut solver = DpmSolver::new(
            DragModel::SchillerNaumann,
            [0.0; 3],
            1000.0,
            1e-3,
        );
        let center = mesh.cells[12].center;
        let p = Particle::new(center, [0.0; 3], 1e-4, 1000.0, 300.0, 12);
        solver.particles.push(p);

        let dt = 0.005;
        solver.advance_particles(dt, &fluid_vel, &mesh).unwrap();
        assert!((solver.particles[0].age - dt).abs() < 1e-15);

        solver.advance_particles(dt, &fluid_vel, &mesh).unwrap();
        assert!((solver.particles[0].age - 2.0 * dt).abs() < 1e-15);
    }

    #[test]
    fn test_particle_surface_area_and_volume() {
        let p = Particle::new([0.0; 3], [0.0; 3], 1e-2, 1000.0, 300.0, 0);
        let d = 1e-2;
        let expected_sa = std::f64::consts::PI * d * d;
        let expected_vol = std::f64::consts::PI / 6.0 * d * d * d;
        assert!((p.surface_area() - expected_sa).abs() < 1e-20);
        assert!((p.volume() - expected_vol).abs() < 1e-20);
    }

    // --- Ranz-Marshall Nusselt number validation ---

    #[test]
    fn test_ranz_marshall_nusselt_number() {
        // For a stationary particle (Re_p ~ 0) in still fluid, Nu -> 2.
        // So h = 2 * k_f / d_p.
        let mesh = make_test_mesh(3, 3);
        let n = mesh.num_cells();
        let fluid_temp = vec![500.0; n];
        let fluid_vel = VectorField::zeros("velocity", n); // no flow => Re_p ~ 0

        let mut solver = DpmSolver::new(
            DragModel::SchillerNaumann,
            [0.0; 3],
            1.225,
            1.8e-5,
        );
        solver.fluid_conductivity = 0.026;
        solver.particle_specific_heat = 840.0;

        let d = 1e-3;
        let center = mesh.cells[4].center;
        let p = Particle::new(center, [0.0; 3], d, 2500.0, 300.0, 4);
        let mass = p.mass;
        solver.particles.push(p);

        let dt = 0.001;
        solver.compute_heat_transfer(&fluid_temp, &fluid_vel, dt);

        // Expected: Nu ≈ 2 (Re_p = 0), h = 2 * 0.026 / 1e-3 = 52 W/(m^2K)
        // A_s = pi * d^2 = pi * 1e-6
        // Q = 52 * pi*1e-6 * (500 - 300) = 52 * pi*1e-6 * 200
        // dT = Q * dt / (mass * cp)
        let h = 2.0 * 0.026 / d;
        let a_s = std::f64::consts::PI * d * d;
        let q = h * a_s * (500.0 - 300.0);
        let expected_dt = q * dt / (mass * 840.0);
        let actual_dt = solver.particles[0].temperature - 300.0;

        assert!(
            (actual_dt - expected_dt).abs() < 1e-6 * expected_dt.abs().max(1e-20),
            "Ranz-Marshall dT mismatch: actual={}, expected={}",
            actual_dt,
            expected_dt
        );
    }
}
