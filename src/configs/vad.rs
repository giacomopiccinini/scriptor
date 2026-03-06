use crate::configs::silero::SileroConfig;
use anyhow::Result;
use serde::{Deserialize, Serialize};

/// Available models for VAD (user-facing, for TOML)
#[derive(Debug, Copy, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub enum AvailableVADModel {
    #[default]
    #[serde(rename = "silero-v5")]
    Silero,
}

impl AvailableVADModel {
    /// Returns the TOML key string for this model
    pub fn as_key(&self) -> &'static str {
        match self {
            Self::Silero => "silero-v5",
        }
    }

    /// Convert a key string to the corresponding enum variant
    pub fn from_key(key: &str) -> Self {
        match key {
            "silero-v5" => Self::Silero,
            _ => Self::default(),
        }
    }
}

/// Wrapper for model-specific configurations
pub enum VADConfigKind {
    Silero(SileroConfig),
}

/// Voice Activity Detection configuration (from TOML)
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct VADConfig {
    pub model: AvailableVADModel,
    pub threshold: f32,
}

impl Default for VADConfig {
    fn default() -> Self {
        Self {
            model: AvailableVADModel::default(),
            threshold: 0.1,
        }
    }
}

impl VADConfig {
    /// Get the model-specific config with resolved paths
    pub fn get_model_config(&self) -> Result<VADConfigKind> {
        match self.model {
            AvailableVADModel::Silero => Ok(VADConfigKind::Silero(SileroConfig::default())),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_available_vad_model_as_key() {
        assert_eq!(AvailableVADModel::Silero.as_key(), "silero-v5");
    }

    #[test]
    fn test_available_vad_model_from_key() {
        assert_eq!(
            AvailableVADModel::from_key("silero-v5"),
            AvailableVADModel::Silero
        );
        assert_eq!(
            AvailableVADModel::from_key("unknown"),
            AvailableVADModel::default()
        );
    }

    #[test]
    fn test_vad_config_default() {
        let config = VADConfig::default();
        assert_eq!(config.model, AvailableVADModel::Silero);
        assert!((config.threshold - 0.1).abs() < 1e-6);
    }
}
