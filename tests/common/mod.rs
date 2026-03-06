//! Shared test utilities for integration tests.
//! Each integration test crate uses a subset of these helpers.

#![allow(dead_code)]

use anyhow::Result;
use hound::{SampleFormat, WavSpec, WavWriter};

/// Create a minimal WAV file for testing (1 second of silence at 16kHz mono, 16-bit).
/// Returns the path to the created file. The file is created in a temp directory
/// that is cleaned up when the returned TempDir is dropped.
pub fn create_test_wav() -> Result<(tempfile::TempDir, std::path::PathBuf)> {
    let temp_dir = tempfile::tempdir()?;
    let wav_path = temp_dir.path().join("test.wav");

    let spec = WavSpec {
        channels: 1,
        sample_rate: 16000,
        bits_per_sample: 16,
        sample_format: SampleFormat::Int,
    };

    let mut writer = WavWriter::create(&wav_path, spec)?;
    let num_samples = 16000; // 1 second at 16kHz
    for _ in 0..num_samples {
        writer.write_sample(0i16)?;
    }
    writer.finalize()?;

    Ok((temp_dir, wav_path))
}

/// Create a stereo WAV file for testing (0.1 second, 16kHz, 16-bit).
pub fn create_test_stereo_wav() -> Result<(tempfile::TempDir, std::path::PathBuf)> {
    let temp_dir = tempfile::tempdir()?;
    let wav_path = temp_dir.path().join("test_stereo.wav");

    let spec = WavSpec {
        channels: 2,
        sample_rate: 16000,
        bits_per_sample: 16,
        sample_format: SampleFormat::Int,
    };

    let mut writer = WavWriter::create(&wav_path, spec)?;
    let num_frames = 1600; // 0.1 second at 16kHz
    for _ in 0..num_frames {
        writer.write_sample(0i16)?;
        writer.write_sample(0i16)?;
    }
    writer.finalize()?;

    Ok((temp_dir, wav_path))
}

/// Create a temp directory for config files. Returns the path to the config dir.
pub fn temp_config_dir() -> Result<(tempfile::TempDir, std::path::PathBuf)> {
    let temp_dir = tempfile::tempdir()?;
    let config_dir = temp_dir.path().join("scriptor");
    std::fs::create_dir_all(&config_dir)?;
    Ok((temp_dir, config_dir))
}
