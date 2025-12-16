use crate::stt::audio::{convert_to_mono, resample};
use crate::stt::rec::Recorder;
use crate::stt::vad::VoiceActivityDetector;
use anyhow::{Context, Result};
use cpal::Stream;
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{Device, SupportedStreamConfig};
use hound::WavSpec;
use ringbuf::traits::{Consumer, Observer};
use ringbuf::{
    HeapCons, HeapProd, HeapRb,
    traits::{Producer, Split},
};

/// Configuration for Fractor, responsible of dividing audio in fragmenta (chunks)
#[derive(Debug, Clone)]
pub struct FractorConfig {
    // Minimum duration of a fragmentum, to avoid very short recordings
    min_fragmentum_duration_seconds: f32,
    // Maximum duration of a fragmentum, to avoid large files harder to handle
    max_fragmentum_duration_seconds: f32,
    // A "chunk" is a chunk of samples (typically 512)
    // If in multiple consecutive chunks there's no speech, we declare it a pause
    // The threshold on chunks determines how long a pause should be, e.g.
    // pause_threshold_in_chunks = 16 means ~0.5s of pause at 16kHz
    pause_threshold_in_chunks: u32,
}

impl Default for FractorConfig {
    fn default() -> Self {
        Self {
            min_fragmentum_duration_seconds: 5.0_f32,
            max_fragmentum_duration_seconds: 20.0_f32,
            pause_threshold_in_chunks: 16_u32,
        }
    }
}

impl FractorConfig {
    fn new(
        min_fragmentum_duration_seconds: f32,
        max_fragmentum_duration_seconds: f32,
        pause_threshold_in_chunks: u32,
    ) -> Self {
        Self {
            min_fragmentum_duration_seconds,
            max_fragmentum_duration_seconds,
            pause_threshold_in_chunks,
        }
    }
}

/// State of Fractor
#[derive(Debug, Clone)]
struct FractorState {
    fragmentum_buffer: Vec<f32>,
    read_buffer: Vec<f32>,
    consecutive_silence_chunks: u32,
    vad_audio_buffer: Vec<f32>,
}

impl FractorState {
    fn new(max_input_samples_for_vad: usize, max_samples_in_fragmentum: usize) -> Self {
        Self {
            fragmentum_buffer: Vec::with_capacity(max_samples_in_fragmentum),
            read_buffer: vec![0.0f32; max_input_samples_for_vad],
            consecutive_silence_chunks: 0_u32,
            vad_audio_buffer: Vec::new(),
        }
    }
}

pub struct Fractor {
    /// Recoder
    pub recorder: Recorder,
    /// Voice activity detector
    pub vad: VoiceActivityDetector,
    /// Configuration for Fractor
    pub config: FractorConfig,
    /// State of Fractor
    pub state: FractorState,
    /// Maximum allowed samples in VAD
    pub max_input_samples_for_vad: usize,
    /// Maximum allowed samples in a fragmentum
    pub max_samples_in_fragmentum: usize,
}

impl Fractor {
    /// Constructor with custom Fractor configuration
    pub fn with_config(
        recorder: Recorder,
        vad: VoiceActivityDetector,
        config: FractorConfig,
    ) -> Self {
        // Extract audio details
        let input_channels = recorder.config.wav_config.channels;
        let input_sample_rate = recorder.config.wav_config.sample_rate;
        let vad_chunk_size = vad.config.chunk_size;
        let vad_sample_rate = vad.config.sample_rate;

        // The VAD accepts only a limited number of samples in input at a given sample rate.
        // The stream/recorder might have different sr, hence we need to calculate what the vad chunk size corresponds
        // in the recorder sr (and channels!)
        let max_input_samples_for_vad = (vad_chunk_size as f32 * input_sample_rate as f32
            / vad_sample_rate as f32) as usize
            * input_channels as usize;

        // Compute the maximum number of samples in a fragmentum *we* allow
        let max_samples_in_fragmentum = (config.max_fragmentum_duration_seconds
            * input_sample_rate as f32
            * input_channels as f32) as usize;

        // Init state of Fractor
        let state = FractorState::new(max_input_samples_for_vad, max_samples_in_fragmentum);

        Self {
            recorder,
            vad,
            config,
            state,
            max_input_samples_for_vad,
            max_samples_in_fragmentum,
        }
    }

    /// Constructor with default Factor configuration
    pub fn new(recorder: Recorder, vad: VoiceActivityDetector) -> Self {
        Self::with_config(recorder, vad, FractorConfig::default())
    }

    /// Determine if we should cut the current recording based on duration and pause detection
    fn should_cut(
        &self,
        current_duration_secs: f32,
        current_consecutive_silence_in_chunks: u32,
    ) -> bool {
        if current_duration_secs < self.config.min_fragmentum_duration_seconds {
            return false; // Too short, keep recording
        }
        if current_duration_secs >= self.config.max_fragmentum_duration_seconds {
            return true; // Force cut at max duration
        }
        // In target window: cut on pause.
        current_consecutive_silence_in_chunks >= self.config.pause_threshold_in_chunks
    }

    // Start recording and fragmenting
    fn start_recording(&mut self) {
        self.recorder.is_recording = true;
        self.recorder.play();
    }

    // Stop recording and fragmenting
    fn stop_recording(&mut self) {
        self.recorder.is_recording = false;
    }

    fn run(mut self) {
        // Change status of recorder
        self.start_recording();

        while self.recorder.is_recording {
            // Read available samples in small batches
            let available = self.recorder.consumer.occupied_len();
            if available < self.max_input_samples_for_vad {
                continue;
            }

            // Pop a chunk of samples
            let popped = self
                .recorder
                .consumer
                .pop_slice(&mut self.state.read_buffer);
            if popped == 0 {
                continue;
            }

            // Accumulate raw samples for the fragmentum
            self.state
                .fragmentum_buffer
                .extend_from_slice(&self.state.read_buffer[..popped]);

            // Convert to mono and resample for VAD processing
            let mono_samples = convert_to_mono(
                self.state.read_buffer[..popped].to_vec(),
                self.recorder.config.wav_config.channels,
            );
            let resampled = resample(
                mono_samples,
                self.recorder.config.wav_config.sample_rate,
                self.vad.config.sample_rate as u32,
            )
            .unwrap_or_default();
            self.state.vad_audio_buffer.extend(resampled);

            // Process complete VAD chunks (512 samples each)
            while self.state.vad_audio_buffer.len() >= self.vad.config.chunk_size {
                let vad_chunk: Vec<f32> = self
                    .state
                    .vad_audio_buffer
                    .drain(..self.vad.config.chunk_size)
                    .collect();

                // Run VAD prediction
                match self.vad.predict(vad_chunk) {
                    Ok(is_speech) => {
                        if is_speech {
                            self.state.consecutive_silence_chunks = 0;
                        } else {
                            self.state.consecutive_silence_chunks += 1;
                        }
                    }
                    Err(e) => {
                        eprintln!("VAD prediction error: {e}");
                    }
                }
            }

            // Calculate current duration in seconds
            let total_samples = self.state.fragmentum_buffer.len();
            let duration_secs = total_samples as f32
                / (self.recorder.config.wav_config.sample_rate as f32
                    * self.recorder.config.wav_config.channels as f32);

            // Check if we should cut
            if self.should_cut(duration_secs, self.state.consecutive_silence_chunks) {
                println!(
                    "Cutting fragmentum: {:.1}s, silence_chunks: {}",
                    duration_secs, self.state.consecutive_silence_chunks
                );

                // Process the fragmentum (saves to file and runs STT)
                let samples_to_process = std::mem::take(&mut self.state.fragmentum_buffer);

                // TODO
                //process_fragmentum(samples_to_process, wav_config);

                // Reset state for next fragmentum
                self.state.fragmentum_buffer = Vec::with_capacity(self.max_samples_in_fragmentum);
                self.state.consecutive_silence_chunks = 0;
                self.state.vad_audio_buffer.clear();
                self.vad.reset(); // Reset LSTM state for new segment
            }
        }
    }
}
