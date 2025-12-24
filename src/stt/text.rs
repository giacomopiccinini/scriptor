use anyhow::{Context, Result};
use std::fs::{File, OpenOptions};
use std::io::{Write, stdout};
use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::Duration;

/// Spinner handle that can be used to stop the spinner
pub struct Spinner {
    running: Arc<AtomicBool>,
    handle: Option<thread::JoinHandle<()>>,
}

impl Spinner {
    /// Create and start a new spinner with a custom message
    pub fn start(message: &str) -> Self {
        let running = Arc::new(AtomicBool::new(true));
        let running_clone = Arc::clone(&running);
        let message = message.to_string();

        let handle = thread::spawn(move || {
            let frames = ['|', '/', '-', '\\'];
            let mut idx = 0;

            while running_clone.load(Ordering::Relaxed) {
                print!("\r{} {} ", frames[idx], message);
                stdout().flush().unwrap();
                idx = (idx + 1) % frames.len();
                thread::sleep(Duration::from_millis(100));
            }

            // Clear the spinner line
            print!("\r{}\r", " ".repeat(message.len() + 4));
            stdout().flush().unwrap();
        });

        Self {
            running,
            handle: Some(handle),
        }
    }

    /// Stop the spinner
    pub fn stop(mut self) {
        self.running.store(false, Ordering::Relaxed);
        if let Some(handle) = self.handle.take() {
            handle.join().unwrap();
        }
    }
}

impl Drop for Spinner {
    fn drop(&mut self) {
        self.running.store(false, Ordering::Relaxed);
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
    }
}

/// Create a text file at the given path if it doesn't exist
/// Returns Ok(true) if the file was created, Ok(false) if it already existed
pub fn create_file_if_not_exists(path: &Path) -> Result<bool> {
    if path.exists() {
        return Ok(false);
    }

    // Create parent directories if needed
    if let Some(parent) = path.parent() {
        if !parent.exists() {
            std::fs::create_dir_all(parent).with_context(|| {
                format!("Failed to create parent directory: {}", parent.display())
            })?;
        }
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
