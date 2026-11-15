//! Audio Preprocessing & Screaming Defense Normalization Pipeline (`TASK-M4-14`, `ERR-08`).
//!
//! Applies FFmpeg EBU R128 `loudnorm` filter (`I=-16:TP=-1.5:LRA=11`) to prevent clipping
//! and token hallucination during streamer scream moments (Vedal/Evil screaming),
//! downmixing to 16kHz mono PCM for Silero VAD and Whisper STT.

use std::fs::File;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::Stdio;

use serde::{Deserialize, Serialize};
use tokio_util::sync::CancellationToken;

use crate::error::{AppError, SidecarError, SttError};
use crate::ingestion::streamer::{assign_child_to_job_guard, ChildProcessGuard};

/// Target sample rate required for Silero VAD and Whisper models (16,000 Hz).
pub const TARGET_SAMPLE_RATE: u32 = 16_000;

/// Target audio channel count (1 = Mono).
pub const TARGET_CHANNELS: u16 = 1;

/// Integrated loudness target in LUFS (-16.0 LUFS per EBU R128 / Broadcast podcast standard).
pub const DEFAULT_TARGET_LUFS: f64 = -16.0;

/// Maximum True Peak ceiling in dBFS (-1.5 dBFS per ERR-08 Screaming Defense).
pub const DEFAULT_TRUE_PEAK_DBFS: f64 = -1.5;

/// Loudness Range target in LU (11.0 LU).
pub const DEFAULT_LOUDNESS_RANGE_LU: f64 = 11.0;

/// Maximum linear amplitude for 16-bit PCM corresponding to -1.5 dBFS ceiling:
/// 32768.0 * 10^(-1.5 / 20.0) ≈ 27570.6 -> 27571.0.
pub const MAX_I16_PEAK_AMPLITUDE: f64 = 27_571.0;

/// Linear scale factor for 16-bit signed PCM conversion.
pub const I16_MAX_F64: f64 = 32_768.0;

/// Small epsilon to prevent log10(0.0) undefined domain errors.
const EPSILON_DBFS: f64 = 1e-9;

/// Configuration parameters for audio normalization and downmixing.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AudioPrepConfig {
    /// Integrated loudness target in LUFS (default: -16.0).
    pub integrated_loudness: f64,
    /// True peak ceiling in dBFS (default: -1.5).
    pub true_peak: f64,
    /// Loudness range target in LU (default: 11.0).
    pub loudness_range: f64,
    /// Audio sampling rate in Hz (default: 16,000).
    pub sample_rate: u32,
    /// Audio channel count (default: 1 = Mono).
    pub channels: u16,
}

impl Default for AudioPrepConfig {
    fn default() -> Self {
        Self {
            integrated_loudness: DEFAULT_TARGET_LUFS,
            true_peak: DEFAULT_TRUE_PEAK_DBFS,
            loudness_range: DEFAULT_LOUDNESS_RANGE_LU,
            sample_rate: TARGET_SAMPLE_RATE,
            channels: TARGET_CHANNELS,
        }
    }
}

/// Metadata report resulting from file-based audio preparation.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AudioPrepResult {
    /// Absolute filesystem path to the prepared audio file.
    pub output_path: PathBuf,
    /// Duration of the audio in seconds.
    pub duration_sec: f64,
    /// Sampling rate in Hz (e.g. 16,000).
    pub sample_rate: u32,
    /// Channel count (e.g. 1).
    pub channels: u16,
    /// Measured peak amplitude in dBFS.
    pub peak_dbfs: f64,
    /// Output file size in bytes.
    pub file_size_bytes: u64,
}

/// In-memory normalized audio buffer supplying samples for both Silero VAD (f32) and RMS/WAV (i16).
#[derive(Debug, Clone, PartialEq)]
pub struct PreparedAudioBuffer {
    /// 32-bit floating point audio samples normalized to [-1.0, 1.0] for Silero VAD.
    pub samples_f32: Vec<f32>,
    /// 16-bit signed integer PCM samples for RMS analysis and WAV serialization.
    pub samples_i16: Vec<i16>,
    /// Sampling rate (always 16,000 Hz).
    pub sample_rate: u32,
    /// Duration in seconds.
    pub duration_sec: f64,
    /// Measured peak amplitude in dBFS (strictly <= -1.5 dBFS).
    pub peak_dbfs: f64,
}

impl PreparedAudioBuffer {
    /// Construct a buffer from 16-bit PCM integer samples.
    #[must_use]
    pub fn from_i16(samples: Vec<i16>, sample_rate: u32) -> Self {
        let sr = if sample_rate == 0 { TARGET_SAMPLE_RATE } else { sample_rate };
        let duration_sec = samples.len() as f64 / sr as f64;
        let peak_dbfs = calculate_peak_dbfs_i16(&samples);

        let mut samples_f32 = Vec::with_capacity(samples.len());
        for &s in &samples {
            let f = (s as f32) / (I16_MAX_F64 as f32);
            samples_f32.push(f.clamp(-1.0, 1.0));
        }

        Self {
            samples_f32,
            samples_i16: samples,
            sample_rate: sr,
            duration_sec,
            peak_dbfs,
        }
    }

    /// Construct a buffer from 32-bit floating point samples in `[-1.0, 1.0]`.
    #[must_use]
    pub fn from_f32(samples: Vec<f32>, sample_rate: u32) -> Self {
        let sr = if sample_rate == 0 { TARGET_SAMPLE_RATE } else { sample_rate };
        let duration_sec = samples.len() as f64 / sr as f64;
        let peak_dbfs = calculate_peak_dbfs_f32(&samples);

        let mut samples_i16 = Vec::with_capacity(samples.len());
        for &s in &samples {
            let clamped = s.clamp(-1.0, 1.0);
            let scaled = (clamped * (I16_MAX_F64 as f32 - 1.0)).round();
            let i = scaled.clamp(i16::MIN as f32, i16::MAX as f32) as i16;
            samples_i16.push(i);
        }

        Self {
            samples_f32: samples,
            samples_i16,
            sample_rate: sr,
            duration_sec,
            peak_dbfs,
        }
    }

    /// Parse a 16-bit Mono/Stereo PCM WAV byte slice into a `PreparedAudioBuffer`.
    pub fn from_wav_bytes(bytes: &[u8]) -> Result<Self, AppError> {
        if bytes.len() < 44 {
            return Err(AppError::Stt(SttError::VadProcessingFailed(
                "WAV payload too short for standard header (< 44 bytes)".to_string(),
            )));
        }

        if &bytes[0..4] != b"RIFF" || &bytes[8..12] != b"WAVE" {
            return Err(AppError::Stt(SttError::VadProcessingFailed(
                "Invalid WAV magic bytes (expected 'RIFF' and 'WAVE')".to_string(),
            )));
        }

        let mut offset = 12;
        let mut sample_rate = TARGET_SAMPLE_RATE;
        let mut channels = 1u16;
        let mut bits_per_sample = 16u16;
        let mut audio_format = 1u16; // 1 = PCM
        let mut audio_data_slice: Option<&[u8]> = None;

        while offset + 8 <= bytes.len() {
            let chunk_id = &bytes[offset..offset + 4];
            let chunk_size = u32::from_le_bytes([
                bytes[offset + 4],
                bytes[offset + 5],
                bytes[offset + 6],
                bytes[offset + 7],
            ]) as usize;
            offset += 8;

            if chunk_id == b"fmt " {
                if chunk_size < 16 || offset + chunk_size > bytes.len() {
                    return Err(AppError::Stt(SttError::VadProcessingFailed(
                        "Malformed fmt chunk in WAV file".to_string(),
                    )));
                }
                audio_format = u16::from_le_bytes([bytes[offset], bytes[offset + 1]]);
                channels = u16::from_le_bytes([bytes[offset + 2], bytes[offset + 3]]);
                sample_rate = u32::from_le_bytes([
                    bytes[offset + 4],
                    bytes[offset + 5],
                    bytes[offset + 6],
                    bytes[offset + 7],
                ]);
                bits_per_sample = u16::from_le_bytes([bytes[offset + 14], bytes[offset + 15]]);
            } else if chunk_id == b"data" {
                let data_end = (offset + chunk_size).min(bytes.len());
                audio_data_slice = Some(&bytes[offset..data_end]);
                break;
            }

            offset += chunk_size;
            // Chunks in RIFF are word-aligned (2 bytes)
            if !chunk_size.is_multiple_of(2) {
                offset += 1;
            }
        }

        let raw_data = match audio_data_slice {
            Some(d) => d,
            None => {
                return Err(AppError::Stt(SttError::VadProcessingFailed(
                    "Missing 'data' chunk in WAV payload".to_string(),
                )))
            }
        };

        if audio_format != 1 || bits_per_sample != 16 {
            return Err(AppError::Stt(SttError::VadProcessingFailed(format!(
                "Unsupported WAV format: audio_format={audio_format}, bits_per_sample={bits_per_sample} (expected 16-bit PCM)",
            ))));
        }

        let total_samples = raw_data.len() / 2;
        let num_frames = if channels == 0 { 0 } else { total_samples / channels as usize };

        let mut mono_samples_i16 = Vec::with_capacity(num_frames);

        if channels == 1 {
            for i in 0..num_frames {
                let idx = i * 2;
                let s = i16::from_le_bytes([raw_data[idx], raw_data[idx + 1]]);
                mono_samples_i16.push(s);
            }
        } else if channels == 2 {
            // Downmix stereo to mono by averaging left and right channels
            for i in 0..num_frames {
                let left_idx = i * 4;
                let right_idx = left_idx + 2;
                let left = i16::from_le_bytes([raw_data[left_idx], raw_data[left_idx + 1]]) as i32;
                let right = i16::from_le_bytes([raw_data[right_idx], raw_data[right_idx + 1]]) as i32;
                let mono = ((left + right) / 2) as i16;
                mono_samples_i16.push(mono);
            }
        } else {
            return Err(AppError::Stt(SttError::VadProcessingFailed(format!(
                "Unsupported channel count: {channels} (expected 1 or 2)",
            ))));
        }

        Ok(Self::from_i16(mono_samples_i16, sample_rate))
    }

    /// Serialize this buffer into standard 44-byte RIFF/WAVE PCM 16-bit mono bytes.
    pub fn to_wav_bytes(&self) -> Result<Vec<u8>, AppError> {
        let num_samples = self.samples_i16.len();
        let subchunk2_size = (num_samples * 2) as u32;
        let chunk_size = 36u32.saturating_add(subchunk2_size);
        let sample_rate = self.sample_rate;
        let num_channels = 1u16;
        let bits_per_sample = 16u16;
        let byte_rate = sample_rate * u32::from(num_channels) * u32::from(bits_per_sample) / 8;
        let block_align = num_channels * bits_per_sample / 8;

        let mut bytes = Vec::with_capacity(44 + num_samples * 2);

        // RIFF header
        bytes.extend_from_slice(b"RIFF");
        bytes.extend_from_slice(&chunk_size.to_le_bytes());
        bytes.extend_from_slice(b"WAVE");

        // "fmt " subchunk
        bytes.extend_from_slice(b"fmt ");
        bytes.extend_from_slice(&16u32.to_le_bytes()); // Subchunk1Size (16 for PCM)
        bytes.extend_from_slice(&1u16.to_le_bytes());  // AudioFormat (1 = PCM)
        bytes.extend_from_slice(&num_channels.to_le_bytes());
        bytes.extend_from_slice(&sample_rate.to_le_bytes());
        bytes.extend_from_slice(&byte_rate.to_le_bytes());
        bytes.extend_from_slice(&block_align.to_le_bytes());
        bytes.extend_from_slice(&bits_per_sample.to_le_bytes());

        // "data" subchunk
        bytes.extend_from_slice(b"data");
        bytes.extend_from_slice(&subchunk2_size.to_le_bytes());

        // Raw PCM samples
        for &sample in &self.samples_i16 {
            bytes.extend_from_slice(&sample.to_le_bytes());
        }

        Ok(bytes)
    }

    /// Write buffer as a WAV file to disk.
    pub fn write_wav_file(&self, path: &Path) -> Result<(), AppError> {
        let bytes = self.to_wav_bytes()?;
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| {
                AppError::Stt(SttError::VadProcessingFailed(format!(
                    "Failed to create directory for WAV output '{}': {e}",
                    parent.display()
                )))
            })?;
        }

        let mut file = File::create(path).map_err(|e| {
            AppError::Stt(SttError::VadProcessingFailed(format!(
                "Failed to create WAV output file '{}': {e}",
                path.display()
            )))
        })?;

        file.write_all(&bytes).map_err(|e| {
            AppError::Stt(SttError::VadProcessingFailed(format!(
                "Failed to write WAV output data to '{}': {e}",
                path.display()
            )))
        })?;

        Ok(())
    }
}

/// Calculate peak amplitude of 16-bit PCM integer samples in dBFS.
#[must_use]
pub fn calculate_peak_dbfs_i16(samples: &[i16]) -> f64 {
    if samples.is_empty() {
        return -96.0;
    }

    let mut max_abs = 0.0f64;
    for &s in samples {
        let abs_val = (s as f64).abs();
        if abs_val > max_abs {
            max_abs = abs_val;
        }
    }

    if max_abs <= EPSILON_DBFS {
        return -96.0;
    }

    let peak_linear = max_abs / I16_MAX_F64;
    (20.0 * peak_linear.log10()).clamp(-96.0, 0.0)
}

/// Calculate peak amplitude of 32-bit floating point samples in dBFS.
#[must_use]
pub fn calculate_peak_dbfs_f32(samples: &[f32]) -> f64 {
    if samples.is_empty() {
        return -96.0;
    }

    let mut max_abs = 0.0f64;
    for &s in samples {
        let abs_val = f64::from(s.abs());
        if abs_val > max_abs {
            max_abs = abs_val;
        }
    }

    if max_abs <= EPSILON_DBFS {
        return -96.0;
    }

    (20.0 * max_abs.log10()).clamp(-96.0, 0.0)
}

/// Computes reconstructed True Peak amplitude in dBFS via 4x inter-sample peak evaluation.
///
/// Detects continuous-time inter-sample overshoot across peak candidates using
/// cubic Hermite interpolation.
#[must_use]
pub fn compute_true_peak_dbfs(samples: &[i16]) -> f64 {
    if samples.is_empty() {
        return -96.0;
    }

    let mut max_continuous_peak = 0.0f64;
    let n = samples.len();

    for i in 0..n {
        let s_i = samples[i] as f64;
        let abs_s = s_i.abs();
        if abs_s > max_continuous_peak {
            max_continuous_peak = abs_s;
        }

        // Evaluate 4x oversampling points between s[i] and s[i+1] when sample is significant
        if i > 0 && i + 2 < n && abs_s > max_continuous_peak * 0.5 {
            let p0 = samples[i - 1] as f64;
            let p1 = s_i;
            let p2 = samples[i + 1] as f64;
            let p3 = samples[i + 2] as f64;

            // Check t = 0.25, 0.5, 0.75 via Catmull-Rom / Hermite cubic interpolation
            for &t in &[0.25f64, 0.5f64, 0.75f64] {
                let t2 = t * t;
                let t3 = t2 * t;
                let v = 0.5 * (
                    (2.0 * p1)
                    + (-p0 + p2) * t
                    + (2.0 * p0 - 5.0 * p1 + 4.0 * p2 - p3) * t2
                    + (-p0 + 3.0 * p1 - 3.0 * p2 + p3) * t3
                );
                let abs_v = v.abs();
                if abs_v > max_continuous_peak {
                    max_continuous_peak = abs_v;
                }
            }
        }
    }

    if max_continuous_peak <= EPSILON_DBFS {
        return -96.0;
    }

    let peak_linear = max_continuous_peak / I16_MAX_F64;
    (20.0 * peak_linear.log10()).clamp(-96.0, 3.0)
}

/// Computes reconstructed True Peak amplitude in dBFS via 4x inter-sample peak evaluation on f32 samples.
#[must_use]
pub fn compute_true_peak_f32(samples: &[f32]) -> f64 {
    if samples.is_empty() {
        return -96.0;
    }

    let mut max_continuous_peak = 0.0f64;
    let n = samples.len();

    for i in 0..n {
        let s_i = f64::from(samples[i]);
        let abs_s = s_i.abs();
        if abs_s > max_continuous_peak {
            max_continuous_peak = abs_s;
        }

        // Evaluate 4x oversampling points between s[i] and s[i+1] when sample is significant
        if i > 0 && i + 2 < n && abs_s > max_continuous_peak * 0.5 {
            let p0 = f64::from(samples[i - 1]);
            let p1 = s_i;
            let p2 = f64::from(samples[i + 1]);
            let p3 = f64::from(samples[i + 2]);

            // Check t = 0.25, 0.5, 0.75 via Catmull-Rom / Hermite cubic interpolation
            for &t in &[0.25f64, 0.5f64, 0.75f64] {
                let t2 = t * t;
                let t3 = t2 * t;
                let v = 0.5 * (
                    (2.0 * p1)
                    + (-p0 + p2) * t
                    + (2.0 * p0 - 5.0 * p1 + 4.0 * p2 - p3) * t2
                    + (-p0 + 3.0 * p1 - 3.0 * p2 + p3) * t3
                );
                let abs_v = v.abs();
                if abs_v > max_continuous_peak {
                    max_continuous_peak = abs_v;
                }
            }
        }
    }

    if max_continuous_peak <= EPSILON_DBFS {
        return -96.0;
    }

    (20.0 * max_continuous_peak.log10()).clamp(-96.0, 3.0)
}

/// Apply screaming defense peak limiter to constrain true peak to `<= max_peak_dbfs` (`ERR-08`).
///
/// Uses an unconditionally bounded smooth soft-knee curve:
/// - Audio below knee threshold (0.75 * target ceiling) remains 100% linear (zero distortion).
/// - High-energy screaming peaks above knee threshold are compressed smoothly via `tanh`,
///   mathematically guaranteeing amplitude never exceeds `10^(max_peak_dbfs / 20)`.
/// - Accounts for continuous reconstructed analog inter-sample overshoot (Gibbs phenomenon).
pub fn apply_screaming_limiter(samples: &mut [f32], max_peak_dbfs: f32) {
    if samples.is_empty() {
        return;
    }

    // Step 1: Smooth soft-knee compression on discrete samples
    let target_ceiling = 10.0f32.powf(max_peak_dbfs / 20.0);
    let knee_start = target_ceiling * 0.75;
    let headroom = target_ceiling - knee_start;

    for s in samples.iter_mut() {
        let abs_val = s.abs();
        if abs_val > knee_start {
            let sign = if *s >= 0.0 { 1.0f32 } else { -1.0f32 };
            let excess = abs_val - knee_start;
            let compressed = knee_start + headroom * (excess / headroom).tanh();
            *s = sign * compressed.min(target_ceiling);
        }
    }

    // Step 2: Continuous reconstructed true peak defense against inter-sample overshoot
    let measured_tp = compute_true_peak_f32(samples);
    if measured_tp > f64::from(max_peak_dbfs) {
        let excess_db = measured_tp - f64::from(max_peak_dbfs);
        let reduction_factor = 10.0f64.powf(-excess_db / 20.0) as f32;
        for s in samples.iter_mut() {
            *s *= reduction_factor;
        }
    }
}

/// Verify that audio samples strictly obey the True Peak <= -1.5 dBFS ceiling (`ED-14.1`).
///
/// Returns `Ok(())` if true peak is at or below `max_tp_dbfs` (plus 0.05 dBFS tolerance),
/// or an error if peak clipping violation is detected.
pub fn verify_screaming_defense(samples: &[i16], max_tp_dbfs: f64) -> Result<(), AppError> {
    let peak = calculate_peak_dbfs_i16(samples);
    let tolerance = 0.05;
    if peak > max_tp_dbfs + tolerance {
        return Err(AppError::Stt(SttError::VadProcessingFailed(format!(
            "ERR-08 Screaming Defense violation: audio peak {peak:.2} dBFS exceeds ceiling {max_tp_dbfs:.2} dBFS",
        ))));
    }
    Ok(())
}

/// Build FFmpeg CLI arguments for loudnorm normalization and 16kHz mono WAV generation.
#[must_use]
pub fn build_loudnorm_ffmpeg_args(
    input_path: &Path,
    output_path: &Path,
    config: &AudioPrepConfig,
) -> Vec<String> {
    vec![
        "-hide_banner".to_string(),
        "-loglevel".to_string(),
        "error".to_string(),
        "-nostdin".to_string(),
        "-y".to_string(),
        "-i".to_string(),
        input_path.to_string_lossy().to_string(),
        "-vn".to_string(),
        "-sn".to_string(),
        "-dn".to_string(),
        "-af".to_string(),
        format!(
            "loudnorm=I={:.1}:TP={:.1}:LRA={:.1}",
            config.integrated_loudness, config.true_peak, config.loudness_range
        ),
        "-ar".to_string(),
        config.sample_rate.to_string(),
        "-ac".to_string(),
        config.channels.to_string(),
        "-c:a".to_string(),
        "pcm_s16le".to_string(),
        output_path.to_string_lossy().to_string(),
    ]
}

/// Build FFmpeg CLI arguments for real-time stdout streaming (s16le pipe).
#[must_use]
pub fn build_loudnorm_pipe_args(
    input_path: &Path,
    config: &AudioPrepConfig,
) -> Vec<String> {
    vec![
        "-hide_banner".to_string(),
        "-loglevel".to_string(),
        "error".to_string(),
        "-nostdin".to_string(),
        "-i".to_string(),
        input_path.to_string_lossy().to_string(),
        "-vn".to_string(),
        "-sn".to_string(),
        "-dn".to_string(),
        "-af".to_string(),
        format!(
            "loudnorm=I={:.1}:TP={:.1}:LRA={:.1}",
            config.integrated_loudness, config.true_peak, config.loudness_range
        ),
        "-ar".to_string(),
        config.sample_rate.to_string(),
        "-ac".to_string(),
        config.channels.to_string(),
        "-f".to_string(),
        "s16le".to_string(),
        "pipe:1".to_string(),
    ]
}

/// Resolve the path to the `ffmpeg` executable.
#[must_use]
pub fn resolve_ffmpeg_binary() -> String {
    if let Ok(p) = std::env::var("NEUROCLIP_FFMPEG_PATH") {
        let trimmed = p.trim();
        if !trimmed.is_empty() {
            return trimmed.to_string();
        }
    }
    "ffmpeg".to_string()
}

/// Check if mock audio preparation mode is enabled via environment variable.
#[must_use]
pub fn is_mock_audio_prep_enabled() -> bool {
    if let Ok(v) = std::env::var("NEUROCLIP_MOCK_AUDIO_PREP") {
        !v.trim().is_empty() && v.trim() != "0"
    } else {
        false
    }
}

/// Normalize and downmix an input audio/video file to a 16kHz mono WAV file on disk.
///
/// Features Windows Job Object assignment, `kill_on_drop(true)` lifetime guarantee,
/// and support for cancellation tokens.
pub async fn prepare_audio_file(
    input_path: &Path,
    output_path: &Path,
    config: &AudioPrepConfig,
    cancel_token: Option<&CancellationToken>,
) -> Result<AudioPrepResult, AppError> {
    if let Some(token) = cancel_token {
        if token.is_cancelled() {
            return Err(AppError::TaskCancelled);
        }
    }

    if is_mock_audio_prep_enabled() {
        // Mock mode: generate a normalized WAV file deterministically
        let buffer = if input_path.exists() {
            if let Ok(mut f) = File::open(input_path) {
                let mut data = Vec::new();
                let _ = f.read_to_end(&mut data);
                PreparedAudioBuffer::from_wav_bytes(&data).unwrap_or_else(|_| {
                    generate_synthetic_normalized_buffer(config.sample_rate, 2.0)
                })
            } else {
                generate_synthetic_normalized_buffer(config.sample_rate, 2.0)
            }
        } else {
            generate_synthetic_normalized_buffer(config.sample_rate, 2.0)
        };

        buffer.write_wav_file(output_path)?;
        let metadata = std::fs::metadata(output_path).map_err(|e| {
            AppError::Stt(SttError::VadProcessingFailed(format!(
                "Failed to read metadata for generated mock WAV: {e}"
            )))
        })?;

        return Ok(AudioPrepResult {
            output_path: output_path.to_path_buf(),
            duration_sec: buffer.duration_sec,
            sample_rate: buffer.sample_rate,
            channels: 1,
            peak_dbfs: buffer.peak_dbfs,
            file_size_bytes: metadata.len(),
        });
    }

    let binary = resolve_ffmpeg_binary();
    let args = build_loudnorm_ffmpeg_args(input_path, output_path, config);

    let mut cmd = tokio::process::Command::new(&binary);
    cmd.args(&args);
    cmd.stdout(Stdio::piped());
    cmd.stderr(Stdio::piped());
    cmd.kill_on_drop(true);

    let child = cmd.spawn().map_err(|e| {
        if e.kind() == std::io::ErrorKind::NotFound {
            AppError::Sidecar(SidecarError::BinaryNotFound {
                name: "ffmpeg".to_string(),
                path: binary.clone(),
            })
        } else {
            AppError::Sidecar(SidecarError::ExecutionFailed {
                binary: "ffmpeg".to_string(),
                exit_code: None,
                details: e.to_string(),
            })
        }
    })?;

    assign_child_to_job_guard(&child);
    let mut guard = ChildProcessGuard::new(child);

    let child_ref = match guard.child_mut() {
        Some(c) => c,
        None => {
            return Err(AppError::Stt(SttError::VadProcessingFailed(
                "Internal error: Child process unavailable in guard".to_string(),
            )))
        }
    };

    let status = if let Some(token) = cancel_token {
        tokio::select! {
            res = child_ref.wait() => {
                res.map_err(|e| AppError::Sidecar(SidecarError::ExecutionFailed {
                    binary: "ffmpeg".to_string(),
                    exit_code: None,
                    details: e.to_string(),
                }))?
            }
            () = token.cancelled() => {
                return Err(AppError::TaskCancelled);
            }
        }
    } else {
        child_ref.wait().await.map_err(|e| {
            AppError::Sidecar(SidecarError::ExecutionFailed {
                binary: "ffmpeg".to_string(),
                exit_code: None,
                details: e.to_string(),
            })
        })?
    };

    if !status.success() {
        return Err(AppError::Stt(SttError::VadProcessingFailed(format!(
            "FFmpeg loudnorm transcode exited with non-zero status: {status}"
        ))));
    }

    let metadata = std::fs::metadata(output_path).map_err(|e| {
        AppError::Stt(SttError::VadProcessingFailed(format!(
            "Failed to access output normalized audio '{}': {e}",
            output_path.display()
        )))
    })?;

    // Read generated WAV to measure peak dBFS and duration
    let mut file = File::open(output_path).map_err(|e| {
        AppError::Stt(SttError::VadProcessingFailed(format!(
            "Failed to open generated normalized audio '{}': {e}",
            output_path.display()
        )))
    })?;
    let mut wav_bytes = Vec::new();
    file.read_to_end(&mut wav_bytes).map_err(|e| {
        AppError::Stt(SttError::VadProcessingFailed(format!(
            "Failed to read generated normalized audio '{}': {e}",
            output_path.display()
        )))
    })?;

    let buffer = PreparedAudioBuffer::from_wav_bytes(&wav_bytes)?;

    Ok(AudioPrepResult {
        output_path: output_path.to_path_buf(),
        duration_sec: buffer.duration_sec,
        sample_rate: buffer.sample_rate,
        channels: 1,
        peak_dbfs: buffer.peak_dbfs,
        file_size_bytes: metadata.len(),
    })
}

/// Normalize audio and load directly into a `PreparedAudioBuffer` in memory.
pub async fn prepare_audio_to_buffer(
    input_path: &Path,
    config: &AudioPrepConfig,
    cancel_token: Option<&CancellationToken>,
) -> Result<PreparedAudioBuffer, AppError> {
    if let Some(token) = cancel_token {
        if token.is_cancelled() {
            return Err(AppError::TaskCancelled);
        }
    }

    if is_mock_audio_prep_enabled() {
        let mut buffer = generate_synthetic_normalized_buffer(config.sample_rate, 2.0);
        apply_screaming_limiter(&mut buffer.samples_f32, config.true_peak as f32);
        return Ok(buffer);
    }

    let binary = resolve_ffmpeg_binary();
    let args = build_loudnorm_pipe_args(input_path, config);

    let mut cmd = tokio::process::Command::new(&binary);
    cmd.args(&args);
    cmd.stdout(Stdio::piped());
    cmd.stderr(Stdio::piped());
    cmd.kill_on_drop(true);

    let mut child = cmd.spawn().map_err(|e| {
        if e.kind() == std::io::ErrorKind::NotFound {
            AppError::Sidecar(SidecarError::BinaryNotFound {
                name: "ffmpeg".to_string(),
                path: binary.clone(),
            })
        } else {
            AppError::Sidecar(SidecarError::ExecutionFailed {
                binary: "ffmpeg".to_string(),
                exit_code: None,
                details: e.to_string(),
            })
        }
    })?;

    assign_child_to_job_guard(&child);

    let stdout = match child.stdout.take() {
        Some(s) => s,
        None => {
            return Err(AppError::Stt(SttError::VadProcessingFailed(
                "Failed to capture stdout pipe from FFmpeg".to_string(),
            )))
        }
    };

    let mut reader = tokio::io::BufReader::new(stdout);
    let mut pcm_bytes = Vec::new();

    if let Some(token) = cancel_token {
        tokio::select! {
            res = tokio::io::AsyncReadExt::read_to_end(&mut reader, &mut pcm_bytes) => {
                res.map_err(|e| AppError::Stt(SttError::VadProcessingFailed(e.to_string())))?;
            }
            () = token.cancelled() => {
                let _ = child.start_kill();
                return Err(AppError::TaskCancelled);
            }
        }
    } else {
        tokio::io::AsyncReadExt::read_to_end(&mut reader, &mut pcm_bytes)
            .await
            .map_err(|e| AppError::Stt(SttError::VadProcessingFailed(e.to_string())))?;
    }

    let status = child.wait().await.map_err(|e| {
        AppError::Sidecar(SidecarError::ExecutionFailed {
            binary: "ffmpeg".to_string(),
            exit_code: None,
            details: e.to_string(),
        })
    })?;

    if !status.success() {
        return Err(AppError::Stt(SttError::VadProcessingFailed(format!(
            "FFmpeg loudnorm streaming transcode exited with non-zero status: {status}"
        ))));
    }

    let num_samples = pcm_bytes.len() / 2;
    let mut samples_i16 = Vec::with_capacity(num_samples);
    for i in 0..num_samples {
        let idx = i * 2;
        let s = i16::from_le_bytes([pcm_bytes[idx], pcm_bytes[idx + 1]]);
        samples_i16.push(s);
    }

    Ok(PreparedAudioBuffer::from_i16(samples_i16, config.sample_rate))
}

/// Helper to generate a clean synthetic sine wave buffer normalized to -1.5 dBFS for tests and mocks.
#[must_use]
pub fn generate_synthetic_normalized_buffer(sample_rate: u32, duration_sec: f64) -> PreparedAudioBuffer {
    let sr = if sample_rate == 0 { TARGET_SAMPLE_RATE } else { sample_rate };
    let total_samples = (sr as f64 * duration_sec).round() as usize;
    let mut samples_f32 = Vec::with_capacity(total_samples);

    // Peak amplitude for -1.5 dBFS: 10^(-1.5 / 20) ≈ 0.841395
    let peak_linear = 10.0f32.powf(-1.5 / 20.0);
    let freq = 440.0; // 440 Hz standard pitch
    let two_pi = 2.0 * std::f64::consts::PI;

    for i in 0..total_samples {
        let t = i as f64 / sr as f64;
        let val = (two_pi * freq * t).sin() as f32 * peak_linear;
        samples_f32.push(val);
    }

    PreparedAudioBuffer::from_f32(samples_f32, sr)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_loudnorm_ffmpeg_args() {
        let config = AudioPrepConfig::default();
        let input = Path::new("clip.mp4");
        let output = Path::new("clip_norm.wav");

        let args = build_loudnorm_ffmpeg_args(input, output, &config);
        assert!(args.contains(&"-af".to_string()));
        assert!(args.contains(&"loudnorm=I=-16.0:TP=-1.5:LRA=11.0".to_string()));
        assert!(args.contains(&"-ar".to_string()));
        assert!(args.contains(&"16000".to_string()));
        assert!(args.contains(&"-ac".to_string()));
        assert!(args.contains(&"1".to_string()));
        assert!(args.contains(&"pcm_s16le".to_string()));
    }

    #[test]
    fn test_build_loudnorm_pipe_args() {
        let config = AudioPrepConfig::default();
        let input = Path::new("clip.mp4");

        let args = build_loudnorm_pipe_args(input, &config);
        assert!(args.contains(&"-f".to_string()));
        assert!(args.contains(&"s16le".to_string()));
        assert!(args.contains(&"pipe:1".to_string()));
    }

    #[test]
    fn test_wav_roundtrip_serialization() {
        let buffer = generate_synthetic_normalized_buffer(16_000, 1.0);
        let wav_bytes = buffer.to_wav_bytes().expect("Serialization must succeed");
        assert!(wav_bytes.len() >= 44);

        let parsed = PreparedAudioBuffer::from_wav_bytes(&wav_bytes).expect("Deserialization must succeed");
        assert_eq!(parsed.sample_rate, 16_000);
        assert_eq!(parsed.samples_i16.len(), buffer.samples_i16.len());
        assert!((parsed.duration_sec - 1.0).abs() < 0.01);
    }

    #[test]
    fn test_screaming_defense_saturation_clamping() {
        // ED-14.1: Streamer screaming saturating 0 dBFS (amplitude = 1.0)
        let mut saturated_samples = vec![1.0f32, -1.0f32, 0.95f32, -0.98f32, 0.5f32, 0.0f32];
        apply_screaming_limiter(&mut saturated_samples, -1.5);

        let max_linear_ceiling = 10.0f32.powf(-1.5 / 20.0);
        for &s in &saturated_samples {
            assert!(
                s.abs() <= max_linear_ceiling + 1e-5,
                "Sample {s} exceeded ceiling {max_linear_ceiling}"
            );
        }

        let peak_dbfs = calculate_peak_dbfs_f32(&saturated_samples);
        assert!(
            peak_dbfs <= -1.5 + 0.01,
            "Peak {peak_dbfs} dBFS must be <= -1.5 dBFS"
        );
    }

    #[test]
    fn test_verify_screaming_defense_passes_on_valid_audio() {
        let buffer = generate_synthetic_normalized_buffer(16_000, 0.5);
        let res = verify_screaming_defense(&buffer.samples_i16, -1.5);
        assert!(res.is_ok());
    }

    #[test]
    fn test_verify_screaming_defense_fails_on_clipping() {
        let clipped_samples = vec![32767i16, -32768i16];
        let res = verify_screaming_defense(&clipped_samples, -1.5);
        assert!(res.is_err());
    }

    #[tokio::test]
    async fn test_mock_audio_prep_file_workflow() {
        std::env::set_var("NEUROCLIP_MOCK_AUDIO_PREP", "1");
        let temp_dir = std::env::temp_dir().join("neuroclip_test_audio_prep");
        let input_path = temp_dir.join("mock_input.wav");
        let output_path = temp_dir.join("mock_output.wav");

        let config = AudioPrepConfig::default();
        let result = prepare_audio_file(&input_path, &output_path, &config, None).await;
        assert!(result.is_ok());

        let prep_res = result.unwrap();
        assert!(prep_res.peak_dbfs <= -1.5 + 0.01);
        assert_eq!(prep_res.sample_rate, 16_000);
        assert!(output_path.exists());

        let _ = std::fs::remove_dir_all(&temp_dir);
        std::env::remove_var("NEUROCLIP_MOCK_AUDIO_PREP");
    }
}
