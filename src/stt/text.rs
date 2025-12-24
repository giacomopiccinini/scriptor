use anyhow::{Context, Result};
use std::fs::{File, OpenOptions};
use std::io::Write;
use std::path::Path;

/// Create a text file at the given path if it doesn't exist
/// Returns Ok(true) if the file was created, Ok(false) if it already existed
pub fn create_file_if_not_exists(path: &Path) -> Result<bool> {
    if path.exists() {
        return Ok(false);
    }

    // Create parent directories if needed
    if let Some(parent) = path.parent()
        && !parent.exists()
    {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("Failed to create parent directory: {}", parent.display()))?;
    }

    File::create(path).with_context(|| format!("Failed to create file: {}", path.display()))?;

    Ok(true)
}

/// Append text to an existing file, adding a newline after the text
pub fn append_text(path: &Path, text: &str) -> Result<()> {
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .with_context(|| format!("Failed to open file for appending: {}", path.display()))?;

    writeln!(file, "{}", text)
        .with_context(|| format!("Failed to write to file: {}", path.display()))?;

    Ok(())
}

/// Append text to a file without adding a newline
pub fn append_text_raw(path: &Path, text: &str) -> Result<()> {
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .with_context(|| format!("Failed to open file for appending: {}", path.display()))?;

    write!(file, "{}", text)
        .with_context(|| format!("Failed to write to file: {}", path.display()))?;

    Ok(())
}
