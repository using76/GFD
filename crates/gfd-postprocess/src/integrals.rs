//! Integral quantities computed over mesh regions.

use gfd_core::{ScalarField, UnstructuredMesh};
use gfd_core::field::Field;

/// Computes the surface integral of a scalar field over specified faces.
///
/// integral = sum_f(field[owner(f)] * area(f))
pub fn surface_integral(
    field: &ScalarField,
    face_ids: &[usize],
    mesh: &UnstructuredMesh,
) -> f64 {
    face_ids
        .iter()
        .filter_map(|&fid| {
            mesh.faces.get(fid).map(|face| {
                let cell_idx = face.owner_cell;
                let value = field.values().get(cell_idx).copied().unwrap_or(0.0);
                value * face.area
            })
        })
        .sum()
}

/// Computes the volume integral of a scalar field over the entire mesh.
///
/// integral = sum_i(field[i] * volume[i])
pub fn volume_integral(field: &ScalarField, mesh: &UnstructuredMesh) -> f64 {
    field
        .values()
        .iter()
        .enumerate()
        .map(|(i, &val)| {
            let volume = mesh.cells.get(i).map(|c| c.volume).unwrap_or(0.0);
            val * volume
        })
        .sum()
}

/// Computes the area-weighted average of a scalar field over specified faces.
///
/// avg = sum_f(field[owner(f)] * area(f)) / sum_f(area(f))
pub fn area_weighted_average(
    field: &ScalarField,
    face_ids: &[usize],
    mesh: &UnstructuredMesh,
) -> f64 {
    let mut weighted_sum = 0.0;
    let mut total_area = 0.0;

    for &fid in face_ids {
        if let Some(face) = mesh.faces.get(fid) {
            let cell_idx = face.owner_cell;
            let value = field.values().get(cell_idx).copied().unwrap_or(0.0);
            weighted_sum += value * face.area;
            total_area += face.area;
        }
    }

    if total_area > 0.0 {
        weighted_sum / total_area
    } else {
        0.0
    }
}

/// Computes the mass-weighted (volume-weighted) average of a scalar field.
///
/// avg = sum_i(field[i] * density[i] * volume[i]) / sum_i(density[i] * volume[i])
pub fn mass_weighted_average(
    field: &ScalarField,
    density: &ScalarField,
    mesh: &UnstructuredMesh,
) -> f64 {
    let field_vals = field.values();
    let density_vals = density.values();
    let n = field.len().min(density.len()).min(mesh.cells.len());

    let mut weighted_sum = 0.0;
    let mut total_mass = 0.0;

    for i in 0..n {
        let vol = mesh.cells[i].volume;
        let mass = density_vals[i] * vol;
        weighted_sum += field_vals[i] * mass;
        total_mass += mass;
    }

    if total_mass > 0.0 {
        weighted_sum / total_mass
    } else {
        0.0
    }
}
