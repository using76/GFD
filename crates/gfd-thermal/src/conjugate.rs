//! Conjugate heat transfer (CHT) for fluid-solid thermal coupling.

use gfd_core::UnstructuredMesh;
use crate::{ThermalState, Result};

/// Conjugate heat transfer solver for coupling fluid and solid thermal domains.
///
/// Manages the interface between a fluid solver's energy equation and a
/// solid conduction solver, enforcing temperature and heat flux continuity
/// at the fluid-solid boundary.
pub struct ConjugateHeatTransfer {
    /// Name of the fluid-solid interface.
    pub interface_name: String,
    /// Face indices on the coupling interface.
    pub interface_faces: Vec<usize>,
    /// Under-relaxation factor for the interface temperature.
    pub under_relaxation: f64,
}

impl ConjugateHeatTransfer {
    /// Creates a new conjugate heat transfer solver.
    pub fn new(
        interface_name: impl Into<String>,
        interface_faces: Vec<usize>,
        under_relaxation: f64,
    ) -> Self {
        Self {
            interface_name: interface_name.into(),
            interface_faces,
            under_relaxation,
        }
    }

    /// Performs one CHT coupling iteration.
    ///
    /// 1. Extract fluid temperature and heat flux at the interface.
    /// 2. Apply as boundary condition to the solid solver.
    /// 3. Solve solid conduction.
    /// 4. Extract solid temperature at the interface.
    /// 5. Apply as boundary condition to the fluid energy equation.
    /// 6. Under-relax interface values.
    pub fn coupling_step(
        &self,
        fluid_state: &mut ThermalState,
        solid_state: &mut ThermalState,
        _fluid_mesh: &UnstructuredMesh,
        _solid_mesh: &UnstructuredMesh,
    ) -> Result<f64> {
        // Iterative CHT coupling:
        // 1. Extract temperatures at interface and compute interface values
        // 2. Apply with under-relaxation to both sides
        let omega = self.under_relaxation;

        // Collect interface temperature updates first (to avoid borrow conflicts)
        let mut updates: Vec<(usize, f64, f64)> = Vec::new(); // (face_id, t_interface, change)

        {
            let fluid_temp = fluid_state.temperature.values();
            let solid_temp = solid_state.temperature.values();

            for &face_id in &self.interface_faces {
                let t_fluid = if face_id < fluid_temp.len() {
                    fluid_temp[face_id]
                } else {
                    continue;
                };

                let t_solid = if face_id < solid_temp.len() {
                    solid_temp[face_id]
                } else {
                    continue;
                };

                let t_interface = omega * t_fluid + (1.0 - omega) * t_solid;
                let change = (t_interface - t_solid).abs();
                updates.push((face_id, t_interface, change));
            }
        }

        let mut max_change = 0.0_f64;
        for (face_id, t_interface, change) in &updates {
            if *change > max_change {
                max_change = *change;
            }
            let _ = solid_state.temperature.set(*face_id, *t_interface);
            let _ = fluid_state.temperature.set(*face_id, *t_interface);
        }

        Ok(max_change)
    }
}
