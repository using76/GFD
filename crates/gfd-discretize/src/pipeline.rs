//! Discretization pipeline: expression AST -> discrete equations.

use gfd_expression::ast::{BinOp, DiffOperator, Expr};
use gfd_core::UnstructuredMesh;

use crate::fvm::diffusion::compute_diffusive_coefficient;
use crate::fvm::FvmSchemes;
use crate::{DiscreteEquation, Result, TermClassification};

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
        equation_ast: &Expr,
        mesh: &UnstructuredMesh,
        _schemes: &FvmSchemes,
    ) -> Result<Vec<DiscreteEquation>> {
        let num_cells = mesh.num_cells();

        // Classify terms in the expression
        let classification = classify_terms(equation_ast);

        // Initialize per-cell equations with zero coefficients
        let mut equations: Vec<DiscreteEquation> = (0..num_cells)
            .map(|cell_id| DiscreteEquation {
                cell_id,
                a_p: 0.0,
                neighbors: Vec::new(),
                source: 0.0,
            })
            .collect();

        // --- Process diffusion terms: laplacian(gamma, phi) ---
        // Discretized as: sum_f D_f (phi_N - phi_P) where D_f = gamma * A_f / d_PN
        // This contributes: a_P += D_f, a_nb = -D_f for each internal face neighbor
        if let Some(ref diff_expr) = classification.diffusion {
            let gamma = extract_diffusion_coefficient(diff_expr);

            for cell_id in 0..num_cells {
                let mut neighbor_map: Vec<(usize, f64)> = Vec::new();
                let mut a_p = 0.0_f64;

                for &face_id in &mesh.cells[cell_id].faces {
                    let face = &mesh.faces[face_id];
                    if let Some(neighbor) = face.neighbor_cell {
                        let other = if neighbor == cell_id {
                            face.owner_cell
                        } else {
                            neighbor
                        };
                        if other == cell_id {
                            continue;
                        }

                        // Compute distance between cell centers
                        let xp = mesh.cells[cell_id].center;
                        let xn = mesh.cells[other].center;
                        let dx = xn[0] - xp[0];
                        let dy = xn[1] - xp[1];
                        let dz = xn[2] - xp[2];
                        let distance = (dx * dx + dy * dy + dz * dz).sqrt();

                        let d_coeff = compute_diffusive_coefficient(gamma, face.area, distance);

                        a_p += d_coeff;
                        neighbor_map.push((other, -d_coeff));
                    }
                }

                equations[cell_id].a_p += a_p;
                equations[cell_id].neighbors = neighbor_map;
            }
        }

        // --- Process source terms ---
        // Source terms contribute to the RHS: b_P += S * V_cell
        for source_expr in &classification.sources {
            let source_val = extract_constant_value(source_expr);
            for cell_id in 0..num_cells {
                let vol = mesh.cells[cell_id].volume;
                equations[cell_id].source += source_val * vol;
            }
        }

        // If no diffusion term was found, the diagonal has no contribution
        // from face fluxes. Apply an identity diagonal so the system remains
        // solvable (source-only equations become: phi_P = source).
        if classification.diffusion.is_none() {
            for cell_id in 0..num_cells {
                equations[cell_id].a_p = 1.0;
            }
        }

        Ok(equations)
    }
}

impl Default for DiscretizationPipeline {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// AST walking helpers
// ---------------------------------------------------------------------------

/// Walk the expression AST and classify terms into diffusion, source, etc.
fn classify_terms(expr: &Expr) -> TermClassification {
    let mut tc = TermClassification {
        temporal: None,
        convection: None,
        diffusion: None,
        sources: Vec::new(),
    };

    classify_recursive(expr, false, &mut tc);
    tc
}

/// Recursively classify terms. `negated` tracks if the current subtree
/// is being subtracted.
fn classify_recursive(expr: &Expr, negated: bool, tc: &mut TermClassification) {
    match expr {
        // Addition: recurse into both sides
        Expr::BinaryOp {
            op: BinOp::Add,
            left,
            right,
        } => {
            classify_recursive(left, negated, tc);
            classify_recursive(right, negated, tc);
        }

        // Subtraction: left keeps sign, right flips
        Expr::BinaryOp {
            op: BinOp::Sub,
            left,
            right,
        } => {
            classify_recursive(left, negated, tc);
            classify_recursive(right, !negated, tc);
        }

        // Differential operators
        Expr::DiffOp { op, .. } => match op {
            DiffOperator::Laplacian => {
                // laplacian(gamma, phi) — if negated it means -laplacian(...)
                // which is the standard diffusion term: -div(gamma grad phi) = source
                if negated {
                    // -laplacian(...) means it was subtracted, so it's
                    // on the LHS as a positive diffusion operator
                    tc.diffusion = Some(expr.clone());
                } else {
                    // +laplacian(...) directly
                    tc.diffusion = Some(expr.clone());
                }
            }
            DiffOperator::TimeDerivative => {
                tc.temporal = Some(expr.clone());
            }
            DiffOperator::Divergence => {
                tc.convection = Some(expr.clone());
            }
            _ => {
                // Unknown differential operator — treat as source
                tc.sources.push(expr.clone());
            }
        },

        // Anything else (numbers, variables, products, etc.) is a source term
        _ => {
            tc.sources.push(expr.clone());
        }
    }
}

/// Extract the diffusion coefficient from a Laplacian expression.
/// `laplacian(gamma, phi)` has operands[0] = gamma.
/// If gamma is a Number, return it directly; otherwise default to 1.0.
fn extract_diffusion_coefficient(expr: &Expr) -> f64 {
    if let Expr::DiffOp {
        op: DiffOperator::Laplacian,
        operands,
    } = expr
    {
        if let Some(first) = operands.first() {
            return extract_constant_value(first);
        }
    }
    1.0
}

/// Try to extract a constant numeric value from an expression.
/// Returns the value if it's a `Number`, otherwise returns 0.0 for
/// non-constant expressions that would need a field evaluation.
fn extract_constant_value(expr: &Expr) -> f64 {
    match expr {
        Expr::Number(v) => *v,
        Expr::UnaryOp {
            op: gfd_expression::ast::UnOp::Neg,
            operand,
        } => -extract_constant_value(operand),
        Expr::BinaryOp {
            op: BinOp::Mul,
            left,
            right,
        } => extract_constant_value(left) * extract_constant_value(right),
        Expr::BinaryOp {
            op: BinOp::Div,
            left,
            right,
        } => {
            let r = extract_constant_value(right);
            if r.abs() > 1e-30 {
                extract_constant_value(left) / r
            } else {
                0.0
            }
        }
        Expr::BinaryOp {
            op: BinOp::Add,
            left,
            right,
        } => extract_constant_value(left) + extract_constant_value(right),
        Expr::BinaryOp {
            op: BinOp::Sub,
            left,
            right,
        } => extract_constant_value(left) - extract_constant_value(right),
        Expr::Constant(name) => match name.as_str() {
            "pi" => std::f64::consts::PI,
            "e" => std::f64::consts::E,
            _ => 0.0,
        },
        // Variables, fields, etc. can't be evaluated to a constant
        _ => 0.0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gfd_core::mesh::cell::Cell;
    use gfd_core::mesh::face::Face;

    /// Create a simple 3-cell 1D mesh along x-axis.
    fn make_3_cell_mesh() -> UnstructuredMesh {
        let cells = vec![
            Cell::new(0, vec![], vec![0, 1], 1.0, [0.5, 0.5, 0.5]),
            Cell::new(1, vec![], vec![1, 2], 1.0, [1.5, 0.5, 0.5]),
            Cell::new(2, vec![], vec![2, 3], 1.0, [2.5, 0.5, 0.5]),
        ];

        let faces = vec![
            // x=0 boundary
            Face::new(0, vec![], 0, None, 1.0, [-1.0, 0.0, 0.0], [0.0, 0.5, 0.5]),
            // x=1 internal
            Face::new(1, vec![], 0, Some(1), 1.0, [1.0, 0.0, 0.0], [1.0, 0.5, 0.5]),
            // x=2 internal
            Face::new(2, vec![], 1, Some(2), 1.0, [1.0, 0.0, 0.0], [2.0, 0.5, 0.5]),
            // x=3 boundary
            Face::new(3, vec![], 2, None, 1.0, [1.0, 0.0, 0.0], [3.0, 0.5, 0.5]),
        ];

        UnstructuredMesh::from_components(vec![], faces, cells, vec![])
    }

    #[test]
    fn pipeline_laplacian_produces_diffusion_coefficients() {
        let mesh = make_3_cell_mesh();
        let pipeline = DiscretizationPipeline::new();
        let schemes = FvmSchemes::default();

        // laplacian(1.0, phi) => gamma=1.0
        let ast = gfd_expression::parse("laplacian(1.0, $phi)").unwrap();
        let eqs = pipeline.discretize(&ast, &mesh, &schemes).unwrap();

        assert_eq!(eqs.len(), 3);

        // Cell 0: one internal face (to cell 1), distance=1.0, D=1*1/1=1
        assert!((eqs[0].a_p - 1.0).abs() < 1e-10, "cell 0 a_p = {}", eqs[0].a_p);
        assert_eq!(eqs[0].neighbors.len(), 1);
        assert_eq!(eqs[0].neighbors[0].0, 1); // neighbor cell id
        assert!((eqs[0].neighbors[0].1 - (-1.0)).abs() < 1e-10);

        // Cell 1: two internal faces (to cell 0 and cell 2)
        assert!((eqs[1].a_p - 2.0).abs() < 1e-10, "cell 1 a_p = {}", eqs[1].a_p);
        assert_eq!(eqs[1].neighbors.len(), 2);

        // Cell 2: one internal face (to cell 1)
        assert!((eqs[2].a_p - 1.0).abs() < 1e-10, "cell 2 a_p = {}", eqs[2].a_p);
        assert_eq!(eqs[2].neighbors.len(), 1);
    }

    #[test]
    fn pipeline_laplacian_with_coefficient() {
        let mesh = make_3_cell_mesh();
        let pipeline = DiscretizationPipeline::new();
        let schemes = FvmSchemes::default();

        // laplacian(0.5, phi) => gamma=0.5, D=0.5*1/1=0.5
        let ast = gfd_expression::parse("laplacian(0.5, $phi)").unwrap();
        let eqs = pipeline.discretize(&ast, &mesh, &schemes).unwrap();

        // Cell 0: D = 0.5
        assert!((eqs[0].a_p - 0.5).abs() < 1e-10);
        assert!((eqs[0].neighbors[0].1 - (-0.5)).abs() < 1e-10);
    }

    #[test]
    fn pipeline_source_term() {
        let mesh = make_3_cell_mesh();
        let pipeline = DiscretizationPipeline::new();
        let schemes = FvmSchemes::default();

        // laplacian(1.0, $phi) + 10.0  =>  diffusion + constant source
        let ast = gfd_expression::parse("laplacian(1.0, $phi) + 10.0").unwrap();
        let eqs = pipeline.discretize(&ast, &mesh, &schemes).unwrap();

        // Each cell has volume=1.0, so source = 10.0 * 1.0 = 10.0
        for eq in &eqs {
            assert!(
                (eq.source - 10.0).abs() < 1e-10,
                "cell {} source = {} (expected 10.0)",
                eq.cell_id, eq.source
            );
        }
    }

    #[test]
    fn pipeline_no_diffusion_no_source_fallback() {
        let mesh = make_3_cell_mesh();
        let pipeline = DiscretizationPipeline::new();
        let schemes = FvmSchemes::default();

        // A pure number with no diffusion operator: classified as source
        // with a non-zero value, so this gets source contribution.
        let ast = gfd_expression::parse("5.0").unwrap();
        let eqs = pipeline.discretize(&ast, &mesh, &schemes).unwrap();

        // No diffusion => identity fallback for a_p, but source = 5.0 * V = 5.0
        for eq in &eqs {
            assert!((eq.a_p - 1.0).abs() < 1e-10, "a_p = {} (expected 1.0)", eq.a_p);
            assert!((eq.source - 5.0).abs() < 1e-10, "source = {} (expected 5.0)", eq.source);
        }
    }
}
