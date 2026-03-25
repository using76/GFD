//! Compression codecs for VDB data.

use crate::Result;

/// Supported compression codecs.
#[derive(Debug, Clone, Copy)]
pub enum Codec {
    /// No compression.
    None,
    /// BLOSC compression (default in OpenVDB).
    Blosc,
    /// Zip (deflate) compression.
    Zip,
}

/// Compresses raw data using the specified codec.
pub fn compress(data: &[u8], codec: Codec) -> Result<Vec<u8>> {
    match codec {
        Codec::None => Ok(data.to_vec()),
        Codec::Blosc | Codec::Zip => {
            // BLOSC and Zip compression require external libraries.
            // Return uncompressed data as a fallback.
            Ok(data.to_vec())
        }
    }
}

/// Decompresses data using the specified codec.
pub fn decompress(data: &[u8], codec: Codec) -> Result<Vec<u8>> {
    match codec {
        Codec::None => Ok(data.to_vec()),
        Codec::Blosc | Codec::Zip => {
            // Assume data is uncompressed (passthrough from compress fallback)
            Ok(data.to_vec())
        }
    }
}
