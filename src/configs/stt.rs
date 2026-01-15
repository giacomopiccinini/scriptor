use crate::configs::parakeet::ParakeetConfig;
use anyhow::Result;
use serde::{Deserialize, Serialize};

/// Available models for STT (user-facing, for TOML)
#[derive(Debug, Copy, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub enum AvailableSTTModel {
    #[serde(rename = "parakeet-tdt-0_6b-v3-fp32")]
    ParakeetTdt06BV3Fp32,

    #[default]
    #[serde(rename = "parakeet-tdt-0_6b-v3-int8")]
    ParakeetTdt06BV3Int8,
}

impl AvailableSTTModel {
    /// Returns the TOML key string for this model
    pub fn as_key(&self) -> &'static str {
        match self {
            Self::ParakeetTdt06BV3Fp32 => "parakeet-tdt-0_6b-v3-fp32",
            Self::ParakeetTdt06BV3Int8 => "parakeet-tdt-0_6b-v3-int8",
        }
    }
}

/// Wrapper for model-specific configurations
pub enum ModelConfigKind {
    Parakeet(ParakeetConfig),
}

/// Speech-to-text configuration (from TOML)
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct STTConfig {
    pub model: AvailableSTTModel,
}

impl STTConfig {
    /// Get the model-specific config with resolved paths
    pub fn get_model_config(&self) -> Result<ModelConfigKind> {
        match self.model {
            AvailableSTTModel::ParakeetTdt06BV3Fp32 => {
                Ok(ModelConfigKind::Parakeet(ParakeetConfig::new(false)?))
            }
            AvailableSTTModel::ParakeetTdt06BV3Int8 => {
                Ok(ModelConfigKind::Parakeet(ParakeetConfig::new(true)?))
            }
        }
    }
}
