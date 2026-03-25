//! Discretization pipeline: expression AST -> discrete equations.

use gfd_expression::ast::Expr;
use gfd_core::UnstructuredMesh;

use crate::fvm::FvmSchemes;
use crate::{DiscreteEquation, Result};

/// The discretization pipeline converts a parsed equation AST and mesh
/// into a set of discrete algebraic equations suitable for assembly
/// into a linear system.
#[derive(Debug, Clone)]
pub struct DiscretizationPipeline {
    _private: (),
}

impl DiscretizationPipeline {
    /// Create a new discretization pipeline.
    pub fn new() -> Self {
        Self { _private: () }
    }

    /// Discretize a governing equation on the given mesh using the specified schemes.
    ///
    /// # Arguments
    /// * `equation_ast` - The parsed expression AST representing the PDE.
    /// * `mesh` - The unstructured mesh to discretize on.
    /// * `schemes` - The FVM numerical schemes to use.
    ///
    /// # Returns
    /// A vector of `DiscreteEquation`, one per mesh cell.
    pub fn discretize(
        &self,
        _equation_ast: &Expr,
        _mesh: &UnstructuredMesh,
        _schemes: &FvmSchemes,
    ) -> Result<Vec<DiscreteEquation>> {
        // Simplified discretization pipeline:
        // Walk the AST, classify terms, and produce discrete equations per cell.
        // For now, produce stub equations with identity diagonal and zero off-diagonal.
        let num_cells = _mesh.num_cells();
        let mut equations = Vec::with_capacity(num_cells);

        for cell_id in 0..num_cells {
            // Build neighbor list from cell faces
            let mut neighbors = Vec::new();
            for &face_id in &_mesh.cells[cell_id].faces {
                let face = &_mesh.faces[face_id];
                if let Some(neighbor) = face.neighbor_cell {
                    let other = if neighbor == cell_id {
                        face.owner_cell
                    } else {
                        neighbor
                    };
                    if other != cell_id {
                        neighbors.push((other, 0.0_f64));
                    }
                }
            }

            equations.push(DiscreteEquation {
                cell_id,
                a_p: 1.0,
                neighbors,
                source: 0.0,
            });
        }

        Ok(equations)
    }
}

impl Default for DiscretizationPipeline {
    fn default() -> Self {
        Self::new()
    }
}
