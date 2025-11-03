use super::onnx::InferenceConfig;
use anyhow::Result;
use std::path::Path;

/// Taken from https://github.com/cjpais/transcribe-rs/blob/main/src/lib.rs
/// The result of a transcription operation.
///
/// Contains both the full transcribed text and detailed timing information
/// for individual segments within the audio.
#[derive(Debug)]
pub struct Transcription {
    /// The complete transcribed text from the audio
    pub text: String,
    /// Individual segments with timing information
    pub segments: Option<Vec<SegmentTranscription>>,
}

/// Taken from https://github.com/cjpais/transcribe-rs/blob/main/src/lib.rs
/// A single transcribed segment with timing information.
///
/// Represents a portion of the transcribed audio with start and end timestamps
/// and the corresponding text content.
#[derive(Debug)]
pub struct SegmentTranscription {
    /// Start time of the segment in seconds
    pub start: f32,
    /// End time of the segment in seconds
    pub end: f32,
    /// The transcribed text for this segment
    pub text: String,
}

/// Inspired by https://github.com/cjpais/transcribe-rs/blob/main/src/lib.rs
// Trait for speech-to-text (STT) models
pub trait STTModel {
    /// Model-specific configuration parameters
    type ModelConfig;

    /// Load model weights and config
    fn new(mdeol_config: Self::ModelConfig, inference_config: InferenceConfig) -> Result<Self>
    where
        Self: Sized;

    /// Load audio from .wav file
    fn load_audio(&self, audio_path: &Path) -> Result<Vec<f32>>;

    /// Transcribe samples
    fn transcribe(&mut self, audio_samples: Vec<f32>) -> Result<Transcription>;
}
