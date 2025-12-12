use crate::stt::onnx::{InferenceConfig, load_onnx_model};
use anyhow::{Context, Result};
use ndarray::{Array1, Array2, Array3};
use ort::session::Session;
use ort::value::TensorRef;
use std::path::PathBuf;

/// Silero VAD v5 requires exactly 512 samples per chunk at 16kHz
const CHUNK_SIZE: usize = 512;
/// Sample rate is fixed at 16kHz for Silero VAD
const SAMPLE_RATE: i64 = 16000;
/// Hidden state dimensions for Silero VAD v5: shape (2, 1, 128)
/// - 2 layers (stacked LSTM)
/// - 1 batch size
/// - 128 hidden units per layer
const STATE_SHAPE: (usize, usize, usize) = (2, 1, 128);

/// Configuration for VoiceActivityDetector
#[derive(Debug, Clone)]
pub struct VadConfig {
    /// Path to directory containing `silero-vad.onnx`
    pub model_dir: PathBuf,
    /// Speech probability threshold for `predict()`. Range: [0.0, 1.0]
    pub threshold: f32,
}

/// Minimalistic Voice Activity Detector using Silero VAD v5
///
/// Silero VAD is an LSTM-based recurrent neural network. The `state` field holds
/// the LSTM hidden states (h, c) which carry temporal context from previous audio
/// chunks. This allows the model to make better predictions by "remembering" what
/// it heard before.
///
/// - For **streaming audio**: keep the state between calls to maintain context
/// - For **isolated chunks**: call `reset()` before each prediction
/// - The state is automatically updated after each `predict_proba()` call
pub struct VoiceActivityDetector {
    model: Session,
    config: VadConfig,
    /// LSTM hidden state, shape (2, 1, 128). Updated after each inference.
    state: Array3<f32>,
}

impl VoiceActivityDetector {
    /// Create a new VAD from config
    pub fn new(config: VadConfig, inference_config: InferenceConfig) -> Result<Self> {
        let model_path = config.model_dir.join("silero-vad.onnx");
        let model = load_onnx_model(model_path, inference_config)
            .with_context(|| "Failed to load Silero VAD model")?;

        Ok(Self {
            model,
            config,
            state: Array3::zeros(STATE_SHAPE),
        })
    }

    /// Reset the LSTM state. Call this when switching to a new audio stream.
    pub fn reset(&mut self) {
        self.state = Array3::zeros(STATE_SHAPE);
    }

    /// Returns the expected chunk size (512 samples at 16kHz = 32ms)
    pub fn chunk_size(&self) -> usize {
        CHUNK_SIZE
    }

    /// Returns the configured threshold
    pub fn threshold(&self) -> f32 {
        self.config.threshold
    }

    /// Predict speech probability for an audio chunk.
    ///
    /// Input: f32 samples at 16kHz. Padded/truncated to 512 samples.
    /// Output: probability in [0.0, 1.0].
    pub fn predict_proba(&mut self, samples: Vec<f32>) -> Result<f32> {
        // Prepare input: pad with zeros or truncate to CHUNK_SIZE
        let mut input = Array2::<f32>::zeros((1, CHUNK_SIZE));
        for (i, &sample) in samples.iter().take(CHUNK_SIZE).enumerate() {
            input[[0, i]] = sample;
        }

        // Sample rate as 1D array (model expects this shape)
        let sr = Array1::from_vec(vec![SAMPLE_RATE]);

        // Prepare inputs using views (no copies)
        let inputs = ort::inputs![
            "input" => TensorRef::from_array_view(input.view()).with_context(|| "Failed to instantiate input tensor")?,
            "state" => TensorRef::from_array_view(self.state.view()).with_context(|| "Failed to instantiate state tensor")?,
            "sr" => TensorRef::from_array_view(sr.view()).with_context(|| "Failed to instantiate sr tensor")?,
        ];

        let outputs = self
            .model
            .run(inputs)
            .with_context(|| "VAD inference failed")?;

        // Update LSTM state for next call
        let new_state = outputs
            .get("stateN")
            .with_context(|| "Missing 'stateN' output")?
            .try_extract_array::<f32>()
            .with_context(|| "Failed to extract state")?;
        self.state = new_state
            .to_owned()
            .into_dimensionality()
            .with_context(|| "State has wrong shape")?;

        // Extract speech probability
        let output = outputs
            .get("output")
            .with_context(|| "Missing 'output'")?
            .try_extract_array::<f32>()
            .with_context(|| "Failed to extract output")?;

        Ok(output[[0, 0]])
    }

    /// Predict whether the audio chunk contains speech (bool).
    ///
    /// Returns `true` if speech probability >= threshold.
    pub fn predict(&mut self, samples: Vec<f32>) -> Result<bool> {
        let prob = self
            .predict_proba(samples)
            .with_context(|| "Failed to predict proba")?;
        Ok(prob >= self.config.threshold)
    }
}
