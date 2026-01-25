use super::parakeet::ParakeetModel;
use crate::configs::inference::InferenceConfig;
use crate::configs::stt::{ModelConfigKind, STTConfig};
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
/// Trait for STT model backends (internal implementation detail)
pub trait STTBackend {
    /// Model-specific configuration parameters
    type ModelConfig;

    /// Load model weights and config
    fn load(model_config: Self::ModelConfig, inference_config: InferenceConfig) -> Result<Self>
    where
        Self: Sized;

    /// Load audio from .wav file
    fn load_audio(&self, audio_path: &Path) -> Result<Vec<f32>>;

    /// Transcribe samples
    fn transcribe(&mut self, audio_samples: Vec<f32>) -> Result<Transcription>;
}

/// Object-safe trait for audio transcription
pub trait AudioTranscriber: Send {
    /// Load audio from .wav file
    fn load_audio(&self, audio_path: &Path) -> Result<Vec<f32>>;

    /// Transcribe audio samples to text
    fn transcribe(&mut self, audio_samples: Vec<f32>) -> Result<Transcription>;
}

/// Blanket implementation: any STTBackend automatically implements AudioTranscriber
impl<T: STTBackend + Send> AudioTranscriber for T {
    fn load_audio(&self, audio_path: &Path) -> Result<Vec<f32>> {
        <Self as STTBackend>::load_audio(self, audio_path)
    }

    fn transcribe(&mut self, audio_samples: Vec<f32>) -> Result<Transcription> {
        <Self as STTBackend>::transcribe(self, audio_samples)
    }
}

/// High-level speech-to-text model interface
///
/// This struct provides a unified API for loading and using STT models
/// based on user configuration.
pub struct STTModel {
    transcriber: Box<dyn AudioTranscriber>,
}

impl STTModel {
    /// Create a new STT model based on configuration
    pub fn new(stt_config: &STTConfig, inference_config: InferenceConfig) -> Result<Self> {
        let transcriber: Box<dyn AudioTranscriber> = match stt_config.get_model_config()? {
            ModelConfigKind::Parakeet(cfg) => Box::new(ParakeetModel::load(cfg, inference_config)?),
        };
        Ok(Self { transcriber })
    }

    /// Load audio from a .wav file
    pub fn load_audio(&self, audio_path: &Path) -> Result<Vec<f32>> {
        self.transcriber.load_audio(audio_path)
    }

    /// Transcribe audio samples to text
    pub fn transcribe(&mut self, audio_samples: Vec<f32>) -> Result<Transcription> {
        self.transcriber.transcribe(audio_samples)
    }
}

impl Transcription {
    pub fn split_text(&self, target_size: usize) -> Vec<String> {
        let mut chunks = Vec::new();
        let mut current = String::new();

        for sentence in self.text.split_inclusive(['.', '!', '?']) {
            if current.len() + sentence.len() > target_size && !current.is_empty() {
                chunks.push(current.trim().to_string());
                current = String::new();
            }
            current.push_str(sentence);
        }
        if !current.is_empty() {
            chunks.push(current.trim().to_string());
        }
        chunks
    }

    /// Split transcription into chunks with timestamps, grouping segments up to target_size characters
    pub fn split_with_timestamps(&self, target_size: usize) -> Vec<SegmentTranscription> {
        // If no segments available, fall back to split_text with estimated timestamps
        let segments = match &self.segments {
            Some(segs) if !segs.is_empty() => segs,
            _ => {
                // Fallback: split text without timestamps
                return self
                    .split_text(target_size)
                    .into_iter()
                    .map(|text| SegmentTranscription {
                        text,
                        start: 0.0,
                        end: 0.0,
                    })
                    .collect();
            }
        };

        let mut chunks = Vec::new();
        let mut current_text = String::new();
        let mut chunk_start: Option<f32> = None;
        let mut chunk_end: f32 = 0.0;

        for segment in segments {
            let segment_text = segment.text.trim();
            if segment_text.is_empty() {
                continue;
            }

            // Check if adding this segment would exceed target size
            let would_exceed = !current_text.is_empty()
                && current_text.len() + segment_text.len() + 1 > target_size;

            if would_exceed {
                // Save current chunk
                if !current_text.is_empty() {
                    chunks.push(SegmentTranscription {
                        text: current_text.trim().to_string(),
                        start: chunk_start.unwrap_or(0.0),
                        end: chunk_end,
                    });
                }
                // Start new chunk
                current_text = String::new();
                chunk_start = None;
            }

            // Add segment to current chunk
            if !current_text.is_empty() {
                current_text.push(' ');
            }
            current_text.push_str(segment_text);

            // Track timestamps
            if chunk_start.is_none() {
                chunk_start = Some(segment.start);
            }
            chunk_end = segment.end;
        }

        // Don't forget the last chunk
        if !current_text.is_empty() {
            chunks.push(SegmentTranscription {
                text: current_text.trim().to_string(),
                start: chunk_start.unwrap_or(0.0),
                end: chunk_end,
            });
        }

        chunks
    }
}
