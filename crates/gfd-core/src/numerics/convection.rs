//! Convective flux computation functions.

use crate::field::{ScalarField, VectorField};
use crate::mesh::unstructured::UnstructuredMesh;
use crate::Result;
use super::ConvectionScheme;

/// Computes convective fluxes for a scalar field given a velocity field.
///
/// Returns the convective flux contribution for each cell as a scalar field.
pub fn compute_convective_flux(
    phi: &ScalarField,
    velocity: &VectorField,
    mesh: &UnstructuredMesh,
    scheme: ConvectionScheme,
) -> Result<ScalarField> {
    let num_cells = mesh.num_cells();
    let phi_vals = phi.values();
    let _vel_vals = velocity.values();

    // First compute face fluxes
    let face_fluxes = compute_face_fluxes(velocity, mesh)?;

    let mut flux_sum = vec![0.0_f64; num_cells];

    for face_id in 0..mesh.num_faces() {
        let face = &mesh.faces[face_id];
        let owner = face.owner_cell;
        let ff = face_fluxes[face_id];

        if let Some(neighbor) = face.neighbor_cell {
            let phi_o = phi_vals[owner];
            let phi_n = phi_vals[neighbor];

            let face_flux_val = match scheme {
                ConvectionScheme::Central => central_flux(ff, phi_o, phi_n),
                _ => upwind_flux(ff, phi_o, phi_n), // Default to upwind
            };

            flux_sum[owner] += face_flux_val;
            flux_sum[neighbor] -= face_flux_val;
        } else {
            // Boundary face: use owner value
            let phi_o = phi_vals[owner];
            let face_flux_val = ff * phi_o;
            flux_sum[owner] += face_flux_val;
        }
    }

    // Divide by cell volume for per-unit-volume contribution
    for i in 0..num_cells {
        flux_sum[i] /= mesh.cells[i].volume;
    }

    Ok(ScalarField::new("convective_flux", flux_sum))
}

/// Computes the face flux (mass flow rate) for each face from the velocity field.
///
/// Returns a vector of face fluxes (velocity dot face normal times face area).
pub fn compute_face_fluxes(
    velocity: &VectorField,
    mesh: &UnstructuredMesh,
) -> Result<Vec<f64>> {
    let vel_vals = velocity.values();
    let mut face_fluxes = vec![0.0_f64; mesh.num_faces()];

    for face_id in 0..mesh.num_faces() {
        let face = &mesh.faces[face_id];
        let owner = face.owner_cell;

        // Interpolate velocity to face
        let vel_f = if let Some(neighbor) = face.neighbor_cell {
            let vo = vel_vals[owner];
            let vn = vel_vals[neighbor];
            [0.5 * (vo[0] + vn[0]), 0.5 * (vo[1] + vn[1]), 0.5 * (vo[2] + vn[2])]
        } else {
            vel_vals[owner]
        };

        // Face flux = velocity dot face normal times face area
        face_fluxes[face_id] = (vel_f[0] * face.normal[0]
            + vel_f[1] * face.normal[1]
            + vel_f[2] * face.normal[2])
            * face.area;
    }

    Ok(face_fluxes)
}

/// Computes the first-order upwind convective flux for a single face.
///
/// Given the face flux, owner value, and neighbor value, returns the
/// convective contribution.
pub fn upwind_flux(face_flux: f64, phi_owner: f64, phi_neighbor: f64) -> f64 {
    if face_flux >= 0.0 {
        face_flux * phi_owner
    } else {
        face_flux * phi_neighbor
    }
}

/// Computes the central difference convective flux for a single face.
pub fn central_flux(face_flux: f64, phi_owner: f64, phi_neighbor: f64) -> f64 {
    face_flux * 0.5 * (phi_owner + phi_neighbor)
}
