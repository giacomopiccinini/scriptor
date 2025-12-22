//use crate::configs::inference::InferenceConfig;
use crate::configs::scriba::ScribaConfig;
//use crate::stt::fractor::Fractor;
use crate::stt::model::STTModel;
//use crate::stt::rec::Recorder;
//use crate::stt::vad::VoiceActivityDetector;
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
pub fn record_and_transcribe() -> Result<()> {
    todo!()
    // // Read config
    // let config = Config::read().with_context(|| "Failed to read config file")?;
    // let stt_config = &config.default.stt;

    // // Build inference config for VAD
    // let inference_config = build_inference_config(&config)?;

    // // Create recorder with max fragmentum duration from config
    // let recorder = Recorder::new(stt_config.fragmentum_length as f32)
    //     .with_context(|| "Failed to create recorder")?;

    // // Create VAD with threshold of 0.5
    // let vad = VoiceActivityDetector::new(inference_config, 0.5)
    //     .with_context(|| "Failed to create voice activity detector")?;

    // // Create fractor and run
    // let fractor = Fractor::new(recorder, vad);

    // println!("Recording started. Audio fragments will be saved to:");
    // println!(
    //     "  {}",
    //     dirs::data_dir()
    //         .expect("Could not find data directory")
    //         .join("scriba")
    //         .join("audio")
    //         .join(codex_name)
    //         .display()
    // );
    // println!("Press Ctrl+C to stop recording.");

    // fractor.run(codex_name)
}
