use anyhow::{Context, Result};
use cpal::SupportedStreamConfig;
use hound::{SampleFormat, WavReader, WavSpec, WavWriter};
use rubato::{FftFixedIn, Resampler};
use std::path::Path;

/// Read wav file
pub fn read_audio(audio_file_path: &Path) -> Result<(Vec<f32>, WavSpec)> {
    let reader = hound::WavReader::open(audio_file_path)?;
    let spec = reader.spec();

    // Calculate the maximum value based on bits_per_sample
    let max_value = 2_f64.powi(spec.bits_per_sample as i32 - 1);

    let samples: Vec<f32> = match spec.sample_format {
        hound::SampleFormat::Float => reader
            .into_samples::<f32>()
            .map(|s| s.with_context(|| "Couldn't read samples"))
            .collect::<Result<Vec<_>, _>>()?,
        hound::SampleFormat::Int => reader
            .into_samples::<i32>()
            .map(|s| {
                s.with_context(|| "Couldn't read samples")
                    .map(|sample| sample as f32 / max_value as f32)
            })
            .collect::<Result<Vec<_>, _>>()?,
    };

    Ok((samples, spec))
}

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

    // Read samples based on the actual format in the file
    match spec.sample_format {
        SampleFormat::Float => {
            // Read as f32 directly
            let raw_samples: Vec<f32> = reader
                .samples::<f32>()
                .map(|s| s.with_context(|| "Couldn't read samples"))
                .collect::<Result<Vec<_>, _>>()?;

            // Convert to mono if stereo
            raw_samples.chunks(channels).for_each(|chunk| {
                let avg: f32 = chunk.iter().sum::<f32>() / channels as f32;
                samples.push(avg);
            });
        }
        SampleFormat::Int => {
            // Read as i32 and normalize
            let raw_samples: Vec<i32> = reader
                .samples::<i32>()
                .map(|s| s.with_context(|| "Couldn't read samples"))
                .collect::<Result<Vec<_>, _>>()?;

            raw_samples.iter().enumerate().for_each(|(i, &sample)| {
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
        }
    }

    Ok((samples, sr))
}

/// Resample audio file to target sample rate
pub fn resample(samples: Vec<f32>, original_sr: u32, target_sr: u32) -> Result<Vec<f32>> {
    // If resampling is not needed, don't bother
    if original_sr == target_sr {
        return Ok(samples);
    }

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

/// Resample audio file to target sample rate (supports mono or stereo)
pub fn resample_stereo(
    samples: Vec<f32>,
    original_sr: u32,
    target_sr: u32,
    n_channels: u16,
) -> Result<Vec<f32>> {
    if original_sr == target_sr {
        return Ok(samples);
    }

    let n_channels = n_channels as usize;
    let n_frames = samples.len() / n_channels;

    // Deinterleave into separate channel vectors
    let channels: Vec<Vec<f32>> = (0..n_channels)
        .map(|ch| {
            samples
                .iter()
                .skip(ch)
                .step_by(n_channels)
                .copied()
                .collect()
        })
        .collect();

    let mut resampler = FftFixedIn::<f32>::new(
        original_sr as usize,
        target_sr as usize,
        n_frames,
        1024,
        n_channels,
    )
    .with_context(|| "Can't initiate resampler")?;

    let resampled = resampler
        .process(&channels, None)
        .with_context(|| "Can't resample file")?;

    // Re-interleave channels back into a single vector
    let out_frames = resampled[0].len();
    let mut output = Vec::with_capacity(out_frames * n_channels);
    for i in 0..out_frames {
        for ch in &resampled {
            output.push(ch[i]);
        }
    }

    Ok(output)
}

/// Convert stereo samples to mono by averaging left and right channels
pub fn convert_to_mono(samples: Vec<f32>, n_channels: u16) -> Vec<f32> {
    // If mono, exit
    if n_channels == 1 {
        samples
    }
    // If two channels, average them
    else {
        samples
            .chunks(2)
            .map(|pair| {
                if pair.len() == 2 {
                    (pair[0] + pair[1]) / 2.0
                } else {
                    // Should not happen because it's stereo, but we leave room for bug, partial buffer etc.
                    pair[0]
                }
            })
            .collect()
    }
}

/// Convert mono samples to stereo by duplicating each sample to left and right channels
pub fn convert_to_stereo(samples: Vec<f32>, n_channels: u16) -> Vec<f32> {
    // If already stereo, exit
    if n_channels == 2 {
        samples
    }
    // If mono, duplicate each sample for left and right
    else {
        samples
            .into_iter()
            .flat_map(|sample| [sample, sample])
            .collect()
    }
}

/// Write recording to file, with the exact settings as when it was recorded
pub fn write_wav(samples: Vec<f32>, wav_spec: hound::WavSpec, output_path: &Path) -> Result<()> {
    // Calculate the max value based on bits_per_sample for proper scaling
    let max_value = 2_f64.powi((wav_spec.bits_per_sample - 1) as i32);

    // Instantiate the write
    let mut writer = WavWriter::create(output_path, wav_spec)
        .with_context(|| format!("Couldn't write wav to {:?}", output_path))?;

    // Write to file (clamp samples to -1.0 to 1.0 to prevent clipping distortion)
    samples.iter().try_for_each(|&sample| {
        let sample = sample.clamp(-1.0, 1.0);
        match wav_spec.sample_format {
            SampleFormat::Float => {
                writer.write_sample(sample)?; // Write f32 directly
            }
            SampleFormat::Int => match wav_spec.bits_per_sample {
                8 => writer.write_sample((sample as f64 * max_value).round() as i8)?,
                16 => writer.write_sample((sample as f64 * max_value).round() as i16)?,
                24 | 32 => writer.write_sample((sample as f64 * max_value).round() as i32)?,
                _ => anyhow::bail!("Unsupported bits per sample: {}", wav_spec.bits_per_sample),
            },
        }
        Ok(())
    })?;

    writer.finalize()?;
    Ok(())
}

/// Write audio to file
pub fn write_mono_wav(
    samples: Vec<f32>,
    sr: u32,
    bits_per_sample: usize,
    output_path: &Path,
) -> Result<()> {
    // Create a new WAV specification for the audio
    let audio_spec = WavSpec {
        channels: 1_u16,
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
                )));
            }
        }
    }

    Ok(())
}

/// Convert cpal streaming config to hound compatible config
pub fn wav_spec_from_config(config: &SupportedStreamConfig) -> hound::WavSpec {
    // Convert sample format
    let sample_format = if config.sample_format().is_float() {
        hound::SampleFormat::Float
    } else {
        hound::SampleFormat::Int
    };

    // Create hound wave spec
    hound::WavSpec {
        channels: config.channels() as _,
        sample_rate: config.sample_rate().0 as _,
        bits_per_sample: (config.sample_format().sample_size() * 8) as _,
        sample_format,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_temp_wav_mono(samples: &[f32], sr: u32) -> (tempfile::TempDir, std::path::PathBuf) {
        let temp_dir = tempfile::tempdir().unwrap();
        let path = temp_dir.path().join("test.wav");
        let spec = WavSpec {
            channels: 1,
            sample_rate: sr,
            bits_per_sample: 16,
            sample_format: SampleFormat::Int,
        };
        let mut writer = WavWriter::create(&path, spec).unwrap();
        let max_val = 32767.0;
        for &s in samples {
            writer
                .write_sample((s.clamp(-1.0, 1.0) * max_val) as i16)
                .unwrap();
        }
        writer.finalize().unwrap();
        (temp_dir, path)
    }

    #[test]
    fn test_convert_to_mono_identity() {
        let samples = vec![0.5, 0.3, -0.2];
        let result = convert_to_mono(samples.clone(), 1);
        assert_eq!(result, samples);
    }

    #[test]
    fn test_convert_to_mono_stereo() {
        let samples = vec![1.0, 1.0, 0.0, 0.0, -0.5, 0.5];
        let result = convert_to_mono(samples, 2);
        assert_eq!(result, vec![1.0, 0.0, 0.0]);
    }

    #[test]
    fn test_convert_to_stereo_identity() {
        let samples = vec![0.5, 0.3, -0.2];
        let result = convert_to_stereo(samples.clone(), 2);
        assert_eq!(result, samples);
    }

    #[test]
    fn test_convert_to_stereo_from_mono() {
        let samples = vec![0.5, 0.3];
        let result = convert_to_stereo(samples, 1);
        assert_eq!(result, vec![0.5, 0.5, 0.3, 0.3]);
    }

    #[test]
    fn test_resample_identity() {
        let samples = vec![0.1, 0.2, 0.3];
        let result = resample(samples.clone(), 16000, 16000).unwrap();
        assert_eq!(result, samples);
    }

    #[test]
    fn test_resample_downsample() {
        let samples: Vec<f32> = (0..3200).map(|i| (i as f32 / 3200.0) * 0.5).collect();
        let result = resample(samples, 16000, 8000).unwrap();
        assert_eq!(result.len(), 1600);
    }

    #[test]
    fn test_read_audio_roundtrip() {
        let original = vec![0.5, -0.3, 0.0, 0.8];
        let (_temp, path) = create_temp_wav_mono(&original, 16000);
        let (read, spec) = read_audio(&path).unwrap();
        assert_eq!(spec.channels, 1);
        assert_eq!(spec.sample_rate, 16000);
        assert_eq!(read.len(), 4);
        for (a, b) in original.iter().zip(read.iter()) {
            assert!((a - b).abs() < 1e-4, "expected {} got {}", a, b);
        }
    }

    #[test]
    fn test_read_audio_file_mono() {
        let samples = vec![0.1, 0.2, 0.3];
        let (_temp, path) = create_temp_wav_mono(&samples, 44100);
        let (read, sr) = read_audio_file_mono(&path).unwrap();
        assert_eq!(sr, 44100);
        assert_eq!(read.len(), 3);
    }

    #[test]
    fn test_read_audio_file_mono_unsupported_channels() {
        let temp_dir = tempfile::tempdir().unwrap();
        let path = temp_dir.path().join("multichannel.wav");
        let spec = WavSpec {
            channels: 4,
            sample_rate: 16000,
            bits_per_sample: 16,
            sample_format: SampleFormat::Int,
        };
        let mut writer = WavWriter::create(&path, spec).unwrap();
        for _ in 0..64000 {
            writer.write_sample(0i16).unwrap();
        }
        writer.finalize().unwrap();
        assert!(read_audio_file_mono(&path).is_err());
    }

    #[test]
    fn test_write_wav_roundtrip() {
        let samples = vec![0.5, -0.5, 0.0];
        let spec = WavSpec {
            channels: 1,
            sample_rate: 16000,
            bits_per_sample: 16,
            sample_format: SampleFormat::Int,
        };
        let temp_dir = tempfile::tempdir().unwrap();
        let path = temp_dir.path().join("out.wav");
        write_wav(samples.clone(), spec, &path).unwrap();
        let (read, _) = read_audio(&path).unwrap();
        assert_eq!(read.len(), 3);
        for (a, b) in samples.iter().zip(read.iter()) {
            assert!((a - b).abs() < 1e-3);
        }
    }

    #[test]
    fn test_write_mono_wav_roundtrip() {
        let samples = vec![0.25, -0.25];
        let temp_dir = tempfile::tempdir().unwrap();
        let path = temp_dir.path().join("mono.wav");
        write_mono_wav(samples.clone(), 16000, 16, &path).unwrap();
        let (read, sr) = read_audio_file_mono(&path).unwrap();
        assert_eq!(sr, 16000);
        assert_eq!(read.len(), 2);
    }

    #[test]
    fn test_write_mono_wav_unsupported_bits() {
        let samples = vec![0.5];
        let temp_dir = tempfile::tempdir().unwrap();
        let path = temp_dir.path().join("bad.wav");
        assert!(write_mono_wav(samples, 16000, 4, &path).is_err());
    }
}
