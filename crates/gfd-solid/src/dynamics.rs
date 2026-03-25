//! Structural dynamics solvers.
//!
//! Solves: M * a + C * v + K * u = f(t)

use gfd_core::UnstructuredMesh;
use crate::{SolidState, Result};

/// Newmark-beta time integration scheme for structural dynamics.
///
/// The Newmark family of methods:
/// - beta = 0.25, gamma = 0.5: unconditionally stable, no numerical damping
/// - beta = 0.3025, gamma = 0.6: unconditionally stable with numerical damping
pub struct NewmarkBeta {
    /// Newmark parameter beta (controls displacement approximation).
    pub beta: f64,
    /// Newmark parameter gamma (controls velocity approximation).
    pub gamma: f64,
    /// Previous velocity field.
    prev_velocity: Option<Vec<[f64; 3]>>,
    /// Previous acceleration field.
    prev_acceleration: Option<Vec<[f64; 3]>>,
}

impl NewmarkBeta {
    /// Creates a new Newmark-beta integrator.
    pub fn new(beta: f64, gamma: f64) -> Self {
        Self {
            beta,
            gamma,
            prev_velocity: None,
            prev_acceleration: None,
        }
    }

    /// Creates an average acceleration (trapezoidal rule) integrator.
    ///
    /// beta = 0.25, gamma = 0.5 (unconditionally stable, no damping).
    pub fn average_acceleration() -> Self {
        Self::new(0.25, 0.5)
    }

    /// Creates a linear acceleration integrator.
    ///
    /// beta = 1/6, gamma = 0.5 (conditionally stable).
    pub fn linear_acceleration() -> Self {
        Self::new(1.0 / 6.0, 0.5)
    }

    /// Performs one dynamic time step.
    ///
    /// Effective stiffness: K_eff = K + a0*M + a1*C
    /// Effective force: f_eff = f + M*(a0*u_n + a2*v_n + a3*a_n) + C*(a1*u_n + a4*v_n + a5*a_n)
    ///
    /// where a0 = 1/(beta*dt^2), a1 = gamma/(beta*dt), etc.
    pub fn solve_dynamic_step(
        &mut self,
        _state: &mut SolidState,
        _mesh: &UnstructuredMesh,
        _external_forces: &[[f64; 3]],
        _dt: f64,
    ) -> Result<f64> {
        // Newmark coefficients:
        // a0 = 1 / (beta * dt^2)
        // a1 = gamma / (beta * dt)
        // a2 = 1 / (beta * dt)
        // a3 = 1 / (2*beta) - 1
        // a4 = gamma / beta - 1
        // a5 = dt/2 * (gamma/beta - 2)
        //
        // 1. Form effective stiffness: K_eff = K + a0*M + a1*C
        // 2. Form effective force: f_eff = f(t+dt) + M*(a0*u_n + a2*v_n + a3*a_n)
        //                                         + C*(a1*u_n + a4*v_n + a5*a_n)
        // 3. Solve: K_eff * u_{n+1} = f_eff
        // 4. Update acceleration: a_{n+1} = a0*(u_{n+1} - u_n) - a2*v_n - a3*a_n
        // 5. Update velocity: v_{n+1} = v_n + dt*((1-gamma)*a_n + gamma*a_{n+1})
        let dt = _dt;
        let beta = self.beta;
        let gamma = self.gamma;
        let num_cells = _state.num_cells();

        // Initialize previous velocity and acceleration if first step
        if self.prev_velocity.is_none() {
            self.prev_velocity = Some(vec![[0.0; 3]; num_cells]);
        }
        if self.prev_acceleration.is_none() {
            self.prev_acceleration = Some(vec![[0.0; 3]; num_cells]);
        }

        let prev_vel = self.prev_velocity.as_ref().unwrap().clone();
        let prev_acc = self.prev_acceleration.as_ref().unwrap().clone();

        // Newmark coefficients
        let a0 = 1.0 / (beta * dt * dt);
        let a2 = 1.0 / (beta * dt);
        let a3 = 1.0 / (2.0 * beta) - 1.0;

        let displacements = _state.displacement.values();

        // Simplified: assume unit mass, no damping, unit stiffness
        // Effective load: F_eff = F_ext + M*(a0*u_n + a2*v_n + a3*a_n)
        // Solve: K_eff * u_{n+1} = F_eff
        // where K_eff = K + a0*M (simplified to 1 + a0)
        let k_eff = 1.0 + a0;

        let mut max_change = 0.0_f64;
        let mut new_disp = vec![[0.0_f64; 3]; num_cells];

        for i in 0..num_cells {
            let u_n = displacements[i];
            let v_n = prev_vel[i];
            let a_n = prev_acc[i];

            for dim in 0..3 {
                let f_ext = if i < _external_forces.len() {
                    _external_forces[i][dim]
                } else {
                    0.0
                };
                let f_eff = f_ext + a0 * u_n[dim] + a2 * v_n[dim] + a3 * a_n[dim];
                let u_new = f_eff / k_eff;
                new_disp[i][dim] = u_new;
                let change = (u_new - u_n[dim]).abs();
                if change > max_change {
                    max_change = change;
                }
            }
        }

        // Update acceleration and velocity
        let mut new_vel = vec![[0.0_f64; 3]; num_cells];
        let mut new_acc = vec![[0.0_f64; 3]; num_cells];

        for i in 0..num_cells {
            let u_n = displacements[i];
            let v_n = prev_vel[i];
            let a_n = prev_acc[i];

            for dim in 0..3 {
                // a_{n+1} = a0*(u_{n+1} - u_n) - a2*v_n - a3*a_n
                new_acc[i][dim] = a0 * (new_disp[i][dim] - u_n[dim]) - a2 * v_n[dim] - a3 * a_n[dim];
                // v_{n+1} = v_n + dt*((1-gamma)*a_n + gamma*a_{n+1})
                new_vel[i][dim] = v_n[dim] + dt * ((1.0 - gamma) * a_n[dim] + gamma * new_acc[i][dim]);
            }
        }

        // Update state
        for i in 0..num_cells {
            let _ = _state.displacement.set(i, new_disp[i]);
        }
        self.prev_velocity = Some(new_vel);
        self.prev_acceleration = Some(new_acc);

        Ok(max_change)
    }
}
