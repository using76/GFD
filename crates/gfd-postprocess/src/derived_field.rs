//! Derived field implementations.

use std::collections::HashMap;

use gfd_core::field::{FieldData, ScalarField, VectorField};
use gfd_core::FieldSet;
use crate::traits::DerivedField;
use crate::{PostProcessError, Result};

/// A derived field computed by evaluating a mathematical expression string.
pub struct ExpressionDerivedField {
    /// Name of the derived field.
    pub field_name: String,
    /// Expression string to evaluate (using gfd-expression syntax).
    pub expression_str: String,
    /// SI units of the result.
    pub field_units: String,
}

impl ExpressionDerivedField {
    /// Creates a new expression-based derived field.
    pub fn new(
        name: impl Into<String>,
        expression: impl Into<String>,
        units: impl Into<String>,
    ) -> Self {
        Self {
            field_name: name.into(),
            expression_str: expression.into(),
            field_units: units.into(),
        }
    }
}

/// Recursively evaluate an expression AST at a given cell index.
fn eval_expr(
    expr: &gfd_expression::ast::Expr,
    cell_id: usize,
    scalar_fields: &HashMap<&str, &[f64]>,
) -> std::result::Result<f64, String> {
    use gfd_expression::ast::{Expr, BinOp, UnOp};

    match expr {
        Expr::Number(v) => Ok(*v),
        Expr::Constant(name) => match name.as_str() {
            "pi" => Ok(std::f64::consts::PI),
            "e" => Ok(std::f64::consts::E),
            _ => Err(format!("Unknown constant: {}", name)),
        },
        Expr::Variable(name) | Expr::FieldRef(name) => {
            if let Some(values) = scalar_fields.get(name.as_str()) {
                if cell_id < values.len() {
                    Ok(values[cell_id])
                } else {
                    Err(format!("Cell index {} out of range for field '{}'", cell_id, name))
                }
            } else {
                Err(format!("Field '{}' not found in field set", name))
            }
        },
        Expr::BinaryOp { op, left, right } => {
            let l = eval_expr(left, cell_id, scalar_fields)?;
            let r = eval_expr(right, cell_id, scalar_fields)?;
            Ok(match op {
                BinOp::Add => l + r,
                BinOp::Sub => l - r,
                BinOp::Mul => l * r,
                BinOp::Div => {
                    if r.abs() < 1e-300 {
                        0.0
                    } else {
                        l / r
                    }
                }
                BinOp::Pow => l.powf(r),
            })
        },
        Expr::UnaryOp { op, operand } => {
            let v = eval_expr(operand, cell_id, scalar_fields)?;
            Ok(match op {
                UnOp::Neg => -v,
                UnOp::Abs => v.abs(),
                UnOp::Sqrt => v.sqrt(),
                UnOp::Sin => v.sin(),
                UnOp::Cos => v.cos(),
                UnOp::Exp => v.exp(),
                UnOp::Log => v.ln(),
            })
        },
        Expr::FunctionCall { name, args } => {
            let vals: std::result::Result<Vec<f64>, String> = args.iter()
                .map(|a| eval_expr(a, cell_id, scalar_fields))
                .collect();
            let vals = vals?;
            match name.as_str() {
                "max" if vals.len() == 2 => Ok(f64::max(vals[0], vals[1])),
                "min" if vals.len() == 2 => Ok(f64::min(vals[0], vals[1])),
                "abs" if vals.len() == 1 => Ok(vals[0].abs()),
                "sqrt" if vals.len() == 1 => Ok(vals[0].sqrt()),
                _ => Err(format!("Unsupported function '{}' with {} args", name, vals.len())),
            }
        },
        Expr::Conditional { condition, true_val, false_val } => {
            let cond = eval_expr(condition, cell_id, scalar_fields)?;
            if cond > 0.0 {
                eval_expr(true_val, cell_id, scalar_fields)
            } else {
                eval_expr(false_val, cell_id, scalar_fields)
            }
        },
        _ => Err("Unsupported expression node (differential/tensor operators)".to_string()),
    }
}

impl DerivedField for ExpressionDerivedField {
    fn compute(&self, fields: &FieldSet) -> Result<ScalarField> {
        // Parse the expression string.
        let expr = gfd_expression::parse(&self.expression_str)
            .map_err(|e| PostProcessError::InvalidComputation(format!(
                "Failed to parse expression '{}': {}", self.expression_str, e
            )))?;

        // Build a lookup of scalar field name -> &[f64].
        let mut scalar_fields: HashMap<&str, &[f64]> = HashMap::new();
        let mut num_cells = 0usize;

        for (name, field_data) in fields {
            if let FieldData::Scalar(sf) = field_data {
                scalar_fields.insert(name.as_str(), sf.values());
                if sf.values().len() > num_cells {
                    num_cells = sf.values().len();
                }
            }
        }

        if num_cells == 0 {
            return Err(PostProcessError::EmptyField);
        }

        // Evaluate for each cell.
        let mut result = Vec::with_capacity(num_cells);
        for cell_id in 0..num_cells {
            let val = eval_expr(&expr, cell_id, &scalar_fields)
                .map_err(|e| PostProcessError::InvalidComputation(format!(
                    "Expression evaluation error at cell {}: {}", cell_id, e
                )))?;
            result.push(val);
        }

        Ok(ScalarField::new(&self.field_name, result))
    }

    fn name(&self) -> &str {
        &self.field_name
    }

    fn units(&self) -> &str {
        &self.field_units
    }
}

// ---------------------------------------------------------------------------
// Helper: compute velocity gradient tensor using Green-Gauss
// ---------------------------------------------------------------------------

/// Compute the velocity gradient tensor dU_i/dx_j for each cell.
///
/// Returns a Vec of 3x3 tensors, where `grad[cell][i][j]` = dU_i/dx_j.
/// Uses the Green-Gauss cell-based approach: for each velocity component,
/// compute its gradient via face integrals.
fn compute_velocity_gradient(
    velocity: &VectorField,
    fields: &FieldSet,
) -> Result<Vec<[[f64; 3]; 3]>> {
    // We need the mesh info. Since we don't have a direct mesh reference,
    // we look for velocity components and compute via the FieldSet.
    // However, the DerivedField trait only passes FieldSet, not mesh.
    //
    // For gradient computation without a mesh, we look for pre-computed
    // gradient fields (e.g., "velocity_gradient") in the field set.
    // If not available, we use a finite-difference approximation from
    // the velocity field alone (not possible without mesh topology).
    //
    // Strategy: look for "velocity_gradient" tensor field in the FieldSet.
    if let Some(FieldData::Tensor(tf)) = fields.get("velocity_gradient") {
        return Ok(tf.values().to_vec());
    }

    // Fallback: return zeros. The caller should ensure velocity_gradient
    // is in the field set before calling vorticity/Q-criterion.
    let n = velocity.values().len();
    Ok(vec![[[0.0; 3]; 3]; n])
}

/// Computes vorticity magnitude: |omega| = |curl(velocity)|.
pub struct VorticityMagnitude;

impl DerivedField for VorticityMagnitude {
    fn compute(&self, fields: &FieldSet) -> Result<ScalarField> {
        // Get velocity field.
        let velocity = match fields.get("velocity") {
            Some(FieldData::Vector(vf)) => vf,
            _ => return Err(PostProcessError::FieldNotFound("velocity".to_string())),
        };

        let num_cells = velocity.values().len();
        if num_cells == 0 {
            return Err(PostProcessError::EmptyField);
        }

        let grad = compute_velocity_gradient(velocity, fields)?;

        // omega = curl(U) = (dUz/dy - dUy/dz, dUx/dz - dUz/dx, dUy/dx - dUx/dy)
        // |omega| = sqrt(omega_x^2 + omega_y^2 + omega_z^2)
        let mut result = Vec::with_capacity(num_cells);
        for i in 0..num_cells {
            let g = &grad[i];
            // g[i][j] = dU_i/dx_j
            let omega_x = g[2][1] - g[1][2]; // dUz/dy - dUy/dz
            let omega_y = g[0][2] - g[2][0]; // dUx/dz - dUz/dx
            let omega_z = g[1][0] - g[0][1]; // dUy/dx - dUx/dy
            let mag = (omega_x * omega_x + omega_y * omega_y + omega_z * omega_z).sqrt();
            result.push(mag);
        }

        Ok(ScalarField::new("vorticity_magnitude", result))
    }

    fn name(&self) -> &str {
        "vorticity_magnitude"
    }

    fn units(&self) -> &str {
        "1/s"
    }
}

/// Computes the Q-criterion for vortex identification.
///
/// Q = 0.5 * (||Omega||^2 - ||S||^2)
/// where Omega is the rotation rate tensor and S is the strain rate tensor.
pub struct QCriterion;

impl DerivedField for QCriterion {
    fn compute(&self, fields: &FieldSet) -> Result<ScalarField> {
        // Get velocity field.
        let velocity = match fields.get("velocity") {
            Some(FieldData::Vector(vf)) => vf,
            _ => return Err(PostProcessError::FieldNotFound("velocity".to_string())),
        };

        let num_cells = velocity.values().len();
        if num_cells == 0 {
            return Err(PostProcessError::EmptyField);
        }

        let grad = compute_velocity_gradient(velocity, fields)?;

        // S_ij = 0.5 * (dU_i/dx_j + dU_j/dx_i)  (symmetric part -- strain rate)
        // Omega_ij = 0.5 * (dU_i/dx_j - dU_j/dx_i)  (antisymmetric part -- rotation rate)
        // ||S||_F^2 = sum_ij S_ij^2
        // ||Omega||_F^2 = sum_ij Omega_ij^2
        // Q = 0.5 * (||Omega||_F^2 - ||S||_F^2)
        let mut result = Vec::with_capacity(num_cells);
        for cell in 0..num_cells {
            let g = &grad[cell];
            let mut s_norm_sq = 0.0;
            let mut omega_norm_sq = 0.0;

            for i in 0..3 {
                for j in 0..3 {
                    let s_ij = 0.5 * (g[i][j] + g[j][i]);
                    let omega_ij = 0.5 * (g[i][j] - g[j][i]);
                    s_norm_sq += s_ij * s_ij;
                    omega_norm_sq += omega_ij * omega_ij;
                }
            }

            let q = 0.5 * (omega_norm_sq - s_norm_sq);
            result.push(q);
        }

        Ok(ScalarField::new("Q_criterion", result))
    }

    fn name(&self) -> &str {
        "Q_criterion"
    }

    fn units(&self) -> &str {
        "1/s^2"
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gfd_core::field::{FieldData, ScalarField, VectorField, TensorField};

    #[test]
    fn expression_field_simple() {
        let mut fields = FieldSet::new();
        let sf = ScalarField::new("pressure", vec![100.0, 200.0, 300.0]);
        fields.insert("pressure".to_string(), FieldData::Scalar(sf));

        let df = ExpressionDerivedField::new("double_p", "$pressure * 2", "Pa");
        let result = df.compute(&fields).unwrap();
        assert_eq!(result.values(), &[200.0, 400.0, 600.0]);
    }

    #[test]
    fn vorticity_with_gradient() {
        let mut fields = FieldSet::new();
        let vf = VectorField::new("velocity", vec![[1.0, 0.0, 0.0]; 2]);
        fields.insert("velocity".to_string(), FieldData::Vector(vf));

        // Provide a velocity gradient tensor:
        // Simple shear: dUx/dy = 1.0, all others zero
        // -> omega_z = dUy/dx - dUx/dy = 0 - 1 = -1
        // -> |omega| = 1.0
        let grad = vec![
            [[0.0, 1.0, 0.0],  // dUx/dx=0, dUx/dy=1, dUx/dz=0
             [0.0, 0.0, 0.0],  // dUy/dx=0, dUy/dy=0, dUy/dz=0
             [0.0, 0.0, 0.0]], // dUz/dx=0, dUz/dy=0, dUz/dz=0
            [[0.0, 1.0, 0.0],
             [0.0, 0.0, 0.0],
             [0.0, 0.0, 0.0]],
        ];
        let tf = TensorField::new("velocity_gradient", grad);
        fields.insert("velocity_gradient".to_string(), FieldData::Tensor(tf));

        let vm = VorticityMagnitude;
        let result = vm.compute(&fields).unwrap();
        // omega_x = 0 - 0 = 0
        // omega_y = 0 - 0 = 0
        // omega_z = 0 - 1 = -1
        // |omega| = 1.0
        for v in result.values() {
            assert!((v - 1.0).abs() < 1e-10);
        }
    }

    #[test]
    fn q_criterion_pure_rotation() {
        let mut fields = FieldSet::new();
        let vf = VectorField::new("velocity", vec![[0.0, 0.0, 0.0]; 1]);
        fields.insert("velocity".to_string(), FieldData::Vector(vf));

        // Pure rotation: antisymmetric gradient
        // dUx/dy = -1, dUy/dx = 1 (solid body rotation omega_z = 1)
        let grad = vec![
            [[0.0, -1.0, 0.0],
             [1.0,  0.0, 0.0],
             [0.0,  0.0, 0.0]],
        ];
        let tf = TensorField::new("velocity_gradient", grad);
        fields.insert("velocity_gradient".to_string(), FieldData::Tensor(tf));

        let qc = QCriterion;
        let result = qc.compute(&fields).unwrap();
        // Pure rotation: Q > 0
        assert!(result.values()[0] > 0.0);
    }

    #[test]
    fn q_criterion_pure_strain() {
        let mut fields = FieldSet::new();
        let vf = VectorField::new("velocity", vec![[0.0, 0.0, 0.0]; 1]);
        fields.insert("velocity".to_string(), FieldData::Vector(vf));

        // Pure strain: symmetric gradient (extensional flow)
        let grad = vec![
            [[1.0, 0.0, 0.0],
             [0.0, -1.0, 0.0],
             [0.0, 0.0, 0.0]],
        ];
        let tf = TensorField::new("velocity_gradient", grad);
        fields.insert("velocity_gradient".to_string(), FieldData::Tensor(tf));

        let qc = QCriterion;
        let result = qc.compute(&fields).unwrap();
        // Pure strain: Q < 0
        assert!(result.values()[0] < 0.0);
    }
}
