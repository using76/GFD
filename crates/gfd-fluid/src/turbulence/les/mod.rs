//! Large Eddy Simulation (LES) subgrid-scale model adapter.
//!
//! Provides SGS viscosity computation for LES simulations.

use gfd_core::{ScalarField, VectorField, UnstructuredMesh};
use crate::Result;

/// Type of LES subgrid-scale model.
#[derive(Debug, Clone, Copy)]
pub enum LesModel {
    /// Smagorinsky model: nu_sgs = (C_s * Delta)^2 * |S|
    Smagorinsky { cs: f64 },
    /// Dynamic Smagorinsky with Germano identity for computing C_s dynamically.
    DynamicSmagorinsky,
    /// WALE (Wall-Adapting Local Eddy-viscosity) model.
    Wale { cw: f64 },
    /// Sigma model.
    Sigma { c_sigma: f64 },
}

impl Default for LesModel {
    fn default() -> Self {
        LesModel::Smagorinsky { cs: 0.1 }
    }
}

/// LES solver adapter for computing subgrid-scale viscosity.
///
/// Wraps an LES model to compute the subgrid-scale (SGS) eddy viscosity
/// that models the effect of unresolved turbulent scales on the resolved flow.
pub struct LesSolver {
    /// The subgrid-scale model to use.
    pub model: LesModel,
}

impl LesSolver {
    /// Creates a new LES solver with the given SGS model.
    pub fn new(model: LesModel) -> Self {
        Self { model }
    }

    /// Creates a new LES solver with the standard Smagorinsky model.
    pub fn smagorinsky(cs: f64) -> Self {
        Self {
            model: LesModel::Smagorinsky { cs },
        }
    }

    /// Creates a new LES solver with the WALE model.
    pub fn wale(cw: f64) -> Self {
        Self {
            model: LesModel::Wale { cw },
        }
    }

    /// Computes the subgrid-scale viscosity field.
    ///
    /// For Smagorinsky: nu_sgs = (C_s * Delta)^2 * |S|
    /// where Delta is the filter width (cube root of cell volume)
    /// and |S| = sqrt(2 * S_ij * S_ij) is the strain rate magnitude.
    ///
    /// For WALE: uses the traceless symmetric part of the square of the
    /// velocity gradient tensor for better near-wall behavior.
    pub fn compute_sgs_viscosity(
        &self,
        velocity: &VectorField,
        mesh: &UnstructuredMesh,
    ) -> Result<ScalarField> {
        use gfd_core::gradient::{GreenGaussCellBasedGradient, GradientComputer};

        let n = mesh.num_cells();
        let grad_computer = GreenGaussCellBasedGradient;

        // Compute velocity gradients for all models that need them
        let ux = ScalarField::new("ux", velocity.values().iter().map(|v| v[0]).collect());
        let uy = ScalarField::new("uy", velocity.values().iter().map(|v| v[1]).collect());
        let uz = ScalarField::new("uz", velocity.values().iter().map(|v| v[2]).collect());

        let grad_ux = grad_computer.compute(&ux, mesh).map_err(crate::FluidError::CoreError)?;
        let grad_uy = grad_computer.compute(&uy, mesh).map_err(crate::FluidError::CoreError)?;
        let grad_uz = grad_computer.compute(&uz, mesh).map_err(crate::FluidError::CoreError)?;

        match self.model {
            LesModel::Smagorinsky { cs } => {
                let mut nu_sgs = vec![0.0; n];
                for i in 0..n {
                    let gux = grad_ux.values()[i];
                    let guy = grad_uy.values()[i];
                    let guz = grad_uz.values()[i];

                    // Strain rate tensor S_ij = 0.5*(dui/dxj + duj/dxi)
                    // |S| = sqrt(2 * S_ij * S_ij)
                    let g = [[gux[0], gux[1], gux[2]],
                             [guy[0], guy[1], guy[2]],
                             [guz[0], guz[1], guz[2]]];
                    let mut s_sq = 0.0;
                    for ii in 0..3 {
                        for jj in 0..3 {
                            let s_ij = 0.5 * (g[ii][jj] + g[jj][ii]);
                            s_sq += s_ij * s_ij;
                        }
                    }
                    let s_mag = (2.0 * s_sq).sqrt(); // |S| = sqrt(2 * S_ij * S_ij)

                    // Filter width delta = V^(1/3)
                    let delta = mesh.cells[i].volume.cbrt();

                    // nu_sgs = (Cs * delta)^2 * |S|
                    nu_sgs[i] = (cs * delta).powi(2) * s_mag;
                }
                Ok(ScalarField::new("nu_sgs", nu_sgs))
            }
            LesModel::DynamicSmagorinsky => {
                // Simplified: use Cs = 0.1 (Germano identity would compute this dynamically)
                let cs = 0.1;
                let mut nu_sgs = vec![0.0; n];
                for i in 0..n {
                    let gux = grad_ux.values()[i];
                    let guy = grad_uy.values()[i];
                    let guz = grad_uz.values()[i];

                    let g = [[gux[0], gux[1], gux[2]],
                             [guy[0], guy[1], guy[2]],
                             [guz[0], guz[1], guz[2]]];
                    let mut s_sq = 0.0;
                    for ii in 0..3 {
                        for jj in 0..3 {
                            let s_ij = 0.5 * (g[ii][jj] + g[jj][ii]);
                            s_sq += s_ij * s_ij;
                        }
                    }
                    let s_mag = (2.0 * s_sq).sqrt();
                    let delta = mesh.cells[i].volume.cbrt();
                    nu_sgs[i] = (cs * delta).powi(2) * s_mag;
                }
                Ok(ScalarField::new("nu_sgs", nu_sgs))
            }
            LesModel::Wale { cw } => {
                let mut nu_sgs = vec![0.0; n];
                for i in 0..n {
                    let gux = grad_ux.values()[i];
                    let guy = grad_uy.values()[i];
                    let guz = grad_uz.values()[i];

                    let g = [[gux[0], gux[1], gux[2]],
                             [guy[0], guy[1], guy[2]],
                             [guz[0], guz[1], guz[2]]];

                    // Compute g^2 = g_ik * g_kj
                    let mut g2 = [[0.0; 3]; 3];
                    for ii in 0..3 {
                        for jj in 0..3 {
                            for kk in 0..3 {
                                g2[ii][jj] += g[ii][kk] * g[kk][jj];
                            }
                        }
                    }

                    // Trace of g^2
                    let trace_g2 = g2[0][0] + g2[1][1] + g2[2][2];

                    // S^d_ij = 0.5*(g^2_ij + g^2_ji) - (1/3)*delta_ij*trace(g^2)
                    let mut sd_sq = 0.0; // S^d_ij * S^d_ij
                    let mut s_sq = 0.0;  // S_ij * S_ij
                    for ii in 0..3 {
                        for jj in 0..3 {
                            let delta_ij = if ii == jj { 1.0 } else { 0.0 };
                            let sd_ij = 0.5 * (g2[ii][jj] + g2[jj][ii])
                                - (1.0 / 3.0) * delta_ij * trace_g2;
                            sd_sq += sd_ij * sd_ij;

                            let s_ij = 0.5 * (g[ii][jj] + g[jj][ii]);
                            s_sq += s_ij * s_ij;
                        }
                    }

                    let delta = mesh.cells[i].volume.cbrt();

                    // WALE: nu_sgs = (Cw*delta)^2 * (Sd:Sd)^(3/2) / ((S:S)^(5/2) + (Sd:Sd)^(5/4) + 1e-30)
                    let numerator = sd_sq.powf(1.5);
                    let denominator = s_sq.powf(2.5) + sd_sq.powf(1.25) + 1e-30;
                    nu_sgs[i] = (cw * delta).powi(2) * numerator / denominator;
                }
                Ok(ScalarField::new("nu_sgs", nu_sgs))
            }
            LesModel::Sigma { c_sigma } => {
                // Sigma model: compute singular values of velocity gradient tensor
                let mut nu_sgs = vec![0.0; n];
                for i in 0..n {
                    let gux = grad_ux.values()[i];
                    let guy = grad_uy.values()[i];
                    let guz = grad_uz.values()[i];

                    let g = [[gux[0], gux[1], gux[2]],
                             [guy[0], guy[1], guy[2]],
                             [guz[0], guz[1], guz[2]]];

                    // Compute G = g^T * g (symmetric positive semi-definite)
                    let mut gg = [[0.0; 3]; 3];
                    for ii in 0..3 {
                        for jj in 0..3 {
                            for kk in 0..3 {
                                gg[ii][jj] += g[kk][ii] * g[kk][jj];
                            }
                        }
                    }

                    // Compute eigenvalues of 3x3 symmetric matrix using analytical formula
                    // The singular values of g are sqrt(eigenvalues of g^T*g)
                    let p1 = gg[0][1] * gg[0][1] + gg[0][2] * gg[0][2] + gg[1][2] * gg[1][2];
                    let (sigma1, sigma2, sigma3) = if p1 < 1e-30 {
                        // gg is diagonal
                        let mut eigs = [gg[0][0], gg[1][1], gg[2][2]];
                        eigs.sort_by(|a, b| b.partial_cmp(a).unwrap());
                        (eigs[0].max(0.0).sqrt(), eigs[1].max(0.0).sqrt(), eigs[2].max(0.0).sqrt())
                    } else {
                        let q = (gg[0][0] + gg[1][1] + gg[2][2]) / 3.0;
                        let p2 = (gg[0][0] - q).powi(2) + (gg[1][1] - q).powi(2) + (gg[2][2] - q).powi(2) + 2.0 * p1;
                        let p = (p2 / 6.0).sqrt();
                        // B = (1/p) * (gg - q*I)
                        let b00 = (gg[0][0] - q) / p;
                        let b11 = (gg[1][1] - q) / p;
                        let b22 = (gg[2][2] - q) / p;
                        let b01 = gg[0][1] / p;
                        let b02 = gg[0][2] / p;
                        let b12 = gg[1][2] / p;
                        // det(B)
                        let det_b = b00 * (b11 * b22 - b12 * b12)
                            - b01 * (b01 * b22 - b12 * b02)
                            + b02 * (b01 * b12 - b11 * b02);
                        let r = det_b / 2.0;
                        let phi_angle = if r <= -1.0 {
                            std::f64::consts::PI / 3.0
                        } else if r >= 1.0 {
                            0.0
                        } else {
                            r.acos() / 3.0
                        };
                        let eig1 = q + 2.0 * p * phi_angle.cos();
                        let eig3 = q + 2.0 * p * (phi_angle + 2.0 * std::f64::consts::PI / 3.0).cos();
                        let eig2 = 3.0 * q - eig1 - eig3;
                        (eig1.max(0.0).sqrt(), eig2.max(0.0).sqrt(), eig3.max(0.0).sqrt())
                    };

                    // Sort: sigma1 >= sigma2 >= sigma3
                    let mut sigmas = [sigma1, sigma2, sigma3];
                    sigmas.sort_by(|a, b| b.partial_cmp(a).unwrap());
                    let (s1, s2, s3) = (sigmas[0], sigmas[1], sigmas[2]);

                    let delta = mesh.cells[i].volume.cbrt();

                    if s1 > 1e-30 {
                        let d_sigma = s3 * (s1 - s2) * (s2 - s3) / (s1 * s1);
                        nu_sgs[i] = (c_sigma * delta).powi(2) * d_sigma.max(0.0);
                    }
                }
                Ok(ScalarField::new("nu_sgs", nu_sgs))
            }
        }
    }
}
