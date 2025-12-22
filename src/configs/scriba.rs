use crate::configs::db::DBConfig;
use crate::configs::fractor::FractorConfig;
use crate::configs::inference::InferenceConfig;
use crate::configs::stt::STTConfig;
use crate::configs::theme::ThemeConfig;
use crate::configs::vad::VADConfig;
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

/// Config file definition
#[derive(Deserialize, Serialize)]
pub struct ScribaConfig {
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
    pub stt: STTConfig,
    pub theme: ThemeConfig,
}

impl Default for ScribaConfig {
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

impl ScribaConfig {
    /// Write config struct to scriba.toml file
    pub fn write(&self, config_path: &PathBuf) -> Result<()> {
        // Convert config to string to be written to config file
        let toml_content =
            toml::to_string_pretty(&self).with_context(|| "Failed to serialize scriba.toml")?;

        // Write string to file
        fs::write(config_path, toml_content).with_context(|| {
            format!(
                "Failed to write yomo.toml file to {}",
                config_path.display()
            )
        })?;

        Ok(())
    }

    /// Read and serialize a scriba.toml file
    pub fn read() -> Result<Self> {
        // Use config directory to standardize storage of config file
        let config_dir = dirs::config_dir().unwrap().join("scriba");

        // Define the config file path
        let config_path = config_dir.join("scriba.toml");

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

        // Serialize scriba.toml into YomoProject struct
        let scriba_config: ScribaConfig = toml::from_str(
            &fs::read_to_string(config_path).with_context(|| "Failed to read into string")?,
        )
        .with_context(|| "Failed to serialize into struct")?;

        Ok(scriba_config)
    }

    /// Get scriba default config
    pub fn get_default(&self) -> Result<DefaultConfig> {
        Ok(self.default.clone())
    }
}
