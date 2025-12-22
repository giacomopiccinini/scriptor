use crate::cli::commands::{record_and_transcribe, transcribe_from_file};
use crate::tui::entrypoint::run_tui;
use anyhow::Result;
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
        /// Directory where recordings are saved (optional)
        output_dir: Option<PathBuf>,
    },
}

pub fn run_cli() -> Result<()> {
    // Parse the arguments
    let args = Cli::parse();

    match args.command {
        Some(Commands::FromFile { file }) => transcribe_from_file(&file),
        Some(Commands::Record { output_dir }) => record_and_transcribe(output_dir),
        None => run_tui().map_err(|e| anyhow::anyhow!("{}", e)),
    }
}
