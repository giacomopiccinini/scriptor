use crate::stt::model::STTModel;
use crate::stt::text::append_text;
use anyhow::{Context, Result};
use chrono::{DateTime, Local};
use std::path::PathBuf;
use std::sync::mpsc::{Receiver, SyncSender};

#[derive(Debug, Clone)]
pub struct FragmentumToTranscribe {
    pub path: PathBuf,
    pub start_datetime: DateTime<Local>,
}

pub struct FragmentumQueue {
    tx: SyncSender<FragmentumToTranscribe>,
    rx: Receiver<FragmentumToTranscribe>,
}

impl FragmentumQueue {
    pub fn new(max_queue_elements: usize) -> Self {
        // Create queue
        let (tx, rx) = std::sync::mpsc::sync_channel::<FragmentumToTranscribe>(max_queue_elements);

        Self { tx, rx }
    }
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
