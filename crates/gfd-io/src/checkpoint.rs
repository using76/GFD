//! Checkpoint save/load for simulation restart.

use std::collections::HashMap;

use gfd_core::FieldSet;
use gfd_core::field::{FieldData, ScalarField, VectorField, TensorField};
use crate::Result;
use crate::IoError;

/// Internal serializable representation of a checkpoint.
#[derive(serde::Serialize, serde::Deserialize)]
struct CheckpointData {
    /// Iteration number at checkpoint.
    iteration: usize,
    /// Simulation time at checkpoint.
    time: f64,
    /// Scalar fields: name -> values.
    scalar_fields: HashMap<String, Vec<f64>>,
    /// Vector fields: name -> flattened [x0,y0,z0,x1,y1,z1,...].
    vector_fields: HashMap<String, Vec<f64>>,
    /// Tensor fields: name -> flattened row-major 9 components per cell.
    tensor_fields: HashMap<String, Vec<f64>>,
}

/// Saves a simulation checkpoint to disk.
///
/// The checkpoint contains all field data needed to restart the simulation
/// from this point, serialized as JSON for human-readability.
pub fn save_checkpoint(
    path: &str,
    fields: &FieldSet,
    iteration: usize,
    time: f64,
) -> Result<()> {
    let mut scalar_fields = HashMap::new();
    let mut vector_fields = HashMap::new();
    let mut tensor_fields = HashMap::new();

    for (name, field_data) in fields {
        match field_data {
            FieldData::Scalar(sf) => {
                scalar_fields.insert(name.clone(), sf.values().to_vec());
            }
            FieldData::Vector(vf) => {
                let flat: Vec<f64> = vf.values().iter()
                    .flat_map(|v| v.iter().copied())
                    .collect();
                vector_fields.insert(name.clone(), flat);
            }
            FieldData::Tensor(tf) => {
                let flat: Vec<f64> = tf.values().iter()
                    .flat_map(|t| t.iter().flat_map(|row| row.iter().copied()))
                    .collect();
                tensor_fields.insert(name.clone(), flat);
            }
        }
    }

    let data = CheckpointData {
        iteration,
        time,
        scalar_fields,
        vector_fields,
        tensor_fields,
    };

    let json = serde_json::to_string_pretty(&data)?;
    std::fs::write(path, json)?;

    Ok(())
}

/// Loads a simulation checkpoint from disk.
///
/// Returns the field set and the (iteration, time) at which the checkpoint was saved.
pub fn load_checkpoint(path: &str) -> Result<(FieldSet, usize, f64)> {
    let contents = std::fs::read_to_string(path)
        .map_err(|_| IoError::FileNotFound(path.to_string()))?;

    let data: CheckpointData = serde_json::from_str(&contents)?;

    let mut fields = FieldSet::new();

    // Reconstruct scalar fields.
    for (name, values) in data.scalar_fields {
        let sf = ScalarField::new(&name, values);
        fields.insert(name, FieldData::Scalar(sf));
    }

    // Reconstruct vector fields.
    for (name, flat) in data.vector_fields {
        let num_cells = flat.len() / 3;
        let mut vecs = Vec::with_capacity(num_cells);
        for i in 0..num_cells {
            vecs.push([flat[i * 3], flat[i * 3 + 1], flat[i * 3 + 2]]);
        }
        let vf = VectorField::new(&name, vecs);
        fields.insert(name, FieldData::Vector(vf));
    }

    // Reconstruct tensor fields.
    for (name, flat) in data.tensor_fields {
        let num_cells = flat.len() / 9;
        let mut tensors = Vec::with_capacity(num_cells);
        for i in 0..num_cells {
            let base = i * 9;
            tensors.push([
                [flat[base], flat[base + 1], flat[base + 2]],
                [flat[base + 3], flat[base + 4], flat[base + 5]],
                [flat[base + 6], flat[base + 7], flat[base + 8]],
            ]);
        }
        let tf = TensorField::new(&name, tensors);
        fields.insert(name, FieldData::Tensor(tf));
    }

    Ok((fields, data.iteration, data.time))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip_checkpoint() {
        let mut fields = FieldSet::new();
        let sf = ScalarField::new("pressure", vec![1.0, 2.0, 3.0]);
        fields.insert("pressure".to_string(), FieldData::Scalar(sf));
        let vf = VectorField::new("velocity", vec![[1.0, 2.0, 3.0], [4.0, 5.0, 6.0]]);
        fields.insert("velocity".to_string(), FieldData::Vector(vf));

        let path = std::env::temp_dir().join("gfd_checkpoint_test.json");
        let path_str = path.to_str().unwrap();

        save_checkpoint(path_str, &fields, 42, 1.5).unwrap();
        let (loaded_fields, iteration, time) = load_checkpoint(path_str).unwrap();

        assert_eq!(iteration, 42);
        assert!((time - 1.5).abs() < 1e-12);

        // Check scalar field
        if let Some(FieldData::Scalar(sf)) = loaded_fields.get("pressure") {
            assert_eq!(sf.values(), &[1.0, 2.0, 3.0]);
        } else {
            panic!("Expected scalar field 'pressure'");
        }

        // Check vector field
        if let Some(FieldData::Vector(vf)) = loaded_fields.get("velocity") {
            assert_eq!(vf.values(), &[[1.0, 2.0, 3.0], [4.0, 5.0, 6.0]]);
        } else {
            panic!("Expected vector field 'velocity'");
        }

        // Cleanup
        let _ = std::fs::remove_file(path_str);
    }
}
