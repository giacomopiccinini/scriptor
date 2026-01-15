use anyhow::{Context, Result};
use futures_util::StreamExt;
use std::path::PathBuf;
use tokio::{fs::File, io::AsyncWriteExt};

/// Download file from AWS Cloudfront using streaming (useful for large files, e.g. models)
pub async fn download_file(file_url: &str, local_destination: PathBuf) -> Result<()> {
    // Interrogate the endpoint (Cloudfront)
    let response = reqwest::get(file_url)
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
