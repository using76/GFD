//! STL surface mesh reader.

use gfd_core::UnstructuredMesh;
use crate::mesh_reader::MeshReader;
use crate::Result;

/// Reader for STL (STereoLithography) surface mesh files.
///
/// Supports both ASCII and binary STL formats.
pub struct StlReader {
    /// Whether to merge duplicate vertices within this tolerance.
    pub merge_tolerance: f64,
}

impl StlReader {
    /// Creates a new STL reader.
    pub fn new() -> Self {
        Self {
            merge_tolerance: 1e-10,
        }
    }
}

impl Default for StlReader {
    fn default() -> Self {
        Self::new()
    }
}

impl MeshReader for StlReader {
    fn read(&self, path: &str) -> Result<UnstructuredMesh> {
        use gfd_core::mesh::node::Node;
        use gfd_core::mesh::face::Face;
        use gfd_core::mesh::cell::Cell;
        use std::collections::HashMap;

        let data = std::fs::read(path).map_err(|e| {
            crate::IoError::FileNotFound(format!("{}: {}", path, e))
        })?;

        // Try to detect ASCII vs binary STL
        let is_ascii = data.starts_with(b"solid") && data.len() < 1_000_000
            && std::str::from_utf8(&data).map_or(false, |s| s.contains("facet"));

        let mut vertices: Vec<[f64; 3]> = Vec::new();
        let mut normals: Vec<[f64; 3]> = Vec::new();

        if is_ascii {
            // Parse ASCII STL
            let text = std::str::from_utf8(&data).map_err(|e| {
                crate::IoError::ParseError(format!("Invalid UTF-8 in STL: {}", e))
            })?;

            let mut current_normal = [0.0_f64; 3];
            let mut tri_verts = Vec::new();

            for line in text.lines() {
                let trimmed = line.trim();
                if trimmed.starts_with("facet normal") {
                    let parts: Vec<&str> = trimmed.split_whitespace().collect();
                    if parts.len() >= 5 {
                        current_normal = [
                            parts[2].parse().unwrap_or(0.0),
                            parts[3].parse().unwrap_or(0.0),
                            parts[4].parse().unwrap_or(0.0),
                        ];
                    }
                } else if trimmed.starts_with("vertex") {
                    let parts: Vec<&str> = trimmed.split_whitespace().collect();
                    if parts.len() >= 4 {
                        let v = [
                            parts[1].parse().unwrap_or(0.0),
                            parts[2].parse().unwrap_or(0.0),
                            parts[3].parse().unwrap_or(0.0),
                        ];
                        tri_verts.push(v);
                    }
                } else if trimmed.starts_with("endfacet") {
                    if tri_verts.len() == 3 {
                        vertices.push(tri_verts[0]);
                        vertices.push(tri_verts[1]);
                        vertices.push(tri_verts[2]);
                        normals.push(current_normal);
                    }
                    tri_verts.clear();
                }
            }
        } else {
            // Parse binary STL
            if data.len() < 84 {
                return Err(crate::IoError::ParseError(
                    "Binary STL file too short".to_string(),
                ));
            }

            // Skip 80-byte header
            let num_triangles = u32::from_le_bytes([data[80], data[81], data[82], data[83]]) as usize;

            let expected_size = 84 + num_triangles * 50;
            if data.len() < expected_size {
                return Err(crate::IoError::ParseError(format!(
                    "Binary STL truncated: expected {} bytes, got {}",
                    expected_size,
                    data.len()
                )));
            }

            let read_f32 = |offset: usize| -> f64 {
                f32::from_le_bytes([
                    data[offset],
                    data[offset + 1],
                    data[offset + 2],
                    data[offset + 3],
                ]) as f64
            };

            for i in 0..num_triangles {
                let base = 84 + i * 50;
                let n = [
                    read_f32(base),
                    read_f32(base + 4),
                    read_f32(base + 8),
                ];
                normals.push(n);

                for v in 0..3 {
                    let vbase = base + 12 + v * 12;
                    vertices.push([
                        read_f32(vbase),
                        read_f32(vbase + 4),
                        read_f32(vbase + 8),
                    ]);
                }
            }
        }

        // Merge duplicate vertices
        let tol = self.merge_tolerance;
        let mut unique_nodes: Vec<Node> = Vec::new();
        let mut vertex_map: Vec<usize> = Vec::new(); // maps each raw vertex index to unique node index
        let mut spatial_map: HashMap<(i64, i64, i64), Vec<usize>> = HashMap::new();

        for v in &vertices {
            let key = (
                (v[0] / tol.max(1e-15)).round() as i64,
                (v[1] / tol.max(1e-15)).round() as i64,
                (v[2] / tol.max(1e-15)).round() as i64,
            );

            let mut found = None;
            if let Some(candidates) = spatial_map.get(&key) {
                for &idx in candidates {
                    let n = &unique_nodes[idx];
                    let dx = v[0] - n.position[0];
                    let dy = v[1] - n.position[1];
                    let dz = v[2] - n.position[2];
                    if dx * dx + dy * dy + dz * dz <= tol * tol {
                        found = Some(idx);
                        break;
                    }
                }
            }

            let node_idx = match found {
                Some(idx) => idx,
                None => {
                    let idx = unique_nodes.len();
                    unique_nodes.push(Node::new(idx, *v));
                    spatial_map.entry(key).or_default().push(idx);
                    idx
                }
            };
            vertex_map.push(node_idx);
        }

        // Build faces and a single surface cell per triangle
        let num_tris = normals.len();
        let mut faces = Vec::with_capacity(num_tris);
        let mut cells = Vec::with_capacity(num_tris);

        for i in 0..num_tris {
            let v0 = vertex_map[i * 3];
            let v1 = vertex_map[i * 3 + 1];
            let v2 = vertex_map[i * 3 + 2];

            let n = normals[i];
            let n_len = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt().max(1e-30);
            let unit_n = [n[0] / n_len, n[1] / n_len, n[2] / n_len];

            // Compute face center and area
            let p0 = unique_nodes[v0].position;
            let p1 = unique_nodes[v1].position;
            let p2 = unique_nodes[v2].position;
            let center = [
                (p0[0] + p1[0] + p2[0]) / 3.0,
                (p0[1] + p1[1] + p2[1]) / 3.0,
                (p0[2] + p1[2] + p2[2]) / 3.0,
            ];

            // Area = 0.5 * ||(p1-p0) x (p2-p0)||
            let e1 = [p1[0] - p0[0], p1[1] - p0[1], p1[2] - p0[2]];
            let e2 = [p2[0] - p0[0], p2[1] - p0[1], p2[2] - p0[2]];
            let cross = [
                e1[1] * e2[2] - e1[2] * e2[1],
                e1[2] * e2[0] - e1[0] * e2[2],
                e1[0] * e2[1] - e1[1] * e2[0],
            ];
            let area = 0.5 * (cross[0] * cross[0] + cross[1] * cross[1] + cross[2] * cross[2]).sqrt();

            faces.push(Face::new(i, vec![v0, v1, v2], i, None, area, unit_n, center));
            cells.push(Cell::new(i, vec![v0, v1, v2], vec![i], area, center));
        }

        Ok(UnstructuredMesh::from_components(unique_nodes, faces, cells, vec![]))
    }
}
