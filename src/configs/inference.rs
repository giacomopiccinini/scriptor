/// Common inference configuration for all ML models using ONNX Runtime
///
/// The actual conversion of InferenceConfig into objects ort understands is deferred to
/// model loading. The reason is that some of the ort objects do not implement clone/copy so it might become cumbersome
/// when loading multiple models.
use anyhow::Result;
use serde::{Deserialize, Serialize};

/// Available execution providers
#[derive(Debug, Copy, Clone, Serialize, Deserialize)]
pub enum ExecutionProvider {
    Cuda,
    Tensorrt,
    Coreml,
    Cpu,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct InferenceConfig {
    pub graph_optimization_level: usize,
    pub n_intra_threads: usize,
    pub parallel_execution: bool,
    pub execution_providers: Vec<ExecutionProvider>,
}

/// Default (decent) inference config
impl Default for InferenceConfig {
    fn default() -> Self {
        Self {
            graph_optimization_level: 3,
            n_intra_threads: 4,
            parallel_execution: true,
            execution_providers: vec![ExecutionProvider::Cuda],
        }
    }
}

// TODO: check if necessary?
/// Parse device string from config to ExecutionProvider
fn parse_device(device: &str) -> Result<ExecutionProvider> {
    match device.to_lowercase().as_str() {
        "coreml" => Ok(ExecutionProvider::Coreml),
        "cuda" => Ok(ExecutionProvider::Cuda),
        "tensorrt" => Ok(ExecutionProvider::Tensorrt),
        "cpu" => Ok(ExecutionProvider::Cpu),
        _ => anyhow::bail!(
            "Unknown device: {}. Use cuda, tensorrt, coreml, or cpu.",
            device
        ),
    }
}
