use super::silero::SileroVAD;
use crate::configs::inference::InferenceConfig;
use crate::configs::vad::{VADConfig, VADConfigKind};
use anyhow::Result;

/// Trait for VAD model backends (internal implementation detail)
pub trait VADBackend {
    /// Model-specific configuration parameters
    type ModelConfig;

    /// Load VAD model from config
    fn load(
        model_config: Self::ModelConfig,
        inference_config: InferenceConfig,
        threshold: f32,
    ) -> Result<Self>
    where
        Self: Sized;

    /// Reset the internal state (for streaming, call between audio streams)
    fn reset(&mut self);

    /// Returns the expected chunk size in samples
    fn chunk_size(&self) -> usize;

    /// Returns the expected sample rate in Hz
    fn sample_rate(&self) -> u32;

    /// Returns the configured threshold
    fn threshold(&self) -> f32;

    /// Predict speech probability for an audio chunk
    /// Output: probability in [0.0, 1.0]
    fn predict_proba(&mut self, samples: Vec<f32>) -> Result<f32>;

    /// Predict whether the audio chunk contains speech
    /// Returns `true` if speech probability >= threshold
    fn predict(&mut self, samples: Vec<f32>) -> Result<bool>;
}

/// Object-safe trait for voice activity detection
pub trait VoiceDetector: Send {
    /// Reset the internal state
    fn reset(&mut self);

    /// Returns the expected chunk size in samples
    fn chunk_size(&self) -> usize;

    /// Returns the expected sample rate in Hz
    fn sample_rate(&self) -> u32;

    /// Returns the configured threshold
    fn threshold(&self) -> f32;

    /// Predict speech probability for an audio chunk
    fn predict_proba(&mut self, samples: Vec<f32>) -> Result<f32>;

    /// Predict whether the audio chunk contains speech
    fn predict(&mut self, samples: Vec<f32>) -> Result<bool>;
}

/// Blanket implementation: any VADBackend automatically implements VoiceDetector
impl<T: VADBackend + Send> VoiceDetector for T {
    fn reset(&mut self) {
        <Self as VADBackend>::reset(self)
    }

    fn chunk_size(&self) -> usize {
        <Self as VADBackend>::chunk_size(self)
    }

    fn sample_rate(&self) -> u32 {
        <Self as VADBackend>::sample_rate(self)
    }

    fn threshold(&self) -> f32 {
        <Self as VADBackend>::threshold(self)
    }

    fn predict_proba(&mut self, samples: Vec<f32>) -> Result<f32> {
        <Self as VADBackend>::predict_proba(self, samples)
    }

    fn predict(&mut self, samples: Vec<f32>) -> Result<bool> {
        <Self as VADBackend>::predict(self, samples)
    }
}

/// High-level voice activity detection interface
///
/// This struct provides a unified API for loading and using VAD models
/// based on user configuration.
pub struct VADModel {
    detector: Box<dyn VoiceDetector>,
}

impl VADModel {
    /// Create a new VAD model based on configuration
    pub fn new(vad_config: &VADConfig, inference_config: InferenceConfig) -> Result<Self> {
        let threshold = vad_config.threshold;
        let detector: Box<dyn VoiceDetector> = match vad_config.get_model_config()? {
            VADConfigKind::Silero(cfg) => {
                Box::new(SileroVAD::load(cfg, inference_config, threshold)?)
            }
        };
        Ok(Self { detector })
    }

    /// Reset the internal state (call between audio streams)
    pub fn reset(&mut self) {
        self.detector.reset()
    }

    /// Returns the expected chunk size in samples
    pub fn chunk_size(&self) -> usize {
        self.detector.chunk_size()
    }

    /// Returns the expected sample rate in Hz
    pub fn sample_rate(&self) -> u32 {
        self.detector.sample_rate()
    }

    /// Returns the configured threshold
    pub fn threshold(&self) -> f32 {
        self.detector.threshold()
    }

    /// Predict speech probability for an audio chunk
    pub fn predict_proba(&mut self, samples: Vec<f32>) -> Result<f32> {
        self.detector.predict_proba(samples)
    }

    /// Predict whether the audio chunk contains speech
    pub fn predict(&mut self, samples: Vec<f32>) -> Result<bool> {
        self.detector.predict(samples)
    }
}
