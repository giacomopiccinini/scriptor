use anyhow::{Context, Result};
use rubato::{FftFixedIn, Resampler};
use std::path::Path;
use hound::{SampleFormat, WavSpec, WavReader, WavWriter};

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

/// Write audio to file
pub fn write_mono_wav(samples: Vec<f32>, sr: u32, bits_per_sample: usize, output_path: &Path) -> Result<()> {

    // Create a new WAV specification for the audio
    let audio_spec = WavSpec {
        channels: 1 as u16,
        sample_rate: sr,
        bits_per_sample: bits_per_sample as u16,
        sample_format: SampleFormat::Int,
    };

    // Init writer
    let mut writer = WavWriter::create(output_path, audio_spec)
        .with_context(|| format!("Couldn't write to {:?}", output_path))?;

    // Calculate the max value based on bits_per_sample for proper scaling
    let max_value = 2_f64.powi((audio_spec.bits_per_sample - 1) as i32);

    // Write samples interleaved
    for i in 0..samples.len() {
        // Scale back to the appropriate integer range
        let scaled_sample = (samples[i] as f64 * max_value).round();

        // Write sample based on bits_per_sample
        match audio_spec.bits_per_sample {
            8 => writer.write_sample(scaled_sample as i8)?,
            16 => writer.write_sample(scaled_sample as i16)?,
            24 | 32 => writer.write_sample(scaled_sample as i32)?,
            _ => {
                return Err(anyhow::Error::msg(format!(
                    "Unsupported bits per sample: {}",
                    audio_spec.bits_per_sample
                )))
            }
        }
    }

    Ok(())
}