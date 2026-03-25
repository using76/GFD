//! Radiation heat transfer models.

pub mod p1;
pub mod discrete_ordinates;
pub mod view_factor;

use gfd_core::{ScalarField, UnstructuredMesh};
use crate::Result;

/// Trait for radiation heat transfer models.
pub trait RadiationModel {
    /// Solves the radiation transport equations and returns the radiative source term.
    ///
    /// The returned scalar field contains the volumetric radiative source [W/m^3]
    /// to be added to the energy equation.
    fn solve(
        &mut self,
        temperature: &ScalarField,
        mesh: &UnstructuredMesh,
    ) -> Result<ScalarField>;

    /// Returns the name of the radiation model.
    fn name(&self) -> &str;
}
