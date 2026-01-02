use crate::configs::fractor::FractorConfig;
use crate::stt::audio::{convert_to_mono, resample, write_wav};
use crate::stt::queue::FragmentumToTranscribe;
use crate::stt::rec::Recorder;
use crate::stt::vad::VADModel;
use anyhow::{Context, Result};
use chrono::{DateTime, Local};
use ringbuf::traits::{Consumer, Observer};
use std::fs;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::SyncSender;
use uuid::{NoContext, Timestamp, Uuid};

/// State of Fractor
#[derive(Debug, Clone)]
pub struct FractorState {
    fragmentum_buffer: Vec<f32>,
    read_buffer: Vec<f32>,
    consecutive_silence_chunks: u32,
    vad_audio_buffer: Vec<f32>,
    start_datetime: DateTime<Local>,
}

impl FractorState {
    fn new(target_input_samples_for_vad: usize, max_samples_in_fragmentum: usize) -> Self {
        Self {
            fragmentum_buffer: Vec::with_capacity(max_samples_in_fragmentum),
            read_buffer: vec![0.0f32; target_input_samples_for_vad],
            consecutive_silence_chunks: 0_u32,
            vad_audio_buffer: Vec::new(),
            start_datetime: Local::now(),
        }
    }

    fn reset(&mut self) {
        self.fragmentum_buffer.clear();
        self.read_buffer.fill(0.0f32);
        self.consecutive_silence_chunks = 0_u32;
        self.vad_audio_buffer.clear();
        self.start_datetime = Local::now();
    }
}

pub struct Fractor {
    /// Recoder
    pub recorder: Recorder,
    /// Voice activity detector
    pub vad: VADModel,
    /// Configuration for Fractor
    pub config: FractorConfig,
    /// State of Fractor
    pub state: FractorState,
    /// Maximum allowed samples in VAD
    pub target_input_samples_for_vad: usize,
    /// Maximum allowed samples in a fragmentum
    pub max_samples_in_fragmentum: usize,
}

impl Fractor {
    /// Constructor with custom Fractor configuration
    pub fn with_config(recorder: Recorder, vad: VADModel, config: FractorConfig) -> Self {
        // Extract audio details
        let input_channels = recorder.config.wav_config.channels;
        let input_sample_rate = recorder.config.wav_config.sample_rate;
        let vad_chunk_size = vad.chunk_size();
        let vad_sample_rate = vad.sample_rate();

        // The VAD accepts only a limited number of samples in input at a given sample rate.
        // The stream/recorder might have different sr, hence we need to calculate what the vad chunk size corresponds
        // in the recorder sr (and channels!)
        let target_input_samples_for_vad = (vad_chunk_size as f32 * input_sample_rate as f32
            / vad_sample_rate as f32) as usize
            * input_channels as usize;

        // Compute the maximum number of samples in a fragmentum *we* allow
        let max_samples_in_fragmentum = (config.max_fragmentum_duration_seconds
            * input_sample_rate as f32
            * input_channels as f32) as usize;

        // Init state of Fractor
        let state = FractorState::new(target_input_samples_for_vad, max_samples_in_fragmentum);

        Self {
            recorder,
            vad,
            config,
            state,
            target_input_samples_for_vad,
            max_samples_in_fragmentum,
        }
    }

    /// Constructor with default Factor configuration
    pub fn new(recorder: Recorder, vad: VADModel) -> Self {
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

    /// Start recording and fragmenting
    fn start_recording(&mut self) -> Result<()> {
        self.recorder.is_recording = true;
        self.recorder.play().with_context(|| "Unable to play")?;
        Ok(())
    }

    /// Stop recording and fragmenting
    fn stop_recording(&mut self) {
        self.recorder.is_recording = false;
    }

    /// Flush the current buffer contents to a fragmentum and send for transcription
    fn flush_current_buffer(
        &mut self,
        output_dir: &PathBuf,
        tx: &SyncSender<FragmentumToTranscribe>,
    ) -> Result<()> {
        // Drain any remaining samples from the ring buffer into fragmentum_buffer
        let remaining_in_ring = self.recorder.consumer.occupied_len();
        if remaining_in_ring > 0 {
            let mut drain_buffer = vec![0.0f32; remaining_in_ring];
            let drained = self.recorder.consumer.pop_slice(&mut drain_buffer);
            self.state
                .fragmentum_buffer
                .extend_from_slice(&drain_buffer[..drained]);
        }

        // Save and send the fragmentum if there's any content
        if !self.state.fragmentum_buffer.is_empty() {
            let samples_to_process = std::mem::take(&mut self.state.fragmentum_buffer);

            // Save fragmentum
            let fragmentum_path = self
                .save_fragmentum(samples_to_process, self.state.start_datetime, output_dir)
                .with_context(|| "Failed to save recording during flush")?;

            // Create queue element
            let fragmentum_queue_element = FragmentumToTranscribe {
                path: fragmentum_path,
                start_datetime: self.state.start_datetime,
            };

            // Add to queue
            tx.send(fragmentum_queue_element)
                .with_context(|| "Failed to send fragment to transcription queue during flush")?;

            // Reset states for next fragmentum
            self.state.reset();
            self.vad.reset();
        }

        Ok(())
    }

    /// Save fragmentum to audio file
    fn save_fragmentum(
        &self,
        samples: Vec<f32>,
        datetime: DateTime<Local>,
        output_dir: &PathBuf,
    ) -> Result<PathBuf> {
        // Create unique ID using uuid v7 (for fun!)
        let ts = Timestamp::from_unix(
            NoContext,
            datetime.timestamp() as u64,
            datetime.timestamp_subsec_nanos(),
        );
        let id = Uuid::new_v7(ts);

        // Format the name of the file (filesystem-safe datetime format)
        let filename = format!("{}_{}.wav", datetime.format("%Y-%m-%d@%H:%M:%S"), id);

        // Create directory if it doesn't exist
        if !output_dir.exists() {
            fs::create_dir_all(output_dir)
                .with_context(|| format!("Failed to create directory: {}", output_dir.display()))?;
        }

        // Define output path
        let output_path = output_dir.join(filename);

        write_wav(samples, self.recorder.config.wav_config, &output_path)
            .with_context(|| "Failed to write audio to file")?;

        Ok(output_path)
    }

    /// Run the fractor. Returns the temp directory to clean up (if any) after transcription completes.
    pub fn run(
        mut self,
        output_dir: Option<PathBuf>,
        stop_signal: Arc<AtomicBool>,
        pause_signal: Arc<AtomicBool>,
        tx: SyncSender<FragmentumToTranscribe>,
    ) -> Result<Option<PathBuf>> {
        // Change status of recorder
        self.start_recording().with_context(|| "Unable to play")?;

        // We always store audio because it is needed by STT implementation.
        // If not required to save it, we remove it at the end of the processing
        let (output_dir, erase) = if let Some(dir) = output_dir {
            (dir, false)
        } else {
            (std::env::temp_dir().join("scriptor_audio"), true)
        };

        // Track if we're currently paused
        let mut is_paused = false;

        while self.recorder.is_recording && !stop_signal.load(Ordering::Relaxed) {
            // Check for pause signal
            let should_pause = pause_signal.load(Ordering::Relaxed);

            if should_pause && !is_paused {
                // Entering pause state: flush buffer and pause stream
                self.flush_current_buffer(&output_dir, &tx)?;
                self.recorder
                    .pause()
                    .with_context(|| "Failed to pause stream")?;
                self.recorder.clear_buffer();
                is_paused = true;
                continue;
            } else if !should_pause && is_paused {
                // Exiting pause state: resume stream
                self.recorder
                    .play()
                    .with_context(|| "Failed to resume stream")?;
                is_paused = false;
                continue;
            } else if is_paused {
                // Still paused, sleep briefly to avoid busy-waiting
                std::thread::sleep(std::time::Duration::from_millis(50));
                continue;
            }

            // Read available samples in small batches
            let available_samples_in_buffer = self.recorder.consumer.occupied_len();

            // If we are not meeting the requirements for the VAD, we keep recording
            // Remember that the target value is in the "recorder" frame, i.e. depends on the recorder
            // sr and channels. It might not coincide with the number of samples the VAD expects.
            // That's why we convert to mono and resample later on.
            if available_samples_in_buffer < self.target_input_samples_for_vad {
                continue;
            }

            // If we arrive here, we have the right number of samples
            // Pop a chunk of samples
            let popped = self
                .recorder
                .consumer
                .pop_slice(&mut self.state.read_buffer);
            if popped == 0 {
                continue;
            }

            // Accumulate raw samples for the fragmentum into a buffer
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
                self.vad.sample_rate(),
            )
            .unwrap_or_default();

            // Add the processed samples to the buffer that will feed the VAD
            self.state.vad_audio_buffer.extend(resampled);

            // Process complete VAD chunks
            while self.state.vad_audio_buffer.len() >= self.vad.chunk_size() {
                let vad_chunk: Vec<f32> = self
                    .state
                    .vad_audio_buffer
                    .drain(..self.vad.chunk_size())
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

            // Check if we should cut the recording because enough pauses have accumulated
            if self.should_cut(duration_secs, self.state.consecutive_silence_chunks) {
                // Process the fragmentum (saves to file and runs STT)
                let samples_to_process = std::mem::take(&mut self.state.fragmentum_buffer);

                // Save fragmentum
                let fragmentum_path = self
                    .save_fragmentum(samples_to_process, self.state.start_datetime, &output_dir)
                    .with_context(|| "Failed to save recording")?;

                // Create queue element
                let fragmentum_queue_element = FragmentumToTranscribe {
                    path: fragmentum_path,
                    start_datetime: self.state.start_datetime,
                };

                // Add to queue
                tx.send(fragmentum_queue_element)
                    .with_context(|| "Failed to send to transcription queue")?;

                // Reset states for next fragmentum
                self.state.reset(); // Reset state of fractor buffer
                self.vad.reset(); // Reset LSTM state for new segment
            }
        }

        // Flush any remaining buffer content before stopping
        self.flush_current_buffer(&output_dir, &tx)?;

        // Stop the recording
        self.stop_recording();

        // Return the temp directory to clean up after transcription completes (if any)
        if erase {
            Ok(Some(output_dir))
        } else {
            Ok(None)
        }
    }
}
