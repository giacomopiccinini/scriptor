use serde::{Deserialize, Serialize};

/// Configuration for Fractor, responsible of dividing audio in fragmenta (chunks)
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct FractorConfig {
    // Minimum duration of a fragmentum, to avoid very short recordings
    pub min_fragmentum_duration_seconds: f32,
    // Maximum duration of a fragmentum, to avoid large files harder to handle
    pub max_fragmentum_duration_seconds: f32,
    // A "chunk" is a chunk of samples (typically 512)
    // If in multiple consecutive chunks there's no speech, we declare it a pause
    // The threshold on chunks determines how long a pause should be, e.g.
    // pause_threshold_in_chunks = 16 means ~0.5s of pause at 16kHz
    pub pause_threshold_in_chunks: u32,
}

impl Default for FractorConfig {
    fn default() -> Self {
        Self {
            min_fragmentum_duration_seconds: 10.0_f32,
            max_fragmentum_duration_seconds: 20.0_f32,
            pause_threshold_in_chunks: 24_u32,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fractor_config_default() {
        let config = FractorConfig::default();
        assert_eq!(config.min_fragmentum_duration_seconds, 10.0);
        assert_eq!(config.max_fragmentum_duration_seconds, 20.0);
        assert_eq!(config.pause_threshold_in_chunks, 24);
    }
}
