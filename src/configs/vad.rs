use crate::configs::silero::SileroConfig;
use anyhow::Result;
use serde::{Deserialize, Serialize};

/// Available models for VAD (user-facing, for TOML)
#[derive(Debug, Copy, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub enum AvailableVADModel {
    #[default]
    #[serde(rename = "silero")]
    Silero,
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
