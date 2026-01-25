use crate::stt::audio::wav_spec_from_config;
use anyhow::{Context, Result};
use cpal::Stream;
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{Device, SupportedStreamConfig};
use hound::WavSpec;
use ringbuf::{
    HeapCons, HeapProd, HeapRb,
    traits::{Consumer, Observer, Producer, Split},
};
use std::sync::Arc;
use std::sync::atomic::AtomicBool;

pub struct Recorder {
    pub stream: Stream,
    pub consumer: HeapCons<f32>,
    pub config: RecorderConfig,
    pub is_recording: Arc<AtomicBool>,
}

pub struct RecorderConfig {
    input_device: Device,
    input_config: SupportedStreamConfig,
    pub wav_config: WavSpec,
    buffer_capacity: usize,
}

impl RecorderConfig {
    /// Create a new RecorderConfig with the given max fragmentum duration.
    /// This only sets up the configuration and doesn't create the audio stream yet,
    /// allowing the config to be safely sent across threads.
    ///
    /// If `device_name` is provided, attempts to find that device. Falls back to
    /// system default if not found.
    pub fn new(max_fragmentum_duration_seconds: f32, device_name: Option<&str>) -> Result<Self> {
        // Set up recording device
        let (input_device, input_config) =
            setup_recording(device_name).with_context(|| "Unable to set up recording")?;

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
    /// Create a new Recorder from an existing RecorderConfig.
    /// This creates the audio stream and should be called from the thread
    /// where the stream will be used (required for macOS CoreAudio compatibility).
    pub fn from_config(config: RecorderConfig) -> Result<Self> {
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
            is_recording: Arc::new(AtomicBool::new(false)),
        })
    }

    /// Convenience constructor that creates both config and stream.
    /// Note: On macOS, the returned Recorder cannot be sent across threads.
    /// Use RecorderConfig::new() + Recorder::from_config() for cross-thread scenarios.
    pub fn new(max_fragmentum_duration_seconds: f32, device_name: Option<&str>) -> Result<Self> {
        let config = RecorderConfig::new(max_fragmentum_duration_seconds, device_name)
            .with_context(|| "Unable to create recorder config")?;
        Self::from_config(config)
    }

    /// Start recording
    pub fn play(&self) -> Result<()> {
        self.stream
            .play()
            .with_context(|| "Unable to start the stream")?;
        Ok(())
    }

    /// Pause recording
    pub fn pause(&self) -> Result<()> {
        self.stream
            .pause()
            .with_context(|| "Unable to pause the stream")?;
        Ok(())
    }

    /// Clear any remaining samples from the ring buffer
    pub fn clear_buffer(&mut self) {
        // Drain all samples from the consumer
        while self.consumer.occupied_len() > 0 {
            let mut discard = vec![0.0f32; self.consumer.occupied_len().min(1024)];
            self.consumer.pop_slice(&mut discard);
        }
    }
}

/// Setup the recording by finding the specified device (or default) with its config.
/// If `device_name` is provided, attempts to find that device by name.
/// Falls back to system default if the named device is not found.
fn setup_recording(device_name: Option<&str>) -> Result<(Device, SupportedStreamConfig)> {
    // Get default audio host
    let host = cpal::default_host();

    // Find the input device - either by name or use default
    let input_device = if let Some(name) = device_name {
        // Try to find the device by name
        let found_device = host
            .input_devices()
            .ok()
            .and_then(|mut devices| devices.find(|d| d.name().ok().as_deref() == Some(name)));

        match found_device {
            Some(device) => device,
            None => {
                eprintln!(
                    "Warning: Configured device '{}' not found, using system default",
                    name
                );
                host.default_input_device()
                    .with_context(|| "Unable to find default input device")?
            }
        }
    } else {
        host.default_input_device()
            .with_context(|| "Unable to find default input device")?
    };

    // Use default configuration for audio input. If this doesn't match the requirements from
    // STT model, we fix it down the line, not here.
    let input_config = input_device
        .default_input_config()
        .with_context(|| "Unable to find default input config")?;

    Ok((input_device, input_config))
}

/// Enumerate all available input devices and return their names.
/// Returns a list of device names that can be used with `RecorderConfig::new()`.
///
/// On Linux, this temporarily suppresses stderr to avoid ALSA warnings
/// about unavailable OSS devices (/dev/dsp) which are harmless but noisy.
pub fn enumerate_input_devices() -> Vec<String> {
    #[cfg(target_os = "linux")]
    let _guard = suppress_stderr();

    let host = cpal::default_host();
    host.input_devices()
        .map(|devices| devices.filter_map(|d| d.name().ok()).collect())
        .unwrap_or_default()
}

/// RAII guard that suppresses stderr on Linux during device enumeration.
/// Stderr is restored when the guard is dropped.
#[cfg(target_os = "linux")]
struct StderrSuppressor {
    original_fd: Option<std::os::unix::io::RawFd>,
}

#[cfg(target_os = "linux")]
impl Drop for StderrSuppressor {
    fn drop(&mut self) {
        use std::os::unix::io::AsRawFd;
        if let Some(original_fd) = self.original_fd {
            // Restore original stderr
            unsafe {
                libc::dup2(original_fd, std::io::stderr().as_raw_fd());
                libc::close(original_fd);
            }
        }
    }
}

#[cfg(target_os = "linux")]
fn suppress_stderr() -> StderrSuppressor {
    use std::os::unix::io::AsRawFd;

    let stderr_fd = std::io::stderr().as_raw_fd();

    // Duplicate the original stderr so we can restore it later
    let original_fd = unsafe { libc::dup(stderr_fd) };
    if original_fd == -1 {
        return StderrSuppressor { original_fd: None };
    }

    // Open /dev/null and redirect stderr to it
    let dev_null = unsafe { libc::open(b"/dev/null\0".as_ptr() as *const libc::c_char, libc::O_WRONLY) };
    if dev_null != -1 {
        unsafe {
            libc::dup2(dev_null, stderr_fd);
            libc::close(dev_null);
        }
    }

    StderrSuppressor {
        original_fd: Some(original_fd),
    }
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
    stream
        .pause()
        .with_context(|| "Unable to stop streaming during setup")?;

    Ok((stream, consumer))
}
