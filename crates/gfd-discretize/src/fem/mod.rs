//! Finite Element Method discretization.

pub mod weak_form;
pub mod shape_fn;
pub mod quadrature;
pub mod assembly;

use serde::{Deserialize, Serialize};

/// Supported finite element types.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ElementType {
    /// 3-node triangle (linear).
    Tri3,
    /// 6-node triangle (quadratic).
    Tri6,
    /// 4-node quadrilateral (bilinear).
    Quad4,
    /// 8-node quadrilateral (serendipity quadratic).
    Quad8,
    /// 4-node tetrahedron (linear).
    Tet4,
    /// 10-node tetrahedron (quadratic).
    Tet10,
    /// 8-node hexahedron (trilinear).
    Hex8,
    /// 20-node hexahedron (serendipity quadratic).
    Hex20,
    /// 27-node hexahedron (triquadratic).
    Hex27,
    /// 6-node wedge (pentahedron).
    Wedge6,
    /// 5-node pyramid.
    Pyramid5,
}
