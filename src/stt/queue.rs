use crate::stt::model::STTModel;
use crate::stt::text::append_text;
use crate::tui::db::models::{Fragmentum, NewFragmentum};
use anyhow::{Context, Result};
use chrono::{DateTime, Local};
use sqlx::SqlitePool;
use std::path::PathBuf;
use std::sync::mpsc::{Receiver, SyncSender};

#[derive(Debug, Clone)]
pub struct FragmentumToTranscribe {
    pub path: PathBuf,
    pub start_datetime: DateTime<Local>,
}

/// Create a new channel pair for fragmentum transcription queue
pub fn create_fragmentum_channel(
    max_queue_elements: usize,
) -> (
    SyncSender<FragmentumToTranscribe>,
    Receiver<FragmentumToTranscribe>,
) {
    std::sync::mpsc::sync_channel::<FragmentumToTranscribe>(max_queue_elements)
}

/// Transcribe audio fragments to a file. Returns the STTModel for reuse.
pub fn transcriber_to_file_worker(
    mut stt_model: STTModel,
    transcription_file: PathBuf,
    rx: Receiver<FragmentumToTranscribe>,
) -> Result<STTModel> {
    // Loop until channel is closed (sender dropped)
    while let Ok(item) = rx.recv() {
        // Load and transcribe
        let audio = stt_model
            .load_audio(&item.path)
            .with_context(|| "Failed to load fragmentum")?;
        let result = stt_model
            .transcribe(audio)
            .with_context(|| "Failed to transcribe fragmentum")?;

        // Append to file
        append_text(&transcription_file, &result.text)
            .with_context(|| "Failed to append transcription to file")?;
    }
    Ok(stt_model)
}

/// Transcribe audio fragments to stdout. Returns the STTModel for reuse.
pub fn transcriber_to_stdout_worker(
    mut stt_model: STTModel,
    rx: Receiver<FragmentumToTranscribe>,
) -> Result<STTModel> {
    println!();
    // Loop until channel is closed (sender dropped)
    while let Ok(item) = rx.recv() {
        // Load and transcribe
        let audio = stt_model
            .load_audio(&item.path)
            .with_context(|| "Failed to load fragmentum")?;
        let result = stt_model
            .transcribe(audio)
            .with_context(|| "Failed to transcribe fragmentum")?;

        // Print to stdout
        println!("{}", result.text);
    }
    Ok(stt_model)
}

/// Uses a tokio runtime to perform async DB operations from within a sync thread.
/// Returns the STTModel for reuse.
pub fn transcriber_to_db_worker(
    mut stt_model: STTModel,
    folio_id: i64,
    pool: SqlitePool,
    rx: Receiver<FragmentumToTranscribe>,
    runtime_handle: tokio::runtime::Handle,
) -> Result<STTModel> {
    // Use the passed handle instead of trying to get current (which fails in std::thread)
    let handle = runtime_handle;

    // Loop until channel is closed (sender dropped)
    while let Ok(item) = rx.recv() {
        // Load and transcribe
        let audio = stt_model
            .load_audio(&item.path)
            .with_context(|| "Failed to load fragmentum")?;
        let result = stt_model
            .transcribe(audio)
            .with_context(|| "Failed to transcribe fragmentum")?;

        // Create fragmentum in DB
        let new_fragmentum = NewFragmentum {
            folio_id,
            path: item.path.display().to_string(),
            content: result.text,
            timestamp_start: None,
            timestamp_end: None,
        };

        // Run async DB operation on the existing runtime
        handle.block_on(async {
            Fragmentum::create(&pool, new_fragmentum)
                .await
                .with_context(|| "Failed to save fragmentum to database")
        })?;
    }
    Ok(stt_model)
}
