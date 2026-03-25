//! Finite Volume Method discretization.

pub mod convection;
pub mod diffusion;
pub mod temporal;
pub mod source;
pub mod gradient;
pub mod interpolation;

use gfd_core::{ConvectionScheme, DiffusionScheme, TemporalScheme, GradientMethod};
use serde::{Deserialize, Serialize};

/// Collection of numerical schemes used for FVM discretization.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FvmSchemes {
    /// Convective flux scheme.
    pub convection: ConvectionScheme,
    /// Diffusive flux scheme.
    pub diffusion: DiffusionScheme,
    /// Temporal discretization scheme.
    pub temporal: TemporalScheme,
    /// Gradient reconstruction method.
    pub gradient: GradientMethod,
}

impl Default for FvmSchemes {
    fn default() -> Self {
        Self {
            convection: ConvectionScheme::FirstOrderUpwind,
            diffusion: DiffusionScheme::Central,
            temporal: TemporalScheme::Euler,
            gradient: GradientMethod::GreenGaussCellBased,
        }
    }
}
