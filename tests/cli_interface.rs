//! Integration tests for CLI interface (Clap parsing).

use clap::Parser;
use scriptor::cli::interface::{Cli, Commands};
use std::path::PathBuf;

fn parse_args(args: &[&str]) -> Cli {
    let all_args: Vec<&str> = std::iter::once("scriptor")
        .chain(args.iter().copied())
        .collect();
    Cli::parse_from(all_args)
}

#[test]
fn test_cli_parse_from_file() {
    let cli = parse_args(&["from-file", "/path/to/audio.wav"]);
    match &cli.command {
        Some(Commands::FromFile { file }) => {
            assert_eq!(file, &PathBuf::from("/path/to/audio.wav"));
        }
        _ => panic!("Expected FromFile command"),
    }
}

#[test]
fn test_cli_parse_record() {
    let cli = parse_args(&["record", "-t", "out.txt", "-a", "/audio/dir"]);
    match &cli.command {
        Some(Commands::Record {
            transcription_file,
            audio_dir,
        }) => {
            assert_eq!(transcription_file.as_ref(), Some(&PathBuf::from("out.txt")));
            assert_eq!(audio_dir.as_ref(), Some(&PathBuf::from("/audio/dir")));
        }
        _ => panic!("Expected Record command"),
    }
}

#[test]
fn test_cli_parse_play() {
    let cli = parse_args(&["play", "/path/to/wav.wav"]);
    match &cli.command {
        Some(Commands::Play { input }) => {
            assert_eq!(input, &PathBuf::from("/path/to/wav.wav"));
        }
        _ => panic!("Expected Play command"),
    }
}

#[test]
fn test_cli_parse_no_command_tui() {
    let cli = parse_args(&[]);
    assert!(cli.command.is_none());
}
