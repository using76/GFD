//! Mesh-to-GPU data transfer utilities.
//!
//! Converts the AoS (Array of Structures) `UnstructuredMesh` into a SoA
//! (Structure of Arrays) layout suitable for GPU kernels.

use crate::device::GpuDeviceHandle;
use crate::memory::GpuVector;
use crate::Result;
use gfd_core::mesh::unstructured::UnstructuredMesh;

/// Mesh data laid out in Structure-of-Arrays format for GPU access.
///
/// Integer indices (owner, neighbor) are stored as `f64` for uniform GPU
/// vector storage.  Boundary faces use `usize::MAX` (cast to f64) as the
/// neighbour sentinel.
pub struct MeshGpuData {
    /// Face owner cell index (as f64).
    pub face_owner: GpuVector,
    /// Face neighbour cell index (as f64; sentinel for boundary faces).
    pub face_neighbor: GpuVector,
    /// Face outward normal — x component.
    pub face_normal_x: GpuVector,
    /// Face outward normal — y component.
    pub face_normal_y: GpuVector,
    /// Face outward normal — z component.
    pub face_normal_z: GpuVector,
    /// Face area.
    pub face_area: GpuVector,
    /// Cell volume.
    pub cell_volume: GpuVector,
    /// Number of faces.
    pub num_faces: usize,
    /// Number of cells.
    pub num_cells: usize,
}

impl std::fmt::Debug for MeshGpuData {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MeshGpuData")
            .field("num_faces", &self.num_faces)
            .field("num_cells", &self.num_cells)
            .finish()
    }
}

/// Convert an `UnstructuredMesh` to SoA layout and upload to the given device.
///
/// When compiled without the `cuda` feature the data is simply stored in CPU
/// `Vec`s wrapped inside `GpuVector`.
pub fn upload_mesh(
    mesh: &UnstructuredMesh,
    device: &GpuDeviceHandle,
) -> Result<MeshGpuData> {
    let nf = mesh.num_faces();
    let nc = mesh.num_cells();

    let mut owner = Vec::with_capacity(nf);
    let mut neighbor = Vec::with_capacity(nf);
    let mut normal_x = Vec::with_capacity(nf);
    let mut normal_y = Vec::with_capacity(nf);
    let mut normal_z = Vec::with_capacity(nf);
    let mut area = Vec::with_capacity(nf);

    for face in &mesh.faces {
        owner.push(face.owner_cell as f64);
        neighbor.push(face.neighbor_cell.unwrap_or(0) as f64);
        normal_x.push(face.normal[0]);
        normal_y.push(face.normal[1]);
        normal_z.push(face.normal[2]);
        area.push(face.area);
    }

    let mut volume = Vec::with_capacity(nc);
    for cell in &mesh.cells {
        volume.push(cell.volume);
    }

    Ok(MeshGpuData {
        face_owner: GpuVector::from_cpu(&owner, device)?,
        face_neighbor: GpuVector::from_cpu(&neighbor, device)?,
        face_normal_x: GpuVector::from_cpu(&normal_x, device)?,
        face_normal_y: GpuVector::from_cpu(&normal_y, device)?,
        face_normal_z: GpuVector::from_cpu(&normal_z, device)?,
        face_area: GpuVector::from_cpu(&area, device)?,
        cell_volume: GpuVector::from_cpu(&volume, device)?,
        num_faces: nf,
        num_cells: nc,
    })
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use gfd_core::mesh::cell::Cell;
    use gfd_core::mesh::face::Face;
    use gfd_core::mesh::node::Node;

    /// Smoke test: upload a tiny 2-cell mesh and verify round-trip.
    #[test]
    fn test_upload_mesh_cpu_fallback() {
        let nodes = vec![
            Node::new(0, [0.0, 0.0, 0.0]),
            Node::new(1, [1.0, 0.0, 0.0]),
            Node::new(2, [1.0, 1.0, 0.0]),
            Node::new(3, [0.0, 1.0, 0.0]),
        ];

        let faces = vec![
            Face::new(0, vec![0, 1], 0, Some(1), 1.0, [1.0, 0.0, 0.0], [0.5, 0.0, 0.0]),
            Face::new(1, vec![1, 2], 0, None, 1.0, [0.0, 1.0, 0.0], [1.0, 0.5, 0.0]),
            Face::new(2, vec![2, 3], 1, None, 1.0, [-1.0, 0.0, 0.0], [0.5, 1.0, 0.0]),
        ];

        let cells = vec![
            Cell::new(0, vec![0, 1, 2, 3], vec![0, 1], 1.0, [0.25, 0.25, 0.0]),
            Cell::new(1, vec![0, 1, 2, 3], vec![0, 2], 1.0, [0.75, 0.75, 0.0]),
        ];

        let mesh = UnstructuredMesh::from_components(nodes, faces, cells, vec![]);
        let device = GpuDeviceHandle::cpu_fallback();
        let gpu_mesh = upload_mesh(&mesh, &device).unwrap();

        assert_eq!(gpu_mesh.num_faces, 3);
        assert_eq!(gpu_mesh.num_cells, 2);

        // Verify owner of face 0 is cell 0.
        let mut owner_host = vec![0.0; 3];
        gpu_mesh.face_owner.to_cpu(&mut owner_host).unwrap();
        assert!((owner_host[0] - 0.0).abs() < 1e-15);
    }
}
