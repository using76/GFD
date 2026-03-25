//! VDB file I/O utilities.

use crate::{VdbGrid, Result};

/// Writes VDB grids to a binary stream.
pub fn write_to_bytes(_grids: &[VdbGrid]) -> Result<Vec<u8>> {
    // Full OpenVDB binary format is complex; return error until linked with OpenVDB C++ library.
    Err(crate::VdbError::IoError(
        "VDB binary serialization is not yet implemented.".to_string(),
    ))
}

/// Reads VDB grids from a binary stream.
pub fn read_from_bytes(_data: &[u8]) -> Result<Vec<VdbGrid>> {
    Err(crate::VdbError::IoError(
        "VDB binary deserialization is not yet implemented.".to_string(),
    ))
}
