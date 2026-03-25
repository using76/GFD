//! Diffusive flux computation functions.

use crate::field::ScalarField;
use crate::mesh::unstructured::UnstructuredMesh;
use crate::Result;
use super::DiffusionScheme;

/// Computes diffusive fluxes for a scalar field with given diffusivity.
///
/// Returns the diffusive flux contribution for each cell as a scalar field.
pub fn compute_diffusive_flux(
    phi: &ScalarField,
    gamma: f64,
    mesh: &UnstructuredMesh,
    _scheme: DiffusionScheme,
) -> Result<ScalarField> {
    let num_cells = mesh.num_cells();
    let phi_vals = phi.values();
    let mut flux_sum = vec![0.0_f64; num_cells];

    for face_id in 0..mesh.num_faces() {
        let face = &mesh.faces[face_id];
        let owner = face.owner_cell;

        if let Some(neighbor) = face.neighbor_cell {
            // Distance between cell centers
            let dx = mesh.cells[neighbor].center[0] - mesh.cells[owner].center[0];
            let dy = mesh.cells[neighbor].center[1] - mesh.cells[owner].center[1];
            let dz = mesh.cells[neighbor].center[2] - mesh.cells[owner].center[2];
            let distance = (dx * dx + dy * dy + dz * dz).sqrt().max(1e-30);

            let diff_flux = orthogonal_diffusion_flux(
                gamma,
                face.area,
                phi_vals[owner],
                phi_vals[neighbor],
                distance,
            );

            flux_sum[owner] += diff_flux;
            flux_sum[neighbor] -= diff_flux;
        }
        // Boundary faces: zero-gradient (no diffusive flux contribution)
    }

    // Divide by cell volume
    for i in 0..num_cells {
        flux_sum[i] /= mesh.cells[i].volume;
    }

    Ok(ScalarField::new("diffusive_flux", flux_sum))
}

/// Computes the diffusive flux for a scalar field with a spatially varying diffusivity.
///
/// `gamma` is a scalar field of diffusivity values at cell centers.
pub fn compute_diffusive_flux_variable(
    phi: &ScalarField,
    gamma: &ScalarField,
    mesh: &UnstructuredMesh,
    _scheme: DiffusionScheme,
) -> Result<ScalarField> {
    let num_cells = mesh.num_cells();
    let phi_vals = phi.values();
    let gamma_vals = gamma.values();
    let mut flux_sum = vec![0.0_f64; num_cells];

    for face_id in 0..mesh.num_faces() {
        let face = &mesh.faces[face_id];
        let owner = face.owner_cell;

        if let Some(neighbor) = face.neighbor_cell {
            // Harmonic mean of diffusivity at face
            let go = gamma_vals[owner];
            let gn = gamma_vals[neighbor];
            let gamma_f = if (go + gn).abs() > 1e-30 {
                2.0 * go * gn / (go + gn)
            } else {
                0.0
            };

            let dx = mesh.cells[neighbor].center[0] - mesh.cells[owner].center[0];
            let dy = mesh.cells[neighbor].center[1] - mesh.cells[owner].center[1];
            let dz = mesh.cells[neighbor].center[2] - mesh.cells[owner].center[2];
            let distance = (dx * dx + dy * dy + dz * dz).sqrt().max(1e-30);

            let diff_flux = orthogonal_diffusion_flux(
                gamma_f,
                face.area,
                phi_vals[owner],
                phi_vals[neighbor],
                distance,
            );

            flux_sum[owner] += diff_flux;
            flux_sum[neighbor] -= diff_flux;
        }
    }

    for i in 0..num_cells {
        flux_sum[i] /= mesh.cells[i].volume;
    }

    Ok(ScalarField::new("diffusive_flux", flux_sum))
}

/// Computes the orthogonal diffusive flux for a single internal face.
///
/// Uses simple two-point gradient approximation: gamma * A * (phi_N - phi_P) / d.
pub fn orthogonal_diffusion_flux(
    gamma: f64,
    area: f64,
    phi_owner: f64,
    phi_neighbor: f64,
    distance: f64,
) -> f64 {
    gamma * area * (phi_neighbor - phi_owner) / distance
}
