//! JSON-RPC message types bridging the Electron GUI and the Rust CAD backend.
//!
//! Wire format follows the JSON-RPC 2.0 envelope; method-specific params/result
//! variants are serialized via serde tag="method".

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "method", content = "params", rename_all = "snake_case")]
pub enum CadRequest {
    /// Create a new empty document.
    DocumentNew,
    /// Return the feature tree as JSON.
    TreeGet,
    /// Create a primitive feature (box/cylinder/sphere/...).
    PrimitiveCreate { kind: String, params: serde_json::Value },
    /// Tessellate a feature's result shape.
    Tessellate { feature_id: u32 },
    /// Import a STEP file.
    ImportStep { path: String },
    /// Export to STEP.
    ExportStep { path: String },
    /// Compute a measurement.
    Measure { kind: String, args: serde_json::Value },
    /// Run shape healing.
    Heal { shape_id: u32, options: serde_json::Value },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CadResponse {
    pub ok: bool,
    pub result: Option<serde_json::Value>,
    pub error: Option<String>,
}

impl CadResponse {
    pub fn ok(value: serde_json::Value) -> Self {
        Self { ok: true, result: Some(value), error: None }
    }
    pub fn err(msg: impl Into<String>) -> Self {
        Self { ok: false, result: None, error: Some(msg.into()) }
    }
}
