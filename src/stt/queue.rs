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

pub fn transcriber_to_file_worker(
    mut stt_model: STTModel,
    transcription_file: PathBuf,
    rx: Receiver<FragmentumToTranscribe>,
) -> Result<()> {
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
    Ok(())
}

pub fn transcriber_to_stdout_worker(
    mut stt_model: STTModel,
    rx: Receiver<FragmentumToTranscribe>,
) -> Result<()> {
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
    Ok(())
}

/// Transcription worker that writes results to the database
/// Uses a tokio runtime to perform async DB operations from within a sync thread
pub fn transcriber_to_db_worker(
    mut stt_model: STTModel,
    folio_id: i64,
    pool: SqlitePool,
    rx: Receiver<FragmentumToTranscribe>,
) -> Result<()> {
    // Create a tokio runtime for async DB operations
    let rt = tokio::runtime::Runtime::new()
        .with_context(|| "Failed to create tokio runtime for transcription worker")?;

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
            content: result.text,
        };

        // Run async DB operation
        rt.block_on(async {
            Fragmentum::create(&pool, new_fragmentum)
                .await
                .with_context(|| "Failed to save fragmentum to database")
        })?;
    }
    Ok(())
}
