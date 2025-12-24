use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Configuration for VoiceActivityDetector
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct SileroConfig {
    /// Path to VAD model
    pub model_path: PathBuf,
    /// Sample rate for VAD
    pub sample_rate: i64,
    /// Number of samples in a chunk for speech detection
    pub chunk_size: usize,
    /// Shape of tensor describing the state
    pub state_shape: (usize, usize, usize),
}

impl Default for SileroConfig {
    fn default() -> Self {
        let model_path = dirs::data_dir()
            .expect("Could not find data directory")
            .join("scriptor")
            .join("models")
            .join("vad")
            .join("silero-vad.onnx");

        Self {
            model_path,
            sample_rate: 16_000,
            chunk_size: 512,
            state_shape: (2, 1, 128),
        }
    }
}
