//! ALE (Arbitrary Lagrangian-Eulerian) mesh velocity computation.
//!
//! Computes the mesh velocity field from the difference between old and new mesh positions.

use gfd_core::field::vector::VectorField;
use gfd_core::mesh::unstructured::UnstructuredMesh;

/// Computes the mesh velocity field from the difference between two mesh states.
///
/// For each node, the velocity is `u_mesh = (x_new - x_old) / dt`. The nodal velocities
/// are then interpolated to cell centers using simple averaging.
///
/// # Arguments
/// * `mesh_old` - The mesh at the previous time step.
/// * `mesh_new` - The mesh at the current time step.
/// * `dt` - The time step size.
///
/// # Returns
/// A `VectorField` with one velocity vector per cell, representing the mesh velocity
/// at cell centers.
///
/// # Panics
/// Panics if the two meshes have different numbers of nodes or cells.
pub fn compute_mesh_velocity(
    mesh_old: &UnstructuredMesh,
    mesh_new: &UnstructuredMesh,
    dt: f64,
) -> VectorField {
    assert_eq!(
        mesh_old.nodes.len(),
        mesh_new.nodes.len(),
        "Old and new mesh must have the same number of nodes"
    );
    assert_eq!(
        mesh_old.cells.len(),
        mesh_new.cells.len(),
        "Old and new mesh must have the same number of cells"
    );
    assert!(dt > 0.0, "Time step must be positive");

    let inv_dt = 1.0 / dt;
    let n_nodes = mesh_old.nodes.len();

    // Compute node velocities
    let mut node_velocity = vec![[0.0f64; 3]; n_nodes];
    for i in 0..n_nodes {
        let p_old = mesh_old.nodes[i].position;
        let p_new = mesh_new.nodes[i].position;
        node_velocity[i] = [
            (p_new[0] - p_old[0]) * inv_dt,
            (p_new[1] - p_old[1]) * inv_dt,
            (p_new[2] - p_old[2]) * inv_dt,
        ];
    }

    // Interpolate to cell centers (average of cell node velocities)
    let n_cells = mesh_new.cells.len();
    let mut cell_velocity = Vec::with_capacity(n_cells);

    for cell in &mesh_new.cells {
        let n = cell.nodes.len() as f64;
        if n < 1.0 {
            cell_velocity.push([0.0, 0.0, 0.0]);
            continue;
        }
        let mut vx = 0.0f64;
        let mut vy = 0.0f64;
        let mut vz = 0.0f64;
        for &nid in &cell.nodes {
            let nv = node_velocity[nid];
            vx += nv[0];
            vy += nv[1];
            vz += nv[2];
        }
        cell_velocity.push([vx / n, vy / n, vz / n]);
    }

    VectorField::new("mesh_velocity", cell_velocity)
}

/// Computes the swept volume (geometric conservation law) for each face.
///
/// For ALE formulations, the face swept volume is needed for the GCL.
/// It is computed as the volume swept by the face during the time step.
///
/// # Arguments
/// * `mesh_old` - The mesh at the previous time step.
/// * `mesh_new` - The mesh at the current time step.
///
/// # Returns
/// A vector with one swept volume per face.
pub fn compute_face_swept_volume(
    mesh_old: &UnstructuredMesh,
    mesh_new: &UnstructuredMesh,
) -> Vec<f64> {
    assert_eq!(
        mesh_old.faces.len(),
        mesh_new.faces.len(),
        "Old and new mesh must have the same number of faces"
    );

    let n_faces = mesh_old.faces.len();
    let mut swept_volumes = Vec::with_capacity(n_faces);

    for fi in 0..n_faces {
        let face_old = &mesh_old.faces[fi];
        let face_new = &mesh_new.faces[fi];

        // Approximate swept volume: face_center_displacement dot face_normal * face_area
        // Using average of old and new normals and areas.
        let disp = [
            face_new.center[0] - face_old.center[0],
            face_new.center[1] - face_old.center[1],
            face_new.center[2] - face_old.center[2],
        ];

        let avg_normal = [
            0.5 * (face_old.normal[0] + face_new.normal[0]),
            0.5 * (face_old.normal[1] + face_new.normal[1]),
            0.5 * (face_old.normal[2] + face_new.normal[2]),
        ];
        let avg_area = 0.5 * (face_old.area + face_new.area);

        let swept = (disp[0] * avg_normal[0]
            + disp[1] * avg_normal[1]
            + disp[2] * avg_normal[2])
            * avg_area;

        swept_volumes.push(swept);
    }

    swept_volumes
}

#[cfg(test)]
mod tests {
    use super::*;
    use gfd_core::mesh::structured::StructuredMesh;

    #[test]
    fn test_zero_velocity_same_mesh() {
        let mesh = StructuredMesh::uniform(3, 3, 1, 3.0, 3.0, 1.0).to_unstructured();
        let vel = compute_mesh_velocity(&mesh, &mesh, 1.0);
        for v in vel.values() {
            assert!(
                v[0].abs() < 1e-12 && v[1].abs() < 1e-12 && v[2].abs() < 1e-12,
                "Same mesh should give zero velocity, got {v:?}"
            );
        }
    }

    #[test]
    fn test_uniform_translation() {
        let mesh_old = StructuredMesh::uniform(2, 2, 1, 2.0, 2.0, 1.0).to_unstructured();
        let mut mesh_new = mesh_old.clone();

        // Translate all nodes by [1, 0, 0]
        for node in &mut mesh_new.nodes {
            node.position[0] += 1.0;
        }

        let dt = 0.5;
        let vel = compute_mesh_velocity(&mesh_old, &mesh_new, dt);

        // Expected velocity: [1.0/0.5, 0, 0] = [2.0, 0, 0]
        for v in vel.values() {
            assert!(
                (v[0] - 2.0).abs() < 1e-12,
                "x-velocity should be 2.0, got {}",
                v[0]
            );
            assert!(v[1].abs() < 1e-12, "y-velocity should be 0");
            assert!(v[2].abs() < 1e-12, "z-velocity should be 0");
        }
    }

    #[test]
    fn test_swept_volume_no_motion() {
        let mesh = StructuredMesh::uniform(2, 2, 1, 2.0, 2.0, 1.0).to_unstructured();
        let swept = compute_face_swept_volume(&mesh, &mesh);
        for &sv in &swept {
            assert!(
                sv.abs() < 1e-12,
                "No motion should give zero swept volume, got {sv}"
            );
        }
    }

    #[test]
    fn test_mesh_velocity_field_size() {
        let mesh = StructuredMesh::uniform(3, 3, 1, 3.0, 3.0, 1.0).to_unstructured();
        let vel = compute_mesh_velocity(&mesh, &mesh, 1.0);
        assert_eq!(
            vel.values().len(),
            mesh.cells.len(),
            "Velocity field should have one value per cell"
        );
    }

    #[test]
    #[should_panic(expected = "Time step must be positive")]
    fn test_negative_dt_panics() {
        let mesh = StructuredMesh::uniform(2, 2, 1, 2.0, 2.0, 1.0).to_unstructured();
        compute_mesh_velocity(&mesh, &mesh, -1.0);
    }
}
