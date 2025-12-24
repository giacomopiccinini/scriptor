use crate::configs::db::DBConfig;
use crate::configs::fractor::FractorConfig;
use crate::configs::inference::InferenceConfig;
use crate::configs::queue::QueueConfig;
use crate::configs::stt::STTConfig;
use crate::configs::theme::ThemeConfig;
use crate::configs::vad::VADConfig;
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

/// Config file definition
#[derive(Deserialize, Serialize)]
pub struct ScriptorConfig {
    pub default: DefaultConfig,
    pub dbs: Vec<DBConfig>,
    pub stts: Vec<STTConfig>,
    pub vads: Vec<VADConfig>,
}

/// Default configurations for db, theme and speech-to-text options
#[derive(Deserialize, Serialize, Clone, Default)]
pub struct DefaultConfig {
    pub db: DBConfig,
    pub inference: InferenceConfig,
    pub vad: VADConfig,
    pub fractor: FractorConfig,
    pub queue: QueueConfig,
    pub stt: STTConfig,
    pub theme: ThemeConfig,
}

impl Default for ScriptorConfig {
    /// By default, the name is the default name with default config
    fn default() -> Self {
        Self {
            default: DefaultConfig::default(),
            dbs: vec![DBConfig::default()],
            stts: vec![STTConfig::default()],
            vads: vec![VADConfig::default()],
        }
    }
}

impl ScriptorConfig {
    /// Write config struct to scriptor.toml file
    pub fn write(&self, config_path: &PathBuf) -> Result<()> {
        // Convert config to string to be written to config file
        let toml_content =
            toml::to_string_pretty(&self).with_context(|| "Failed to serialize scriptor.toml")?;

        // Write string to file
        fs::write(config_path, toml_content).with_context(|| {
            format!(
                "Failed to write yomo.toml file to {}",
                config_path.display()
            )
        })?;

        Ok(())
    }

    /// Read and serialize a scriptor.toml file
    pub fn read() -> Result<Self> {
        // Use config directory to standardize storage of config file
        let config_dir = dirs::config_dir().unwrap().join("scriptor");

        // Define the config file path
        let config_path = config_dir.join("scriptor.toml");

        // Create config if not existing
        if !config_dir.exists() | !config_path.exists() {
            // Create directory
            std::fs::create_dir_all(&config_dir)
                .with_context(|| "Failed to create config directory")?;

            // Create default config
            let config = Self::default();

            // Create config file
            config
                .write(&config_path)
                .with_context(|| "Failed to create config file")?;

            // Create default config
            return Ok(Self::default());
        }

        // Serialize scriptor.toml into YomoProject struct
        let scriptor_config: ScriptorConfig = toml::from_str(
            &fs::read_to_string(config_path).with_context(|| "Failed to read into string")?,
        )
        .with_context(|| "Failed to serialize into struct")?;

        Ok(scriptor_config)
    }

    /// Get scriptor default config
    pub fn get_default(&self) -> Result<DefaultConfig> {
        Ok(self.default.clone())
    }
}
