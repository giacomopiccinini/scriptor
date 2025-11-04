use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

// Default variables
const DEFAULT_DB_NAME: &str = "archivum";
const DEFAULT_DB_FILE: &str = "archivum.db";

const DEFAULT_MODEL: &str = "parakeet-int8";
const DEFAULT_DEVICE: &str = "cuda";
const DEFAULT_GRAPH_OPTIMIZATION_LEVEL: usize = 3;
const DEFAULT_N_INTRA_THREADS: usize = 4;
const DEFAULT_PARALLEL_EXECUTION: bool = true;
const DEFAULT_FRAGMENTUM_LENGTH: usize = 7; // Each fragmentum lasts for 7 seconds

const DEFAULT_BACKGROUND_COLOR: &str = "#F3E5AB";
const DEFAULT_FOREGROUND_COLOR: &str = "#1A1A1A";
const DEFAULT_HIGHLIGHT_COLOR: &str = "#8B0000";

/// Config file definition
#[derive(Deserialize, Serialize)]
pub struct Config {
    pub default: DefaultConfig,
    pub dbs: Vec<DBConfig>,
}

/// Default configurations for db, theme and speech-to-text options
#[derive(Deserialize, Serialize, Clone)]
pub struct DefaultConfig {
    pub db: DBConfig,
    pub theme: ThemeConfig,
    pub stt: STTConfig,
}

/// Database configuration
#[derive(Deserialize, Serialize, Clone)]
pub struct DBConfig {
    pub name: String,
    pub connection_str: String,
}

/// Theme configuration
#[derive(Deserialize, Serialize, Clone)]
pub struct ThemeConfig {
    pub background: String,
    pub foreground: String,
    pub highlight: String,
}

/// Speech-to-text configuration
#[derive(Deserialize, Serialize, Clone)]
pub struct STTConfig {
    pub model: String,
    pub device: String,
    pub graph_optimization_level: usize,
    pub n_intra_threads: usize,
    pub parallel_execution: bool,
    pub fragmentum_length: usize,
}

impl Default for DBConfig {
    fn default() -> Self {
        // Use data directory to standardize storage
        let data_dir = dirs::data_dir().unwrap().join("scriba");

        // Create directory
        std::fs::create_dir_all(&data_dir).unwrap();

        // Create path to db
        let path = data_dir.join(DEFAULT_DB_FILE);

        // Create connection string (only SQLite is admissible)
        let connection_str = format!("sqlite:{}", path.display());

        Self {
            name: DEFAULT_DB_NAME.to_string(),
            connection_str,
        }
    }
}

impl Default for ThemeConfig {
    fn default() -> Self {
        Self {
            background: DEFAULT_BACKGROUND_COLOR.to_string(),
            foreground: DEFAULT_FOREGROUND_COLOR.to_string(),
            highlight: DEFAULT_HIGHLIGHT_COLOR.to_string(),
        }
    }
}

impl Default for STTConfig {
    fn default() -> Self {
        Self {
            model: DEFAULT_MODEL.to_string(),
            device: DEFAULT_DEVICE.to_string(),
            graph_optimization_level: DEFAULT_GRAPH_OPTIMIZATION_LEVEL,
            n_intra_threads: DEFAULT_N_INTRA_THREADS,
            parallel_execution: DEFAULT_PARALLEL_EXECUTION,
            fragmentum_length: DEFAULT_FRAGMENTUM_LENGTH,
        }
    }
}

impl Default for DefaultConfig {
    fn default() -> Self {
        Self {
            db: DBConfig::default(),
            theme: ThemeConfig::default(),
            stt: STTConfig::default(),
        }
    }
}

impl Default for Config {
    /// By default, the name is the default name with default config
    fn default() -> Self {
        Self {
            default: DefaultConfig::default(),
            dbs: vec![DBConfig::default()],
        }
    }
}

impl Config {
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
        let scriba_config: Config = toml::from_str(
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
