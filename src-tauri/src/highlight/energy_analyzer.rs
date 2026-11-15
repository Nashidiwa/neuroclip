//! Multimodal Energy Analyzer & Adaptive Baseline Peak Detection Subsystem.
//!
//! Complies with `docs/CONVENTIONS.md` (zero-panic invariant, Result<T, AppError>),
//! `docs/ARCHITECTURE.md` §4.3 & §5.4, and `docs/TASK_BOARD.md` TASK-M4-11.

use std::fs::File;
use std::io::{BufReader, Read, Seek, SeekFrom};
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::error::{AppError, HighlightError};
use crate::highlight::audio_rms::{
    chunk_samples_to_blocks, rms_to_dbfs, AudioBlockMetrics, DEFAULT_BLOCK_DURATION_SEC,
    DEFAULT_SAMPLE_RATE, MIN_RMS_THRESHOLD, SILENCE_FLOOR_DBFS,
};

/// Default lookback baseline window duration in seconds (30.0s).
pub const DEFAULT_BASELINE_WINDOW_SEC: f64 = 30.0;

/// Default threshold for relative dB spike over baseline (+18.0 dB).
/// A sudden volume jump of >= 18.0 dB (e.g. +21.0 dB per UT-11.3) triggers a peak.
pub const DEFAULT_PEAK_DB_SPIKE_THRESHOLD: f64 = 18.0;

/// Alias for `DEFAULT_PEAK_DB_SPIKE_THRESHOLD`.
pub const PEAK_DB_THRESHOLD: f64 = DEFAULT_PEAK_DB_SPIKE_THRESHOLD;

/// Default statistical threshold in standard deviations above baseline mean (>= 2.5 sigma).
pub const DEFAULT_PEAK_SIGMA_THRESHOLD: f64 = 2.5;

/// Alias for `DEFAULT_PEAK_SIGMA_THRESHOLD`.
pub const PEAK_SIGMA_THRESHOLD: f64 = DEFAULT_PEAK_SIGMA_THRESHOLD;

/// Minimum variance epsilon to avoid division by zero when calculating Z-scores.
pub const EPSILON_RMS: f64 = 1e-6;

/// Audio RMS data point compliant with Schema 5.4 and IT-11.1 data contract.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct AudioRmsPoint {
    /// Start timestamp of the 500ms block in seconds.
    pub timestamp_sec: f64,
    /// Root Mean Square energy normalized in range [0.0, 1.0].
    pub rms_energy: f64,
    /// Whether this block represents an energy peak (surge >= 18dB OR outlier >= 2.5 sigma).
    pub is_peak: bool,
    /// Optional Decibels relative to Full Scale [-96.0, 0.0] dBFS.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dbfs: Option<f64>,
    /// Optional relative volume spike over local 30s baseline in dB.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub delta_db: Option<f64>,
    /// Optional statistical Z-score over local 30s baseline.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub z_score: Option<f64>,
}

impl AudioRmsPoint {
    /// Create a minimal AudioRmsPoint with mandatory contract fields.
    #[must_use]
    pub fn new(timestamp_sec: f64, rms_energy: f64, is_peak: bool) -> Self {
        Self {
            timestamp_sec,
            rms_energy,
            is_peak,
            dbfs: None,
            delta_db: None,
            z_score: None,
        }
    }

    /// Create an AudioRmsPoint populated with full diagnostic metrics.
    #[must_use]
    pub fn with_diagnostics(
        timestamp_sec: f64,
        rms_energy: f64,
        is_peak: bool,
        dbfs: f64,
        delta_db: f64,
        z_score: f64,
    ) -> Self {
        Self {
            timestamp_sec,
            rms_energy,
            is_peak,
            dbfs: Some(dbfs),
            delta_db: Some(delta_db),
            z_score: Some(z_score),
        }
    }
}

/// Container for complete audio RMS series response.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct AudioRmsSeriesResponse {
    /// Optional identifier of the analyzed VOD.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub vod_id: Option<String>,
    /// Duration of each analyzed block in seconds (typically 0.5s).
    pub block_duration_sec: f64,
    /// Duration of the lookback baseline window in seconds (typically 30.0s).
    pub baseline_window_sec: f64,
    /// Total number of 500ms blocks in the series.
    pub total_blocks: usize,
    /// Total number of detected energy peaks.
    pub peak_count: usize,
    /// Full array of sequential audio RMS points.
    pub points: Vec<AudioRmsPoint>,
}

/// Configuration settings for the audio energy analyzer.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EnergyAnalyzerConfig {
    /// Audio sample rate in Hz (default: 16,000 Hz).
    pub sample_rate: usize,
    /// Block duration in seconds (default: 0.5s = 500ms).
    pub block_duration_sec: f64,
    /// Lookback baseline window duration in seconds (default: 30.0s).
    pub baseline_window_sec: f64,
    /// Volume surge threshold in dB above baseline (default: 18.0 dB).
    pub peak_db_threshold: f64,
    /// Statistical outlier threshold in standard deviations above baseline (default: 2.5 sigma).
    pub peak_sigma_threshold: f64,
}

impl Default for EnergyAnalyzerConfig {
    fn default() -> Self {
        Self {
            sample_rate: DEFAULT_SAMPLE_RATE,
            block_duration_sec: DEFAULT_BLOCK_DURATION_SEC,
            baseline_window_sec: DEFAULT_BASELINE_WINDOW_SEC,
            peak_db_threshold: DEFAULT_PEAK_DB_SPIKE_THRESHOLD,
            peak_sigma_threshold: DEFAULT_PEAK_SIGMA_THRESHOLD,
        }
    }
}

/// Statistics calculated over the 30-second sliding baseline window.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct EnergyBaselineStats {
    /// Mean linear normalized RMS over the baseline lookback window.
    pub mean_rms: f64,
    /// Standard deviation of linear normalized RMS over baseline lookback window.
    pub std_dev_rms: f64,
    /// Mean dBFS over the baseline lookback window.
    pub mean_dbfs: f64,
}

/// Detect energy peaks over pre-chunked `AudioBlockMetrics` using an adaptive 30s sliding baseline window.
///
/// Implemented via $O(N)$ running cumulative prefix sums.
///
/// # Dual Peak Detection Criteria:
/// A block is flagged `is_peak = true` if and only if it is NOT silence and:
/// 1. Relative Volume Spike: $\Delta\text{dB} \ge \text{peak\_db\_threshold}$ (e.g. +21dB spike $\ge$ 18dB), OR
/// 2. Statistical Outlier: $(R - \mu) \ge \text{peak\_sigma\_threshold} \cdot \sigma$ (e.g. $\ge 2.5\sigma$).
///
/// # Invariants:
/// - Silence blocks (RMS <= -96dBFS) can NEVER be peaks.
/// - Cold start (first block) has no prior baseline: `is_peak = false`.
///
/// # Errors
/// Returns `AppError::Highlight(HighlightError::AudioAnalysisFailed)` if parameters are non-finite or <= 0.
pub fn detect_energy_peaks(
    blocks: &[AudioBlockMetrics],
    baseline_window_sec: f64,
    block_duration_sec: f64,
    peak_db_threshold: f64,
    peak_sigma_threshold: f64,
) -> Result<Vec<AudioRmsPoint>, AppError> {
    if !baseline_window_sec.is_finite() || baseline_window_sec <= 0.0 {
        return Err(AppError::Highlight(HighlightError::AudioAnalysisFailed(
            format!("Invalid baseline_window_sec: {baseline_window_sec} (must be finite and > 0)"),
        )));
    }
    if !block_duration_sec.is_finite() || block_duration_sec <= 0.0 {
        return Err(AppError::Highlight(HighlightError::AudioAnalysisFailed(
            format!("Invalid block_duration_sec: {block_duration_sec} (must be finite and > 0)"),
        )));
    }

    let n = blocks.len();
    if n == 0 {
        return Ok(Vec::new());
    }

    let lookback_k = (baseline_window_sec / block_duration_sec)
        .round()
        .max(1.0) as usize;

    // Cumulative prefix sums for RMS, RMS^2, and dBFS enabling O(1) sliding window statistics
    let mut prefix_rms = Vec::with_capacity(n + 1);
    let mut prefix_rms_sq = Vec::with_capacity(n + 1);
    let mut prefix_dbfs = Vec::with_capacity(n + 1);

    prefix_rms.push(0.0);
    prefix_rms_sq.push(0.0);
    prefix_dbfs.push(0.0);

    let mut running_rms = 0.0;
    let mut running_rms_sq = 0.0;
    let mut running_dbfs = 0.0;

    for b in blocks {
        running_rms += b.normalized_rms;
        running_rms_sq += b.normalized_rms * b.normalized_rms;
        running_dbfs += b.dbfs;

        prefix_rms.push(running_rms);
        prefix_rms_sq.push(running_rms_sq);
        prefix_dbfs.push(running_dbfs);
    }

    let mut points = Vec::with_capacity(n);

    for i in 0..n {
        let b = &blocks[i];
        let start = i.saturating_sub(lookback_k);
        let m = i - start;

        let (mu_rms, sigma_rms, mu_dbfs) = if m == 0 {
            // Cold start: no preceding blocks in window
            (b.normalized_rms, 0.0, b.dbfs)
        } else {
            let sum_rms = prefix_rms[i] - prefix_rms[start];
            let sum_rms_sq = prefix_rms_sq[i] - prefix_rms_sq[start];
            let sum_dbfs = prefix_dbfs[i] - prefix_dbfs[start];
            let m_f = m as f64;

            let mean_rms = sum_rms / m_f;
            let mean_rms_sq = sum_rms_sq / m_f;
            let variance = (mean_rms_sq - mean_rms * mean_rms).max(0.0);

            (mean_rms, variance.sqrt(), sum_dbfs / m_f)
        };

        // Silence gating: blocks at or below silence floor are never peaks
        let is_silence = b.normalized_rms <= MIN_RMS_THRESHOLD || b.dbfs <= SILENCE_FLOOR_DBFS;

        // Relative volume spike evaluation
        let delta_db_from_mean = b.dbfs - mu_dbfs;
        let delta_db_from_ratio = if mu_rms > MIN_RMS_THRESHOLD && b.normalized_rms > 0.0 {
            20.0 * (b.normalized_rms / mu_rms).log10()
        } else if b.normalized_rms > MIN_RMS_THRESHOLD {
            b.dbfs - SILENCE_FLOOR_DBFS
        } else {
            0.0
        };
        let delta_db = delta_db_from_mean.max(delta_db_from_ratio);

        // Statistical Z-score evaluation
        let z_score = if sigma_rms > EPSILON_RMS {
            (b.normalized_rms - mu_rms) / sigma_rms
        } else {
            0.0
        };

        // Dual-condition peak classification
        let is_db_peak = delta_db >= peak_db_threshold;
        let is_sigma_peak =
            sigma_rms > EPSILON_RMS && (b.normalized_rms - mu_rms) >= peak_sigma_threshold * sigma_rms;

        let is_peak = !is_silence && (is_db_peak || is_sigma_peak);

        points.push(AudioRmsPoint::with_diagnostics(
            b.timestamp_sec,
            b.normalized_rms,
            is_peak,
            b.dbfs,
            delta_db,
            z_score,
        ));
    }

    Ok(points)
}

/// Convenience function detecting energy peaks using default 30s baseline, 18dB jump, and 2.5 sigma.
///
/// # Errors
/// Propagates error if baseline configuration parameters are invalid.
pub fn detect_energy_peaks_default(
    blocks: &[AudioBlockMetrics],
) -> Result<Vec<AudioRmsPoint>, AppError> {
    detect_energy_peaks(
        blocks,
        DEFAULT_BASELINE_WINDOW_SEC,
        DEFAULT_BLOCK_DURATION_SEC,
        DEFAULT_PEAK_DB_SPIKE_THRESHOLD,
        DEFAULT_PEAK_SIGMA_THRESHOLD,
    )
}

/// Analyze raw PCM samples directly using default configuration parameters.
///
/// Returns complete array of 500ms `AudioRmsPoint` entries.
///
/// # Errors
/// Propagates error if sample rate is 0 or processing fails.
pub fn analyze_audio_samples_default(
    samples: &[i16],
    sample_rate: usize,
) -> Result<Vec<AudioRmsPoint>, AppError> {
    let blocks = chunk_samples_to_blocks(samples, sample_rate, DEFAULT_BLOCK_DURATION_SEC)?;
    detect_energy_peaks_default(&blocks)
}

/// Analyze raw PCM samples with explicit `EnergyAnalyzerConfig`.
///
/// # Errors
/// Propagates error if configuration is invalid or processing fails.
pub fn analyze_audio_samples(
    samples: &[i16],
    config: &EnergyAnalyzerConfig,
) -> Result<AudioRmsSeriesResponse, AppError> {
    let blocks = chunk_samples_to_blocks(samples, config.sample_rate, config.block_duration_sec)?;
    let points = detect_energy_peaks(
        &blocks,
        config.baseline_window_sec,
        config.block_duration_sec,
        config.peak_db_threshold,
        config.peak_sigma_threshold,
    )?;

    let peak_count = points.iter().filter(|p| p.is_peak).count();
    let total_blocks = points.len();

    Ok(AudioRmsSeriesResponse {
        vod_id: None,
        block_duration_sec: config.block_duration_sec,
        baseline_window_sec: config.baseline_window_sec,
        total_blocks,
        peak_count,
        points,
    })
}

/// Process a stream of 16-bit PCM bytes from a reader in 500ms chunks with bounded memory consumption.
///
/// Peak RAM is strictly bounded to the chunk size (< 64KB) during ingestion.
///
/// # Errors
/// Returns error on I/O failure or invalid configuration parameters.
pub fn process_pcm_reader<R: Read>(
    mut reader: R,
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
            format!("Invalid block_duration_sec: {block_duration_sec}"),
        )));
    }

    let block_samples = ((sample_rate as f64) * block_duration_sec)
        .round()
        .max(1.0) as usize;
    let block_bytes = block_samples * 2; // 2 bytes per 16-bit PCM sample

    let mut byte_buffer = vec![0u8; block_bytes];
    let mut sample_buffer = Vec::with_capacity(block_samples);
    let mut blocks = Vec::new();
    let mut block_index = 0;

    loop {
        let mut total_read = 0;
        while total_read < block_bytes {
            match reader.read(&mut byte_buffer[total_read..]) {
                Ok(0) => break, // EOF reached
                Ok(n) => total_read += n,
                Err(e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
                Err(e) => {
                    return Err(AppError::Highlight(HighlightError::AudioAnalysisFailed(
                        format!("I/O error reading PCM audio stream: {e}"),
                    )));
                }
            }
        }

        if total_read == 0 {
            break;
        }

        // Convert little-endian bytes to i16 samples
        sample_buffer.clear();
        let samples_count = total_read / 2;
        for i in 0..samples_count {
            let lo = byte_buffer[i * 2];
            let hi = byte_buffer[i * 2 + 1];
            sample_buffer.push(i16::from_le_bytes([lo, hi]));
        }

        if !sample_buffer.is_empty() {
            let raw_rms = crate::highlight::audio_rms::calculate_raw_rms(&sample_buffer);
            let dbfs = rms_to_dbfs(raw_rms);
            let normalized_rms = if raw_rms <= MIN_RMS_THRESHOLD {
                0.0
            } else {
                raw_rms
            };
            let timestamp_sec = (block_index as f64) * block_duration_sec;

            blocks.push(AudioBlockMetrics {
                index: block_index,
                timestamp_sec,
                sample_count: sample_buffer.len(),
                raw_rms,
                dbfs,
                normalized_rms,
            });

            block_index += 1;
        }

        if total_read < block_bytes {
            // Partial block at EOF
            break;
        }
    }

    Ok(blocks)
}

/// Analyze a WAV audio file on disk with streaming reading without loading the entire file into RAM.
///
/// Supports standard 16-bit PCM WAV files (mono or stereo downmixed to mono).
///
/// # Errors
/// Returns error if the file cannot be opened, has an invalid WAV/RIFF header, or processing fails.
pub fn analyze_wav_file(path: &Path) -> Result<Vec<AudioRmsPoint>, AppError> {
    let file = File::open(path).map_err(|e| {
        AppError::Highlight(HighlightError::AudioAnalysisFailed(format!(
            "Failed to open WAV file '{}': {e}",
            path.display()
        )))
    })?;

    let mut reader = BufReader::new(file);

    // Read and parse standard 44-byte RIFF/WAVE header
    let mut header = [0u8; 44];
    reader.read_exact(&mut header).map_err(|e| {
        AppError::Highlight(HighlightError::AudioAnalysisFailed(format!(
            "Failed to read WAV header from '{}': {e}",
            path.display()
        )))
    })?;

    if &header[0..4] != b"RIFF" || &header[8..12] != b"WAVE" {
        return Err(AppError::Highlight(HighlightError::AudioAnalysisFailed(
            format!("Invalid WAV magic header in '{}'", path.display()),
        )));
    }

    // Audio format must be 1 (uncompressed PCM)
    let audio_format = u16::from_le_bytes([header[20], header[21]]);
    if audio_format != 1 {
        return Err(AppError::Highlight(HighlightError::AudioAnalysisFailed(
            format!("Unsupported WAV audio format ({audio_format}), only uncompressed PCM (1) supported"),
        )));
    }

    let channels = u16::from_le_bytes([header[22], header[23]]) as usize;
    let sample_rate = u32::from_le_bytes([header[24], header[25], header[26], header[27]]) as usize;
    let bits_per_sample = u16::from_le_bytes([header[34], header[35]]);

    if bits_per_sample != 16 {
        return Err(AppError::Highlight(HighlightError::AudioAnalysisFailed(
            format!("Unsupported bits per sample ({bits_per_sample}), only 16-bit PCM supported"),
        )));
    }

    if channels != 1 && channels != 2 {
        return Err(AppError::Highlight(HighlightError::AudioAnalysisFailed(
            format!("Unsupported channel count ({channels}), only mono (1) or stereo (2) supported"),
        )));
    }

    // Locate the "data" subchunk start
    if &header[36..40] != b"data" {
        // Fallback: Seek to search for data chunk if header has non-standard fmt length
        reader.seek(SeekFrom::Start(12)).map_err(|e| {
            AppError::Highlight(HighlightError::AudioAnalysisFailed(format!(
                "Seek failed in WAV file: {e}"
            )))
        })?;

        let mut found_data = false;
        let mut chunk_header = [0u8; 8];
        while reader.read_exact(&mut chunk_header).is_ok() {
            let chunk_id = &chunk_header[0..4];
            let chunk_size = u32::from_le_bytes([
                chunk_header[4],
                chunk_header[5],
                chunk_header[6],
                chunk_header[7],
            ]) as i64;

            if chunk_id == b"data" {
                found_data = true;
                break;
            }

            if chunk_size < 0 {
                break;
            }
            if reader.seek(SeekFrom::Current(chunk_size)).is_err() {
                break;
            }
        }

        if !found_data {
            return Err(AppError::Highlight(HighlightError::AudioAnalysisFailed(
                format!("Could not locate 'data' subchunk in WAV file '{}'", path.display()),
            )));
        }
    }

    // If stereo, convert to mono on the fly
    let blocks = if channels == 1 {
        process_pcm_reader(reader, sample_rate, DEFAULT_BLOCK_DURATION_SEC)?
    } else {
        // Stereo reader downmixing to mono
        process_stereo_pcm_reader(reader, sample_rate, DEFAULT_BLOCK_DURATION_SEC)?
    };

    detect_energy_peaks_default(&blocks)
}

/// Helper processing stereo 16-bit PCM by downmixing left and right channels to mono.
fn process_stereo_pcm_reader<R: Read>(
    mut reader: R,
    sample_rate: usize,
    block_duration_sec: f64,
) -> Result<Vec<AudioBlockMetrics>, AppError> {
    let block_samples = ((sample_rate as f64) * block_duration_sec)
        .round()
        .max(1.0) as usize;
    let block_bytes = block_samples * 4; // 2 channels * 2 bytes = 4 bytes per stereo sample

    let mut byte_buffer = vec![0u8; block_bytes];
    let mut sample_buffer = Vec::with_capacity(block_samples);
    let mut blocks = Vec::new();
    let mut block_index = 0;

    loop {
        let mut total_read = 0;
        while total_read < block_bytes {
            match reader.read(&mut byte_buffer[total_read..]) {
                Ok(0) => break,
                Ok(n) => total_read += n,
                Err(e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
                Err(e) => {
                    return Err(AppError::Highlight(HighlightError::AudioAnalysisFailed(
                        format!("I/O error reading stereo WAV: {e}"),
                    )));
                }
            }
        }

        if total_read == 0 {
            break;
        }

        sample_buffer.clear();
        let stereo_samples_count = total_read / 4;
        for i in 0..stereo_samples_count {
            let left = i16::from_le_bytes([byte_buffer[i * 4], byte_buffer[i * 4 + 1]]);
            let right = i16::from_le_bytes([byte_buffer[i * 4 + 2], byte_buffer[i * 4 + 3]]);
            // Average stereo channels
            let mono = ((i32::from(left) + i32::from(right)) / 2) as i16;
            sample_buffer.push(mono);
        }

        if !sample_buffer.is_empty() {
            let raw_rms = crate::highlight::audio_rms::calculate_raw_rms(&sample_buffer);
            let dbfs = rms_to_dbfs(raw_rms);
            let normalized_rms = if raw_rms <= MIN_RMS_THRESHOLD {
                0.0
            } else {
                raw_rms
            };
            let timestamp_sec = (block_index as f64) * block_duration_sec;

            blocks.push(AudioBlockMetrics {
                index: block_index,
                timestamp_sec,
                sample_count: sample_buffer.len(),
                raw_rms,
                dbfs,
                normalized_rms,
            });

            block_index += 1;
        }

        if total_read < block_bytes {
            break;
        }
    }

    Ok(blocks)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_energy_peaks_empty() {
        let res = detect_energy_peaks_default(&[]).unwrap();
        assert!(res.is_empty());
    }

    #[test]
    fn test_detect_energy_peaks_silence_cannot_be_peak() {
        let mut blocks = Vec::new();
        for i in 0..100 {
            blocks.push(AudioBlockMetrics {
                index: i,
                timestamp_sec: (i as f64) * 0.5,
                sample_count: 8000,
                raw_rms: 0.0,
                dbfs: SILENCE_FLOOR_DBFS,
                normalized_rms: 0.0,
            });
        }
        let points = detect_energy_peaks_default(&blocks).unwrap();
        assert_eq!(points.len(), 100);
        for p in &points {
            assert!(!p.is_peak, "Silence block at ts {} was flagged as peak", p.timestamp_sec);
        }
    }

    #[test]
    fn test_detect_energy_peaks_volume_spike_21db() {
        // UT-11.3: Baseline at -30dBFS for 30s (60 blocks), then +21dB spike to -9dBFS at block 60
        let baseline_rms = 10.0_f64.powf(-30.0 / 20.0);
        let spike_rms = 10.0_f64.powf(-9.0 / 20.0); // +21dB spike

        let mut blocks = Vec::new();
        for i in 0..60 {
            blocks.push(AudioBlockMetrics {
                index: i,
                timestamp_sec: (i as f64) * 0.5,
                sample_count: 8000,
                raw_rms: baseline_rms,
                dbfs: -30.0,
                normalized_rms: baseline_rms,
            });
        }

        // Block 60: sudden volume spike (+21dB)
        blocks.push(AudioBlockMetrics {
            index: 60,
            timestamp_sec: 30.0,
            sample_count: 8000,
            raw_rms: spike_rms,
            dbfs: -9.0,
            normalized_rms: spike_rms,
        });

        let points = detect_energy_peaks_default(&blocks).unwrap();
        assert_eq!(points.len(), 61);

        // Blocks 0..59 are steady baseline
        for (i, p) in points[..60].iter().enumerate() {
            assert!(!p.is_peak, "Baseline block {i} should not be a peak");
        }

        // Block 60 must be a peak
        assert!(points[60].is_peak, "Spike block 60 (+21dB) must be flagged is_peak = true");
        assert_eq!(points[60].timestamp_sec, 30.0);
        if let Some(delta) = points[60].delta_db {
            assert!((delta - 21.0).abs() < 0.1, "Delta dB should be ~21.0 dB, got {delta}");
        }
    }

    #[test]
    fn test_detect_energy_peaks_statistical_outlier_sigma() {
        // UT-11.2: Baseline with variance, block >= 2.5 sigma is marked peak
        let mut blocks = Vec::new();
        // 60 blocks fluctuating between 0.09 and 0.11 (mean = 0.10, std dev approx 0.01)
        for i in 0..60 {
            let val = if i % 2 == 0 { 0.09 } else { 0.11 };
            let db = rms_to_dbfs(val);
            blocks.push(AudioBlockMetrics {
                index: i,
                timestamp_sec: (i as f64) * 0.5,
                sample_count: 8000,
                raw_rms: val,
                dbfs: db,
                normalized_rms: val,
            });
        }

        // Block 60: outlier at 0.14 (> 3.5 sigma)
        let outlier_val = 0.14;
        let outlier_db = rms_to_dbfs(outlier_val);
        blocks.push(AudioBlockMetrics {
            index: 60,
            timestamp_sec: 30.0,
            sample_count: 8000,
            raw_rms: outlier_val,
            dbfs: outlier_db,
            normalized_rms: outlier_val,
        });

        let points = detect_energy_peaks_default(&blocks).unwrap();
        assert!(
            points[60].is_peak,
            "Block 60 outlier >= 2.5 sigma should be flagged is_peak = true"
        );
        if let Some(z) = points[60].z_score {
            assert!(z >= 2.5, "Z-score should be >= 2.5, got {z}");
        }
    }

    #[test]
    fn test_process_pcm_reader_synthetic() {
        let count = 16000; // 1s at 16kHz
        let mut raw_bytes = Vec::with_capacity(count * 2);
        for _ in 0..count {
            raw_bytes.extend_from_slice(&1000i16.to_le_bytes());
        }

        let blocks = process_pcm_reader(&raw_bytes[..], 16000, 0.5).unwrap();
        assert_eq!(blocks.len(), 2);
        assert_eq!(blocks[0].sample_count, 8000);
        assert_eq!(blocks[1].sample_count, 8000);
    }
}
