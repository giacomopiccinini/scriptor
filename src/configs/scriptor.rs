use crate::configs::db::DBConfig;
use crate::configs::fractor::FractorConfig;
use crate::configs::inference::InferenceConfig;
use crate::configs::queue::QueueConfig;
use crate::configs::stt::STTConfig;
use crate::configs::theme::ThemeConfig;
use crate::configs::vad::VADConfig;
use crate::utils::aws::ModelsConfig;
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

/// Config file definition
#[derive(Deserialize, Serialize)]
pub struct ScriptorConfig {
    pub default: DefaultConfig,
    pub dbs: Vec<DBConfig>,
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
    /// Input device name for recording. None means use system default.
    pub input_device: Option<String>,
}

impl Default for ScriptorConfig {
    /// By default, the name is the default name with default config
    fn default() -> Self {
        Self {
            default: DefaultConfig::default(),
            dbs: vec![DBConfig::default()],
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

    /// Check for missing model files based on the selected STT and VAD models
    pub fn check_missing(&self, available_models: &ModelsConfig) -> Option<Vec<PathBuf>> {
        let scriptor_dir = dirs::data_dir().unwrap().join("scriptor");
        let mut missing = Vec::new();

        // Check STT model files
        let stt_key = self.default.stt.model.as_key();
        if let Some(files) = available_models.get_stt_files(stt_key) {
            for file in files {
                let relative_path = PathBuf::from("models").join("stt").join(stt_key).join(file);
                let path = scriptor_dir.join(&relative_path);
                if !path.exists() {
                    missing.push(relative_path);
                }
            }
        }

        // Check VAD model files
        let vad_key = self.default.vad.model.as_key();
        if let Some(files) = available_models.get_vad_files(vad_key) {
            for file in files {
                let relative_path = PathBuf::from("models").join("vad").join(vad_key).join(file);
                let path = scriptor_dir.join(&relative_path);
                if !path.exists() {
                    missing.push(relative_path);
                }
            }
        }

        if missing.is_empty() {
            None
        } else {
            Some(missing)
        }
    }
}
