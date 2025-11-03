use crate::stt::model::STTModel;
use crate::stt::onnx::ExecutionProvider;
use crate::stt::onnx::InferenceConfig;
use crate::stt::parakeet::{ParakeetConfig, ParakeetModel};
use anyhow::Result;
use clap::Parser;
use std::path::Path;
use std::path::PathBuf;

#[derive(Debug, Copy, Clone, clap::ValueEnum)]
pub enum Model {
    Parakeet,
    ParakeetInt8,
}

#[derive(Parser)]
#[command(version, about, long_about = None)]
pub struct Cli {
    /// Input .wav file to be transcribed
    #[arg(long, short)]
    pub input: Option<String>,

    /// Speech-to-text model
    #[arg(long, short, default_value = "parakeet-int8")]
    pub model: Option<Model>,

    /// Accelerator device
    #[arg(long, short, default_value = "cuda")]
    pub device: ExecutionProvider,

    /// Graph optimization level
    #[arg(long, short, default_value = "3")]
    pub graph_opt_lev: Option<usize>,

    /// Number of intra threads
    #[arg(long, short, default_value = "4")]
    pub n_intra_threads: Option<usize>,

    /// Enable parallel execution
    #[arg(long, short, default_value = "true")]
    pub par_x: Option<bool>,
}

fn transcribe(
    input_path: &Path,
    model: Model,
    device: ExecutionProvider,
    graph_optimization_level: usize,
    n_intra_threads: usize,
    parallel_execution: bool,
) -> Result<()> {
    // Configure model
    let model_config = ParakeetConfig {
        quantized: matches!(model, Model::ParakeetInt8),
        model_dir: PathBuf::from("models"),
    };

    // Use default inference config
    let inference_config = InferenceConfig {
        graph_optimization_level,
        n_intra_threads,
        parallel_execution,
        execution_providers: vec![device],
    };

    // Load model
    let mut model = ParakeetModel::new(model_config, inference_config)?;

    // Load audio
    let audio_samples = model.load_audio(input_path)?;

    // Transcribe
    let transcription = model.transcribe(audio_samples)?;

    // Print results
    println!("{}", transcription.text);

    Ok(())
}

pub fn run_cli() -> Result<()> {
    // Parse the arguments
    let args = Cli::parse();

    if let Some(input_path) = &args.input {
        let input = Path::new(input_path);

        // Sanity checks
        if !input.exists() {
            anyhow::bail!("Input file {} does not exist", input.display());
        }
        if !input.is_file() {
            anyhow::bail!("Input {} is not a file", input.display());
        }
        if input.extension().and_then(|ext| ext.to_str()) != Some("wav") {
            anyhow::bail!("Input {} is not a .wav file", input.display());
        }

        // Run transcription
        transcribe(
            input,
            args.model.unwrap(),
            args.device,
            args.graph_opt_lev.unwrap(),
            args.n_intra_threads.unwrap(),
            args.par_x.unwrap(),
        )?;
    } else {
        println!("I will use the TUI");
    }

    Ok(())
}
