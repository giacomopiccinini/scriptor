use anyhow::{Context, Result};
use hound::WavReader;
use rubato::{FftFixedIn, Resampler};
use std::path::Path;

/// Read audio file from file path and convert to mono by averaging left and right channel
pub fn read_audio_file_mono(audio_file_path: &Path) -> Result<(Vec<f32>, u32)> {
    // Open the WAV file
    let mut reader =
        WavReader::open(audio_file_path).with_context(|| "Failed to open audio file")?;

    // Extract info from file
    let spec = reader.spec();
    let sr = spec.sample_rate;
    let channels = spec.channels as usize;
    let bits_per_sample = spec.bits_per_sample;

    // Exit if if more than 2 channels
    if channels > 2 {
        return Err(anyhow::anyhow!(
            "Unsupported number of channels: {}. Only mono and stereo are supported.",
            channels
        ));
    }

    // Init samples vec
    let mut samples: Vec<f32> = Vec::new();

    // Calculate the maximum value based on bits_per_sample
    let max_value = 2_f64.powi(bits_per_sample as i32 - 1);

    // Define accumulator to compute average in case of stereo (using i64 to prevent overflow)
    let mut acc = 0_i64;

    // Read into samples vec
    reader
        .samples::<i32>()
        .map(|s| s.with_context(|| "Couldn't read samples"))
        .collect::<Result<Vec<_>, _>>()?
        .iter()
        .enumerate()
        .for_each(|(i, &sample)| {
            if channels == 2 {
                acc += sample as i64;
                if i % 2 != 0 {
                    // Average and normalize by dividing by max_value
                    samples.push(acc as f32 / 2.0 / max_value as f32);
                    acc = 0_i64;
                }
            } else if channels == 1 {
                // Normalize by dividing by max_value
                samples.push(sample as f32 / max_value as f32);
            }
        });

    Ok((samples, sr))
}

/// Resample audio file to target sample rate
pub fn resample(samples: Vec<f32>, original_sr: u32, target_sr: u32) -> Result<Vec<f32>> {
    // Initialize the resampler
    let mut resampler = FftFixedIn::<f32>::new(
        original_sr as usize,
        target_sr as usize,
        samples.len(), // Number of frames per channel (1 channel)
        1024,
        1, // Always mono by construction
    )
    .with_context(|| "Can't initiate resampler")?;

    // Perform the resampling
    let mut resampled = resampler
        .process(&[samples], None)
        .with_context(|| "Can't resample file")?;

    // Take ownership of the first channel, avoiding cloning
    Ok(resampled.swap_remove(0))
}
