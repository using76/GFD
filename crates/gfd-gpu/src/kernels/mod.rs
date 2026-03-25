//! Custom GPU compute kernels for GFD operations.
//!
//! Each sub-module contains a GPU kernel (stub) and a fully-functional CPU
//! fallback. The CUDA kernels will be compiled to PTX and loaded at runtime
//! once the build-system integration is in place.

pub mod correction;
pub mod flux;
pub mod gradient;
pub mod reduction;
