//! gfd-cad-feature — parametric feature tree.
//!
//! Each feature takes inputs (sketches, other features, references) and
//! produces a shape. A re-execution context walks the tree in dependency
//! order and caches results.

use gfd_cad_topo::{ShapeArena, ShapeId, TopoError};
use serde::{Deserialize, Serialize};

pub mod array;
pub mod chamfer;
pub mod fillet;
pub mod helix;
pub mod mirror;
pub mod offset;
pub mod pad;
pub mod platonic;
pub mod profile;
pub mod transform;
pub mod wedge;
pub mod pocket;
pub mod primitive;
pub mod pyramid;
pub mod revolve;

pub use chamfer::{chamfered_box_solid, chamfered_box_top_edges};
pub use fillet::{filleted_box_solid, filleted_box_top_edges, filleted_cylinder_solid};
pub use helix::{archimedean_spiral_path, helix_length, helix_path, torus_knot_path};
pub use platonic::{dodecahedron_solid, icosahedron_solid, icosphere_solid, octahedron_solid, tetrahedron_solid};
pub use profile::{
    airfoil_naca4_profile, c_channel_profile, capsule_revolve_profile, cup_revolve_profile, disc_solid,
    ellipse_profile, frustum_revolve_profile, gear_prism_solid, gear_profile_simple,
    i_beam_profile, l_angle_profile, rectangle_profile, regular_ngon_profile, t_beam_profile,
    z_section_profile,
    ring_revolve_profile, rounded_rectangle_profile, slot_prism_solid, slot_profile,
    star_prism_solid, star_profile, torus_revolve_profile, tube_solid,
};
pub use mirror::{mirror_shape, MirrorPlane};
pub use offset::offset_polygon_2d;
pub use array::{circular_array, linear_array, rectangular_array};
pub use transform::{rotate_shape, scale_shape, translate_shape};
pub use wedge::wedge_solid;
pub use pad::{pad_polygon_along, pad_polygon_xy, pad_polygon_xy_signed};
pub use pocket::pocket_polygon_xy;
pub use primitive::{box_solid, cone_solid, cube_solid, cylinder_solid, honeycomb_pattern_solid, rectangular_prism_solid, sphere_solid, spiral_staircase_solid, stairs_solid, torus_solid};
pub use pyramid::{ngon_prism_solid, pyramid_solid};
pub use revolve::{revolve_profile_z, revolve_profile_z_partial};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct FeatureId(pub u32);

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Primitive {
    Box { lx: f64, ly: f64, lz: f64 },
    Cylinder { radius: f64, height: f64 },
    Sphere { radius: f64 },
    Cone { r1: f64, r2: f64, height: f64 },
    Torus { major: f64, minor: f64 },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Feature {
    Primitive(Primitive),
    Pad { sketch_idx: u32, length: f64 },
    Pocket { target: FeatureId, sketch_idx: u32, depth: f64 },
    Revolve { sketch_idx: u32, angle_deg: f64 },
    Fillet { target: FeatureId, edges: Vec<ShapeId>, radius: f64 },
    Chamfer { target: FeatureId, edges: Vec<ShapeId>, distance: f64 },
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct FeatureTree {
    pub features: Vec<Feature>,
    /// Cached resulting shape id per feature (post-execute).
    pub results: Vec<Option<ShapeId>>,
}

impl FeatureTree {
    pub fn new() -> Self { Self::default() }

    pub fn push(&mut self, f: Feature) -> FeatureId {
        let id = FeatureId(self.features.len() as u32);
        self.features.push(f);
        self.results.push(None);
        id
    }

    pub fn len(&self) -> usize { self.features.len() }
    pub fn is_empty(&self) -> bool { self.features.is_empty() }

    pub fn result_of(&self, id: FeatureId) -> Option<ShapeId> {
        self.results.get(id.0 as usize).copied().flatten()
    }
}

#[derive(Debug, thiserror::Error)]
pub enum FeatureError {
    #[error("feature {0:?} references unknown feature")]
    BadRef(FeatureId),
    #[error("feature not yet implemented")]
    Unimplemented,
    #[error(transparent)]
    Topo(#[from] TopoError),
}

pub type FeatureResult<T> = Result<T, FeatureError>;

/// Execute every feature in the tree in order, writing results back into the
/// tree's `results` cache. Primitive features are supported; Pad / Pocket /
/// Revolve / Fillet / Chamfer return `Unimplemented` until later iterations.
pub fn execute(tree: &mut FeatureTree, arena: &mut ShapeArena) -> FeatureResult<()> {
    let n = tree.features.len();
    for i in 0..n {
        let feat = tree.features[i].clone();
        let id = match feat {
            Feature::Primitive(p) => match p {
                Primitive::Box { lx, ly, lz }        => primitive::box_solid(arena, lx, ly, lz)?,
                Primitive::Sphere { radius }         => primitive::sphere_solid(arena, radius)?,
                Primitive::Cylinder { radius, height } => primitive::cylinder_solid(arena, radius, height)?,
                Primitive::Cone { r1, r2, height }   => primitive::cone_solid(arena, r1, r2, height)?,
                Primitive::Torus { major, minor }    => primitive::torus_solid(arena, major, minor)?,
            },
            _ => return Err(FeatureError::Unimplemented),
        };
        tree.results[i] = Some(id);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn execute_primitive_box() {
        let mut tree = FeatureTree::new();
        let id = tree.push(Feature::Primitive(Primitive::Box { lx: 1.0, ly: 2.0, lz: 3.0 }));
        let mut arena = ShapeArena::new();
        execute(&mut tree, &mut arena).unwrap();
        assert!(tree.result_of(id).is_some());
    }
}
