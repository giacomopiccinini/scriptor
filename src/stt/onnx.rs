use crate::configs::inference::{ExecutionProvider, InferenceConfig};
use anyhow::{Context, Result};
use ort::execution_providers::ExecutionProviderDispatch;
use ort::execution_providers::{
    CPUExecutionProvider, CUDAExecutionProvider, CoreMLExecutionProvider, TensorRTExecutionProvider,
};
use ort::session::{Session, builder::GraphOptimizationLevel};
use std::fs;
use std::path::PathBuf;

/// Load a ONNX model from a locally-saved .onnx file.
/// Applies graph optimization of various level, introduces intra threads and
/// uses execution providers.
/// Parallel execution: allows independent operators to run concurrently on different threads
/// Intra threads: controls intra-op parallelism, splitting computation within a single operator across multiple CPU threads
pub fn load_onnx_model(
    onnx_file_path: PathBuf,
    inference_config: InferenceConfig,
) -> Result<Session> {
    // Sanity check on file
    if !onnx_file_path.exists() {
        anyhow::bail!(format!(
            "Model not admissible. {} does not exist!",
            onnx_file_path.display()
        ))
    };
    if !onnx_file_path.is_file() {
        anyhow::bail!("Model not admissible. Model is not a file.")
    };
    if onnx_file_path.extension().and_then(|ext| ext.to_str()) != Some("onnx") {
        anyhow::bail!("Model is not in ONNX format")
    };

    // Convert to actual GraphOptimizationLevel object used by ort
    let graph_optimization_level = match inference_config.graph_optimization_level {
        1 => GraphOptimizationLevel::Level1,
        2 => GraphOptimizationLevel::Level2,
        3 => GraphOptimizationLevel::Level3,
        _ => anyhow::bail!(format!(
            "Graph optimization level {} not admissible. Admissible values are 1, 2, or 3.",
            inference_config.graph_optimization_level
        )),
    };

    // Convert to list of actual ExecutionProvider object used by ort
    let execution_providers: Vec<ExecutionProviderDispatch> = inference_config
        .execution_providers
        .into_iter()
        .map(|e| match e {
            ExecutionProvider::Cuda => CUDAExecutionProvider::default().build(),
            ExecutionProvider::Tensorrt => TensorRTExecutionProvider::default().build(),
            ExecutionProvider::Coreml => CoreMLExecutionProvider::default().build(),
            ExecutionProvider::Cpu => CPUExecutionProvider::default().build(),
        })
        .collect();

    // Load model with requested optimizations
    let onnx_model = Session::builder()
        .with_context(|| "Failed to initialize ORT session")?
        .with_optimization_level(graph_optimization_level)
        .with_context(|| "Failed to introduce graph optimization")?
        .with_intra_threads(inference_config.n_intra_threads)
        .with_context(|| "Failed use of intra threads")?
        .with_parallel_execution(inference_config.parallel_execution)
        .with_context(|| "Failed use of parallel execution")?
        .with_execution_providers(execution_providers)
        .with_context(|| "Failed use of execution providers")?
        .commit_from_file(onnx_file_path)
        .with_context(|| "Failed to load ONNX model from file")?;

    Ok(onnx_model)
}

/// Read vocabulary from txt file.
/// The vocabulary file must be organized such that every line contains a token.
/// The line number is assumed to coincide with the token id.
pub fn load_vocabulary(vocabulary_file_path: PathBuf) -> Result<Vec<String>> {
    // Sanity check on file
    if !vocabulary_file_path.is_file()
        || vocabulary_file_path
            .extension()
            .and_then(|ext| ext.to_str())
            != Some("txt")
    {
        anyhow::bail!("Vocabulary file is not admissible. Must be a single .txt file.")
    }

    // Read txt file and return vector of strings (each string is a token)
    Ok(fs::read_to_string(vocabulary_file_path)
        .with_context(|| "Unable to read vocabulary file")?
        .lines()
        .map(|s| s.to_string())
        .collect())
}
