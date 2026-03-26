//! STL file reader.
//!
//! Reads both ASCII and binary STL files into a triangle mesh representation.

use crate::geometry::distance_field::Triangle;
use crate::Result;

/// A mesh composed of triangles, as read from an STL file.
#[derive(Debug, Clone)]
pub struct StlMesh {
    /// The triangles forming the surface.
    pub triangles: Vec<Triangle>,
    /// The outward-facing normal for each triangle.
    pub normals: Vec<[f64; 3]>,
}

impl StlMesh {
    /// Returns the number of triangles.
    pub fn num_triangles(&self) -> usize {
        self.triangles.len()
    }

    /// Returns true if the mesh is empty.
    pub fn is_empty(&self) -> bool {
        self.triangles.is_empty()
    }
}

/// Read an ASCII STL file from its contents.
///
/// ASCII STL format:
/// ```text
/// solid name
///   facet normal ni nj nk
///     outer loop
///       vertex v1x v1y v1z
///       vertex v2x v2y v2z
///       vertex v3x v3y v3z
///     endloop
///   endfacet
/// endsolid name
/// ```
///
/// # Arguments
/// * `content` - The full text of the STL file.
///
/// # Returns
/// An `StlMesh` containing the triangles and normals.
pub fn read_stl_ascii(content: &str) -> Result<StlMesh> {
    let mut triangles = Vec::new();
    let mut normals = Vec::new();

    let mut current_normal = [0.0f64; 3];
    let mut current_vertices: Vec<[f64; 3]> = Vec::new();
    let mut in_facet = false;

    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        let lower = trimmed.to_lowercase();

        if lower.starts_with("facet normal") {
            let parts: Vec<&str> = trimmed.split_whitespace().collect();
            if parts.len() >= 5 {
                current_normal = [
                    parse_f64(parts[2])?,
                    parse_f64(parts[3])?,
                    parse_f64(parts[4])?,
                ];
            }
            in_facet = true;
            current_vertices.clear();
        } else if lower.starts_with("vertex") && in_facet {
            let parts: Vec<&str> = trimmed.split_whitespace().collect();
            if parts.len() >= 4 {
                let v = [
                    parse_f64(parts[1])?,
                    parse_f64(parts[2])?,
                    parse_f64(parts[3])?,
                ];
                current_vertices.push(v);
            }
        } else if lower.starts_with("endfacet") {
            if current_vertices.len() == 3 {
                triangles.push(Triangle::new(
                    current_vertices[0],
                    current_vertices[1],
                    current_vertices[2],
                ));
                normals.push(current_normal);
            }
            in_facet = false;
            current_vertices.clear();
        }
    }

    if triangles.is_empty() {
        return Err(crate::MeshError::GeometryError(
            "No triangles found in ASCII STL".to_string(),
        ));
    }

    Ok(StlMesh { triangles, normals })
}

/// Read a binary STL file from raw bytes.
///
/// Binary STL format:
/// - 80 bytes: header
/// - 4 bytes: number of triangles (u32 LE)
/// - For each triangle:
///   - 12 bytes: normal (3 x f32 LE)
///   - 36 bytes: 3 vertices (9 x f32 LE)
///   - 2 bytes: attribute byte count
///
/// # Arguments
/// * `data` - The raw bytes of the STL file.
///
/// # Returns
/// An `StlMesh` containing the triangles and normals.
pub fn read_stl_binary(data: &[u8]) -> Result<StlMesh> {
    if data.len() < 84 {
        return Err(crate::MeshError::GeometryError(
            "Binary STL too short (< 84 bytes)".to_string(),
        ));
    }

    let num_triangles = u32::from_le_bytes([data[80], data[81], data[82], data[83]]) as usize;

    let expected_size = 84 + num_triangles * 50;
    if data.len() < expected_size {
        return Err(crate::MeshError::GeometryError(format!(
            "Binary STL truncated: expected {} bytes, got {}",
            expected_size,
            data.len()
        )));
    }

    let mut triangles = Vec::with_capacity(num_triangles);
    let mut normals = Vec::with_capacity(num_triangles);

    let mut offset = 84;
    for _ in 0..num_triangles {
        let nx = read_f32_le(data, offset) as f64;
        let ny = read_f32_le(data, offset + 4) as f64;
        let nz = read_f32_le(data, offset + 8) as f64;

        let v0 = [
            read_f32_le(data, offset + 12) as f64,
            read_f32_le(data, offset + 16) as f64,
            read_f32_le(data, offset + 20) as f64,
        ];
        let v1 = [
            read_f32_le(data, offset + 24) as f64,
            read_f32_le(data, offset + 28) as f64,
            read_f32_le(data, offset + 32) as f64,
        ];
        let v2 = [
            read_f32_le(data, offset + 36) as f64,
            read_f32_le(data, offset + 40) as f64,
            read_f32_le(data, offset + 44) as f64,
        ];

        triangles.push(Triangle::new(v0, v1, v2));
        normals.push([nx, ny, nz]);

        offset += 50; // 12 (normal) + 36 (vertices) + 2 (attribute)
    }

    if triangles.is_empty() {
        return Err(crate::MeshError::GeometryError(
            "No triangles in binary STL".to_string(),
        ));
    }

    Ok(StlMesh { triangles, normals })
}

fn parse_f64(s: &str) -> Result<f64> {
    s.parse::<f64>().map_err(|e| {
        crate::MeshError::GeometryError(format!("Failed to parse float '{}': {}", s, e))
    })
}

fn read_f32_le(data: &[u8], offset: usize) -> f32 {
    f32::from_le_bytes([
        data[offset],
        data[offset + 1],
        data[offset + 2],
        data[offset + 3],
    ])
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_ascii_stl() -> String {
        r#"solid test
  facet normal 0 0 1
    outer loop
      vertex 0.0 0.0 0.0
      vertex 1.0 0.0 0.0
      vertex 0.0 1.0 0.0
    endloop
  endfacet
  facet normal 0 0 1
    outer loop
      vertex 1.0 0.0 0.0
      vertex 1.0 1.0 0.0
      vertex 0.0 1.0 0.0
    endloop
  endfacet
endsolid test"#
            .to_string()
    }

    fn sample_binary_stl() -> Vec<u8> {
        let mut data = vec![0u8; 84]; // 80 header + 4 count

        // Number of triangles = 1
        data[80] = 1;
        data[81] = 0;
        data[82] = 0;
        data[83] = 0;

        // Normal: 0, 0, 1
        data.extend_from_slice(&0.0f32.to_le_bytes());
        data.extend_from_slice(&0.0f32.to_le_bytes());
        data.extend_from_slice(&1.0f32.to_le_bytes());

        // Vertex 0: (0, 0, 0)
        data.extend_from_slice(&0.0f32.to_le_bytes());
        data.extend_from_slice(&0.0f32.to_le_bytes());
        data.extend_from_slice(&0.0f32.to_le_bytes());

        // Vertex 1: (1, 0, 0)
        data.extend_from_slice(&1.0f32.to_le_bytes());
        data.extend_from_slice(&0.0f32.to_le_bytes());
        data.extend_from_slice(&0.0f32.to_le_bytes());

        // Vertex 2: (0, 1, 0)
        data.extend_from_slice(&0.0f32.to_le_bytes());
        data.extend_from_slice(&1.0f32.to_le_bytes());
        data.extend_from_slice(&0.0f32.to_le_bytes());

        // Attribute byte count
        data.extend_from_slice(&[0u8, 0]);

        data
    }

    #[test]
    fn test_read_ascii_stl() {
        let content = sample_ascii_stl();
        let mesh = read_stl_ascii(&content).unwrap();
        assert_eq!(mesh.triangles.len(), 2);
        assert_eq!(mesh.normals.len(), 2);
    }

    #[test]
    fn test_read_ascii_stl_normals() {
        let content = sample_ascii_stl();
        let mesh = read_stl_ascii(&content).unwrap();
        for normal in &mesh.normals {
            assert!((normal[2] - 1.0).abs() < 1e-10, "Normal should be [0,0,1]");
        }
    }

    #[test]
    fn test_read_ascii_stl_vertices() {
        let content = sample_ascii_stl();
        let mesh = read_stl_ascii(&content).unwrap();
        let tri = &mesh.triangles[0];
        assert!((tri.v0[0] - 0.0).abs() < 1e-10);
        assert!((tri.v1[0] - 1.0).abs() < 1e-10);
        assert!((tri.v2[1] - 1.0).abs() < 1e-10);
    }

    #[test]
    fn test_read_binary_stl() {
        let data = sample_binary_stl();
        let mesh = read_stl_binary(&data).unwrap();
        assert_eq!(mesh.triangles.len(), 1);
        assert_eq!(mesh.normals.len(), 1);
    }

    #[test]
    fn test_read_binary_stl_normal() {
        let data = sample_binary_stl();
        let mesh = read_stl_binary(&data).unwrap();
        let n = &mesh.normals[0];
        assert!((n[2] - 1.0).abs() < 1e-6, "Normal z should be 1.0, got {}", n[2]);
    }

    #[test]
    fn test_read_binary_stl_too_short() {
        let data = vec![0u8; 50];
        let result = read_stl_binary(&data);
        assert!(result.is_err());
    }

    #[test]
    fn test_read_ascii_stl_empty() {
        let content = "solid empty\nendsolid empty";
        let result = read_stl_ascii(content);
        assert!(result.is_err());
    }
}
