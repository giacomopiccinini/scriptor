use crate::cli::commands::{record_and_transcribe, transcribe_from_file};
use crate::tui::entrypoint::run_tui;
use anyhow::Result;
use clap::{Parser, Subcommand};
use std::path::PathBuf;

/// Local speech-to-text CLI & TUI
#[derive(Parser)]
#[clap(name = "scriptor", version)]
//#[command(version, about, long_about = None)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Commands>,
}

// /// Swiss-army knife for media inspection and manipulation
// #[derive(Debug, Parser)]
// #[clap(name = "rush", version)]
// pub struct App {
//     #[clap(subcommand)]
//     command: Command,
// }

#[derive(Subcommand)]
pub enum Commands {
    /// Transcribe an existing WAV
    FromFile {
        /// Path to the .wav file
        #[arg(required = true)]
        file: PathBuf,
    },
    /// Record & transcribe
    Record {
        /// File .txt where transcription is saved
        #[arg(short)]
        transcription_file: Option<PathBuf>,
        /// Directory where recordings are saved
        #[arg(short)]
        audio_dir: Option<PathBuf>,
    },
}

pub fn run_cli() -> Result<()> {
    // Parse the arguments
    let args = Cli::parse();

    match args.command {
        Some(Commands::FromFile { file }) => transcribe_from_file(&file),
        Some(Commands::Record {
            transcription_file,
            audio_dir,
        }) => record_and_transcribe(transcription_file, audio_dir),
        None => run_tui().map_err(|e| anyhow::anyhow!("{}", e)),
    }
}
