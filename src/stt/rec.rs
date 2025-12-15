use anyhow::{Context, Result};
use cpal::Stream;
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{Device, SupportedStreamConfig};
use hound::WavSpec;
use ringbuf::{
    HeapCons, HeapProd, HeapRb,
    traits::{Consumer, Observer, Producer, Split},
};
use std::thread;

pub struct Recorder {
    stream: Stream,
    consumer: HeapCons<f32>,
    config: RecorderConfig,
}

struct RecorderConfig {
    input_device: Device,
    input_config: SupportedStreamConfig,
    wav_config: WavSpec,
    buffer_capacity: usize,
}

impl RecorderConfig {
    fn new(max_fragmentum_duration_seconds: f32) -> Result<Self> {
        // Set up recording device
        let (input_device, input_config) =
            setup_recording().with_context(|| "Unable to set up recording")?;

        // Convert the input config (cpal-style) into wav config (hound-style)
        let wav_config = wav_spec_from_config(&input_config);

        // Extract parameters for buffer sizing and resampling strategy
        let sample_rate = input_config.sample_rate().0;
        let channels = input_config.channels();

        // Compute buffer capacity
        let buffer_capacity =
            estimate_buffer_capacity(sample_rate, channels, max_fragmentum_duration_seconds);

        Ok(Self {
            input_device: input_device,
            input_config: input_config,
            wav_config: wav_config,
            buffer_capacity: buffer_capacity,
        })
    }
}

impl Recorder {
    fn new(max_fragmentum_duration_seconds: f32) -> Result<Self> {
        // Call construct for recorder config
        let config = RecorderConfig::new(max_fragmentum_duration_seconds)
            .with_context(|| "Unable to create recorder config")?;

        // Create the audio stream
        let (stream, mut consumer) = setup_audio_stream(
            config.input_device.clone(),
            config.input_config.clone(),
            config.buffer_capacity.clone(),
        )
        .with_context(|| "Unable to create an audio stream")?;

        Ok(Self {
            stream: stream,
            consumer: consumer,
            config: config,
        })
    }

    /// Start recording
    fn play(self) -> Result<()> {
        self.stream
            .play()
            .with_context(|| "Unable to start the stream")?;
        Ok(())
    }
}

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
fn estimate_buffer_capacity(
    sample_rate: u32,
    n_channels: u16,
    max_fragmentum_duration_seconds: f32,
) -> usize {
    2 * (sample_rate as f32 * n_channels as f32 * max_fragmentum_duration_seconds) as usize
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

// /// Determine if we should cut the current recording based on duration and pause detection
// fn should_cut(
//     duration_secs: f32,
//     consecutive_silence_chunks: u32,
//     min_recording_duration_seconds: f32,
//     max_recording_duration_seconds: f32,
//     pause_chunks_threshold: u32,
// ) -> bool {
//     if duration_secs < min_recording_duration_seconds as f32 {
//         return false; // Too short, keep recording
//     }
//     if duration_secs >= max_recording_duration_seconds as f32 {
//         return true; // Force cut at max duration
//     }
//     // In target window: cut on pause.
//     // A "chunk" is a chunk of samples (typically 512)
//     // If in multiple consecutive chunks there's no speech, we declare it a pause
//     // The threshold on chunks determines how long a pause should be, e.g.
//     // pause_chunks_threshold = 16 means ~0.5s of pause at 16kHz
//     consecutive_silence_chunks >= pause_chunks_threshold
// }

// /// Pre-processing pipeline for STT
// fn preprocess_audio_for_stt(samples: Vec<f32>, wav_config: WavSpec) -> Result<Vec<f32>> {
//     resample(
//         convert_to_mono(samples, wav_config.channels),
//         wav_config.sample_rate,
//         TARGET_SAMPLE_RATE,
//     )
// }

// fn process_fragmentum(samples: Vec<f32>, wav_config: WavSpec) {
//     thread::spawn(move || {
//         let stt_samples = preprocess_audio_for_stt(samples.clone(), wav_config.clone())
//             .expect("Unable to pre-process audio for STT");
//         let filename = PathBuf::from(format!(
//             "rec_{}.wav",
//             Local::now().format("%Y-%m-%d_%H:%M:%S")
//         ));
//         write_wav(samples, wav_config.clone(), &filename)
//             .expect("Unable to write fragmentum to file");
//     });
// }

// fn main() -> Result<()> {
//     // Set up recording device
//     let (input_device, input_config) =
//         setup_recording().with_context(|| "Unable to set up recording")?;

//     // Convert the input config (cpal-style) into wav config (hound-style)
//     let wav_config = wav_spec_from_config(&input_config);

//     // Extract parameters for buffer sizing and resampling strategy
//     let sample_rate = input_config.sample_rate().0;
//     let channels = input_config.channels();

//     // Compute buffer capacity
//     //let buffer_capacity = estimate_buffer_capacity(sample_rate, channels);

//     // Create the audio stream
//     let (stream, mut consumer) = setup_audio_stream(input_device, input_config, buffer_capacity)
//         .with_context(|| "Unable to create an audio stream")?;

//     // Start the stream
//     stream
//         .play()
//         .with_context(|| "Unable to start the stream")?;

//     // Pre-allocate a buffer to read samples into
//     let samples_per_fragmentum =
//         (MAX_FRAGMENTUM_LENGTH_SECONDS * sample_rate * channels as u32) as usize;
//     let mut fragmentum_buffer = vec![0.0f32; samples_per_fragmentum];

//     loop {
//         // Small sleep to avoid busy-waiting
//         thread::sleep(std::time::Duration::from_millis(100));

//         // Check if we have enough samples for a chunk
//         if consumer.occupied_len() >= samples_per_fragmentum {
//             // Pop exactly samples_per_chunk samples into our pre-allocated buffer
//             let popped = consumer.pop_slice(&mut fragmentum_buffer);

//             if popped == samples_per_fragmentum {
//                 // Clone the data to pass to the processing thread
//                 // (the chunk_buffer will be reused for the next chunk)
//                 process_fragmentum(fragmentum_buffer.clone(), wav_config);
//             }
//         }
//     }

//     Ok(())
// }
