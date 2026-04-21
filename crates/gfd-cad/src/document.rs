use serde::{Deserialize, Serialize};

use gfd_cad_feature::FeatureTree;
use gfd_cad_sketch::Sketch;
use gfd_cad_topo::ShapeArena;

/// Top-level CAD document: owns the shape arena, feature tree, and sketches.
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct Document {
    pub arena: ShapeArena,
    pub features: FeatureTree,
    pub sketches: Vec<Sketch>,
}

impl Document {
    pub fn new() -> Self { Self::default() }
}
