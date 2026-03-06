use crate::configs::fractor::FractorConfig;
use crate::stt::audio::{convert_to_mono, resample, write_wav};
use crate::stt::queue::FragmentumToTranscribe;
use crate::stt::rec::{Recorder, RecorderConfig};
use crate::stt::vad::VADModel;
use anyhow::{Context, Result};
use chrono::{DateTime, Local};
use hound::WavSpec;
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
    /// Cumulative offset in seconds from the start of the recording session
    recording_offset_secs: f32,
}

impl FractorState {
    fn new(target_input_samples_for_vad: usize, max_samples_in_fragmentum: usize) -> Self {
        Self {
            fragmentum_buffer: Vec::with_capacity(max_samples_in_fragmentum),
            read_buffer: vec![0.0f32; target_input_samples_for_vad],
            consecutive_silence_chunks: 0_u32,
            vad_audio_buffer: Vec::new(),
            start_datetime: Local::now(),
            recording_offset_secs: 0.0,
        }
    }

    fn reset(&mut self) {
        self.fragmentum_buffer.clear();
        self.read_buffer.fill(0.0f32);
        self.consecutive_silence_chunks = 0_u32;
        self.vad_audio_buffer.clear();
        self.start_datetime = Local::now();
        // Note: recording_offset_secs is NOT reset here - it accumulates across the session
    }

    /// Reset the recording session (called when starting a new recording)
    fn reset_session(&mut self) {
        self.reset();
        self.recording_offset_secs = 0.0;
    }
}

pub struct Fractor {
    /// Recorder configuration (stream created lazily in run() for thread safety)
    pub recorder_config: RecorderConfig,
    /// Cached wav spec from recorder config
    pub wav_config: WavSpec,
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
    /// Constructor with custom Fractor configuration.
    /// Takes RecorderConfig instead of Recorder to allow cross-thread transfer on macOS.
    /// The actual Recorder (with cpal::Stream) is created inside run().
    pub fn with_config(
        recorder_config: RecorderConfig,
        vad: VADModel,
        config: FractorConfig,
    ) -> Self {
        // Extract audio details from config
        let wav_config = recorder_config.wav_config;
        let input_channels = wav_config.channels;
        let input_sample_rate = wav_config.sample_rate;
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
            recorder_config,
            wav_config,
            vad,
            config,
            state,
            target_input_samples_for_vad,
            max_samples_in_fragmentum,
        }
    }

    /// Constructor with default Fractor configuration.
    /// Takes RecorderConfig instead of Recorder to allow cross-thread transfer on macOS.
    pub fn new(recorder_config: RecorderConfig, vad: VADModel) -> Self {
        Self::with_config(recorder_config, vad, FractorConfig::default())
    }

    /// Determine if we should cut the current recording based on duration and pause detection
    fn should_cut(
        config: &FractorConfig,
        current_duration_secs: f32,
        current_consecutive_silence_in_chunks: u32,
    ) -> bool {
        if current_duration_secs < config.min_fragmentum_duration_seconds {
            return false; // Too short, keep recording
        }
        if current_duration_secs >= config.max_fragmentum_duration_seconds {
            return true; // Force cut at max duration
        }
        // In target window: cut on pause.
        current_consecutive_silence_in_chunks >= config.pause_threshold_in_chunks
    }

    /// Start recording and fragmenting
    fn start_recording(recorder: &mut Recorder) -> Result<()> {
        recorder.is_recording = Arc::new(AtomicBool::new(true));
        recorder.play().with_context(|| "Unable to play")?;

        // On some Linux audio backends (PipeWire/PulseAudio), pause() only stops
        // the stream callback but the driver continues buffering audio. When play()
        // resumes, all that stale audio floods into the ring buffer. We need to:
        // 1. Wait briefly for any OS-buffered audio to be delivered
        // 2. Clear the ring buffer to discard stale samples
        std::thread::sleep(std::time::Duration::from_millis(100));
        recorder.clear_buffer();

        Ok(())
    }

    /// Stop recording and fragmenting
    fn stop_recording(recorder: &mut Recorder) -> Result<()> {
        recorder.is_recording = Arc::new(AtomicBool::new(false));
        recorder.pause().with_context(|| "Unable to stop")?;
        Ok(())
    }

    /// Flush the current buffer contents to a fragmentum and send for transcription
    fn flush_current_buffer(
        recorder: &mut Recorder,
        state: &mut FractorState,
        vad: &mut VADModel,
        wav_config: &WavSpec,
        output_dir: &PathBuf,
        tx: &SyncSender<FragmentumToTranscribe>,
    ) -> Result<()> {
        // Drain any remaining samples from the ring buffer into fragmentum_buffer
        let remaining_in_ring = recorder.consumer.occupied_len();
        if remaining_in_ring > 0 {
            let mut drain_buffer = vec![0.0f32; remaining_in_ring];
            let drained = recorder.consumer.pop_slice(&mut drain_buffer);
            state
                .fragmentum_buffer
                .extend_from_slice(&drain_buffer[..drained]);
        }

        // Save and send the fragmentum if there's any content
        if !state.fragmentum_buffer.is_empty() {
            let samples_to_process = std::mem::take(&mut state.fragmentum_buffer);

            // Calculate duration of this fragmentum
            let duration_secs = samples_to_process.len() as f32
                / (wav_config.sample_rate as f32 * wav_config.channels as f32);

            // Calculate timestamps
            let timestamp_start = state.recording_offset_secs;
            let timestamp_end = state.recording_offset_secs + duration_secs;

            // Save fragmentum
            let fragmentum_path = Self::save_fragmentum(
                wav_config,
                samples_to_process,
                state.start_datetime,
                output_dir,
            )
            .with_context(|| "Failed to save recording during flush")?;

            // Create queue element with timestamps
            let fragmentum_queue_element = FragmentumToTranscribe {
                path: fragmentum_path,
                start_datetime: state.start_datetime,
                timestamp_start,
                timestamp_end,
            };

            // Add to queue
            tx.send(fragmentum_queue_element)
                .with_context(|| "Failed to send fragment to transcription queue during flush")?;

            // Update the cumulative offset for the next fragmentum
            state.recording_offset_secs = timestamp_end;

            // Reset states for next fragmentum (but not the offset)
            state.reset();
            vad.reset();
        }

        Ok(())
    }

    /// Save fragmentum to audio file
    fn save_fragmentum(
        wav_config: &WavSpec,
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

        write_wav(samples, *wav_config, &output_path)
            .with_context(|| "Failed to write audio to file")?;

        Ok(output_path)
    }

    /// Run the fractor. Returns the temp directory to clean up (if any) after transcription completes,
    /// along with the VADModel so it can be reused for subsequent recordings.
    ///
    /// # Arguments
    ///
    /// * `initial_offset_secs` - When extending an existing folio, pass the max `timestamp_end` from
    ///   its fragmenta so new timestamps continue from the last. Use `0.0` for a new recording.
    pub fn run(
        self,
        output_dir: Option<PathBuf>,
        stop_signal: Arc<AtomicBool>,
        pause_signal: Arc<AtomicBool>,
        tx: SyncSender<FragmentumToTranscribe>,
        initial_offset_secs: f32,
    ) -> Result<(Option<PathBuf>, VADModel)> {
        // Destructure self to avoid partial move issues after moving recorder_config
        let Self {
            recorder_config,
            wav_config,
            mut vad,
            config,
            mut state,
            target_input_samples_for_vad,
            max_samples_in_fragmentum: _,
        } = self;

        // Create the Recorder here (inside the thread) to avoid Send issues with cpal::Stream on macOS.
        // CoreAudio's Stream contains non-Send types, so we must create it in the thread where it will be used.
        let mut recorder = Recorder::from_config(recorder_config)
            .with_context(|| "Failed to create recorder from config")?;

        // Change status of recorder
        Self::start_recording(&mut recorder).with_context(|| "Unable to play")?;

        // Reset the entire session (including offset) for a fresh recording
        state.reset_session();
        // When extending a folio, continue timestamps from the last fragmentum
        state.recording_offset_secs = initial_offset_secs;

        // We always store audio because it is needed by STT implementation.
        // If not required to save it, we remove it at the end of the processing
        let (output_dir, erase) = if let Some(dir) = output_dir {
            (dir, false)
        } else {
            (std::env::temp_dir().join("scriptor_audio"), true)
        };

        // Track if we're currently paused
        let mut is_paused = false;

        while recorder.is_recording.load(Ordering::SeqCst) && !stop_signal.load(Ordering::SeqCst) {
            // Check for pause signal
            let should_pause = pause_signal.load(Ordering::SeqCst);

            if should_pause && !is_paused {
                // Entering pause state: flush buffer (save pre-pause audio), then pause stream
                Self::flush_current_buffer(
                    &mut recorder,
                    &mut state,
                    &mut vad,
                    &wav_config,
                    &output_dir,
                    &tx,
                )?;
                recorder.pause().with_context(|| "Failed to pause stream")?;
                recorder.clear_buffer();
                is_paused = true;
                continue;
            } else if !should_pause && is_paused {
                // Exiting pause state: resume stream, then discard stale audio.
                // On Linux (PipeWire/PulseAudio), the OS buffers audio while "paused".
                // When play() resumes, that buffered audio floods in.
                // Discard it so
                // speech said while paused does not appear after resume.
                recorder.play().with_context(|| "Failed to resume stream")?;
                std::thread::sleep(std::time::Duration::from_millis(100));
                recorder.clear_buffer();
                is_paused = false;
                continue;
            } else if is_paused {
                // Still paused. On some Linux backends (PipeWire/PulseAudio), cpal's pause()
                // does not stop the stream callback, audio keeps accumulating in the ring buffer.
                // Discard it so we don't process stale audio when resuming.
                recorder.clear_buffer();
                std::thread::sleep(std::time::Duration::from_millis(50));
                continue;
            }

            // Read available samples in small batches
            let available_samples_in_buffer = recorder.consumer.occupied_len();

            // If we are not meeting the requirements for the VAD, we keep recording
            // Remember that the target value is in the "recorder" frame, i.e. depends on the recorder
            // sr and channels. It might not coincide with the number of samples the VAD expects.
            // That's why we convert to mono and resample later on.
            if available_samples_in_buffer < target_input_samples_for_vad {
                continue;
            }

            // If we arrive here, we have the right number of samples
            // Pop a chunk of samples
            let popped = recorder.consumer.pop_slice(&mut state.read_buffer);
            if popped == 0 {
                continue;
            }

            // Accumulate raw samples for the fragmentum into a buffer
            state
                .fragmentum_buffer
                .extend_from_slice(&state.read_buffer[..popped]);

            // Convert to mono and resample for VAD processing
            let mono_samples =
                convert_to_mono(state.read_buffer[..popped].to_vec(), wav_config.channels);
            let resampled = resample(mono_samples, wav_config.sample_rate, vad.sample_rate())
                .unwrap_or_default();

            // Add the processed samples to the buffer that will feed the VAD
            state.vad_audio_buffer.extend(resampled);

            // Process complete VAD chunks
            while state.vad_audio_buffer.len() >= vad.chunk_size() {
                let vad_chunk: Vec<f32> =
                    state.vad_audio_buffer.drain(..vad.chunk_size()).collect();

                // Run VAD prediction
                match vad.predict(vad_chunk) {
                    Ok(is_speech) => {
                        if is_speech {
                            state.consecutive_silence_chunks = 0;
                        } else {
                            state.consecutive_silence_chunks += 1;
                        }
                    }
                    Err(e) => {
                        tracing::error!("VAD prediction error: {e}");
                    }
                }
            }

            // Calculate current duration in seconds
            let total_samples = state.fragmentum_buffer.len();
            let duration_secs =
                total_samples as f32 / (wav_config.sample_rate as f32 * wav_config.channels as f32);

            // Check if we should cut the recording because enough pauses have accumulated
            if Self::should_cut(&config, duration_secs, state.consecutive_silence_chunks) {
                // Process the fragmentum (saves to file and runs STT)
                let samples_to_process = std::mem::take(&mut state.fragmentum_buffer);

                // Calculate duration of this fragmentum
                let fragmentum_duration_secs = samples_to_process.len() as f32
                    / (wav_config.sample_rate as f32 * wav_config.channels as f32);

                // Calculate timestamps
                let timestamp_start = state.recording_offset_secs;
                let timestamp_end = state.recording_offset_secs + fragmentum_duration_secs;

                // Save fragmentum
                let fragmentum_path = Self::save_fragmentum(
                    &wav_config,
                    samples_to_process,
                    state.start_datetime,
                    &output_dir,
                )
                .with_context(|| "Failed to save recording")?;

                // Create queue element with timestamps
                let fragmentum_queue_element = FragmentumToTranscribe {
                    path: fragmentum_path,
                    start_datetime: state.start_datetime,
                    timestamp_start,
                    timestamp_end,
                };

                // Add to queue
                tx.send(fragmentum_queue_element)
                    .with_context(|| "Failed to send to transcription queue")?;

                // Update the cumulative offset for the next fragmentum
                state.recording_offset_secs = timestamp_end;

                // Reset states for next fragmentum (but not the offset)
                state.reset(); // Reset state of fractor buffer
                vad.reset(); // Reset LSTM state for new segment
            }
        }

        // Flush any remaining buffer content before stopping
        Self::flush_current_buffer(
            &mut recorder,
            &mut state,
            &mut vad,
            &wav_config,
            &output_dir,
            &tx,
        )
        .with_context(|| "Failed to flush buffer")?;

        // Stop the recording
        Self::stop_recording(&mut recorder).with_context(|| "Failed to stop recording")?;

        // Return the temp directory to clean up after transcription completes (if any),
        // along with the VAD model for reuse
        if erase {
            Ok((Some(output_dir), vad))
        } else {
            Ok((None, vad))
        }
    }
}
