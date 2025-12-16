use crate::stt::fractor::Fractor;
use crate::stt::model::STTModel;
use crate::stt::onnx::{ExecutionProvider, InferenceConfig};
use crate::stt::parakeet::{ParakeetConfig, ParakeetModel};
use crate::stt::rec::Recorder;
use crate::stt::vad::VoiceActivityDetector;
use crate::tui::db::config::Config;
use crate::tui::entrypoint::run_tui;
use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser)]
#[command(version, about, long_about = None)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Commands>,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Transcribe a WAV file using config file settings
    FromFile {
        /// Path to the .wav file
        file: PathBuf,
    },
    /// Start recording and split into fragments
    Record {
        /// Name of the codex (output directory for fragments)
        codex_name: String,
    },
}

/// Parse device string from config to ExecutionProvider
fn parse_device(device: &str) -> Result<ExecutionProvider> {
    match device.to_lowercase().as_str() {
        "cuda" => Ok(ExecutionProvider::Cuda),
        "tensorrt" => Ok(ExecutionProvider::Tensorrt),
        "coreml" => Ok(ExecutionProvider::Coreml),
        "cpu" => Ok(ExecutionProvider::Cpu),
        _ => anyhow::bail!("Unknown device: {}. Use cuda, tensorrt, coreml, or cpu.", device),
    }
}

/// Build InferenceConfig from the STTConfig in the config file
fn build_inference_config(config: &Config) -> Result<InferenceConfig> {
    let stt_config = &config.default.stt;
    Ok(InferenceConfig {
        graph_optimization_level: stt_config.graph_optimization_level,
        n_intra_threads: stt_config.n_intra_threads,
        parallel_execution: stt_config.parallel_execution,
        execution_providers: vec![parse_device(&stt_config.device)?],
    })
}

/// Transcribe a WAV file using settings from the config file
fn transcribe_from_file(file: &PathBuf) -> Result<()> {
    // Validate file
    if !file.exists() {
        anyhow::bail!("Input file {} does not exist", file.display());
    }
    if !file.is_file() {
        anyhow::bail!("Input {} is not a file", file.display());
    }
    if file.extension().and_then(|ext| ext.to_str()) != Some("wav") {
        anyhow::bail!("Input {} is not a .wav file", file.display());
    }

    // Read config
    let config = Config::read().with_context(|| "Failed to read config file")?;
    let stt_config = &config.default.stt;

    // Build inference config
    let inference_config = build_inference_config(&config)?;

    // Configure model based on config
    let quantized = stt_config.model.to_lowercase().contains("int8");
    let model_config = ParakeetConfig {
        quantized,
        model_dir: dirs::data_dir()
            .expect("Could not find data directory")
            .join("scriba")
            .join("models"),
    };

    // Load model
    let mut model = ParakeetModel::new(model_config, inference_config)?;

    // Load audio
    let audio_samples = model.load_audio(file)?;

    // Transcribe
    let transcription = model.transcribe(audio_samples)?;

    // Print results
    println!("{}", transcription.text);

    Ok(())
}

/// Start recording and split audio into fragments using VAD
fn run_record(codex_name: &str) -> Result<()> {
    // Read config
    let config = Config::read().with_context(|| "Failed to read config file")?;
    let stt_config = &config.default.stt;

    // Build inference config for VAD
    let inference_config = build_inference_config(&config)?;

    // Create recorder with max fragmentum duration from config
    let recorder = Recorder::new(stt_config.fragmentum_length as f32)
        .with_context(|| "Failed to create recorder")?;

    // Create VAD with threshold of 0.5
    let vad = VoiceActivityDetector::new(inference_config, 0.5)
        .with_context(|| "Failed to create voice activity detector")?;

    // Create fractor and run
    let fractor = Fractor::new(recorder, vad);

    println!("Recording started. Audio fragments will be saved to:");
    println!(
        "  {}",
        dirs::data_dir()
            .expect("Could not find data directory")
            .join("scriba")
            .join("audio")
            .join(codex_name)
            .display()
    );
    println!("Press Ctrl+C to stop recording.");

    fractor.run(codex_name)
}

pub fn run_cli() -> Result<()> {
    // Parse the arguments
    let args = Cli::parse();

    match args.command {
        Some(Commands::FromFile { file }) => transcribe_from_file(&file),
        Some(Commands::Record { codex_name }) => run_record(&codex_name),
        None => run_tui().map_err(|e| anyhow::anyhow!("{}", e)),
    }
}
