use crate::configs::scriptor::ScriptorConfig;
use crate::stt::fractor::Fractor;
use crate::stt::model::STTModel;
use crate::stt::playback::Player;
use crate::stt::queue::FragmentumToTranscribe;
use crate::stt::queue::{transcriber_to_file_worker, transcriber_to_stdout_worker};
use crate::stt::rec::Recorder;
use crate::stt::text::create_file_if_not_exists;
use crate::stt::vad::VADModel;
use anyhow::{Context, Result};
use crossterm::style::Stylize;
use spinoff::{Color, Spinner, Streams, spinners};
use sqlx::any;
use std::io::{self, BufRead};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use walkdir::WalkDir;

/// Transcribe an existing WAV file
pub fn transcribe_from_file(file: &Path) -> Result<()> {
    // Validate file
    if !file.exists() {
        anyhow::bail!("Input file {} does not exist.", file.display());
    }
    if !file.is_file() {
        anyhow::bail!("Input {} is not a file.", file.display());
    }
    if file.extension().and_then(|ext| ext.to_str()) != Some("wav") {
        anyhow::bail!("Input {} is not a .wav file.", file.display());
    }

    // Read config
    let config = ScriptorConfig::read().with_context(|| "Failed to read config file")?;

    // Load STT model
    let mut stt_model = STTModel::new(&config.default.stt, config.default.inference)?;

    // Load audio
    let audio_samples = stt_model.load_audio(file)?;

    // Transcribe
    let transcription = stt_model.transcribe(audio_samples)?;

    // Print results
    println!("{}", transcription.text);

    Ok(())
}

/// Record and transcribe
pub fn record_and_transcribe(
    transcription_file: Option<PathBuf>,
    audio_dir: Option<PathBuf>,
) -> Result<()> {
    // Create transcription file if provided
    if let Some(ref path) = transcription_file {
        create_file_if_not_exists(path)
            .with_context(|| format!("Failed to create transcription file {}", path.display()))?;
    }

    // Sanity check on output dir for audio files
    let audio_dir = if let Some(dir) = audio_dir {
        if !dir.exists() {
            std::fs::create_dir_all(&dir)
                .with_context(|| format!("Failed to create output directory {}", dir.display()))?;
        }
        Some(dir)
    } else {
        None
    };

    // Create spinner
    let mut spinner = Spinner::new_with_stream(
        spinners::Dots,
        "Loading models...",
        Color::Blue,
        Streams::Stderr,
    );

    // Read config
    let config = ScriptorConfig::read().with_context(|| "Failed to read config file")?;

    // Load STT model
    let stt_model = STTModel::new(&config.default.stt, config.default.inference.clone())?;

    // Create recorder with max fragmentum duration from config
    let recorder = Recorder::new(config.default.fractor.max_fragmentum_duration_seconds)
        .with_context(|| "Failed to create recorder")?;

    // Create VAD model
    let vad_model = VADModel::new(&config.default.vad, config.default.inference.clone())
        .with_context(|| "Failed to create voice activity detector")?;

    // Create fractor
    let fractor = Fractor::new(recorder, vad_model);

    // Create stop signal
    let stop_signal = Arc::new(AtomicBool::new(false));
    let stop_signal_clone = Arc::clone(&stop_signal);

    // Create queue
    let (tx, rx) = std::sync::mpsc::sync_channel::<FragmentumToTranscribe>(
        config.default.queue.max_queue_elements,
    );

    // Signal successful model loading
    spinner.success("Models loaded!");
    eprintln!(
        "{} {}",
        "● [ON AIR]".red().bold(),
        "Press Enter to stop recording.".italic()
    );

    // Run fractor in a separate thread
    let fractor_handle = thread::spawn(move || fractor.run(audio_dir, stop_signal_clone, tx));
    let transcriber_handle = thread::spawn(move || {
        if let Some(transcription_file) = transcription_file {
            transcriber_to_file_worker(stt_model, transcription_file, rx)
        } else {
            transcriber_to_stdout_worker(stt_model, rx)
        }
    });

    // Wait for Enter key
    let stdin = io::stdin();
    let _ = stdin.lock().lines().next();

    // Signal stop and wait for fractor to finish
    stop_signal.store(true, Ordering::Relaxed);

    // Wait for fractor thread to complete and get temp dir to clean up (if any)
    let temp_dir_to_erase = match fractor_handle.join() {
        Ok(result) => result?,
        Err(_) => anyhow::bail!("Recording thread panicked"),
    };

    // Wait for transcriber to finish processing all items
    match transcriber_handle.join() {
        Ok(result) => result?,
        Err(_) => anyhow::bail!("Transcribing thread panicked"),
    }

    // Clean up temp directory after transcription is complete
    if let Some(temp_dir) = temp_dir_to_erase
        && temp_dir.exists()
    {
        std::fs::remove_dir_all(&temp_dir).with_context(|| {
            format!(
                "Unable to remove temp audio files at {}",
                temp_dir.display()
            )
        })?;
    }

    Ok(())
}

/// Play single wav file or directory containing wav files
pub fn play(input: PathBuf) -> Result<()> {
    if !input.exists() {
        anyhow::bail!("Input path does not exist")
    };
    if input.is_file() && !input.extension().map_or(false, |ext| ext == "wav") {
        anyhow::bail!("Input needs to be have .wav file")
    };

    let files: Vec<PathBuf> = if input.is_file() {
        vec![input]
    } else {
        WalkDir::new(input)
            .max_depth(0)
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(|e| e.path().extension().map_or(false, |ext| ext == "wav"))
            .map(|e| e.path().to_path_buf())
            .collect()
    };

    if files.len() == 0 {
        anyhow::bail!("No .wav files found")
    };

    let mut player = Player::new(Some(files)).with_context(|| "Failed to create player")?;
    player.play().with_context(|| "Failed to play recordings")?;

    Ok(())
}
