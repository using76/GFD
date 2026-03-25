//! Point probes for monitoring field values at specific locations.

use std::io::Write;

use serde::Deserialize;
use gfd_core::field::FieldData;
use crate::Result;
use crate::IoError;

/// A point probe that records field values at a specific location.
#[derive(Debug, Clone, Deserialize)]
pub struct Probe {
    /// Name of the probe.
    pub name: String,
    /// Location in 3D space [x, y, z].
    pub location: [f64; 3],
    /// Names of the fields to monitor at this location.
    pub fields: Vec<String>,
}

impl Probe {
    /// Creates a new probe.
    pub fn new(
        name: impl Into<String>,
        location: [f64; 3],
        fields: Vec<String>,
    ) -> Self {
        Self {
            name: name.into(),
            location,
            fields,
        }
    }
}

/// Writer that outputs probe data to files over the course of a simulation.
pub struct ProbeWriter {
    /// List of probes to monitor.
    pub probes: Vec<Probe>,
    /// Output directory for probe files.
    pub output_dir: String,
    /// Collected time history: Vec<(time, Vec<Vec<f64>>)>
    /// Outer Vec: one entry per time sample.
    /// Middle Vec: one entry per probe.
    /// Inner Vec: one value per field monitored by that probe.
    history: Vec<(f64, Vec<Vec<f64>>)>,
}

impl ProbeWriter {
    /// Creates a new probe writer.
    pub fn new(probes: Vec<Probe>, output_dir: impl Into<String>) -> Self {
        Self {
            probes,
            output_dir: output_dir.into(),
            history: Vec::new(),
        }
    }

    /// Finds the index of the cell whose center is nearest to the given point.
    fn find_nearest_cell(
        mesh: &gfd_core::UnstructuredMesh,
        point: &[f64; 3],
    ) -> usize {
        let mut best_cell = 0;
        let mut best_dist_sq = f64::MAX;

        for (i, cell) in mesh.cells.iter().enumerate() {
            let dx = cell.center[0] - point[0];
            let dy = cell.center[1] - point[1];
            let dz = cell.center[2] - point[2];
            let dist_sq = dx * dx + dy * dy + dz * dz;
            if dist_sq < best_dist_sq {
                best_dist_sq = dist_sq;
                best_cell = i;
            }
        }

        best_cell
    }

    /// Samples all probes at the current time and stores the values.
    pub fn sample(
        &mut self,
        time: f64,
        fields: &gfd_core::FieldSet,
        mesh: &gfd_core::UnstructuredMesh,
    ) -> Result<()> {
        let mut probe_values = Vec::with_capacity(self.probes.len());

        for probe in &self.probes {
            let cell_id = Self::find_nearest_cell(mesh, &probe.location);
            let mut values = Vec::with_capacity(probe.fields.len());

            for field_name in &probe.fields {
                let val = if let Some(field_data) = fields.get(field_name) {
                    match field_data {
                        FieldData::Scalar(sf) => {
                            sf.get(cell_id).unwrap_or(0.0)
                        }
                        FieldData::Vector(vf) => {
                            // For vector fields, record the magnitude at this cell.
                            if let Ok(v) = vf.get(cell_id) {
                                (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt()
                            } else {
                                0.0
                            }
                        }
                        FieldData::Tensor(_tf) => {
                            // For tensor fields, record the trace.
                            if let Ok(t) = _tf.get(cell_id) {
                                t[0][0] + t[1][1] + t[2][2]
                            } else {
                                0.0
                            }
                        }
                    }
                } else {
                    0.0
                };
                values.push(val);
            }

            probe_values.push(values);
        }

        self.history.push((time, probe_values));
        Ok(())
    }

    /// Writes accumulated probe data to CSV files.
    ///
    /// One CSV file per probe, named `<output_dir>/<probe_name>.csv`.
    pub fn write(&self) -> Result<()> {
        // Ensure output directory exists.
        std::fs::create_dir_all(&self.output_dir)?;

        for (probe_idx, probe) in self.probes.iter().enumerate() {
            let file_path = format!("{}/{}.csv", self.output_dir, probe.name);
            let mut file = std::fs::File::create(&file_path)
                .map_err(|e| IoError::WriteError(format!("Cannot create {}: {}", file_path, e)))?;

            // Write header.
            write!(file, "time").map_err(|e| IoError::WriteError(e.to_string()))?;
            for field_name in &probe.fields {
                write!(file, ",{}", field_name).map_err(|e| IoError::WriteError(e.to_string()))?;
            }
            writeln!(file).map_err(|e| IoError::WriteError(e.to_string()))?;

            // Write data rows.
            for (time, all_probe_vals) in &self.history {
                write!(file, "{:.6e}", time).map_err(|e| IoError::WriteError(e.to_string()))?;
                if let Some(vals) = all_probe_vals.get(probe_idx) {
                    for v in vals {
                        write!(file, ",{:.6e}", v).map_err(|e| IoError::WriteError(e.to_string()))?;
                    }
                }
                writeln!(file).map_err(|e| IoError::WriteError(e.to_string()))?;
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gfd_core::field::{ScalarField, FieldData};
    use gfd_core::mesh::cell::Cell;
    use gfd_core::mesh::unstructured::UnstructuredMesh;
    use std::collections::HashMap;

    #[test]
    fn probe_sampling_and_write() {
        // Create a simple 2-cell mesh.
        let cells = vec![
            Cell::new(0, vec![], vec![], 1.0, [0.5, 0.5, 0.5]),
            Cell::new(1, vec![], vec![], 1.0, [1.5, 0.5, 0.5]),
        ];
        let mesh = UnstructuredMesh::from_components(vec![], vec![], cells, vec![]);

        // Create a field set with a scalar field.
        let mut fields: gfd_core::FieldSet = HashMap::new();
        let sf = ScalarField::new("temperature", vec![100.0, 200.0]);
        fields.insert("temperature".to_string(), FieldData::Scalar(sf));

        // Create a probe near cell 1.
        let probe = Probe::new(
            "sensor1",
            [1.4, 0.5, 0.5],
            vec!["temperature".to_string()],
        );

        let output_dir = std::env::temp_dir().join("gfd_probe_test");
        let output_dir_str = output_dir.to_str().unwrap().to_string();

        let mut writer = ProbeWriter::new(vec![probe], &output_dir_str);
        writer.sample(0.0, &fields, &mesh).unwrap();
        writer.sample(0.1, &fields, &mesh).unwrap();
        writer.write().unwrap();

        // Verify the CSV file was created.
        let csv_path = format!("{}/sensor1.csv", output_dir_str);
        let contents = std::fs::read_to_string(&csv_path).unwrap();
        assert!(contents.contains("time,temperature"));
        // Probe is near cell 1, so value should be 200.0
        assert!(contents.contains("2.000000e2"));

        // Cleanup
        let _ = std::fs::remove_dir_all(&output_dir_str);
    }
}
