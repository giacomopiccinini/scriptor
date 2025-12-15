use crate::stt::vad::VoiceActivityDetector;

/// Configuration for Fractor, responsible of dividing audio in fragmenta (chunks)
#[derive(Debug, Clone)]
pub struct FractorConfig {
    // Minimum duration of a fragmentum, to avoid very short recordings
    min_fragmentum_duration_seconds: f32,
    // Maximum duration of a fragmentum, to avoid large files harder to handle
    max_fragmentum_duration_seconds: f32,
    // A "chunk" is a chunk of samples (typically 512)
    // If in multiple consecutive chunks there's no speech, we declare it a pause
    // The threshold on chunks determines how long a pause should be, e.g.
    // pause_threshold_in_chunks = 16 means ~0.5s of pause at 16kHz
    pause_threshold_in_chunks: u32,
}

impl Default for FractorConfig {
    fn default() -> Self {
        Self {
            min_fragmentum_duration_seconds: 5.0_f32,
            max_fragmentum_duration_seconds: 20.0_f32,
            pause_threshold_in_chunks: 16_u32,
        }
    }
}

impl FractorConfig {
    fn new(
        min_fragmentum_duration_seconds: f32,
        max_fragmentum_duration_seconds: f32,
        pause_threshold_in_chunks: u32,
    ) -> Self {
        Self {
            min_fragmentum_duration_seconds,
            max_fragmentum_duration_seconds,
            pause_threshold_in_chunks,
        }
    }
}

pub struct Fractor {
    /// Voice activity detector
    pub vad: VoiceActivityDetector,
    /// Configuration for Fractor
    pub config: FractorConfig,
}

impl Fractor {
    /// Constructor with custom Fractor configuration
    pub fn with_config(vad: VoiceActivityDetector, config: FractorConfig) -> Self {
        Self {
            vad: vad,
            config: config,
        }
    }

    /// Constructor with default Factor configuration
    pub fn new(vad: VoiceActivityDetector) -> Self {
        Self::with_config(vad, FractorConfig::default())
    }

    /// Determine if we should cut the current recording based on duration and pause detection
    fn should_cut(
        self,
        current_duration_secs: f32,
        current_consecutive_silence_in_chunks: u32,
    ) -> bool {
        if current_duration_secs < self.config.min_fragmentum_duration_seconds as f32 {
            return false; // Too short, keep recording
        }
        if current_duration_secs >= self.config.max_fragmentum_duration_seconds as f32 {
            return true; // Force cut at max duration
        }
        // In target window: cut on pause.
        current_consecutive_silence_in_chunks >= self.config.pause_threshold_in_chunks
    }
}
