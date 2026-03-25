//! Fluid material models.

pub mod newtonian;
pub mod non_newtonian;
pub mod ideal_gas;

use crate::traits::MaterialProperty;

/// A complete fluid material definition.
#[derive(Debug)]
pub struct FluidMaterial {
    /// Density model.
    pub density: Box<dyn MaterialProperty>,
    /// Dynamic viscosity model.
    pub viscosity: Box<dyn MaterialProperty>,
    /// Specific heat capacity model.
    pub specific_heat: Box<dyn MaterialProperty>,
    /// Thermal conductivity model.
    pub conductivity: Box<dyn MaterialProperty>,
}

impl FluidMaterial {
    /// Creates a new fluid material from the given property models.
    pub fn new(
        density: Box<dyn MaterialProperty>,
        viscosity: Box<dyn MaterialProperty>,
        specific_heat: Box<dyn MaterialProperty>,
        conductivity: Box<dyn MaterialProperty>,
    ) -> Self {
        Self {
            density,
            viscosity,
            specific_heat,
            conductivity,
        }
    }
}
