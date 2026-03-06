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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_split_text_empty() {
        let t = Transcription {
            text: String::new(),
            segments: None,
        };
        assert_eq!(t.split_text(10), Vec::<String>::new());
    }

    #[test]
    fn test_split_text_single_sentence() {
        let t = Transcription {
            text: "Hello world.".to_string(),
            segments: None,
        };
        assert_eq!(t.split_text(100), vec!["Hello world."]);
    }

    #[test]
    fn test_split_text_multiple_sentences() {
        let t = Transcription {
            text: "First. Second! Third?".to_string(),
            segments: None,
        };
        // Use small target_size to force splitting at sentence boundaries
        assert_eq!(t.split_text(8), vec!["First.", "Second!", "Third?"]);
    }

    #[test]
    fn test_split_text_respects_target_size() {
        let t = Transcription {
            text: "A. B. C. D.".to_string(),
            segments: None,
        };
        let chunks = t.split_text(3);
        assert!(chunks.len() >= 2);
        for chunk in &chunks {
            assert!(chunk.len() <= 4, "chunk '{}' exceeds size", chunk);
        }
    }

    #[test]
    fn test_split_with_timestamps_fallback_no_segments() {
        let t = Transcription {
            text: "Hello. World.".to_string(),
            segments: None,
        };
        let chunks = t.split_with_timestamps(8);
        assert_eq!(chunks.len(), 2);
        assert_eq!(chunks[0].text, "Hello.");
        assert_eq!(chunks[0].start, 0.0);
        assert_eq!(chunks[0].end, 0.0);
    }

    #[test]
    fn test_split_with_timestamps_fallback_empty_segments() {
        let t = Transcription {
            text: "Hi.".to_string(),
            segments: Some(vec![]),
        };
        let chunks = t.split_with_timestamps(100);
        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0].text, "Hi.");
    }

    #[test]
    fn test_split_with_timestamps_with_segments() {
        let t = Transcription {
            text: "Hello world. How are you?".to_string(),
            segments: Some(vec![
                SegmentTranscription {
                    start: 0.0,
                    end: 1.0,
                    text: "Hello world.".to_string(),
                },
                SegmentTranscription {
                    start: 1.0,
                    end: 2.0,
                    text: "How are you?".to_string(),
                },
            ]),
        };
        let chunks = t.split_with_timestamps(15);
        assert_eq!(chunks.len(), 2);
        assert_eq!(chunks[0].text, "Hello world.");
        assert_eq!(chunks[0].start, 0.0);
        assert_eq!(chunks[0].end, 1.0);
        assert_eq!(chunks[1].text, "How are you?");
    }

    #[test]
    fn test_split_with_timestamps_chunks_by_size() {
        let t = Transcription {
            text: "A. B. C.".to_string(),
            segments: Some(vec![
                SegmentTranscription {
                    start: 0.0,
                    end: 0.5,
                    text: "A.".to_string(),
                },
                SegmentTranscription {
                    start: 0.5,
                    end: 1.0,
                    text: "B.".to_string(),
                },
                SegmentTranscription {
                    start: 1.0,
                    end: 1.5,
                    text: "C.".to_string(),
                },
            ]),
        };
        let chunks = t.split_with_timestamps(3);
        assert_eq!(chunks.len(), 3);
    }
}
