use crate::configs::scriba::ScribaConfig;
use crate::stt::fractor::Fractor;
use crate::stt::model::STTModel;
use crate::stt::rec::Recorder;
use crate::stt::vad::VADModel;
use anyhow::{Context, Result};
use std::path::PathBuf;

/// Transcribe a WAV file using settings from the config file
pub fn transcribe_from_file(file: &PathBuf) -> Result<()> {
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
    let config = ScribaConfig::read().with_context(|| "Failed to read config file")?;

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

/// Start recording and split audio into fragments using VAD
pub fn record_and_transcribe(output_dir: Option<PathBuf>) -> Result<()> {
    // Sanity check on output dir for audio files
    let output_dir = if let Some(dir) = output_dir {
        // TODO this fails but we need to put some guardrails
        // if !dir.is_dir() {
        //     anyhow::bail!("Output path {} is not a directory.", dir.display());
        // }
        if !dir.exists() {
            std::fs::create_dir_all(&dir)
                .with_context(|| format!("Failed to create output directory {}", dir.display()))?;
        }
        Some(dir)
    } else {
        None
    };

    // Read config
    let config = ScribaConfig::read().with_context(|| "Failed to read config file")?;

    // Load STT model
    let mut stt_model = STTModel::new(&config.default.stt, config.default.inference.clone())?;

    // Create recorder with max fragmentum duration from config
    let recorder = Recorder::new(config.default.fractor.max_fragmentum_duration_seconds)
        .with_context(|| "Failed to create recorder")?;

    // Create VAD model
    let vad_model = VADModel::new(&config.default.vad, config.default.inference.clone())
        .with_context(|| "Failed to create voice activity detector")?;

    // Create fractor and run
    let fractor = Fractor::new(recorder, vad_model);

    println!("Recording started.");
    println!("Press Ctrl+C to stop recording.");

    fractor.run(output_dir);

    Ok(())
}
