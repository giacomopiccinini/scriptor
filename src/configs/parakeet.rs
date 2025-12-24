use anyhow::{Context, Result};
use std::path::PathBuf;

/// Parakeet-specific configuration
pub struct ParakeetConfig {
    pub quantized: bool,
    pub encoder_path: PathBuf,
    pub decoder_joint_path: PathBuf,
    pub preprocessor_path: PathBuf,
    pub vocab_path: PathBuf,
}

impl ParakeetConfig {
    /// Load the appropriate Parakeet model depending on the fact that we want the quantized (int8)
    /// version or the full-precision (fp32) version.
    pub fn new(quantized: bool) -> Result<Self> {
        if quantized {
            let model_name = "parakeet-tdt-0.6b-v3-int8".to_string();
            let model_dir_path = dirs::data_dir()
                .with_context(|| "Failed to find data dir")?
                .join("scriptor")
                .join("models")
                .join("stt")
                .join(model_name);
            let encoder_path = model_dir_path.join("encoder-model.int8.onnx");
            let decoder_joint_path = model_dir_path.join("decoder_joint-model.int8.onnx");
            let preprocessor_path = model_dir_path.join("nemo128.onnx");
            let vocab_path = model_dir_path.join("vocabulary.txt");

            return Ok(Self {
                quantized: true,
                encoder_path: encoder_path,
                decoder_joint_path: decoder_joint_path,
                preprocessor_path: preprocessor_path,
                vocab_path: vocab_path,
            });
        } else {
            let model_name = "parakeet-tdt-0.6b-v3-fp32".to_string();
            let model_dir_path = dirs::data_dir()
                .with_context(|| "Failed to find data dir")?
                .join("scriptor")
                .join("models")
                .join("stt")
                .join(model_name);
            let encoder_path = model_dir_path.join("encoder-model.onnx");
            let decoder_joint_path = model_dir_path.join("decoder_joint-model.onnx");
            let preprocessor_path = model_dir_path.join("nemo128.onnx");
            let vocab_path = model_dir_path.join("vocabulary.txt");

            return Ok(Self {
                quantized: true,
                encoder_path: encoder_path,
                decoder_joint_path: decoder_joint_path,
                preprocessor_path: preprocessor_path,
                vocab_path: vocab_path,
            });
        }
    }
}
