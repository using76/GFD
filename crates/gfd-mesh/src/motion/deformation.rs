//! Spring-based mesh deformation.
//!
//! Uses the spring analogy method to propagate boundary displacements
//! into the interior of the mesh while preserving mesh quality.

use gfd_core::mesh::unstructured::UnstructuredMesh;

use std::collections::{HashMap, HashSet};

use crate::Result;

/// Deforms the mesh by propagating boundary displacements into the interior
/// using the spring analogy method.
///
/// For each internal node, the new position is computed iteratively:
/// ```text
/// x_new = x + omega * sum(k_ij * (x_j - x)) / sum(k_ij)
/// ```
/// where `k_ij = 1 / |x_i - x_j|` is the inverse-distance spring stiffness.
///
/// # Arguments
/// * `mesh` - The mesh to deform (modified in place).
/// * `boundary_displacement` - Map from boundary patch name to displacement vectors
///   for each node in that patch (indexed by position within the patch's face nodes).
/// * `iterations` - Number of relaxation iterations.
///
/// # Returns
/// `Ok(())` on success.
pub fn deform_mesh(
    mesh: &mut UnstructuredMesh,
    boundary_displacement: &HashMap<String, Vec<[f64; 3]>>,
    iterations: usize,
) -> Result<()> {
    let omega = 0.5; // relaxation factor

    // Identify boundary nodes and their prescribed displacements
    let mut node_displacement: HashMap<usize, [f64; 3]> = HashMap::new();
    let mut boundary_nodes: HashSet<usize> = HashSet::new();

    // First, collect all boundary nodes (even those without displacement)
    for face in &mesh.faces {
        if face.is_boundary() {
            for &nid in &face.nodes {
                boundary_nodes.insert(nid);
            }
        }
    }

    // Apply prescribed displacements for named patches
    for patch in &mesh.boundary_patches {
        if let Some(displacements) = boundary_displacement.get(&patch.name) {
            // Collect unique nodes from this patch in order
            let mut patch_nodes: Vec<usize> = Vec::new();
            let mut seen: HashSet<usize> = HashSet::new();
            for &fid in &patch.face_ids {
                for &nid in &mesh.faces[fid].nodes {
                    if seen.insert(nid) {
                        patch_nodes.push(nid);
                    }
                }
            }

            // Apply displacements to patch nodes
            for (i, &nid) in patch_nodes.iter().enumerate() {
                if i < displacements.len() {
                    node_displacement.insert(nid, displacements[i]);
                }
            }
        }
    }

    // Apply boundary displacements immediately
    for (&nid, &disp) in &node_displacement {
        mesh.nodes[nid].position[0] += disp[0];
        mesh.nodes[nid].position[1] += disp[1];
        mesh.nodes[nid].position[2] += disp[2];
    }

    // Build node-to-neighbor connectivity from cell nodes
    let num_nodes = mesh.nodes.len();
    let mut neighbors: Vec<Vec<usize>> = vec![Vec::new(); num_nodes];

    for cell in &mesh.cells {
        let cn = &cell.nodes;
        let n = cn.len();
        for i in 0..n {
            for j in (i + 1)..n {
                if !neighbors[cn[i]].contains(&cn[j]) {
                    neighbors[cn[i]].push(cn[j]);
                }
                if !neighbors[cn[j]].contains(&cn[i]) {
                    neighbors[cn[j]].push(cn[i]);
                }
            }
        }
    }

    // Iterative relaxation for internal nodes
    for _ in 0..iterations {
        let positions: Vec<[f64; 3]> = mesh.nodes.iter().map(|n| n.position).collect();
        let mut new_positions = positions.clone();

        for nid in 0..num_nodes {
            if boundary_nodes.contains(&nid) {
                continue; // Boundary nodes are fixed
            }

            let nbrs = &neighbors[nid];
            if nbrs.is_empty() {
                continue;
            }

            let xi = positions[nid];
            let mut sum_k = 0.0f64;
            let mut sum_kx = [0.0f64; 3];

            for &nbr in nbrs {
                let xj = positions[nbr];
                let dist = ((xj[0] - xi[0]).powi(2)
                    + (xj[1] - xi[1]).powi(2)
                    + (xj[2] - xi[2]).powi(2))
                .sqrt();
                if dist < 1e-30 {
                    continue;
                }
                let k = 1.0 / dist; // Inverse distance stiffness
                sum_k += k;
                sum_kx[0] += k * xj[0];
                sum_kx[1] += k * xj[1];
                sum_kx[2] += k * xj[2];
            }

            if sum_k > 1e-30 {
                let avg = [sum_kx[0] / sum_k, sum_kx[1] / sum_k, sum_kx[2] / sum_k];
                new_positions[nid][0] = xi[0] + omega * (avg[0] - xi[0]);
                new_positions[nid][1] = xi[1] + omega * (avg[1] - xi[1]);
                new_positions[nid][2] = xi[2] + omega * (avg[2] - xi[2]);
            }
        }

        // Apply new positions
        for nid in 0..num_nodes {
            mesh.nodes[nid].position = new_positions[nid];
        }
    }

    // Recompute cell centers
    for cell in &mut mesh.cells {
        let n = cell.nodes.len() as f64;
        let mut cx = 0.0;
        let mut cy = 0.0;
        let mut cz = 0.0;
        for &nid in &cell.nodes {
            let p = mesh.nodes[nid].position;
            cx += p[0];
            cy += p[1];
            cz += p[2];
        }
        cell.center = [cx / n, cy / n, cz / n];
    }

    // Recompute face centers
    for face in &mut mesh.faces {
        let n = face.nodes.len() as f64;
        if n < 1.0 {
            continue;
        }
        let mut cx = 0.0;
        let mut cy = 0.0;
        let mut cz = 0.0;
        for &nid in &face.nodes {
            let p = mesh.nodes[nid].position;
            cx += p[0];
            cy += p[1];
            cz += p[2];
        }
        face.center = [cx / n, cy / n, cz / n];
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use gfd_core::mesh::structured::StructuredMesh;

    #[test]
    fn test_deform_no_displacement() {
        let mut mesh = StructuredMesh::uniform(3, 3, 1, 3.0, 3.0, 1.0).to_unstructured();
        let original_positions: Vec<[f64; 3]> =
            mesh.nodes.iter().map(|n| n.position).collect();

        let displacements: HashMap<String, Vec<[f64; 3]>> = HashMap::new();
        deform_mesh(&mut mesh, &displacements, 10).unwrap();

        // No displacement => no change
        for (i, node) in mesh.nodes.iter().enumerate() {
            let orig = original_positions[i];
            let new = node.position;
            let diff = ((new[0] - orig[0]).powi(2)
                + (new[1] - orig[1]).powi(2)
                + (new[2] - orig[2]).powi(2))
            .sqrt();
            assert!(
                diff < 1e-10,
                "Node {i} should not move without displacement, moved by {diff}"
            );
        }
    }

    #[test]
    fn test_deform_with_displacement() {
        let mut mesh = StructuredMesh::uniform(3, 3, 1, 3.0, 3.0, 1.0).to_unstructured();

        // Get the number of unique nodes on the ymax boundary
        let ymax_patch = mesh.boundary_patch("ymax").unwrap();
        let mut ymax_nodes: HashSet<usize> = HashSet::new();
        for &fid in &ymax_patch.face_ids {
            for &nid in &mesh.faces[fid].nodes {
                ymax_nodes.insert(nid);
            }
        }
        let n_ymax_nodes = ymax_nodes.len();

        // Move the ymax boundary up by 0.5
        let displacements: HashMap<String, Vec<[f64; 3]>> = {
            let mut m = HashMap::new();
            m.insert(
                "ymax".to_string(),
                vec![[0.0, 0.5, 0.0]; n_ymax_nodes],
            );
            m
        };

        deform_mesh(&mut mesh, &displacements, 20).unwrap();

        // Check that ymax boundary nodes have moved up
        for &nid in &ymax_nodes {
            assert!(
                mesh.nodes[nid].position[1] > 3.0,
                "Ymax node {nid} should have moved up, y={}",
                mesh.nodes[nid].position[1]
            );
        }
    }

    #[test]
    fn test_deform_zero_iterations() {
        let mut mesh = StructuredMesh::uniform(2, 2, 1, 2.0, 2.0, 1.0).to_unstructured();

        let ymax_patch = mesh.boundary_patch("ymax").unwrap();
        let mut ymax_nodes: HashSet<usize> = HashSet::new();
        for &fid in &ymax_patch.face_ids {
            for &nid in &mesh.faces[fid].nodes {
                ymax_nodes.insert(nid);
            }
        }

        let displacements: HashMap<String, Vec<[f64; 3]>> = {
            let mut m = HashMap::new();
            m.insert(
                "ymax".to_string(),
                vec![[0.0, 1.0, 0.0]; ymax_nodes.len()],
            );
            m
        };

        deform_mesh(&mut mesh, &displacements, 0).unwrap();

        // Boundary nodes should still be displaced even with 0 iterations
        for &nid in &ymax_nodes {
            assert!(
                mesh.nodes[nid].position[1] > 2.0,
                "Boundary displacement should apply even with 0 iterations"
            );
        }
    }
}
