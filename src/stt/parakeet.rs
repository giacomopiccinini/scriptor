use super::audio::{read_audio_file_mono, resample};
use super::model::{STTModel, Transcription};
use super::onnx::{InferenceConfig, load_onnx_model, load_vocabulary};
use super::transcription::{TimestampGranularity, TimestampedResult, convert_timestamps};
use anyhow::{Context, Result};
use ndarray::{Array, Array1, Array2, Array3, ArrayD, ArrayViewD, IxDyn};
use once_cell::sync::Lazy;
use ort::inputs;
use ort::session::Session;
use ort::value::TensorRef;
use regex::Regex;
use std::path::Path;
use std::path::PathBuf;

/// Model-specific constants
const SUBSAMPLING_FACTOR: usize = 8;
const WINDOW_SIZE: f32 = 0.01;
const MAX_TOKENS_PER_STEP: usize = 10;
const SR: u32 = 16_000;
const BLANK_IDX: i32 = 8192;

/// Regex for decoding spaces in transcription
static DECODE_SPACE_RE: Lazy<Result<Regex, regex::Error>> =
    Lazy::new(|| Regex::new(r"\A\s|\s\B|(\s)\b"));

/// Parakeet-specific configuration
pub struct ParakeetConfig {
    pub quantized: bool,
    pub model_dir: PathBuf,
}

/// Parakeet model implementing RNN-T architecture
pub struct ParakeetModel {
    encoder: Session,
    decoder_joint: Session,
    preprocessor: Session,
    vocab: Vec<String>,
    vocab_size: usize,
}

/// Decoder state for RNN-T decoding (two hidden states)
pub type DecoderState = (Array3<f32>, Array3<f32>);

/// Core inference implementation for ParakeetModel
impl ParakeetModel {
    /// Apply preprocessing to raw waveforms
    fn preprocess(
        &mut self,
        waveforms: &ArrayViewD<f32>,
        waveforms_lens: &ArrayViewD<i64>,
    ) -> Result<(ArrayD<f32>, ArrayD<i64>)> {
        let inputs = inputs![
            "waveforms" => TensorRef::from_array_view(waveforms.view())?,
            "waveforms_lens" => TensorRef::from_array_view(waveforms_lens.view())?,
        ];
        let outputs = self.preprocessor.run(inputs)?;

        let features = outputs
            .get("features")
            .context("Preprocessor output missing 'features'")?
            .try_extract_array()?;
        let features_lens = outputs
            .get("features_lens")
            .context("Preprocessor output missing 'features_lens'")?
            .try_extract_array()?;

        Ok((features.to_owned(), features_lens.to_owned()))
    }

    /// Encode audio features using the encoder
    fn encode(
        &mut self,
        audio_signal: &ArrayViewD<f32>,
        length: &ArrayViewD<i64>,
    ) -> Result<(ArrayD<f32>, ArrayD<i64>)> {
        let inputs = inputs![
            "audio_signal" => TensorRef::from_array_view(audio_signal.view())?,
            "length" => TensorRef::from_array_view(length.view())?,
        ];
        let outputs = self.encoder.run(inputs)?;

        let encoder_output = outputs
            .get("outputs")
            .context("Encoder output missing 'outputs'")?
            .try_extract_array()?;
        let encoded_lengths = outputs
            .get("encoded_lengths")
            .context("Encoder output missing 'encoded_lengths'")?
            .try_extract_array()?;

        // Permute axes to match expected shape
        let encoder_output = encoder_output.permuted_axes(IxDyn(&[0, 2, 1]));

        Ok((encoder_output.to_owned(), encoded_lengths.to_owned()))
    }

    /// Initialize decoder state with zeros
    fn create_decoder_state(&self) -> Result<DecoderState> {
        let inputs = &self.decoder_joint.inputs;

        // Extract shapes from model inputs
        let state1_shape = inputs
            .iter()
            .find(|input| input.name == "input_states_1")
            .context("Decoder missing 'input_states_1' input")?
            .input_type
            .tensor_shape()
            .context("Cannot extract tensor shape for 'input_states_1'")?;

        let state2_shape = inputs
            .iter()
            .find(|input| input.name == "input_states_2")
            .context("Decoder missing 'input_states_2' input")?
            .input_type
            .tensor_shape()
            .context("Cannot extract tensor shape for 'input_states_2'")?;

        // Create zero-initialized states for batch_size=1
        let state1 = Array::zeros((state1_shape[0] as usize, 1, state1_shape[2] as usize));

        let state2 = Array::zeros((state2_shape[0] as usize, 1, state2_shape[2] as usize));

        Ok((state1, state2))
    }

    /// Single decoding step for RNN-T
    fn decode_step(
        &mut self,
        prev_tokens: &[i32],
        prev_state: &DecoderState,
        encoder_out: &ArrayViewD<f32>,
    ) -> Result<(ArrayD<f32>, DecoderState)> {
        // Get last token or use blank if no previous tokens
        let target_token = prev_tokens.last().copied().unwrap_or(BLANK_IDX);

        // Prepare inputs with proper shapes
        let encoder_outputs = encoder_out
            .to_owned()
            .insert_axis(ndarray::Axis(0))
            .insert_axis(ndarray::Axis(2));
        let targets = Array2::from_shape_vec((1, 1), vec![target_token])?;
        let target_length = Array1::from_vec(vec![1]);

        let inputs = inputs![
            "encoder_outputs" => TensorRef::from_array_view(encoder_outputs.view())?,
            "targets" => TensorRef::from_array_view(targets.view())?,
            "target_length" => TensorRef::from_array_view(target_length.view())?,
            "input_states_1" => TensorRef::from_array_view(prev_state.0.view())?,
            "input_states_2" => TensorRef::from_array_view(prev_state.1.view())?,
        ];

        let outputs = self.decoder_joint.run(inputs)?;

        let logits = outputs
            .get("outputs")
            .context("Decoder output missing 'outputs'")?
            .try_extract_array()?;
        let state1 = outputs
            .get("output_states_1")
            .context("Decoder output missing 'output_states_1'")?
            .try_extract_array()?;
        let state2 = outputs
            .get("output_states_2")
            .context("Decoder output missing 'output_states_2'")?
            .try_extract_array()?;

        // Remove batch dimension from logits
        let logits = logits.remove_axis(ndarray::Axis(0));

        // Convert states to correct dimensionality
        let state1_3d = state1.to_owned().into_dimensionality::<ndarray::Ix3>()?;
        let state2_3d = state2.to_owned().into_dimensionality::<ndarray::Ix3>()?;

        Ok((logits.to_owned(), (state1_3d, state2_3d)))
    }

    /// Decode encoded sequence into tokens using greedy search
    fn decode_sequence(
        &mut self,
        encodings: &ArrayViewD<f32>,
        encodings_len: usize,
    ) -> Result<(Vec<i32>, Vec<usize>)> {
        let mut prev_state = self.create_decoder_state()?;
        let mut tokens = Vec::new();
        let mut timestamps = Vec::new();

        let mut t = 0;
        let mut emitted_tokens = 0;

        while t < encodings_len {
            let encoder_step = encodings.slice(ndarray::s![t, ..]);
            let encoder_step_dyn = encoder_step.to_owned().into_dyn();
            let (probs, new_state) =
                self.decode_step(&tokens, &prev_state, &encoder_step_dyn.view())?;

            // Extract vocabulary logits (handle both RNN-T and TDT models)
            let vocab_logits_slice = probs.as_slice().context("Cannot convert logits to slice")?;

            let vocab_logits = if probs.len() > self.vocab_size {
                // TDT model: only use first vocab_size elements
                &vocab_logits_slice[..self.vocab_size]
            } else {
                // Regular RNN-T model
                vocab_logits_slice
            };

            // Greedy decoding: select token with highest probability
            let token = vocab_logits
                .iter()
                .enumerate()
                .max_by(|(_, a), (_, b)| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal))
                .map(|(idx, _)| idx as i32)
                .unwrap_or(BLANK_IDX);

            // Update state and tokens if non-blank
            if token != BLANK_IDX {
                prev_state = new_state;
                tokens.push(token);
                timestamps.push(t);
                emitted_tokens += 1;
            }

            // Advance time step
            if token == BLANK_IDX || emitted_tokens == MAX_TOKENS_PER_STEP {
                t += 1;
                emitted_tokens = 0;
            }
        }

        Ok((tokens, timestamps))
    }

    /// Convert token IDs to text with timestamps
    fn decode_tokens(&self, ids: Vec<i32>, timestamps: Vec<usize>) -> TimestampedResult {
        // Map token IDs to strings
        let tokens: Vec<String> = ids
            .iter()
            .filter_map(|&id| {
                let idx = id as usize;
                if idx < self.vocab.len() {
                    Some(self.vocab[idx].clone())
                } else {
                    None
                }
            })
            .collect();

        // Apply regex to clean up spaces
        let text = match &*DECODE_SPACE_RE {
            Ok(regex) => regex
                .replace_all(&tokens.join(""), |caps: &regex::Captures| {
                    if caps.get(1).is_some() { " " } else { "" }
                })
                .to_string(),
            Err(_) => tokens.join(""),
        };

        // Convert frame indices to timestamps in seconds
        let float_timestamps: Vec<f32> = timestamps
            .iter()
            .map(|&t| WINDOW_SIZE * SUBSAMPLING_FACTOR as f32 * t as f32)
            .collect();

        TimestampedResult {
            text,
            timestamps: float_timestamps,
            tokens,
        }
    }

    /// Process a batch of waveforms and return timestamped results
    fn recognize_batch(
        &mut self,
        waveforms: &ArrayViewD<f32>,
        waveforms_len: &ArrayViewD<i64>,
    ) -> Result<Vec<TimestampedResult>> {
        // Preprocess and encode
        let (features, features_lens) = self.preprocess(waveforms, waveforms_len)?;
        let (encoder_out, encoder_out_lens) =
            self.encode(&features.view(), &features_lens.view())?;

        // Decode each item in batch
        let mut results = Vec::new();
        for (encodings, &encodings_len) in encoder_out.outer_iter().zip(encoder_out_lens.iter()) {
            let (tokens, timestamps) =
                self.decode_sequence(&encodings.view(), encodings_len as usize)?;
            let result = self.decode_tokens(tokens, timestamps);
            results.push(result);
        }

        Ok(results)
    }

    /// **FORWARD METHOD**: Main inference method for transcribing audio samples
    ///
    /// Takes raw audio samples and returns detailed token-level transcription
    pub fn transcribe_samples(&mut self, samples: Vec<f32>) -> Result<TimestampedResult> {
        let samples_len = samples.len();

        // Create batched input arrays (batch size determined by input)
        let waveforms = Array2::from_shape_vec((1, samples_len), samples)?.into_dyn();
        let waveforms_lens = Array1::from_vec(vec![samples_len as i64]).into_dyn();

        // Run batched recognition
        let results = self.recognize_batch(&waveforms.view(), &waveforms_lens.view())?;

        // Extract single result
        results
            .into_iter()
            .next()
            .context("No transcription result returned from model")
    }
}

/// Implement STTModel trait for Parakeet
impl STTModel for ParakeetModel {
    type ModelConfig = ParakeetConfig;

    fn new(model_config: Self::ModelConfig, inference_config: InferenceConfig) -> Result<Self> {
        // Determine model paths based on quantization
        let encoder_path = if model_config.quantized {
            model_config.model_dir.join("encoder-model.int8.onnx")
        } else {
            model_config.model_dir.join("encoder-model.onnx")
        };
        let decoder_joint_path = if model_config.quantized {
            model_config.model_dir.join("decoder_joint-model.int8.onnx")
        } else {
            model_config.model_dir.join("decoder_joint-model.onnx")
        };

        // Load ONNX models
        let encoder = load_onnx_model(encoder_path, inference_config.clone())
            .with_context(|| "Failed to load encoder")?;
        let decoder_joint = load_onnx_model(decoder_joint_path, inference_config.clone())
            .with_context(|| "Failed to load decoder joint")?;
        let preprocessor = load_onnx_model(
            model_config.model_dir.join("nemo128.onnx"),
            inference_config.clone(),
        )
        .with_context(|| "Failed to load preprocessor")?;

        // Load vocabulary
        let vocab = load_vocabulary(model_config.model_dir.join("vocabulary.txt"))
            .with_context(|| "Failed to load vocabulary")?;
        let vocab_size = vocab.len();

        Ok(ParakeetModel {
            encoder,
            decoder_joint,
            preprocessor,
            vocab,
            vocab_size,
        })
    }

    fn load_audio(&self, audio_path: &Path) -> Result<Vec<f32>> {
        // Read audio file as mono
        let (mut audio, original_sr) =
            read_audio_file_mono(audio_path).with_context(|| "Failed to read audio file")?;

        // Resample to model's expected sample rate
        if original_sr != SR {
            audio = resample(audio, original_sr, SR).with_context(|| "Failed to resample audio")?;
        }

        Ok(audio)
    }

    fn transcribe(&mut self, audio_samples: Vec<f32>) -> Result<Transcription> {
        // Get token-level transcription
        let timestamped_result = self.transcribe_samples(audio_samples)?;

        // Convert to token-level segments (default granularity)
        let segments = convert_timestamps(&timestamped_result, TimestampGranularity::Token);

        Ok(Transcription {
            text: timestamped_result.text,
            segments: Some(segments),
        })
    }
}
