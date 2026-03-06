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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_file_if_not_exists_creates() {
        let temp_dir = tempfile::tempdir().unwrap();
        let path = temp_dir.path().join("new_file.txt");
        assert!(!path.exists());
        let created = create_file_if_not_exists(&path).unwrap();
        assert!(created);
        assert!(path.exists());
    }

    #[test]
    fn test_create_file_if_not_exists_already_exists() {
        let temp_dir = tempfile::tempdir().unwrap();
        let path = temp_dir.path().join("existing.txt");
        std::fs::write(&path, "content").unwrap();
        let created = create_file_if_not_exists(&path).unwrap();
        assert!(!created);
    }

    #[test]
    fn test_create_file_if_not_exists_creates_parent_dirs() {
        let temp_dir = tempfile::tempdir().unwrap();
        let path = temp_dir
            .path()
            .join("subdir")
            .join("nested")
            .join("file.txt");
        assert!(!path.exists());
        create_file_if_not_exists(&path).unwrap();
        assert!(path.exists());
    }

    #[test]
    fn test_append_text() {
        let temp_dir = tempfile::tempdir().unwrap();
        let path = temp_dir.path().join("append.txt");
        append_text(&path, "line1").unwrap();
        append_text(&path, "line2").unwrap();
        let content = std::fs::read_to_string(&path).unwrap();
        assert_eq!(content, "line1\nline2\n");
    }

    #[test]
    fn test_append_text_raw() {
        let temp_dir = tempfile::tempdir().unwrap();
        let path = temp_dir.path().join("append_raw.txt");
        append_text_raw(&path, "part1").unwrap();
        append_text_raw(&path, "part2").unwrap();
        let content = std::fs::read_to_string(&path).unwrap();
        assert_eq!(content, "part1part2");
    }
}
