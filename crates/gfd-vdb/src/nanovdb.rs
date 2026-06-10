//! NanoVDB export (experimental, header-compatible dense container).
//!
//! NanoVDB is NVIDIA's single-file, GPU-friendly, read-only VDB representation
//! used by Omniverse / Isaac Sim. A fully GPU-loadable `.nvdb` requires the
//! OpenVDB→NanoVDB toolchain (C++), which is not available in this build.
//!
//! This writer emits the genuine **NanoVDB FileHeader** (magic + version + grid
//! count + codec) followed by a dense float voxel payload + grid metadata. It is
//! self-roundtrip-verified (see tests). Producing the full NanoVDB node tree for
//! native GPU readers is a documented follow-up; until then, convert a real
//! OpenVDB with `nanovdb_convert`.

use crate::{Result, VdbError};

/// NanoVDB file magic: `0x304244566f6e614e` (little-endian "NanoVDB0").
pub const NANOVDB_MAGIC: u64 = 0x304244566f6e614e;
/// Packed NanoVDB version (major=32, minor=6, patch=0).
pub const NANOVDB_VERSION: u32 = (32u32 << 21) | (6u32 << 10);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NvdbHeader {
    pub magic: u64,
    pub version: u32,
    pub grid_count: u16,
    pub codec: u16,
}

fn push_u16(buf: &mut Vec<u8>, v: u16) { buf.extend_from_slice(&v.to_le_bytes()); }
fn push_u32(buf: &mut Vec<u8>, v: u32) { buf.extend_from_slice(&v.to_le_bytes()); }
fn push_u64(buf: &mut Vec<u8>, v: u64) { buf.extend_from_slice(&v.to_le_bytes()); }
fn push_f64(buf: &mut Vec<u8>, v: f64) { buf.extend_from_slice(&v.to_le_bytes()); }
fn push_f32(buf: &mut Vec<u8>, v: f32) { buf.extend_from_slice(&v.to_le_bytes()); }

/// Serialize a dense float grid into the NanoVDB-header container.
pub fn nanovdb_bytes(name: &str, dims: [u32; 3], voxel_size: [f64; 3], data: &[f32]) -> Vec<u8> {
    let mut buf = Vec::with_capacity(64 + data.len() * 4);
    // FileHeader (NanoVDB-compatible).
    push_u64(&mut buf, NANOVDB_MAGIC);
    push_u32(&mut buf, NANOVDB_VERSION);
    push_u16(&mut buf, 1); // gridCount
    push_u16(&mut buf, 0); // codec = NONE
    // Grid metadata (GFD dense payload section).
    let nb = name.as_bytes();
    push_u32(&mut buf, nb.len() as u32);
    buf.extend_from_slice(nb);
    push_u32(&mut buf, dims[0]);
    push_u32(&mut buf, dims[1]);
    push_u32(&mut buf, dims[2]);
    push_f64(&mut buf, voxel_size[0]);
    push_f64(&mut buf, voxel_size[1]);
    push_f64(&mut buf, voxel_size[2]);
    push_u64(&mut buf, data.len() as u64);
    for &v in data {
        push_f32(&mut buf, v);
    }
    buf
}

/// Write a dense float grid as a `.nvdb` (header-compatible) file.
pub fn write_nanovdb(path: &str, name: &str, dims: [u32; 3], voxel_size: [f64; 3], data: &[f32]) -> Result<()> {
    let bytes = nanovdb_bytes(name, dims, voxel_size, data);
    std::fs::write(path, bytes).map_err(|e| VdbError::IoError(format!("Failed to write NanoVDB '{}': {}", path, e)))
}

/// Read back the header + dims + voxel count (for self-verification).
pub fn read_nanovdb_meta(bytes: &[u8]) -> Result<(NvdbHeader, [u32; 3], u64)> {
    if bytes.len() < 16 {
        return Err(VdbError::IoError("NanoVDB: file too short".into()));
    }
    let rd_u16 = |o: usize| u16::from_le_bytes([bytes[o], bytes[o + 1]]);
    let rd_u32 = |o: usize| u32::from_le_bytes([bytes[o], bytes[o + 1], bytes[o + 2], bytes[o + 3]]);
    let rd_u64 = |o: usize| {
        let mut a = [0u8; 8];
        a.copy_from_slice(&bytes[o..o + 8]);
        u64::from_le_bytes(a)
    };
    let header = NvdbHeader {
        magic: rd_u64(0),
        version: rd_u32(8),
        grid_count: rd_u16(12),
        codec: rd_u16(14),
    };
    if header.magic != NANOVDB_MAGIC {
        return Err(VdbError::IoError("NanoVDB: bad magic".into()));
    }
    let name_len = rd_u32(16) as usize;
    let mut o = 20 + name_len;
    let dims = [rd_u32(o), rd_u32(o + 4), rd_u32(o + 8)];
    o += 12 + 24; // dims + voxel_size(3*f64)
    let voxel_count = rd_u64(o);
    Ok((header, dims, voxel_count))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nanovdb_header_and_dims_roundtrip() {
        let data: Vec<f32> = (0..8).map(|i| i as f32).collect();
        let bytes = nanovdb_bytes("pressure", [2, 2, 2], [0.1, 0.1, 0.1], &data);
        let (h, dims, count) = read_nanovdb_meta(&bytes).unwrap();
        assert_eq!(h.magic, NANOVDB_MAGIC);
        assert_eq!(h.version, NANOVDB_VERSION);
        assert_eq!(h.grid_count, 1);
        assert_eq!(h.codec, 0);
        assert_eq!(dims, [2, 2, 2]);
        assert_eq!(count, 8);
    }
}
