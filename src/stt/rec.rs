use anyhow::{Context, Result};
use chrono::Local;
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{Device, SupportedStreamConfig};
use std::path::PathBuf;
use std::thread;

use crate::stt::audio::{convert_to_mono, resample, write_wav};
use hound::WavSpec;
use ringbuf::{
    HeapCons, HeapProd, HeapRb,
    traits::{Consumer, Observer, Producer, Split},
};
use rubato::{FftFixedIn, Resampler};

// Define the target duration for a fragmentum
const MAX_FRAGMENTUM_LENGTH_SECONDS: u32 = 10;
const TARGET_SAMPLE_RATE: u32 = 16_000;

// Setup the recording by finding the default device with the corresponding config
fn setup_recording() -> Result<(Device, SupportedStreamConfig)> {
    // Get default audio host and input device
    let host = cpal::default_host();
    let input_device = host
        .default_input_device()
        .with_context(|| "Unable to find default input device")?;

    // Use default configuration for audio input. If this doesn't match the requirements from
    // STT model, we fix it down the line, not here.
    let input_config = input_device
        .default_input_config()
        .with_context(|| "Unable to find default input config")?;

    Ok((input_device, input_config))
}

/// Estimate the capacity of the buffer by assuming it can store
/// *TWICE* as much with respect to the length in seconds of the fragmentum
/// to give some headromm
fn estimate_buffer_capacity(sample_rate: u32, n_channels: u16) -> usize {
    2 * (sample_rate * n_channels as u32 * MAX_FRAGMENTUM_LENGTH_SECONDS) as usize
}

/// Create a lock-free ring buffer with desired capacity
fn create_ring_buffer(buffer_capacity: usize) -> (HeapProd<f32>, HeapCons<f32>) {
    // HeapRb is allocated on the heap with the specified capacity
    let ring_buffer = HeapRb::<f32>::new(buffer_capacity);

    // Split into producer (for audio callback) and consumer (for main loop)
    ring_buffer.split()
}

/// Set up the audio stream to write samples into a ring buffer, collecting them from a device
fn setup_audio_stream(
    input_device: Device,
    input_config: SupportedStreamConfig,
    buffer_capacity: usize,
) -> Result<(cpal::Stream, HeapCons<f32>)> {
    // Create ring buffer with specified capacity.
    // The producer will be used in the stream.
    // The consumer is returned as it will be used for downstream processing
    let (mut producer, consumer) = create_ring_buffer(buffer_capacity);

    // Build input stream that pushes samples into the ring buffer
    let stream = input_device.build_input_stream(
        &input_config.into(),
        move |data: &[f32], _: &_| {
            // push_slice writes as many samples as will fit
            // If buffer is full, excess samples are dropped (lossy but bounded)
            let written = producer.push_slice(data);
            if written < data.len() {
                // This means the consumer isn't keeping up - buffer overflow
                eprintln!(
                    "Warning: Ring buffer overflow! Dropped {} samples",
                    data.len() - written
                );
            }
        },
        |err| eprintln!("Stream error: {err}"), // Closure for error function
        None,                                   // Timeout
    )?;

    Ok((stream, consumer))
}

/// Convert cpal streaming config to hound compatible config to write wav file as the original
fn wav_spec_from_config(config: &SupportedStreamConfig) -> hound::WavSpec {
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
        sample_format: sample_format,
    }
}

/// Pre-processing pipeline for STT
fn preprocess_audio_for_stt(samples: Vec<f32>, wav_config: WavSpec) -> Result<Vec<f32>> {
    resample(
        convert_to_mono(samples, wav_config.channels),
        wav_config.sample_rate,
        TARGET_SAMPLE_RATE,
    )
}

fn process_fragmentum(samples: Vec<f32>, wav_config: WavSpec) {
    thread::spawn(move || {
        let stt_samples = preprocess_audio_for_stt(samples.clone(), wav_config.clone())
            .expect("Unable to pre-process audio for STT");
        let filename = PathBuf::from(format!(
            "rec_{}.wav",
            Local::now().format("%Y-%m-%d_%H:%M:%S")
        ));
        write_wav(samples, wav_config.clone(), &filename)
            .expect("Unable to write fragmentum to file");
    });
}

fn main() -> Result<()> {
    // Set up recording device
    let (input_device, input_config) =
        setup_recording().with_context(|| "Unable to set up recording")?;

    // Convert the input config (cpal-style) into wav config (hound-style)
    let wav_config = wav_spec_from_config(&input_config);

    // Extract parameters for buffer sizing and resampling strategy
    let sample_rate = input_config.sample_rate().0;
    let channels = input_config.channels();

    // Compute buffer capacity
    let buffer_capacity = estimate_buffer_capacity(sample_rate, channels);

    // Create the audio stream
    let (stream, mut consumer) = setup_audio_stream(input_device, input_config, buffer_capacity)
        .with_context(|| "Unable to create an audio stream")?;

    // Start the stream
    stream
        .play()
        .with_context(|| "Unable to start the stream")?;

    // Pre-allocate a buffer to read samples into
    let samples_per_fragmentum =
        (MAX_FRAGMENTUM_LENGTH_SECONDS * sample_rate * channels as u32) as usize;
    let mut fragmentum_buffer = vec![0.0f32; samples_per_fragmentum];

    loop {
        // Small sleep to avoid busy-waiting
        thread::sleep(std::time::Duration::from_millis(100));

        // Check if we have enough samples for a chunk
        if consumer.occupied_len() >= samples_per_fragmentum {
            // Pop exactly samples_per_chunk samples into our pre-allocated buffer
            let popped = consumer.pop_slice(&mut fragmentum_buffer);

            if popped == samples_per_fragmentum {
                // Clone the data to pass to the processing thread
                // (the chunk_buffer will be reused for the next chunk)
                process_fragmentum(fragmentum_buffer.clone(), wav_config);
            }
        }
    }

    Ok(())
}
