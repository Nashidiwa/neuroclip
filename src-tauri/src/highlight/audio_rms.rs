//! Pure Mathematical Primitives for Audio RMS & dBFS Energy Computation.
//!
//! Complies with `docs/CONVENTIONS.md` (zero-panic invariant, Result<T, AppError>),
//! `docs/ARCHITECTURE.md` §4.3 & §5.4, and `docs/TASK_BOARD.md` TASK-M4-11.

#![allow(
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss
)]

use crate::error::{AppError, HighlightError};
use serde::{Deserialize, Serialize};

/// Default audio sampling rate throughout NeuroClip ingestion pipeline (16kHz mono).
pub const DEFAULT_SAMPLE_RATE: usize = 16_000;

/// Default block duration for audio RMS analysis (500 milliseconds = 0.5 seconds).
pub const DEFAULT_BLOCK_DURATION_SEC: f64 = 0.5;

/// Number of 16-bit PCM samples in a standard 500ms block at 16kHz (16,000 * 0.5 = 8,000).
pub const DEFAULT_BLOCK_SAMPLES: usize = 8_000;

/// Silence floor in decibels relative to full scale (-96.0 dBFS).
/// Any signal at or below this floor is clamped to silence (RMS = 0.0).
pub const SILENCE_FLOOR_DBFS: f64 = -96.0;

/// Silence floor amplitude in linear scale: 10^(-96.0 / 20.0) ≈ 1.5848931924611136e-5.
pub const SILENCE_FLOOR_AMPLITUDE: f64 = 1.584_893_192_461_113_6e-5;

/// Minimum linear RMS threshold below which energy is considered digital silence.
pub const MIN_RMS_THRESHOLD: f64 = SILENCE_FLOOR_AMPLITUDE;

/// Theoretical RMS for full-scale 1kHz continuous sine wave: 1 / sqrt(2) ≈ 0.7071067811865475.
pub const SINE_1KHZ_THEORETICAL_RMS: f64 = std::f64::consts::FRAC_1_SQRT_2;

/// Target expected RMS for discrete 1kHz full-scale sine wave per UT-11.1 specification.
#[allow(clippy::approx_constant)]
pub const SINE_1KHZ_EXPECTED_RMS: f64 = 0.7071;

/// Acceptance tolerance for 1kHz sine wave RMS measurement (±0.005) per UT-11.1.
pub const SINE_1KHZ_RMS_TOLERANCE: f64 = 0.005;

/// Full scale divisor for 16-bit signed PCM samples (-32768 to 32767).
pub const I16_MAX_F64: f64 = 32768.0;

/// Metrics computed for a single 500ms block of PCM audio samples.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct AudioBlockMetrics {
    /// Zero-based sequential block index.
    pub index: usize,
    /// Start timestamp of the block in seconds.
    pub timestamp_sec: f64,
    /// Number of PCM samples in this block.
    pub sample_count: usize,
    /// Raw Root Mean Square amplitude in range [0.0, 1.0].
    pub raw_rms: f64,
    /// Decibels relative to Full Scale clamped at [-96.0, 0.0] dBFS.
    pub dbfs: f64,
    /// Linear normalized RMS clamped to [0.0, 1.0], with silence floor mapped to 0.0.
    pub normalized_rms: f64,
}

/// Calculate the raw Root Mean Square (RMS) of 16-bit signed PCM samples.
///
/// Samples are normalized to `[-1.0, 1.0]` by dividing by `32768.0`.
/// - For silence (all zeros), returns `0.0`.
/// - For a full-scale 1kHz sine wave, returns `~0.7071` (±0.005).
/// - For an empty slice, returns `0.0`.
#[must_use]
pub fn calculate_raw_rms(samples: &[i16]) -> f64 {
    if samples.is_empty() {
        return 0.0;
    }

    let n = samples.len() as f64;
    let sum_sq: f64 = samples
        .iter()
        .map(|&s| {
            let v = f64::from(s) / I16_MAX_F64;
            v * v
        })
        .sum();

    let mean_sq = sum_sq / n;
    if !mean_sq.is_finite() || mean_sq <= 0.0 {
        0.0
    } else {
        mean_sq.sqrt().clamp(0.0, 1.0)
    }
}

/// Alias for `calculate_raw_rms` compliant with naming in M4.3 specifications.
#[must_use]
pub fn calculate_pcm_rms(samples: &[i16]) -> f64 {
    calculate_raw_rms(samples)
}

/// Calculate sine-peak normalized RMS in range `[0.0, 1.0]`.
///
/// Multiplies raw RMS by `sqrt(2)` so that a full-scale 1kHz sine wave (raw RMS ~ 0.7071)
/// normalizes to exactly `1.0`. Silence returns `0.0`.
#[must_use]
pub fn calculate_sine_normalized_rms(samples: &[i16]) -> f64 {
    let raw = calculate_raw_rms(samples);
    if raw <= MIN_RMS_THRESHOLD {
        0.0
    } else {
        (raw * std::f64::consts::SQRT_2).clamp(0.0, 1.0)
    }
}

/// Alias for `calculate_sine_normalized_rms`.
#[must_use]
pub fn calculate_normalized_pcm_rms(samples: &[i16]) -> f64 {
    calculate_sine_normalized_rms(samples)
}

/// Convert a linear RMS amplitude value to Decibels relative to Full Scale (dBFS).
///
/// - If `raw_rms` is non-finite, `<= 0.0`, or `<= MIN_RMS_THRESHOLD`, clamps strictly
///   to `SILENCE_FLOOR_DBFS` (-96.0 dBFS).
/// - Otherwise evaluates `20 * log10(raw_rms)` clamped to `[-96.0, 0.0]`.
#[must_use]
pub fn rms_to_dbfs(raw_rms: f64) -> f64 {
    if !raw_rms.is_finite() || raw_rms <= MIN_RMS_THRESHOLD {
        SILENCE_FLOOR_DBFS
    } else {
        (20.0 * raw_rms.log10()).clamp(SILENCE_FLOOR_DBFS, 0.0)
    }
}

/// Convert Decibels relative to Full Scale (dBFS) to linear RMS amplitude.
///
/// - If `dbfs` is non-finite or `<= SILENCE_FLOOR_DBFS` (-96.0 dBFS), returns `0.0`.
/// - Otherwise evaluates `10^(dbfs / 20)` clamped to `[0.0, 1.0]`.
#[must_use]
pub fn dbfs_to_rms(dbfs: f64) -> f64 {
    if !dbfs.is_finite() || dbfs <= SILENCE_FLOOR_DBFS {
        0.0
    } else {
        10.0_f64.powf(dbfs / 20.0).clamp(0.0, 1.0)
    }
}

/// Normalize dBFS linearly across the `[-96.0, 0.0]` dynamic range into `[0.0, 1.0]`.
///
/// Silence (-96.0 dBFS) maps to `0.0`, while full-scale (0.0 dBFS) maps to `1.0`.
#[must_use]
pub fn dbfs_to_normalized_scale(dbfs: f64) -> f64 {
    if !dbfs.is_finite() || dbfs <= SILENCE_FLOOR_DBFS {
        0.0
    } else {
        ((dbfs - SILENCE_FLOOR_DBFS) / (0.0 - SILENCE_FLOOR_DBFS)).clamp(0.0, 1.0)
    }
}

/// Apply silence floor clamp to linear RMS amplitude.
///
/// If `raw_rms <= MIN_RMS_THRESHOLD` (approx -96 dBFS), returns `0.0`.
/// Otherwise returns `raw_rms.clamp(0.0, 1.0)`.
#[must_use]
pub fn apply_silence_clamp(raw_rms: f64) -> f64 {
    if !raw_rms.is_finite() || raw_rms <= MIN_RMS_THRESHOLD {
        0.0
    } else {
        raw_rms.clamp(0.0, 1.0)
    }
}

/// Clamp silence floor returning both clamped linear RMS and clamped dBFS.
#[must_use]
pub fn clamp_silence_floor(raw_rms: f64) -> (f64, f64) {
    if !raw_rms.is_finite() || raw_rms <= MIN_RMS_THRESHOLD {
        (0.0, SILENCE_FLOOR_DBFS)
    } else {
        let clamped_rms = raw_rms.clamp(0.0, 1.0);
        let dbfs = (20.0 * clamped_rms.log10()).clamp(SILENCE_FLOOR_DBFS, 0.0);
        (clamped_rms, dbfs)
    }
}

/// Chunk a continuous slice of 16-bit PCM samples into blocks of duration `block_duration_sec`.
///
/// Computes raw RMS, dBFS, and normalized RMS for each block.
///
/// # Errors
/// Returns `AppError::Highlight(HighlightError::AudioAnalysisFailed)` if `sample_rate == 0`
/// or `block_duration_sec <= 0.0` or not finite.
pub fn chunk_samples_to_blocks(
    samples: &[i16],
    sample_rate: usize,
    block_duration_sec: f64,
) -> Result<Vec<AudioBlockMetrics>, AppError> {
    if sample_rate == 0 {
        return Err(AppError::Highlight(HighlightError::AudioAnalysisFailed(
            "sample_rate must be greater than 0".to_string(),
        )));
    }
    if !block_duration_sec.is_finite() || block_duration_sec <= 0.0 {
        return Err(AppError::Highlight(HighlightError::AudioAnalysisFailed(
            format!("Invalid block_duration_sec: {block_duration_sec} (must be finite and > 0)"),
        )));
    }

    if samples.is_empty() {
        return Ok(Vec::new());
    }

    let block_size = ((sample_rate as f64) * block_duration_sec).round().max(1.0) as usize;

    let num_blocks = samples.len().div_ceil(block_size);
    let mut blocks = Vec::with_capacity(num_blocks);
    let mut offset = 0;
    let mut index = 0;

    while offset < samples.len() {
        let end = (offset + block_size).min(samples.len());
        let chunk = &samples[offset..end];
        let raw_rms = calculate_raw_rms(chunk);
        let dbfs = rms_to_dbfs(raw_rms);
        let normalized_rms = apply_silence_clamp(raw_rms);
        let timestamp_sec = (index as f64) * block_duration_sec;

        blocks.push(AudioBlockMetrics {
            index,
            timestamp_sec,
            sample_count: chunk.len(),
            raw_rms,
            dbfs,
            normalized_rms,
        });

        offset += block_size;
        index += 1;
    }

    Ok(blocks)
}

/// Convenience function chunking samples using default 16kHz sample rate and 500ms block duration.
///
/// # Errors
/// Propagates error if parameters are invalid.
pub fn chunk_samples_to_blocks_default(
    samples: &[i16],
) -> Result<Vec<AudioBlockMetrics>, AppError> {
    chunk_samples_to_blocks(samples, DEFAULT_SAMPLE_RATE, DEFAULT_BLOCK_DURATION_SEC)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_calculate_raw_rms_empty() {
        assert_eq!(calculate_raw_rms(&[]), 0.0);
    }

    #[test]
    fn test_calculate_raw_rms_silence() {
        let silence = vec![0i16; 8000];
        assert_eq!(calculate_raw_rms(&silence), 0.0);
        assert_eq!(calculate_sine_normalized_rms(&silence), 0.0);
    }

    #[test]
    fn test_calculate_raw_rms_1khz_sine() {
        // UT-11.1: 1kHz sine wave raw RMS ~ 0.7071 (+-0.005)
        let sample_rate = 16_000.0;
        let freq = 1000.0;
        let count = 8000; // 500ms at 16kHz = 500 cycles of 16 samples
        let mut sine_samples = Vec::with_capacity(count);

        for i in 0..count {
            let theta = 2.0 * std::f64::consts::PI * freq * (i as f64) / sample_rate;
            let val = (32767.0 * theta.sin()).round() as i16;
            sine_samples.push(val);
        }

        let raw_rms = calculate_raw_rms(&sine_samples);
        let delta = (raw_rms - SINE_1KHZ_EXPECTED_RMS).abs();
        assert!(
            delta <= SINE_1KHZ_RMS_TOLERANCE,
            "Raw RMS {raw_rms} outside expected {SINE_1KHZ_EXPECTED_RMS} ± {SINE_1KHZ_RMS_TOLERANCE}"
        );

        let sine_norm = calculate_sine_normalized_rms(&sine_samples);
        assert!(
            (0.999..=1.0).contains(&sine_norm),
            "Sine normalized RMS should be ~1.0, got {sine_norm}"
        );
    }

    #[test]
    fn test_silence_floor_clamping() {
        // Silence floor clamp at -96dBFS -> RMS = 0.0
        assert_eq!(rms_to_dbfs(0.0), SILENCE_FLOOR_DBFS);
        assert_eq!(rms_to_dbfs(MIN_RMS_THRESHOLD), SILENCE_FLOOR_DBFS);
        assert_eq!(rms_to_dbfs(1e-6), SILENCE_FLOOR_DBFS);

        assert_eq!(dbfs_to_rms(SILENCE_FLOOR_DBFS), 0.0);
        assert_eq!(dbfs_to_rms(-120.0), 0.0);
        assert_eq!(dbfs_to_normalized_scale(SILENCE_FLOOR_DBFS), 0.0);

        let (clamped_rms, clamped_dbfs) = clamp_silence_floor(0.0);
        assert_eq!(clamped_rms, 0.0);
        assert_eq!(clamped_dbfs, SILENCE_FLOOR_DBFS);
    }

    #[test]
    fn test_dbfs_conversions_roundtrip() {
        // Test roundtrip for audible values
        for test_dbfs in [-6.0, -12.0, -24.0, -48.0, -72.0] {
            let rms = dbfs_to_rms(test_dbfs);
            let recovered_dbfs = rms_to_dbfs(rms);
            assert!(
                (recovered_dbfs - test_dbfs).abs() < 1e-4,
                "Failed roundtrip for {test_dbfs} dBFS, got {recovered_dbfs}"
            );
        }
    }

    #[test]
    fn test_chunk_samples_to_blocks_validation() {
        let dummy = vec![0i16; 100];
        assert!(chunk_samples_to_blocks(&dummy, 0, 0.5).is_err());
        assert!(chunk_samples_to_blocks(&dummy, 16000, -0.5).is_err());
        assert!(chunk_samples_to_blocks(&dummy, 16000, f64::NAN).is_err());
    }

    #[test]
    fn test_chunk_samples_to_blocks_metrics() {
        let samples = vec![1000i16; 16000]; // 1 second of audio = 2 blocks of 500ms
        let blocks = chunk_samples_to_blocks(&samples, 16000, 0.5).unwrap();
        assert_eq!(blocks.len(), 2);
        assert_eq!(blocks[0].index, 0);
        assert_eq!(blocks[0].timestamp_sec, 0.0);
        assert_eq!(blocks[0].sample_count, 8000);
        assert_eq!(blocks[1].index, 1);
        assert_eq!(blocks[1].timestamp_sec, 0.5);
        assert_eq!(blocks[1].sample_count, 8000);
    }
}
