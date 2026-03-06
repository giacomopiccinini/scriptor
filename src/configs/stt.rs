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

    /// Convert a key string to the corresponding enum variant
    pub fn from_key(key: &str) -> Self {
        match key {
            "parakeet-tdt-0_6b-v3-fp32" => Self::ParakeetTdt06BV3Fp32,
            "parakeet-tdt-0_6b-v3-int8" => Self::ParakeetTdt06BV3Int8,
            _ => Self::default(),
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_available_stt_model_as_key() {
        assert_eq!(
            AvailableSTTModel::ParakeetTdt06BV3Fp32.as_key(),
            "parakeet-tdt-0_6b-v3-fp32"
        );
        assert_eq!(
            AvailableSTTModel::ParakeetTdt06BV3Int8.as_key(),
            "parakeet-tdt-0_6b-v3-int8"
        );
    }

    #[test]
    fn test_available_stt_model_from_key() {
        assert_eq!(
            AvailableSTTModel::from_key("parakeet-tdt-0_6b-v3-fp32"),
            AvailableSTTModel::ParakeetTdt06BV3Fp32
        );
        assert_eq!(
            AvailableSTTModel::from_key("parakeet-tdt-0_6b-v3-int8"),
            AvailableSTTModel::ParakeetTdt06BV3Int8
        );
        assert_eq!(
            AvailableSTTModel::from_key("unknown"),
            AvailableSTTModel::default()
        );
    }

    #[test]
    fn test_as_key_from_key_roundtrip() {
        for model in [
            AvailableSTTModel::ParakeetTdt06BV3Fp32,
            AvailableSTTModel::ParakeetTdt06BV3Int8,
        ] {
            assert_eq!(AvailableSTTModel::from_key(model.as_key()), model);
        }
    }
}
