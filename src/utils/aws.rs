use anyhow::{Context, Result};
use futures_util::StreamExt;
use futures_util::future::join_all;
use serde::Deserialize;
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use tokio::{fs::File, io::AsyncWriteExt};

#[derive(Deserialize)]
pub struct ModelsConfig {
    stt: HashMap<String, ModelFiles>,
    vad: HashMap<String, ModelFiles>,
}

#[derive(Deserialize)]
pub struct ModelFiles {
    files: Vec<String>,
}

/// Download file from AWS Cloudfront using streaming (useful for large files, e.g. models)
pub async fn download_file(file_url: &str, local_destination: PathBuf) -> Result<()> {
    // Set up client or else it will refuse connection
    let client = reqwest::Client::builder()
        .user_agent("scriptor")
        .build()
        .with_context(|| "Unable to set up client for download")?;

    // Interrogate the endpoint (Cloudfront)
    let response = client
        .get(file_url)
        .send()
        .await
        .with_context(|| "Error interrogating the URL")?
        .error_for_status()
        .with_context(|| "Error downloading the file")?;

    // Instantiate stream of bytes
    let mut stream = response.bytes_stream();

    // Create local file where download will be streamed to
    let mut file = File::create(local_destination).await?;

    // Stream and save chunks to file
    while let Some(chunk) = stream.next().await {
        file.write_all(&chunk?)
            .await
            .with_context(|| "Unable to download chunk")?;
    }

    Ok(())
}

/// Specific instance of download for latest models available
pub async fn download_models_list() -> Result<()> {
    // Use config directory to standardize storage of config file
    let config_dir = dirs::config_dir().unwrap().join("scriptor");

    // Define the config file path
    let config_path = config_dir.join("models.toml");

    // Compose path to models.toml
    let url = format!("https://www.scriptor.giacomopiccinini.xyz/models/models.toml");

    // Download file
    download_file(&url, config_path)
        .await
        .with_context(|| "Unable to download models.toml")?;

    Ok(())
}

/// Download all the missing files in parallel
pub async fn download_missing_files(missing_files: &Vec<PathBuf>) {
    // Set up source and directory
    let base_url = PathBuf::from("https://www.scriptor.giacomopiccinini.xyz");
    let scriptor_dir = dirs::data_dir().unwrap().join("scriptor");

    let download_tasks: Vec<_> = missing_files
        .into_iter()
        .map(|missing_file| {
            // Construct the specific url and local path
            let url = base_url.join(missing_file);
            let local_path = scriptor_dir.join(missing_file);

            async move {
                // Create parent directories
                if let Some(parent) = local_path.parent() {
                    tokio::fs::create_dir_all(parent).await?;
                }
                download_file(&url.to_str().unwrap(), local_path).await
            }
        })
        .collect();

    // Run all downloads concurrently
    let results = join_all(download_tasks).await;

    // Check for errors
    for result in results {
        result.expect("Failed to download model file");
    }
}

impl ModelsConfig {
    /// Read and serialize a models.toml file
    pub fn read() -> Result<Self> {
        // Use config directory to standardize storage of config file
        let config_dir = dirs::config_dir().unwrap().join("scriptor");

        // Define the config file path
        let config_path = config_dir.join("models.toml");

        // Serialize scriptor.toml into YomoProject struct
        let models_config: ModelsConfig = toml::from_str(
            &fs::read_to_string(config_path).with_context(|| "Failed to read into string")?,
        )
        .with_context(|| "Failed to serialize into struct")?;

        Ok(models_config)
    }

    /// Get the list of files for a given STT model
    pub fn get_stt_files(&self, model: &str) -> Option<&Vec<String>> {
        self.stt.get(model).map(|m| &m.files)
    }

    /// Get the list of files for a given VAD model
    pub fn get_vad_files(&self, model: &str) -> Option<&Vec<String>> {
        self.vad.get(model).map(|m| &m.files)
    }
}
