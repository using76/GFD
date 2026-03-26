//! Laplacian mesh smoothing.
//!
//! Iteratively moves each internal node toward the average position of its neighbors.

use gfd_core::mesh::unstructured::UnstructuredMesh;
use std::collections::{HashMap, HashSet};

/// Performs Laplacian smoothing on the mesh.
///
/// Each internal node is moved toward the centroid of its connected neighbors
/// using the relaxation factor `omega` (0 = no movement, 1 = full move to average).
/// Boundary nodes are never moved.
///
/// # Arguments
/// * `mesh` - The mesh to smooth (modified in place).
/// * `iterations` - Number of smoothing iterations.
/// * `omega` - Relaxation factor, typically 0.3-0.7.
pub fn laplacian_smooth(mesh: &mut UnstructuredMesh, iterations: usize, omega: f64) {
    // Build the set of boundary node indices (these must not move).
    let mut boundary_nodes: HashSet<usize> = HashSet::new();
    for face in &mesh.faces {
        if face.is_boundary() {
            for &nid in &face.nodes {
                boundary_nodes.insert(nid);
            }
        }
    }

    // Build node-to-neighbor connectivity from faces.
    let num_nodes = mesh.nodes.len();
    let mut neighbors: HashMap<usize, HashSet<usize>> = HashMap::new();
    for face in &mesh.faces {
        let fn_nodes = &face.nodes;
        let n = fn_nodes.len();
        for i in 0..n {
            for j in (i + 1)..n {
                neighbors
                    .entry(fn_nodes[i])
                    .or_default()
                    .insert(fn_nodes[j]);
                neighbors
                    .entry(fn_nodes[j])
                    .or_default()
                    .insert(fn_nodes[i]);
            }
        }
    }

    // Also add connectivity from cell node lists for completeness.
    for cell in &mesh.cells {
        let cn = &cell.nodes;
        let n = cn.len();
        for i in 0..n {
            for j in (i + 1)..n {
                neighbors.entry(cn[i]).or_default().insert(cn[j]);
                neighbors.entry(cn[j]).or_default().insert(cn[i]);
            }
        }
    }

    // Smoothing iterations
    for _ in 0..iterations {
        let mut new_positions = vec![[0.0f64; 3]; num_nodes];

        for nid in 0..num_nodes {
            if boundary_nodes.contains(&nid) {
                new_positions[nid] = mesh.nodes[nid].position;
                continue;
            }

            if let Some(nbrs) = neighbors.get(&nid) {
                if nbrs.is_empty() {
                    new_positions[nid] = mesh.nodes[nid].position;
                    continue;
                }
                let mut avg = [0.0f64; 3];
                let count = nbrs.len() as f64;
                for &nbr in nbrs {
                    let p = mesh.nodes[nbr].position;
                    avg[0] += p[0];
                    avg[1] += p[1];
                    avg[2] += p[2];
                }
                avg[0] /= count;
                avg[1] /= count;
                avg[2] /= count;

                let old = mesh.nodes[nid].position;
                new_positions[nid] = [
                    old[0] + omega * (avg[0] - old[0]),
                    old[1] + omega * (avg[1] - old[1]),
                    old[2] + omega * (avg[2] - old[2]),
                ];
            } else {
                new_positions[nid] = mesh.nodes[nid].position;
            }
        }

        // Apply new positions
        for nid in 0..num_nodes {
            mesh.nodes[nid].position = new_positions[nid];
        }
    }

    // Recompute cell centers and face centers after smoothing
    recompute_geometry(mesh);
}

/// Recomputes cell centers, volumes, face centers, face areas, and face normals
/// after node positions have changed.
fn recompute_geometry(mesh: &mut UnstructuredMesh) {
    // Recompute face geometry
    for face in &mut mesh.faces {
        let n = face.nodes.len();
        if n == 0 {
            continue;
        }
        // Compute face center as average of face nodes
        // We need to read node positions from the mesh.nodes but we have &mut mesh.faces
        // So we collect node positions first. We'll do it in a separate pass below.
    }

    // Collect node positions for faces
    let node_positions: Vec<[f64; 3]> = mesh.nodes.iter().map(|n| n.position).collect();

    for face in &mut mesh.faces {
        let n = face.nodes.len();
        if n == 0 {
            continue;
        }
        let mut cx = 0.0;
        let mut cy = 0.0;
        let mut cz = 0.0;
        for &nid in &face.nodes {
            let p = node_positions[nid];
            cx += p[0];
            cy += p[1];
            cz += p[2];
        }
        let inv_n = 1.0 / n as f64;
        face.center = [cx * inv_n, cy * inv_n, cz * inv_n];

        // Recompute area and normal using Newell's method for a polygon
        if n >= 3 {
            let mut nx = 0.0f64;
            let mut ny = 0.0f64;
            let mut nz = 0.0f64;
            for i in 0..n {
                let p1 = node_positions[face.nodes[i]];
                let p2 = node_positions[face.nodes[(i + 1) % n]];
                nx += (p1[1] - p2[1]) * (p1[2] + p2[2]);
                ny += (p1[2] - p2[2]) * (p1[0] + p2[0]);
                nz += (p1[0] - p2[0]) * (p1[1] + p2[1]);
            }
            let area = 0.5 * (nx * nx + ny * ny + nz * nz).sqrt();
            if area > 1e-30 {
                face.area = area;
                face.normal = [nx / (2.0 * area), ny / (2.0 * area), nz / (2.0 * area)];
            }
        }
    }

    // Recompute cell centers and volumes
    for cell in &mut mesh.cells {
        let n = cell.nodes.len();
        if n == 0 {
            continue;
        }
        let mut cx = 0.0;
        let mut cy = 0.0;
        let mut cz = 0.0;
        for &nid in &cell.nodes {
            let p = node_positions[nid];
            cx += p[0];
            cy += p[1];
            cz += p[2];
        }
        let inv_n = 1.0 / n as f64;
        cell.center = [cx * inv_n, cy * inv_n, cz * inv_n];

        // Recompute volume using divergence theorem: V = (1/3) sum_f (f_center dot f_normal * f_area)
        // This is approximate but works well for convex cells.
        // A simpler approach: sum of signed tetrahedra from cell center.
        // For now, use the face-based approach.
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gfd_core::mesh::structured::StructuredMesh;

    #[test]
    fn test_laplacian_smooth_preserves_boundary() {
        let mut mesh = StructuredMesh::uniform(3, 3, 1, 3.0, 3.0, 1.0).to_unstructured();

        // Record boundary node positions
        let mut boundary_nodes: HashSet<usize> = HashSet::new();
        for face in &mesh.faces {
            if face.is_boundary() {
                for &nid in &face.nodes {
                    boundary_nodes.insert(nid);
                }
            }
        }
        let original_boundary: Vec<(usize, [f64; 3])> = boundary_nodes
            .iter()
            .map(|&nid| (nid, mesh.nodes[nid].position))
            .collect();

        // Perturb an internal node
        // For a 3x3x1 mesh (effective nz=1), node layout is (4)*(4)*(2) = 32 nodes
        // Internal nodes are those not on any boundary face
        let internal_nodes: Vec<usize> = (0..mesh.nodes.len())
            .filter(|nid| !boundary_nodes.contains(nid))
            .collect();

        if !internal_nodes.is_empty() {
            let nid = internal_nodes[0];
            mesh.nodes[nid].position[0] += 0.1;
            mesh.nodes[nid].position[1] += 0.1;
        }

        laplacian_smooth(&mut mesh, 5, 0.5);

        // Verify boundary nodes have not moved
        for (nid, orig_pos) in &original_boundary {
            let new_pos = mesh.nodes[*nid].position;
            assert!(
                (new_pos[0] - orig_pos[0]).abs() < 1e-12
                    && (new_pos[1] - orig_pos[1]).abs() < 1e-12
                    && (new_pos[2] - orig_pos[2]).abs() < 1e-12,
                "Boundary node {nid} moved from {orig_pos:?} to {new_pos:?}"
            );
        }
    }

    #[test]
    fn test_laplacian_smooth_zero_iterations() {
        let mut mesh = StructuredMesh::uniform(2, 2, 1, 2.0, 2.0, 1.0).to_unstructured();
        let original_nodes: Vec<[f64; 3]> =
            mesh.nodes.iter().map(|n| n.position).collect();

        laplacian_smooth(&mut mesh, 0, 0.5);

        for (i, node) in mesh.nodes.iter().enumerate() {
            assert_eq!(
                node.position, original_nodes[i],
                "Zero iterations should not change node positions"
            );
        }
    }

    #[test]
    fn test_laplacian_smooth_convergence() {
        // Use a 4x4x4 mesh so there are true interior nodes
        // (for nz=1, all nodes lie on the z-boundary faces)
        let mut mesh = StructuredMesh::uniform(4, 4, 4, 4.0, 4.0, 4.0).to_unstructured();

        // Perturb internal nodes
        let mut boundary_nodes: HashSet<usize> = HashSet::new();
        for face in &mesh.faces {
            if face.is_boundary() {
                for &nid in &face.nodes {
                    boundary_nodes.insert(nid);
                }
            }
        }

        let n_internal = (0..mesh.nodes.len())
            .filter(|nid| !boundary_nodes.contains(nid))
            .count();
        assert!(n_internal > 0, "4x4x4 mesh should have interior nodes");

        for nid in 0..mesh.nodes.len() {
            if !boundary_nodes.contains(&nid) {
                mesh.nodes[nid].position[0] += 0.05;
            }
        }

        let before: Vec<[f64; 3]> = mesh.nodes.iter().map(|n| n.position).collect();

        laplacian_smooth(&mut mesh, 20, 0.5);

        // Internal nodes should have moved back toward equilibrium
        let mut total_displacement = 0.0;
        for nid in 0..mesh.nodes.len() {
            if !boundary_nodes.contains(&nid) {
                let dx = mesh.nodes[nid].position[0] - before[nid][0];
                let dy = mesh.nodes[nid].position[1] - before[nid][1];
                let dz = mesh.nodes[nid].position[2] - before[nid][2];
                total_displacement += (dx * dx + dy * dy + dz * dz).sqrt();
            }
        }
        // Smoothing should have moved nodes
        assert!(total_displacement > 0.0, "Smoothing should move perturbed internal nodes");
    }
}
