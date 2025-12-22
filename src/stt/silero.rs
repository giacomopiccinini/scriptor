use super::vad::VADBackend;
use crate::configs::inference::InferenceConfig;
use crate::configs::silero::SileroConfig;
use crate::stt::onnx::load_onnx_model;
use anyhow::{Context, Result};
use ndarray::{Array1, Array2, Array3};
use ort::session::Session;
use ort::value::TensorRef;

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
pub struct SileroVAD {
    /// Loaded ONNX model
    model: Session,
    /// VAD model config
    config: SileroConfig,
    /// Speech probability threshold for `predict()`. Range: [0.0, 1.0]
    threshold: f32,
    /// LSTM hidden state, shape (2, 1, 128). Updated after each inference.
    state: Array3<f32>,
}

impl SileroVAD {
    /// Predict speech probability for an audio chunk.
    ///
    /// Input: f32 samples at 16kHz. Padded/truncated to 512 samples.
    /// Output: probability in [0.0, 1.0].
    fn predict_proba_impl(&mut self, samples: Vec<f32>) -> Result<f32> {
        // Prepare input: pad with zeros or truncate to CHUNK_SIZE
        let mut input = Array2::<f32>::zeros((1, self.config.chunk_size));
        for (i, &sample) in samples.iter().take(self.config.chunk_size).enumerate() {
            input[[0, i]] = sample;
        }

        // Sample rate as 1D array (model expects this shape)
        let sr = Array1::from_vec(vec![self.config.sample_rate]);

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
}

/// Implement VADBackend trait for Silero
impl VADBackend for SileroVAD {
    type ModelConfig = SileroConfig;

    fn load(
        model_config: Self::ModelConfig,
        inference_config: InferenceConfig,
        threshold: f32,
    ) -> Result<Self> {
        // Load ONNX model
        let model = load_onnx_model(model_config.model_path.clone(), inference_config)
            .with_context(|| "Failed to load Silero VAD model")?;

        // Define the state
        let state = Array3::zeros(model_config.state_shape);

        Ok(Self {
            model,
            config: model_config,
            threshold,
            state,
        })
    }

    fn reset(&mut self) {
        self.state = Array3::zeros(self.config.state_shape);
    }

    fn chunk_size(&self) -> usize {
        self.config.chunk_size
    }

    fn sample_rate(&self) -> u32 {
        self.config.sample_rate as u32
    }

    fn threshold(&self) -> f32 {
        self.threshold
    }

    fn predict_proba(&mut self, samples: Vec<f32>) -> Result<f32> {
        self.predict_proba_impl(samples)
    }

    fn predict(&mut self, samples: Vec<f32>) -> Result<bool> {
        let prob = self.predict_proba_impl(samples)?;
        Ok(prob >= self.threshold)
    }
}
