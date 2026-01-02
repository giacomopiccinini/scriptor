use crate::stt::audio::wav_spec_from_config;
use anyhow::{Context, Result};
use cpal::Stream;
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{Device, SupportedStreamConfig};
use hound::WavSpec;
use ringbuf::{
    HeapCons, HeapProd, HeapRb,
    traits::{Producer, Split},
};

pub struct Recorder {
    pub stream: Stream,
    pub consumer: HeapCons<f32>,
    pub config: RecorderConfig,
    pub is_recording: bool,
}

pub struct RecorderConfig {
    input_device: Device,
    input_config: SupportedStreamConfig,
    pub wav_config: WavSpec,
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
            input_device,
            input_config,
            wav_config,
            buffer_capacity,
        })
    }
}

impl Recorder {
    pub fn new(max_fragmentum_duration_seconds: f32) -> Result<Self> {
        // Call construct for recorder config
        let config = RecorderConfig::new(max_fragmentum_duration_seconds)
            .with_context(|| "Unable to create recorder config")?;

        // Create the audio stream
        let (stream, consumer) = setup_audio_stream(
            config.input_device.clone(),
            config.input_config.clone(),
            config.buffer_capacity,
        )
        .with_context(|| "Unable to create an audio stream")?;

        Ok(Self {
            stream,
            consumer,
            config,
            is_recording: false,
        })
    }

    /// Start recording
    pub fn play(&self) -> Result<()> {
        self.stream
            .play()
            .with_context(|| "Unable to start the stream")?;
        Ok(())
    }
}

/// Setup the recording by finding the default device with the corresponding config
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

    // Explicitly pause the stream after creation.
    // On some Linux audio backends (PipeWire/PulseAudio), cpal's build_input_stream()
    // may auto-start the stream instead of creating it in a paused state.
    // This ensures the stream only runs when explicitly started via play().
    stream.pause()?;

    Ok((stream, consumer))
}
