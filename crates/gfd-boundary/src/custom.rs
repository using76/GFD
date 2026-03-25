//! Custom expression-based boundary conditions.

use gfd_core::mesh::face::Face;
use gfd_core::mesh::cell::Cell;
use crate::traits::{BoundaryCondition, BoundaryConditionType};

/// Large number used for the penalty method.
const LARGE_VALUE: f64 = 1.0e30;

/// A boundary condition defined by a mathematical expression string.
///
/// The expression is evaluated at each boundary face location to determine
/// the prescribed value. Requires the gfd-expression engine for evaluation.
#[derive(Debug, Clone)]
pub struct ExpressionBC {
    /// The expression string in GMN format.
    pub expression: String,
    /// Descriptive name for this BC.
    pub bc_name: String,
}

impl ExpressionBC {
    /// Creates a new expression-based BC.
    pub fn new(expression: String, name: String) -> Self {
        Self {
            expression,
            bc_name: name,
        }
    }

    /// Evaluates the expression at the given face location.
    ///
    /// Returns the computed boundary value at the face centroid.
    pub fn evaluate_at_face(&self, face: &Face) -> f64 {
        // In a full implementation, this would:
        // 1. Parse self.expression via gfd_expression::parse
        // 2. Bind face.center[0..3] as x, y, z variables
        // 3. Evaluate the AST
        //
        // For now, return a placeholder based on the face center x-coordinate.
        let _ = &self.expression;
        let _x = face.center[0];
        let _y = face.center[1];
        let _z = face.center[2];
        // Placeholder: a real implementation would evaluate the expression tree.
        0.0
    }
}

impl BoundaryCondition for ExpressionBC {
    fn apply_coefficients(
        &self,
        a_p: &mut f64,
        b: &mut f64,
        face: &Face,
        _cell: &Cell,
    ) {
        // Evaluate the expression at the face centroid and apply as Dirichlet.
        let value = self.evaluate_at_face(face);
        *a_p += LARGE_VALUE;
        *b += LARGE_VALUE * value;
    }

    fn bc_type(&self) -> BoundaryConditionType {
        BoundaryConditionType::Custom(self.expression.clone())
    }

    fn name(&self) -> &str {
        &self.bc_name
    }
}
