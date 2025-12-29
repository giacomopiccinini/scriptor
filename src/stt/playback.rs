use crate::stt::audio::{
    convert_to_mono, convert_to_stereo, read_audio, resample_stereo, wav_spec_from_config,
};
use anyhow::{Context, Result};
use cpal::Stream;
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{Device, SampleFormat, SupportedStreamConfig};
use hound::WavSpec;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, RwLock};

/// Playback queue with double-buffering for gapless transitions
pub struct PlayerQueue {
    /// List of all file paths
    pub files: Vec<PathBuf>,

    /// Config for output based on device, needed to preload correctly the audio files
    pub output_config: WavSpec,

    /// Index of the currently playing file
    pub current_file_index: AtomicUsize,

    /// Currently playing audio
    pub active_audio: Arc<RwLock<Option<Vec<f32>>>>,

    /// Pre-loaded next audio file
    pub next_audio: Arc<RwLock<Option<Vec<f32>>>>,

    /// Playback position within active buffer (sample index)
    pub playback_position: Arc<AtomicUsize>,
}

impl PlayerQueue {
    /// Create a queue, possibly with files
    pub fn new(output_config: WavSpec, audio_files: Option<Vec<PathBuf>>) -> Result<Self> {
        let files = audio_files.unwrap_or_default();

        let queue = Self {
            files,
            output_config,
            current_file_index: AtomicUsize::new(0),
            active_audio: Arc::new(RwLock::new(None)),
            next_audio: Arc::new(RwLock::new(None)),
            playback_position: Arc::new(AtomicUsize::new(0)),
        };

        // If we have files, load the first one and preload next
        if !queue.files.is_empty() {
            queue.load_current_and_preload()?;
        }

        Ok(queue)
    }

    /// Add a file to the queue
    pub fn push(&mut self, path: PathBuf) {
        self.files.push(path);
    }

    /// Get the number of files in the queue
    pub fn len(&self) -> usize {
        self.files.len()
    }

    /// Check if queue is empty
    pub fn is_empty(&self) -> bool {
        self.files.is_empty()
    }

    /// Get current audio file index
    pub fn current(&self) -> usize {
        self.current_file_index.load(Ordering::SeqCst)
    }

    /// Check if playback has finished (all tracks played)
    pub fn is_finished(&self) -> bool {
        let current_idx = self.current_file_index.load(Ordering::SeqCst);
        let position = self.playback_position.load(Ordering::SeqCst);

        let is_last_file = current_idx >= self.files.len().saturating_sub(1);

        let audio_finished = if let Ok(guard) = self.active_audio.read() {
            match &*guard {
                Some(audio) => position >= audio.len(),
                None => true,
            }
        } else {
            false
        };

        is_last_file && audio_finished
    }

    /// Load current file into active_audio and preload the next one
    pub fn load_current_and_preload(&self) -> Result<()> {
        let idx = self.current_file_index.load(Ordering::SeqCst);
        let audio = Self::load_file(&self.files[idx], &self.output_config)
            .with_context(|| "Unable to load current audio file")?;
        *self
            .active_audio
            .write()
            .map_err(|_| anyhow::anyhow!("Audio lock poisoned"))? = Some(audio);
        self.preload_next()?;
        Ok(())
    }

    /// Load audio file and prepare it for queue matching the output sr and n channels.
    /// Bit depth will be handled when building the stream
    pub fn load_file(path: &PathBuf, output_config: &WavSpec) -> Result<Vec<f32>> {
        // Load audio file as is
        let (mut samples, wav_config) =
            read_audio(path).with_context(|| "Unable to read audio file")?;

        // Fix the number of channels
        if wav_config.channels != output_config.channels {
            // Need to make a mono file actually stereo
            if wav_config.channels < output_config.channels {
                samples = convert_to_stereo(samples, 1)
            }
            // Stereo to mono
            else {
                samples = convert_to_mono(samples, 2);
            }
        }

        // Resample
        samples = resample_stereo(
            samples,
            wav_config.sample_rate,
            output_config.sample_rate,
            output_config.channels,
        )
        .with_context(|| "Resampling for playback failed")?;

        Ok(samples)
    }

    /// Jump to a specific audio file
    pub fn jump_to(&self, index: usize) -> Result<()> {
        if index >= self.files.len() {
            anyhow::bail!("Audio file index for playback out of bounds");
        }
        self.current_file_index.store(index, Ordering::SeqCst);
        self.playback_position.store(0, Ordering::SeqCst);
        self.load_current_and_preload()
    }

    /// Preload the next audio file in background
    fn preload_next(&self) -> Result<()> {
        // Get the index of the next audio file
        let next_idx = self.current_file_index.load(Ordering::SeqCst) + 1;

        // If we are already at the last file, we simply say that the next audio file is none and we exit the function
        if next_idx >= self.files.len() {
            *self
                .next_audio
                .write()
                .map_err(|_| anyhow::anyhow!("Audio lock poisoned"))? = None;
            return Ok(());
        }

        // Clone needed objects because we'll need to pass them to a new thread which will take ownership
        let next_file_path = self.files[next_idx].clone();
        let next_audio = self.next_audio.clone();
        let output_config = self.output_config.clone();

        // Spawn loading on a separate thread
        std::thread::spawn(move || {
            if let Ok(audio) = Self::load_file(&next_file_path, &output_config) {
                *next_audio
                    .write()
                    .expect("Audio lock poisoned loading next audio") = Some(audio);
            }
        });

        Ok(())
    }

    /// Called when current track ends to swap buffers
    fn advance(&self) -> Result<()> {
        let mut active_audio = self
            .active_audio
            .write()
            .map_err(|_| anyhow::anyhow!("Cannot acquire lock on active audio"))?;
        let mut next_audio = self
            .next_audio
            .write()
            .map_err(|_| anyhow::anyhow!("Cannot acquire lock on next audio"))?;

        // Swap: preloaded becomes active
        *active_audio = next_audio.take();
        self.playback_position.store(0, Ordering::SeqCst);
        self.current_file_index.fetch_add(1, Ordering::SeqCst);

        drop(active_audio);
        drop(next_audio);

        // Start preloading the next one
        self.preload_next()
            .with_context(|| "Can't preload next audio file while advancing")?;

        Ok(())
    }
}

pub struct PlayerConfig {
    output_device: Device,
    output_config: SupportedStreamConfig,
    pub wav_config: WavSpec,
}

impl PlayerConfig {
    fn new() -> Result<Self> {
        // Set up playback device
        let (output_device, output_config) =
            setup_player().with_context(|| "Unable to set up player")?;

        // Convert the input config (cpal-style) into wav config (hound-style)
        let wav_config = wav_spec_from_config(&output_config);

        Ok(Self {
            output_device,
            output_config,
            wav_config,
        })
    }
}

/// Setup the player by finding the default device with the corresponding config
fn setup_player() -> Result<(Device, SupportedStreamConfig)> {
    // Get default audio host and output device
    let host = cpal::default_host();
    let output_device = host
        .default_output_device()
        .with_context(|| "Unable to find default output device")?;

    // Use default configuration for audio output.
    let output_config = output_device
        .default_output_config()
        .with_context(|| "Unable to find default output config")?;

    Ok((output_device, output_config))
}

pub struct Player {
    pub stream: Option<Stream>,
    pub config: PlayerConfig,
    pub queue: PlayerQueue,
    pub is_playing: bool,
}

impl Player {
    pub fn new(audio_files: Option<Vec<PathBuf>>) -> Result<Self> {
        // Call construct for player config
        let config = PlayerConfig::new().with_context(|| "Unable to create player config")?;

        // Create the queue (will auto-load files if provided)
        let queue = PlayerQueue::new(config.wav_config, audio_files)
            .with_context(|| "Unable to create player queue")?;

        // Create player first with no stream
        let mut player = Self {
            stream: None,
            config,
            queue,
            is_playing: false,
        };

        // Set up stream if we have files loaded
        if !player.queue.is_empty() {
            let stream = player.setup_output_stream(
                player.config.output_config.sample_format(),
                player.queue.active_audio.clone(),
                player.queue.playback_position.clone(),
            )?;
            player.stream = Some(stream);
        }

        Ok(player)
    }

    pub fn setup_output_stream(
        &self,
        sample_format: SampleFormat,
        samples: Arc<RwLock<Option<Vec<f32>>>>,
        position: Arc<AtomicUsize>,
    ) -> Result<cpal::Stream> {
        match sample_format {
            SampleFormat::F32 => self.config.output_device.build_output_stream(
                &self.config.output_config.config(),
                move |data: &mut [f32], _| {
                    fill_buffer_f32(data, &samples, &position);
                },
                |e| eprintln!("{e}"),
                None,
            ),
            SampleFormat::I16 => self.config.output_device.build_output_stream(
                &self.config.output_config.config(),
                move |data: &mut [i16], _| {
                    fill_buffer_i16(data, &samples, &position);
                },
                |e| eprintln!("{e}"),
                None,
            ),
            SampleFormat::I32 => self.config.output_device.build_output_stream(
                &self.config.output_config.config(),
                move |data: &mut [i32], _| {
                    fill_buffer_i32(data, &samples, &position);
                },
                |e| eprintln!("{e}"),
                None,
            ),
            _ => anyhow::bail!("Unsupported sample format"),
        }
        .with_context(|| "Failed to build output stream")
    }

    /// Start playback
    pub fn play(&mut self) -> Result<()> {
        if let Some(ref stream) = self.stream {
            stream
                .play()
                .with_context(|| "Unable to start the stream")?;
            self.is_playing = true;
        }
        Ok(())
    }

    /// Pause playback (keeps current position)
    pub fn pause(&mut self) -> Result<()> {
        if let Some(ref stream) = self.stream {
            stream
                .pause()
                .with_context(|| "Unable to pause the stream")?;
            self.is_playing = false;
        }
        Ok(())
    }

    /// Toggle between play and pause
    pub fn toggle_playback(&mut self) -> Result<()> {
        if self.is_playing {
            self.pause()
        } else {
            self.play()
        }
    }
}

fn fill_buffer_f32(
    data: &mut [f32],
    samples: &Arc<RwLock<Option<Vec<f32>>>>,
    pos: &Arc<AtomicUsize>,
) {
    if let Some(ref audio) = *samples.read().unwrap() {
        let p = pos.load(Ordering::SeqCst);
        for (i, out) in data.iter_mut().enumerate() {
            *out = audio.get(p + i).copied().unwrap_or(0.0);
        }
        pos.fetch_add(data.len(), Ordering::SeqCst);
    } else {
        data.fill(0.0);
    }
}

fn fill_buffer_i16(
    data: &mut [i16],
    samples: &Arc<RwLock<Option<Vec<f32>>>>,
    pos: &Arc<AtomicUsize>,
) {
    if let Some(ref audio) = *samples.read().unwrap() {
        let p = pos.load(Ordering::SeqCst);
        for (i, out) in data.iter_mut().enumerate() {
            let s = audio.get(p + i).copied().unwrap_or(0.0);
            *out = (s.clamp(-1.0, 1.0) * i16::MAX as f32) as i16;
        }
        pos.fetch_add(data.len(), Ordering::SeqCst);
    } else {
        data.fill(0);
    }
}

fn fill_buffer_i32(
    data: &mut [i32],
    samples: &Arc<RwLock<Option<Vec<f32>>>>,
    pos: &Arc<AtomicUsize>,
) {
    if let Some(ref audio) = *samples.read().unwrap() {
        let p = pos.load(Ordering::SeqCst);
        for (i, out) in data.iter_mut().enumerate() {
            let s = audio.get(p + i).copied().unwrap_or(0.0);
            *out = (s.clamp(-1.0, 1.0) * i32::MAX as f32) as i32;
        }
        pos.fetch_add(data.len(), Ordering::SeqCst);
    } else {
        data.fill(0);
    }
}
