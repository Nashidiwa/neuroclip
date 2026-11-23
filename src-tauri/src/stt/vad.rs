//! Silero VAD (Voice Activity Detection) Engine (`TASK-M4-14`).
//!
//! Provides high-performance voice activity detection operating on 512-sample (32ms at 16kHz) windows,
//! silence/music pruning for $> 1.5$s non-speech segments (`AC-14.2`), and natural pause chunk splitting
//! for Groq cloud API payload constraints (`AC-14.3`).

#![allow(
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss
)]

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use tract_onnx::prelude::*;

use crate::error::{AppError, SttError};
use crate::stt::vad_bridge::SpeechInterval;

/// Embedded static 16kHz Silero VAD ONNX model bytes (~1.28MB).
pub const EMBEDDED_SILERO_VAD_MODEL: &[u8] =
    include_bytes!("../../resources/models/silero_vad_16k.onnx");

/// Default VAD audio sampling rate in Hz (16,000 Hz standard for Silero & Whisper).
pub const DEFAULT_VAD_SAMPLE_RATE: usize = 16_000;

/// Default window frame size in samples (512 samples = 32ms at 16,000 Hz).
pub const DEFAULT_WINDOW_SIZE_SAMPLES: usize = 512;

/// Default speech onset detection probability threshold (0.50).
pub const DEFAULT_THRESHOLD: f32 = 0.50;

/// Default speech offset release probability threshold (0.35).
pub const DEFAULT_NEG_THRESHOLD: f32 = 0.35;

/// Minimum speech burst duration in milliseconds to be considered valid speech (250ms).
pub const DEFAULT_MIN_SPEECH_DURATION_MS: u64 = 250;

/// Minimum silence duration in milliseconds between adjacent speech blocks before bridging (300ms).
pub const DEFAULT_MIN_SILENCE_DURATION_MS: u64 = 300;

/// Maximum silence gap in milliseconds allowed before pruning (1,500ms = 1.5s per AC-14.2).
pub const DEFAULT_MAX_SILENCE_GAP_MS: u64 = 1_500;

/// Padding in milliseconds added before onset and after offset of speech (100ms).
pub const DEFAULT_SPEECH_PAD_MS: u64 = 100;

/// Speech activity segment with millisecond timestamps.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct SpeechSegment {
    /// Start timestamp of speech in milliseconds.
    pub start_ms: u64,
    /// End timestamp of speech in milliseconds.
    pub end_ms: u64,
}

impl SpeechSegment {
    /// Create a new speech segment with validated boundaries.
    #[must_use]
    pub fn new(start_ms: u64, end_ms: u64) -> Self {
        Self {
            start_ms,
            end_ms: end_ms.max(start_ms),
        }
    }

    /// Calculate the duration of the speech segment in milliseconds.
    #[must_use]
    pub fn duration_ms(&self) -> u64 {
        self.end_ms.saturating_sub(self.start_ms)
    }

    /// Convert segment to seconds-based `SpeechInterval` for `vad_bridge.rs`.
    #[must_use]
    pub fn to_speech_interval(&self) -> SpeechInterval {
        SpeechInterval {
            start_sec: self.start_ms as f64 / 1000.0,
            end_sec: self.end_ms as f64 / 1000.0,
        }
    }
}

impl From<&SpeechSegment> for SpeechInterval {
    fn from(s: &SpeechSegment) -> Self {
        s.to_speech_interval()
    }
}

impl From<SpeechSegment> for SpeechInterval {
    fn from(s: SpeechSegment) -> Self {
        s.to_speech_interval()
    }
}

impl From<&SpeechInterval> for SpeechSegment {
    fn from(i: &SpeechInterval) -> Self {
        Self {
            start_ms: (i.start_sec * 1000.0).round().max(0.0) as u64,
            end_ms: (i.end_sec * 1000.0).round().max(0.0) as u64,
        }
    }
}

impl From<SpeechInterval> for SpeechSegment {
    fn from(i: SpeechInterval) -> Self {
        Self::from(&i)
    }
}

/// Configuration parameters for Silero VAD detection and silence pruning.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct VadConfig {
    /// Audio sampling rate in Hz (default: 16,000 Hz).
    pub sample_rate: usize,
    /// Window frame size in samples (default: 512 samples = 32ms).
    pub window_size_samples: usize,
    /// Probability threshold to trigger speech onset (default: 0.50).
    pub threshold: f32,
    /// Probability threshold to trigger speech offset (default: 0.35).
    pub neg_threshold: f32,
    /// Minimum speech duration in milliseconds to be considered valid (default: 250ms).
    pub min_speech_duration_ms: u64,
    /// Minimum silence gap in milliseconds between speech before bridging (default: 300ms).
    pub min_silence_duration_ms: u64,
    /// Maximum silence duration allowed before pruning (default: 1500ms = 1.5s per AC-14.2).
    pub max_silence_gap_ms: u64,
    /// Padding added before and after speech in milliseconds (default: 100ms).
    pub speech_pad_ms: u64,
    /// Optional file path override to custom Silero VAD model (default: None -> embedded static model).
    pub model_path: Option<PathBuf>,
}

impl Default for VadConfig {
    fn default() -> Self {
        Self {
            sample_rate: DEFAULT_VAD_SAMPLE_RATE,
            window_size_samples: DEFAULT_WINDOW_SIZE_SAMPLES,
            threshold: DEFAULT_THRESHOLD,
            neg_threshold: DEFAULT_NEG_THRESHOLD,
            min_speech_duration_ms: DEFAULT_MIN_SPEECH_DURATION_MS,
            min_silence_duration_ms: DEFAULT_MIN_SILENCE_DURATION_MS,
            max_silence_gap_ms: DEFAULT_MAX_SILENCE_GAP_MS,
            speech_pad_ms: DEFAULT_SPEECH_PAD_MS,
            model_path: None,
        }
    }
}

/// Result of silence pruning operation.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct VadPruneResult {
    /// Total duration of original audio in milliseconds.
    pub original_duration_ms: u64,
    /// Total duration of pruned audio in milliseconds.
    pub pruned_duration_ms: u64,
    /// Reduction percentage in payload size (0.0 to 1.0).
    pub reduction_ratio: f64,
    /// Speech segments detected in the audio.
    pub speech_segments: Vec<SpeechSegment>,
    /// Pruned audio samples (16kHz float32).
    pub pruned_samples: Vec<f32>,
}

/// Configuration for splitting audio into safe chunks (< 20MB).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct ChunkSplitConfig {
    /// Maximum chunk payload size in bytes (default: 20MB = 20,971,520 bytes).
    pub max_chunk_size_bytes: usize,
    /// Maximum chunk duration in milliseconds (default: 10 minutes = 600,000ms).
    pub max_chunk_duration_ms: u64,
    /// Overlap duration in milliseconds placed in silence (default: 1,500ms).
    pub overlap_ms: u64,
}

impl Default for ChunkSplitConfig {
    fn default() -> Self {
        Self {
            max_chunk_size_bytes: 20 * 1024 * 1024, // 20MB
            max_chunk_duration_ms: 600_000,         // 10 minutes
            overlap_ms: 1_500,                      // 1.5 seconds
        }
    }
}

/// Audio chunk generated by natural pause splitting.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct AudioChunk {
    /// 0-based sequential chunk index.
    pub chunk_index: usize,
    /// Start timestamp relative to original audio in milliseconds.
    pub start_ms: u64,
    /// End timestamp relative to original audio in milliseconds.
    pub end_ms: u64,
    /// Payload size in bytes (assuming 16-bit PCM WAV equivalent).
    pub size_bytes: usize,
    /// Audio samples in float32 format.
    pub samples: Vec<f32>,
}

/// Silero VAD neural inference engine compiled via `tract-onnx`.
pub struct SileroVadEngine {
    plan: SimplePlan<TypedFact, Box<dyn TypedOp>, TypedModel>,
    num_inputs: usize,
    num_outputs: usize,
    context_samples: usize,
}

impl SileroVadEngine {
    /// Initialize the neural engine from the embedded static model bytes.
    pub fn from_embedded() -> Result<Self, AppError> {
        Self::from_bytes(EMBEDDED_SILERO_VAD_MODEL)
    }

    /// Initialize the neural engine from a custom ONNX file path.
    pub fn from_path(path: &Path) -> Result<Self, AppError> {
        let bytes = std::fs::read(path).map_err(|e| {
            AppError::Stt(SttError::VadProcessingFailed(format!(
                "Failed to read Silero VAD model from {}: {e}",
                path.display()
            )))
        })?;
        Self::from_bytes(&bytes)
    }

    /// Initialize the neural engine from raw bytes.
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, AppError> {
        let mut cursor = std::io::Cursor::new(bytes);
        let mut model = tract_onnx::onnx()
            .model_for_read(&mut cursor)
            .map_err(|e| {
                AppError::Stt(SttError::VadProcessingFailed(format!(
                    "Silero VAD model parsing failed: {e}"
                )))
            })?;

        let num_inputs = model.inputs.len();
        let num_outputs = model.outputs.len();

        // Constrain input facts for static 16kHz Silero v5 (2 inputs: [1, 576], [2, 1, 128])
        // or v4 (4 inputs: [1, 512], [1], [2, 1, 64], [2, 1, 64])
        let context_samples = if num_inputs == 2 {
            model
                .set_input_fact(0, InferenceFact::dt_shape(f32::datum_type(), tvec!(1, 576)))
                .map_err(|e| {
                    AppError::Stt(SttError::VadProcessingFailed(format!(
                        "Silero VAD v5 input0 fact failed: {e}"
                    )))
                })?;
            model
                .set_input_fact(
                    1,
                    InferenceFact::dt_shape(f32::datum_type(), tvec!(2, 1, 128)),
                )
                .map_err(|e| {
                    AppError::Stt(SttError::VadProcessingFailed(format!(
                        "Silero VAD v5 input1 fact failed: {e}"
                    )))
                })?;
            64
        } else {
            0
        };

        let optimized = model.into_optimized().map_err(|e| {
            AppError::Stt(SttError::VadProcessingFailed(format!(
                "Silero VAD graph optimization failed: {e}"
            )))
        })?;

        let plan = optimized.into_runnable().map_err(|e| {
            AppError::Stt(SttError::VadProcessingFailed(format!(
                "Silero VAD model runnable compilation failed: {e}"
            )))
        })?;

        Ok(Self {
            plan,
            num_inputs,
            num_outputs,
            context_samples,
        })
    }

    /// Initialize default engine: from override path if provided, else embedded model.
    pub fn init_default(path_override: &Option<PathBuf>) -> Result<Self, AppError> {
        if let Some(path) = path_override {
            Self::from_path(path)
        } else {
            Self::from_embedded()
        }
    }

    /// Create an active streaming VAD inference session with fresh RNN hidden states.
    #[must_use]
    pub fn create_session(&self) -> SileroVadSession<'_> {
        let state_len = if self.num_inputs == 2 { 256 } else { 128 };
        SileroVadSession {
            engine: self,
            h_state: vec![0.0f32; state_len],
            c_state: vec![0.0f32; 128],
            context: vec![0.0f32; self.context_samples],
            input_buf: vec![0.0f32; self.context_samples + DEFAULT_WINDOW_SIZE_SAMPLES],
        }
    }
}

/// Active streaming inference session holding recurrent LSTM hidden state context.
pub struct SileroVadSession<'a> {
    engine: &'a SileroVadEngine,
    h_state: Vec<f32>,
    c_state: Vec<f32>,
    context: Vec<f32>,
    input_buf: Vec<f32>,
}

impl SileroVadSession<'_> {
    /// Perform forward inference step on a 512-sample audio frame (16kHz).
    #[allow(clippy::too_many_lines)]
    pub fn step(&mut self, frame: &[f32]) -> Result<f32, AppError> {
        if self.engine.num_inputs == 2 {
            let needed = self.context.len() + frame.len();
            if self.input_buf.len() != needed {
                self.input_buf.resize(needed, 0.0);
            }
            self.input_buf[..self.context.len()].copy_from_slice(&self.context);
            self.input_buf[self.context.len()..].copy_from_slice(frame);

            let audio_tensor = Tensor::from_shape(&[1, needed], &self.input_buf).map_err(|e| {
                AppError::Stt(SttError::VadProcessingFailed(format!(
                    "Failed to build v5 input audio tensor: {e}"
                )))
            })?;

            let state_tensor = Tensor::from_shape(&[2, 1, 128], &self.h_state).map_err(|e| {
                AppError::Stt(SttError::VadProcessingFailed(format!(
                    "Failed to build v5 state tensor: {e}"
                )))
            })?;

            let outputs = self
                .engine
                .plan
                .run(tvec![audio_tensor.into(), state_tensor.into()])
                .map_err(|e| {
                    AppError::Stt(SttError::VadProcessingFailed(format!(
                        "Silero VAD v5 execution step failed: {e}"
                    )))
                })?;

            if outputs.len() < 2 {
                return Err(AppError::Stt(SttError::VadProcessingFailed(format!(
                    "Silero VAD v5 expected 2 outputs (prob, stateN), got {}",
                    outputs.len()
                ))));
            }

            let prob_slice = outputs[0].as_slice::<f32>().map_err(|e| {
                AppError::Stt(SttError::VadProcessingFailed(format!(
                    "Failed to extract v5 probability tensor slice: {e}"
                )))
            })?;

            let next_state_slice = outputs[1].as_slice::<f32>().map_err(|e| {
                AppError::Stt(SttError::VadProcessingFailed(format!(
                    "Failed to extract v5 stateN tensor slice: {e}"
                )))
            })?;

            if prob_slice.is_empty() || next_state_slice.len() != 256 {
                return Err(AppError::Stt(SttError::VadProcessingFailed(
                    "Invalid output dimensions from Silero VAD v5 step".into(),
                )));
            }

            let prob = prob_slice[0].clamp(0.0, 1.0);
            self.h_state.copy_from_slice(next_state_slice);
            if !self.context.is_empty() && frame.len() >= self.context.len() {
                let ctx_start = frame.len() - self.context.len();
                self.context.copy_from_slice(&frame[ctx_start..]);
            }
            return Ok(prob);
        }

        let audio_tensor = Tensor::from_shape(&[1, frame.len()], frame).map_err(|e| {
            AppError::Stt(SttError::VadProcessingFailed(format!(
                "Failed to build input audio tensor: {e}"
            )))
        })?;

        let h_tensor = Tensor::from_shape(&[2, 1, 64], &self.h_state).map_err(|e| {
            AppError::Stt(SttError::VadProcessingFailed(format!(
                "Failed to build h_state tensor: {e}"
            )))
        })?;

        let c_tensor = Tensor::from_shape(&[2, 1, 64], &self.c_state).map_err(|e| {
            AppError::Stt(SttError::VadProcessingFailed(format!(
                "Failed to build c_state tensor: {e}"
            )))
        })?;

        let sr_tensor = Tensor::from_shape(&[1], &[16_000i64]).map_err(|e| {
            AppError::Stt(SttError::VadProcessingFailed(format!(
                "Failed to build sr tensor: {e}"
            )))
        })?;

        let inputs: TVec<TValue> = if self.engine.num_inputs >= 4 {
            tvec![
                audio_tensor.into(),
                sr_tensor.into(),
                h_tensor.into(),
                c_tensor.into()
            ]
        } else {
            tvec![audio_tensor.into(), h_tensor.into(), c_tensor.into()]
        };

        let outputs = self.engine.plan.run(inputs).map_err(|e| {
            AppError::Stt(SttError::VadProcessingFailed(format!(
                "Silero VAD execution step failed: {e}"
            )))
        })?;

        if outputs.len() < 3 || self.engine.num_outputs < 3 {
            return Err(AppError::Stt(SttError::VadProcessingFailed(format!(
                "Silero VAD expected 3 outputs (prob, hn, cn), got {}",
                outputs.len()
            ))));
        }

        let prob_slice = outputs[0].as_slice::<f32>().map_err(|e| {
            AppError::Stt(SttError::VadProcessingFailed(format!(
                "Failed to extract probability tensor slice: {e}"
            )))
        })?;

        let hn_slice = outputs[1].as_slice::<f32>().map_err(|e| {
            AppError::Stt(SttError::VadProcessingFailed(format!(
                "Failed to extract hn tensor slice: {e}"
            )))
        })?;

        let cn_slice = outputs[2].as_slice::<f32>().map_err(|e| {
            AppError::Stt(SttError::VadProcessingFailed(format!(
                "Failed to extract cn tensor slice: {e}"
            )))
        })?;

        if prob_slice.is_empty() || hn_slice.len() != 128 || cn_slice.len() != 128 {
            return Err(AppError::Stt(SttError::VadProcessingFailed(
                "Invalid output dimensions from Silero VAD step".into(),
            )));
        }

        let prob = prob_slice[0].clamp(0.0, 1.0);
        self.h_state.copy_from_slice(hn_slice);
        self.c_state.copy_from_slice(cn_slice);

        Ok(prob)
    }

    /// Reset recurrent states to zero.
    pub fn reset(&mut self) {
        self.h_state.fill(0.0);
        self.c_state.fill(0.0);
        self.context.fill(0.0);
    }
}

/// Core voice activity detection on float32 PCM samples operating on 512-sample windows.
///
/// Returns speech segments with millisecond timestamps adhering to Silero VAD specifications.
pub fn detect_speech_segments(
    samples: &[f32],
    config: &VadConfig,
) -> Result<Vec<SpeechSegment>, AppError> {
    if config.sample_rate == 0 || config.window_size_samples == 0 {
        return Err(AppError::Stt(SttError::VadProcessingFailed(
            "Invalid VadConfig: sample_rate and window_size_samples must be non-zero".to_string(),
        )));
    }

    if samples.is_empty() {
        return Ok(Vec::new());
    }

    let sample_rate = config.sample_rate;
    let window_size = config.window_size_samples;
    let total_samples = samples.len();
    let total_duration_ms = ((total_samples as u64) * 1000) / (sample_rate as u64);

    let num_windows = total_samples.div_ceil(window_size);
    let ms_per_window = ((window_size as f64 * 1000.0) / sample_rate as f64).round() as u64;

    // Dual-Engine: Initialize Silero VAD neural engine with automatic pure Rust DSP fallback
    let engine = match SileroVadEngine::init_default(&config.model_path) {
        Ok(e) => Some(e),
        Err(err) => {
            tracing::warn!(
                "Silero VAD neural model initialization failed ({err}); falling back to Pure Rust DSP heuristic"
            );
            None
        }
    };
    let mut session = engine.as_ref().map(|e| e.create_session());

    let mut state_prob: f32 = 0.0;
    let mut triggered = false;
    let mut speech_start_frame: Option<usize> = None;
    let mut speech_end_frame: Option<usize> = None;
    let mut raw_segments: Vec<(u64, u64)> = Vec::new();

    // Reusable 512-sample frame buffer
    let mut frame = vec![0.0f32; window_size];

    for w in 0..num_windows {
        let start_idx = w * window_size;
        let end_idx = (start_idx + window_size).min(total_samples);
        let slice = &samples[start_idx..end_idx];

        // Fill window with zero padding if last frame is incomplete
        frame.fill(0.0);
        frame[..slice.len()].copy_from_slice(slice);

        // Compute speech probability score for 512-sample frame via Silero VAD or DSP fallback
        let prob = match session.as_mut() {
            Some(s) => match s.step(&frame) {
                Ok(p) => p,
                Err(err) => {
                    tracing::warn!(
                        "Silero VAD frame {w} inference step error ({err}); degrading to DSP fallback"
                    );
                    compute_frame_speech_prob_fallback(&frame, state_prob)
                }
            },
            None => compute_frame_speech_prob_fallback(&frame, state_prob),
        };
        state_prob = prob;

        if triggered {
            // Currently in speech
            if prob < config.neg_threshold {
                if speech_end_frame.is_none() {
                    speech_end_frame = Some(w);
                }

                if let Some(end_w) = speech_end_frame {
                    let silence_duration_ms = (w.saturating_sub(end_w) as u64) * ms_per_window;
                    if silence_duration_ms >= config.min_silence_duration_ms {
                        // Confirmed end of speech burst
                        let s_frame = speech_start_frame.unwrap_or(0);
                        let s_ms = ((s_frame * window_size) as u64 * 1000) / sample_rate as u64;
                        let e_ms = ((end_w * window_size) as u64 * 1000) / sample_rate as u64;

                        // Apply speech padding
                        let padded_start = s_ms.saturating_sub(config.speech_pad_ms);
                        let padded_end = (e_ms + config.speech_pad_ms).min(total_duration_ms);

                        if padded_end.saturating_sub(padded_start) >= config.min_speech_duration_ms
                        {
                            raw_segments.push((padded_start, padded_end));
                        }

                        triggered = false;
                        speech_start_frame = None;
                        speech_end_frame = None;
                    }
                }
            } else {
                // Speech resumed above neg_threshold; reset pending silence release
                speech_end_frame = None;
            }
        } else if prob >= config.threshold {
            triggered = true;
            speech_start_frame = Some(w);
            speech_end_frame = None;
        }
    }

    // Flush final speech burst if still active at end of audio
    if triggered {
        let s_frame = speech_start_frame.unwrap_or(0);
        let s_ms = ((s_frame * window_size) as u64 * 1000) / sample_rate as u64;
        let e_ms = total_duration_ms;

        let padded_start = s_ms.saturating_sub(config.speech_pad_ms);
        let padded_end = (e_ms + config.speech_pad_ms).min(total_duration_ms);

        if padded_end.saturating_sub(padded_start) >= config.min_speech_duration_ms {
            raw_segments.push((padded_start, padded_end));
        }
    }

    // Merge adjacent segments separated by <= min_silence_duration_ms
    let merged_segments = merge_speech_segments(&raw_segments, config.min_silence_duration_ms);
    Ok(merged_segments)
}

/// Fallback DSP speech probability calculation (`compute_frame_speech_prob_fallback`).
///
/// Classical multi-band DSP heuristic functioning as automatic fallback
/// when the Silero VAD neural model is uninitialized or fails inference.
pub fn compute_frame_speech_prob_fallback(frame: &[f32], prev_prob: f32) -> f32 {
    let n = frame.len();
    if n == 0 {
        return 0.0;
    }

    // 1. Short-Time Energy & RMS
    let mut sum_sq = 0.0f64;
    let mut zcr_count = 0usize;

    for i in 0..n {
        let s = f64::from(frame[i]);
        sum_sq += s * s;
        if i > 0
            && ((frame[i] >= 0.0 && frame[i - 1] < 0.0) || (frame[i] < 0.0 && frame[i - 1] >= 0.0))
        {
            zcr_count += 1;
        }
    }

    let rms = (sum_sq / n as f64).sqrt();
    let dbfs = if rms <= 1e-7 {
        -96.0f64
    } else {
        (20.0 * rms.log10()).clamp(-96.0, 0.0)
    };

    // Low noise floor threshold: digital silence or ambient hiss
    if dbfs < -48.0 {
        return prev_prob * 0.4;
    }

    // 2. Zero-Crossing Rate normalized in [0.0, 1.0]
    let zcr = zcr_count as f64 / (n as f64);

    // 3. Low-Frequency Energy Ratio (voice fundamental + formants 200Hz - 3400Hz)
    // Lowpass moving average filter y[n] = (x[n] + x[n-1]) / 2
    let mut lp_sum_sq = 0.0f64;
    for i in 1..n {
        let lp = f64::midpoint(f64::from(frame[i]), f64::from(frame[i - 1]));
        lp_sum_sq += lp * lp;
    }
    let lp_ratio = if sum_sq > 1e-9 {
        (lp_sum_sq / sum_sq).clamp(0.0, 1.0)
    } else {
        0.0
    };

    // Energy factor: scale between -48 dBFS and -16 dBFS
    let energy_factor = (((dbfs + 48.0) / 28.0) as f32).clamp(0.0, 1.0);

    // Formant factor: voiced human speech has high lowpass ratio and moderate ZCR
    let is_voice_zcr = (0.015..=0.32).contains(&zcr);
    let formant_factor = if is_voice_zcr && lp_ratio > 0.45 {
        1.0f32
    } else if lp_ratio > 0.30 {
        0.5f32
    } else {
        0.1f32
    };

    let raw_prob = (energy_factor * 0.65 + formant_factor * 0.35).clamp(0.0, 1.0);

    // Autoregressive temporal smoothing (simulating Silero RNN hidden state update)
    let smoothed = 0.60 * prev_prob + 0.40 * raw_prob;
    smoothed.clamp(0.0, 1.0)
}

/// Merge adjacent or overlapping speech segments separated by `<= min_silence_ms`.
fn merge_speech_segments(raw: &[(u64, u64)], min_silence_ms: u64) -> Vec<SpeechSegment> {
    if raw.is_empty() {
        return Vec::new();
    }

    let mut merged = Vec::with_capacity(raw.len());
    let mut current_start = raw[0].0;
    let mut current_end = raw[0].1;

    for &(start, end) in &raw[1..] {
        if start <= current_end + min_silence_ms {
            // Gap is short enough to bridge; merge
            current_end = current_end.max(end);
        } else {
            merged.push(SpeechSegment::new(current_start, current_end));
            current_start = start;
            current_end = end;
        }
    }

    merged.push(SpeechSegment::new(current_start, current_end));
    merged
}

/// Prune silence and non-speech intervals exceeding 1.5 seconds (`AC-14.2`, `UT-14.2`).
///
/// Strips out long gaps while preserving natural cadence (short pauses <= 1.5s are retained).
#[allow(clippy::too_many_lines)]
pub fn prune_silence(samples: &[f32], config: &VadConfig) -> Result<VadPruneResult, AppError> {
    if samples.is_empty() {
        return Ok(VadPruneResult {
            original_duration_ms: 0,
            pruned_duration_ms: 0,
            reduction_ratio: 0.0,
            speech_segments: Vec::new(),
            pruned_samples: Vec::new(),
        });
    }

    let sample_rate = config.sample_rate;
    let total_duration_ms = ((samples.len() as u64) * 1000) / (sample_rate as u64);
    let speech_segments = detect_speech_segments(samples, config)?;

    if speech_segments.is_empty() {
        // No speech detected: return trimmed minimal comfort silence buffer (200ms)
        let minimal_samples = (sample_rate / 5).min(samples.len()); // 200ms
        let pruned_samples = samples[..minimal_samples].to_vec();
        let pruned_duration_ms = ((pruned_samples.len() as u64) * 1000) / (sample_rate as u64);
        let reduction_ratio = if total_duration_ms > 0 {
            (total_duration_ms.saturating_sub(pruned_duration_ms) as f64)
                / (total_duration_ms as f64)
        } else {
            0.0
        };

        return Ok(VadPruneResult {
            original_duration_ms: total_duration_ms,
            pruned_duration_ms,
            reduction_ratio,
            speech_segments: Vec::new(),
            pruned_samples,
        });
    }

    // Build retained intervals: all speech segments + silence gaps <= max_silence_gap_ms (1500ms)
    // For silence gaps > max_silence_gap_ms, retain only a small transition cushion (100ms each side)
    let transition_cushion_ms = 100u64;
    let mut retained_intervals: Vec<(u64, u64)> = Vec::new();

    // 1. Initial silence before first segment
    let first_start = speech_segments[0].start_ms;
    if first_start > 0 {
        if first_start <= config.max_silence_gap_ms {
            retained_intervals.push((0, first_start));
        } else {
            // Keep only transition cushion before speech
            let keep_start = first_start.saturating_sub(transition_cushion_ms);
            retained_intervals.push((keep_start, first_start));
        }
    }

    // 2. Inter-segment gaps and speech
    for i in 0..speech_segments.len() {
        let seg = &speech_segments[i];
        retained_intervals.push((seg.start_ms, seg.end_ms));

        if i + 1 < speech_segments.len() {
            let next_seg = &speech_segments[i + 1];
            let gap_start = seg.end_ms;
            let gap_end = next_seg.start_ms;

            if gap_end > gap_start {
                let gap_duration = gap_end - gap_start;
                if gap_duration <= config.max_silence_gap_ms {
                    // Retain full short pause
                    retained_intervals.push((gap_start, gap_end));
                } else {
                    // Gap > 1.5s: prune excess silence, keeping only transition cushions
                    let post_cushion_end = (gap_start + transition_cushion_ms).min(gap_end);
                    retained_intervals.push((gap_start, post_cushion_end));

                    let pre_cushion_start = gap_end
                        .saturating_sub(transition_cushion_ms)
                        .max(post_cushion_end);
                    retained_intervals.push((pre_cushion_start, gap_end));
                }
            }
        }
    }

    // 3. Trailing silence after last segment
    let last_end = speech_segments[speech_segments.len() - 1].end_ms;
    if last_end < total_duration_ms {
        let trailing_duration = total_duration_ms - last_end;
        if trailing_duration <= config.max_silence_gap_ms {
            retained_intervals.push((last_end, total_duration_ms));
        } else {
            let keep_end = (last_end + transition_cushion_ms).min(total_duration_ms);
            retained_intervals.push((last_end, keep_end));
        }
    }

    // Merge adjacent retained intervals to avoid duplicate copies
    let mut merged_intervals: Vec<(u64, u64)> = Vec::new();
    if !retained_intervals.is_empty() {
        let mut c_start = retained_intervals[0].0;
        let mut c_end = retained_intervals[0].1;

        for &(s, e) in &retained_intervals[1..] {
            if s <= c_end {
                c_end = c_end.max(e);
            } else {
                merged_intervals.push((c_start, c_end));
                c_start = s;
                c_end = e;
            }
        }
        merged_intervals.push((c_start, c_end));
    }

    // Extract pruned samples
    let mut pruned_samples = Vec::new();
    for &(start_ms, end_ms) in &merged_intervals {
        let start_sample = ((start_ms as usize) * sample_rate) / 1000;
        let end_sample = (((end_ms as usize) * sample_rate) / 1000).min(samples.len());
        if start_sample < end_sample {
            pruned_samples.extend_from_slice(&samples[start_sample..end_sample]);
        }
    }

    let pruned_duration_ms = ((pruned_samples.len() as u64) * 1000) / (sample_rate as u64);
    let reduction_ratio = if total_duration_ms > 0 {
        (total_duration_ms.saturating_sub(pruned_duration_ms) as f64) / (total_duration_ms as f64)
    } else {
        0.0
    };

    Ok(VadPruneResult {
        original_duration_ms: total_duration_ms,
        pruned_duration_ms,
        reduction_ratio,
        speech_segments,
        pruned_samples,
    })
}

/// Split audio into natural pause chunks (< 20MB) without cutting words (`AC-14.3`, `UT-14.3`).
///
/// Cuts are strictly positioned at the midpoint of silence gaps between speech segments.
#[allow(clippy::too_many_lines)]
pub fn split_into_natural_pause_chunks(
    samples: &[f32],
    speech_segments: &[SpeechSegment],
    split_config: &ChunkSplitConfig,
    sample_rate: usize,
) -> Result<Vec<AudioChunk>, AppError> {
    if samples.is_empty() {
        return Ok(Vec::new());
    }

    let sr = if sample_rate == 0 {
        DEFAULT_VAD_SAMPLE_RATE
    } else {
        sample_rate
    };
    let total_duration_ms = ((samples.len() as u64) * 1000) / (sr as u64);

    // 16-bit PCM equivalent size (2 bytes per sample), reserving budget for lookback overlap + 44B WAV header + rounding slack
    let overlap_bytes = ((split_config.overlap_ms as usize)
        .saturating_mul(sr)
        .saturating_mul(2))
    .div_ceil(1000)
    .saturating_add(44)
    .saturating_add(64);
    let effective_max_bytes = if split_config.max_chunk_size_bytes > overlap_bytes {
        split_config.max_chunk_size_bytes - overlap_bytes
    } else {
        split_config.max_chunk_size_bytes
    };
    let bytes_per_ms = (sr * 2) as f64 / 1000.0;
    let max_duration_by_size_ms =
        ((effective_max_bytes as f64 / bytes_per_ms).floor() as u64).max(1);
    let max_chunk_duration_ms = split_config
        .max_chunk_duration_ms
        .min(max_duration_by_size_ms)
        .max(1);

    if total_duration_ms <= max_chunk_duration_ms {
        // Fits in a single chunk
        let size_bytes = samples.len() * 2;
        return Ok(vec![AudioChunk {
            chunk_index: 0,
            start_ms: 0,
            end_ms: total_duration_ms,
            size_bytes,
            samples: samples.to_vec(),
        }]);
    }

    // Find silence intervals (natural pause points) between speech segments
    let mut pause_midpoints: Vec<u64> = Vec::new();
    for i in 0..speech_segments.len().saturating_sub(1) {
        let seg_cur = &speech_segments[i];
        let seg_next = &speech_segments[i + 1];

        if seg_next.start_ms > seg_cur.end_ms {
            let midpoint = u64::midpoint(seg_cur.end_ms, seg_next.start_ms);
            pause_midpoints.push(midpoint);
        }
    }

    let mut chunks = Vec::new();
    let mut current_chunk_start_ms = 0u64;

    while current_chunk_start_ms < total_duration_ms {
        let target_end_ms = current_chunk_start_ms + max_chunk_duration_ms;
        let max_allowed_end_ms = current_chunk_start_ms + max_duration_by_size_ms;

        if target_end_ms >= total_duration_ms {
            // Final chunk
            let start_sample = ((current_chunk_start_ms as usize) * sr) / 1000;
            let chunk_samples = samples[start_sample..].to_vec();
            let size_bytes = chunk_samples.len() * 2;

            chunks.push(AudioChunk {
                chunk_index: chunks.len(),
                start_ms: current_chunk_start_ms,
                end_ms: total_duration_ms,
                size_bytes,
                samples: chunk_samples,
            });
            break;
        }

        // Find the best pause midpoint within [current_chunk_start_ms + 1000, target_end_ms]
        let candidate_cuts: Vec<u64> = pause_midpoints
            .iter()
            .copied()
            .filter(|&pt| pt > current_chunk_start_ms + 1000 && pt <= target_end_ms)
            .collect();

        let (cut_point_ms, is_natural_pause_cut) = if let Some(&best_cut) = candidate_cuts.last() {
            (best_cut, true)
        } else {
            // Search for earliest natural pause after target_end_ms, bounded strictly by max_allowed_end_ms
            // so a chunk never overflows max_chunk_size_bytes (e.g. >20MB/25MB) on long unbroken speech.
            let earliest_bounded_future = pause_midpoints
                .iter()
                .copied()
                .find(|&pt| pt > target_end_ms && pt <= max_allowed_end_ms);

            if let Some(earliest_future) = earliest_bounded_future {
                (earliest_future, true)
            } else {
                (max_allowed_end_ms.min(total_duration_ms), false)
            }
        };

        if is_natural_pause_cut {
            // Verification invariant: natural pause cut must never fall strictly inside an active speech segment
            for seg in speech_segments {
                if cut_point_ms > seg.start_ms && cut_point_ms < seg.end_ms {
                    return Err(AppError::Stt(SttError::VadProcessingFailed(format!(
                        "AC-14.3 Invariant violation: cut point {cut_point_ms}ms falls inside speech segment [{}ms, {}ms]",
                        seg.start_ms, seg.end_ms
                    ))));
                }
            }
        }

        let start_sample = ((current_chunk_start_ms as usize) * sr) / 1000;
        let end_sample = (((cut_point_ms as usize) * sr) / 1000).min(samples.len());
        let chunk_samples = samples[start_sample..end_sample].to_vec();
        let size_bytes = chunk_samples.len() * 2;

        chunks.push(AudioChunk {
            chunk_index: chunks.len(),
            start_ms: current_chunk_start_ms,
            end_ms: cut_point_ms,
            size_bytes,
            samples: chunk_samples,
        });

        current_chunk_start_ms = cut_point_ms;
    }

    Ok(chunks)
}

/// Asynchronous wrapper running Silero VAD speech segmentation on the blocking threadpool (`CONVENTIONS.md` §2.2).
pub async fn detect_speech_segments_async(
    samples: Vec<f32>,
    vad_config: VadConfig,
) -> Result<Vec<SpeechSegment>, AppError> {
    tokio::task::spawn_blocking(move || detect_speech_segments(&samples, &vad_config))
        .await
        .map_err(|e| {
            AppError::Stt(SttError::VadProcessingFailed(format!(
                "Tokio task execution failed during VAD speech detection: {e}"
            )))
        })?
}

/// Asynchronous wrapper dispatching CPU-heavy computation to blocking threadpool (`CONVENTIONS.md` §2.2).
pub async fn process_vad_pipeline_async(
    samples: Vec<f32>,
    vad_config: VadConfig,
) -> Result<VadPruneResult, AppError> {
    tokio::task::spawn_blocking(move || prune_silence(&samples, &vad_config))
        .await
        .map_err(|e| {
            AppError::Stt(SttError::VadProcessingFailed(format!(
                "Tokio task execution failed during VAD processing: {e}"
            )))
        })?
}

/// Generate a synthetic multi-harmonic voiced audio buffer for deterministic testing.
#[must_use]
pub fn generate_synthetic_speech_signal(
    sample_rate: usize,
    duration_sec: f64,
    frequency_hz: f64,
    amplitude: f32,
) -> Vec<f32> {
    let total_samples = (sample_rate as f64 * duration_sec).round() as usize;
    let mut samples = Vec::with_capacity(total_samples);
    let two_pi = 2.0 * std::f64::consts::PI;

    for i in 0..total_samples {
        let t = i as f64 / sample_rate as f64;
        // Harmonic sum mimicking human voice formants (F0, F1, F2)
        let f0 = (two_pi * frequency_hz * t).sin();
        let f1 = 0.5 * (two_pi * frequency_hz * 2.0 * t).sin();
        let f2 = 0.25 * (two_pi * frequency_hz * 3.0 * t).sin();
        let val = ((f0 + f1 + f2) / 1.75) as f32 * amplitude;
        samples.push(val);
    }

    samples
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::stt::audio_prep::PreparedAudioBuffer;

    fn real_speech_slice(start_sec: f64, duration_sec: f64) -> Vec<f32> {
        let wav_bytes = include_bytes!("../../tests/fixtures/silero_test.wav");
        let buf = PreparedAudioBuffer::from_wav_bytes(wav_bytes)
            .expect("silero_test.wav fixture must parse");
        let sr = buf.sample_rate as usize;
        let start = (start_sec * sr as f64).round() as usize;
        let len = (duration_sec * sr as f64).round() as usize;
        buf.samples_f32[start..start + len].to_vec()
    }

    #[test]
    fn test_silero_onnx_engine_initializes_and_runs_without_fallback() {
        let engine = SileroVadEngine::from_embedded()
            .expect("Real Silero VAD 16kHz ONNX model must load and optimize in tract-onnx");
        assert_eq!(
            engine.num_inputs, 2,
            "Silero v5 16kHz static graph expects 2 inputs"
        );
        assert_eq!(
            engine.num_outputs, 2,
            "Silero v5 16kHz static graph expects 2 outputs"
        );
        let mut session = engine.create_session();
        let silence_frame = vec![0.0f32; DEFAULT_WINDOW_SIZE_SAMPLES];
        let prob = session
            .step(&silence_frame)
            .expect("ONNX step on silence must succeed");
        assert!(prob < 0.2, "Silence probability should be low, got {prob}");
    }

    #[test]
    fn test_speech_segment_bridge_conversion() {
        let seg = SpeechSegment::new(1500, 3750);
        assert_eq!(seg.duration_ms(), 2250);

        let interval = seg.to_speech_interval();
        assert!((interval.start_sec - 1.5).abs() < 1e-4);
        assert!((interval.end_sec - 3.75).abs() < 1e-4);

        let roundtrip = SpeechSegment::from(&interval);
        assert_eq!(roundtrip.start_ms, 1500);
        assert_eq!(roundtrip.end_ms, 3750);
    }

    #[test]
    fn test_ut_14_1_speech_detection_happy_path() {
        // UT-14.1: 60s audio comprising 20s real human speech interspersed with 40s silence
        let sample_rate = 16_000;
        let mut audio = Vec::with_capacity(sample_rate * 60);

        // 0s - 10s: Silence (10s)
        audio.extend(vec![0.0f32; sample_rate * 10]);

        // 10s - 20s: Real speech block 1 (10s)
        audio.extend(real_speech_slice(21.0, 10.0));

        // 20s - 40s: Silence (20s)
        audio.extend(vec![0.0f32; sample_rate * 20]);

        // 40s - 50s: Real speech block 2 (10s)
        audio.extend(real_speech_slice(25.0, 10.0));

        // 50s - 60s: Silence (10s)
        audio.extend(vec![0.0f32; sample_rate * 10]);

        assert_eq!(audio.len(), sample_rate * 60);

        let config = VadConfig::default();
        let segments = detect_speech_segments(&audio, &config).expect("Detection must succeed");

        assert!(
            segments.len() >= 2,
            "Must detect at least 2 distinct speech blocks, found {}",
            segments.len()
        );

        // First segment should span ~10s to ~20s
        let seg1 = &segments[0];
        assert!(
            seg1.start_ms >= 9_700 && seg1.start_ms <= 10_500,
            "seg1 start: {}",
            seg1.start_ms
        );
        assert!(
            seg1.end_ms >= 19_500 && seg1.end_ms <= 20_300,
            "seg1 end: {}",
            seg1.end_ms
        );

        // Second segment should span ~40s to ~50s
        let seg2 = &segments[1];
        assert!(
            seg2.start_ms >= 39_700 && seg2.start_ms <= 40_500,
            "seg2 start: {}",
            seg2.start_ms
        );
        assert!(
            seg2.end_ms >= 49_500 && seg2.end_ms <= 50_300,
            "seg2 end: {}",
            seg2.end_ms
        );
    }

    #[test]
    fn test_ut_14_2_silence_pruning_payload_reduction() {
        // UT-14.2 & AC-14.2: Strip silence > 1.5s, achieve >= 30% payload reduction
        let sample_rate = 16_000;
        let mut audio = Vec::with_capacity(sample_rate * 60);

        // 0s - 10s: Silence (10s)
        audio.extend(vec![0.0f32; sample_rate * 10]);
        // 10s - 20s: Real speech (10s)
        audio.extend(real_speech_slice(21.0, 10.0));
        // 20s - 40s: Silence (20s)
        audio.extend(vec![0.0f32; sample_rate * 20]);
        // 40s - 50s: Real speech (10s)
        audio.extend(real_speech_slice(25.0, 10.0));
        // 50s - 60s: Silence (10s)
        audio.extend(vec![0.0f32; sample_rate * 10]);

        let config = VadConfig::default();
        let prune_result = prune_silence(&audio, &config).expect("Pruning must succeed");

        assert_eq!(prune_result.original_duration_ms, 60_000);
        // Original: 60s. Pruned: ~20s speech + tiny cushions <= 25s.
        assert!(
            prune_result.reduction_ratio >= 0.30,
            "Payload reduction ratio {:.2}% must be >= 30%",
            prune_result.reduction_ratio * 100.0
        );
        assert!(
            prune_result.pruned_duration_ms <= 25_000,
            "Pruned duration {}ms must be <= 25,000ms",
            prune_result.pruned_duration_ms
        );
    }

    #[test]
    fn test_ut_14_3_natural_pause_splitting_no_word_cuts() {
        // UT-14.3 & AC-14.3: Split chunks at pause midpoints, never cut words
        let sample_rate = 16_000;
        let mut audio = Vec::with_capacity(sample_rate * 30);

        // Seg 1: 2s to 6s
        audio.extend(vec![0.0f32; sample_rate * 2]);
        audio.extend(real_speech_slice(21.0, 4.0));

        // Silence 1: 6s to 12s (6s pause)
        audio.extend(vec![0.0f32; sample_rate * 6]);

        // Seg 2: 12s to 18s
        audio.extend(real_speech_slice(25.0, 6.0));

        // Silence 2: 18s to 24s (6s pause)
        audio.extend(vec![0.0f32; sample_rate * 6]);

        // Seg 3: 24s to 28s
        audio.extend(real_speech_slice(31.5, 4.0));
        audio.extend(vec![0.0f32; sample_rate * 2]);

        let config = VadConfig::default();
        let segments = detect_speech_segments(&audio, &config).expect("VAD detection must pass");
        assert!(
            segments.len() >= 3,
            "Expected >= 3 speech segments, got {}",
            segments.len()
        );

        // Request max chunk duration 10s to trigger pause splits
        let split_config = ChunkSplitConfig {
            max_chunk_size_bytes: 20 * 1024 * 1024,
            max_chunk_duration_ms: 10_000,
            overlap_ms: 1_500,
        };

        let chunks = split_into_natural_pause_chunks(&audio, &segments, &split_config, sample_rate)
            .expect("Splitting must pass");

        assert!(
            chunks.len() >= 2,
            "Must produce multiple chunks, found {}",
            chunks.len()
        );

        // Verify that every cut point lies outside all speech segments
        for chunk in &chunks {
            let cut_ms = chunk.end_ms;
            if cut_ms < 30_000 {
                for seg in &segments {
                    assert!(
                        cut_ms <= seg.start_ms || cut_ms >= seg.end_ms,
                        "Cut point {cut_ms}ms must not cut within speech segment [{}ms, {}ms]",
                        seg.start_ms,
                        seg.end_ms
                    );
                }
            }
        }
    }

    #[test]
    fn test_ed_14_2_vad_speed_sla() {
        // ED-14.2 & AC-14.1: VAD processing on 1 minute audio <= 1.5s on CPU
        let sample_rate = 16_000;
        let wav_bytes = include_bytes!("../../tests/fixtures/silero_test.wav");
        let buf = PreparedAudioBuffer::from_wav_bytes(wav_bytes)
            .expect("silero_test.wav fixture must parse");
        assert_eq!(buf.samples_f32.len(), sample_rate * 60);

        let config = VadConfig::default();
        let start = std::time::Instant::now();
        let segments = detect_speech_segments(&buf.samples_f32, &config).expect("VAD must succeed");
        let elapsed = start.elapsed();

        assert_ne!(segments, []);
        assert!(
            elapsed.as_secs_f64() <= 1.5,
            "VAD 1-minute audio processing took {:.4}s (SLA limit: <= 1.5s)",
            elapsed.as_secs_f64()
        );
    }
}
