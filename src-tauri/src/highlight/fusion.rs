//! Multimodal Fusion Engine (TASK-M4-12)
//!
//! Complies with `docs/CONVENTIONS.md` (zero-panic invariant, Result<T, AppError>),
//! `docs/ARCHITECTURE.md` §4.3 & §5.4, and `docs/TASK_BOARD.md` TASK-M4-12.
//!
//! Implements multi-tier weighted composite scoring:
//! $$Score = (0.60 \cdot Z_{\text{norm}} + 0.25 \cdot E_{\text{ratio}} + 0.15 \cdot \text{RMS}_{\text{norm}}) \times 100.0$$
//! and automatic fallback routing to 100% Audio Energy when chat is absent:
//! $$Score = 1.0 \times \text{RMS}_{\text{norm}} \times 100.0$$

#![allow(
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss
)]

use serde::{Deserialize, Serialize};

use crate::error::AppError;
use crate::highlight::emote_filter::EmoteDensityPoint;
use crate::highlight::energy_analyzer::AudioRmsPoint;
use crate::highlight::zscore::ChatVelocityPoint;

/// Weight assigned to chat velocity Z-Score ($Z_{\text{adj}}$) in multimodal fusion.
pub const WEIGHT_CHAT_Z: f64 = 0.60;

/// Weight assigned to Swarm Emote Ratio ($E_{\text{ratio}}$) in multimodal fusion.
pub const WEIGHT_EMOTE_RATIO: f64 = 0.25;

/// Weight assigned to normalized Audio RMS energy in multimodal fusion.
pub const WEIGHT_AUDIO_RMS: f64 = 0.15;

/// Weight assigned to Audio RMS in fallback audio-only mode.
pub const WEIGHT_AUDIO_ONLY_RMS: f64 = 1.0;

/// Reference adjusted Z-Score value that maps to normalized 1.0 (UT-12.1).
pub const MAX_Z_SCORE_REFERENCE: f64 = 10.0;

/// Minimum composite score threshold to consider a point as an active highlight candidate.
pub const MIN_HIGHLIGHT_SCORE_THRESHOLD: f64 = 15.0;

/// Standard chat bucket duration in seconds.
pub const CHAT_BUCKET_WIDTH_SEC: f64 = 5.0;

/// Standard audio RMS block duration in seconds.
pub const AUDIO_BLOCK_WIDTH_SEC: f64 = 0.5;

/// Fused score point across aligned multimodal time series.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct FusedScorePoint {
    /// Timestamp in seconds from stream start.
    pub timestamp_sec: f64,
    /// Final composite score in range [0.0, 100.0].
    pub score: f64,
    /// Raw adjusted Z-score if chat velocity was present.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub chat_z_score: Option<f64>,
    /// Normalized Swarm emote ratio if emote density was present.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub emote_ratio: Option<f64>,
    /// Peak normalized Audio RMS energy in this time bucket.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub audio_rms: Option<f64>,
    /// Top cultural Swarm emotes occurring in this bucket.
    #[serde(default)]
    pub top_emotes: Vec<String>,
    /// Whether any sub-stream flagged a peak at this timestamp.
    pub is_peak: bool,
    /// Flag indicating this point was computed via audio-only fallback mode.
    pub is_audio_only: bool,
}

impl FusedScorePoint {
    /// Create a new FusedScorePoint.
    #[must_use]
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        timestamp_sec: f64,
        score: f64,
        chat_z_score: Option<f64>,
        emote_ratio: Option<f64>,
        audio_rms: Option<f64>,
        top_emotes: Vec<String>,
        is_peak: bool,
        is_audio_only: bool,
    ) -> Self {
        let sanitized_score = if score.is_finite() {
            score.clamp(0.0, 100.0)
        } else {
            0.0
        };

        Self {
            timestamp_sec,
            score: sanitized_score,
            chat_z_score,
            emote_ratio,
            audio_rms,
            top_emotes,
            is_peak,
            is_audio_only,
        }
    }
}

/// Normalize raw adjusted Z-score to $[0.0, 1.0]$ using $Z_{\text{norm}} = \text{clamp}(Z_{\text{adj}} / 10.0, 0.0, 1.0)$.
#[inline]
#[must_use]
pub fn normalize_z_score(z: f64) -> f64 {
    if !z.is_finite() || z <= 0.0 {
        return 0.0;
    }
    (z / MAX_Z_SCORE_REFERENCE).clamp(0.0, 1.0)
}

/// Compute multimodal composite score in $[0.0, 100.0]$:
/// $$Score = (0.60 \cdot Z_{\text{norm}} + 0.25 \cdot E_{\text{ratio}} + 0.15 \cdot \text{RMS}_{\text{norm}}) \times 100.0$$
#[must_use]
pub fn compute_multimodal_score(z_norm: f64, e_ratio: f64, rms_norm: f64) -> f64 {
    let z = if z_norm.is_finite() {
        z_norm.clamp(0.0, 1.0)
    } else {
        0.0
    };
    let e = if e_ratio.is_finite() {
        e_ratio.clamp(0.0, 1.0)
    } else {
        0.0
    };
    let rms = if rms_norm.is_finite() {
        rms_norm.clamp(0.0, 1.0)
    } else {
        0.0
    };

    let raw = WEIGHT_CHAT_Z * z + WEIGHT_EMOTE_RATIO * e + WEIGHT_AUDIO_RMS * rms;
    (raw * 100.0).clamp(0.0, 100.0)
}

/// Compute audio-only fallback score in $[0.0, 100.0]$:
/// $$Score = 1.0 \times \text{RMS}_{\text{norm}} \times 100.0$$
#[inline]
#[must_use]
pub fn compute_audio_only_score(rms_norm: f64) -> f64 {
    if !rms_norm.is_finite() || rms_norm <= 0.0 {
        return 0.0;
    }
    (WEIGHT_AUDIO_ONLY_RMS * rms_norm.clamp(0.0, 1.0) * 100.0).clamp(0.0, 100.0)
}

/// Aggregates audio RMS blocks in window $[start\_sec, end\_sec)$.
/// Returns `(peak_rms, has_peak)`.
#[must_use]
pub fn aggregate_audio_in_window(
    audio_points: &[AudioRmsPoint],
    start_sec: f64,
    end_sec: f64,
) -> (f64, bool) {
    let mut peak_rms = 0.0;
    let mut has_peak = false;

    // Binary search or linear slice for window
    for pt in audio_points {
        if pt.timestamp_sec >= end_sec {
            break;
        }
        if pt.timestamp_sec + AUDIO_BLOCK_WIDTH_SEC > start_sec {
            if pt.rms_energy > peak_rms {
                peak_rms = pt.rms_energy;
            }
            if pt.is_peak {
                has_peak = true;
            }
        }
    }

    (peak_rms, has_peak)
}

/// Perform multimodal fusion across Chat Velocity, Emote Density, and Audio RMS time series.
///
/// If chat velocity and emote density series are empty (e.g. stream without chat replay),
/// automatically falls back to 100% audio energy processing at 500ms resolution (ERR-05).
///
/// # Errors
/// Returns `AppError` if duration is non-positive or series formatting fails.
pub fn fuse_multimodal_series(
    velocity_series: &[ChatVelocityPoint],
    emote_series: &[EmoteDensityPoint],
    audio_series: &[AudioRmsPoint],
    duration_sec: f64,
) -> Result<Vec<FusedScorePoint>, AppError> {
    if !duration_sec.is_finite() || duration_sec < 0.0 {
        return Ok(Vec::new());
    }

    // Audio-Only Fallback check
    let is_chat_available = !velocity_series.is_empty() || !emote_series.is_empty();

    if !is_chat_available {
        // Fallback to 500ms audio blocks directly
        let mut fused_points = Vec::with_capacity(audio_series.len());
        for pt in audio_series {
            if pt.timestamp_sec > duration_sec {
                break;
            }
            let score = compute_audio_only_score(pt.rms_energy);
            fused_points.push(FusedScorePoint::new(
                pt.timestamp_sec,
                score,
                None,
                None,
                Some(pt.rms_energy),
                Vec::new(),
                pt.is_peak,
                true,
            ));
        }
        return Ok(fused_points);
    }

    // Multimodal Alignment: Group into 5.0s chat buckets
    let num_buckets = ((duration_sec / CHAT_BUCKET_WIDTH_SEC).ceil() as usize) + 1;
    let mut fused_points = Vec::with_capacity(num_buckets);

    // Build lookup maps or aligned indexed lookups for 5.0s intervals
    // Since velocity and emote series are generated at k * 5.0s, we can align via bucket index
    let mut vel_idx = 0;
    let mut emote_idx = 0;
    let mut audio_idx = 0;

    let total_steps = ((duration_sec / CHAT_BUCKET_WIDTH_SEC).floor() as usize) + 1;

    for k in 0..total_steps {
        let t_k = (k as f64) * CHAT_BUCKET_WIDTH_SEC;
        if t_k > duration_sec {
            break;
        }
        let t_next = t_k + CHAT_BUCKET_WIDTH_SEC;

        // Advance velocity
        let mut z_score_val = 0.0;
        while vel_idx < velocity_series.len()
            && velocity_series[vel_idx].timestamp_sec < t_k - 0.001
        {
            vel_idx += 1;
        }
        if vel_idx < velocity_series.len()
            && (velocity_series[vel_idx].timestamp_sec - t_k).abs() < 2.5
        {
            z_score_val = velocity_series[vel_idx].z_score;
        }

        // Advance emote
        let mut e_ratio_val = 0.0;
        let mut top_emotes_val = Vec::new();
        while emote_idx < emote_series.len() && emote_series[emote_idx].timestamp_sec < t_k - 0.001
        {
            emote_idx += 1;
        }
        if emote_idx < emote_series.len()
            && (emote_series[emote_idx].timestamp_sec - t_k).abs() < 2.5
        {
            e_ratio_val = emote_series[emote_idx].e_ratio;
            top_emotes_val.clone_from(&emote_series[emote_idx].top_emotes);
        }

        // Aggregate 10x 500ms audio blocks for this 5s bucket
        let mut max_audio_rms = 0.0;
        let mut audio_has_peak = false;
        while audio_idx < audio_series.len()
            && audio_series[audio_idx].timestamp_sec + AUDIO_BLOCK_WIDTH_SEC <= t_k
        {
            audio_idx += 1;
        }

        let mut scan_idx = audio_idx;
        while scan_idx < audio_series.len() && audio_series[scan_idx].timestamp_sec < t_next {
            let apt = &audio_series[scan_idx];
            if apt.rms_energy > max_audio_rms {
                max_audio_rms = apt.rms_energy;
            }
            if apt.is_peak {
                audio_has_peak = true;
            }
            scan_idx += 1;
        }

        // Compute scores
        let z_norm = normalize_z_score(z_score_val);
        let score = compute_multimodal_score(z_norm, e_ratio_val, max_audio_rms);
        let is_peak = audio_has_peak || z_score_val >= 2.0 || score >= 60.0;

        fused_points.push(FusedScorePoint::new(
            t_k,
            score,
            Some(z_score_val),
            Some(e_ratio_val),
            Some(max_audio_rms),
            top_emotes_val,
            is_peak,
            false,
        ));
    }

    Ok(fused_points)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// UT-12.1: Full Multimodal Weighting:
    /// Z_adj = 10.0 (Z_norm = 1.0), E = 0.8, RMS = 0.6 => Score = 89.0 (0.89 * 100)
    #[test]
    fn test_ut_12_1_full_multimodal_weighting() {
        let z_adj = 10.0;
        let z_norm = normalize_z_score(z_adj);
        assert!((z_norm - 1.0).abs() < 1e-6);

        let e_ratio = 0.8;
        let rms_norm = 0.6;

        let score = compute_multimodal_score(z_norm, e_ratio, rms_norm);
        // 0.6 * 1.0 + 0.25 * 0.8 + 0.15 * 0.6 = 0.60 + 0.20 + 0.09 = 0.89 => 89.0
        assert!(
            (score - 89.0).abs() < 1e-4,
            "Expected score 89.0, got {score}"
        );
    }

    /// UT-12.2: Audio-Only Fallback Weighting:
    /// When Z = 0, E = 0 (or absent), Score = 1.0 * RMS * 100.0
    #[test]
    fn test_ut_12_2_audio_only_fallback() {
        let rms_norm = 0.75;
        let score = compute_audio_only_score(rms_norm);
        assert!(
            (score - 75.0).abs() < 1e-4,
            "Expected score 75.0, got {score}"
        );

        let silence_score = compute_audio_only_score(0.0);
        assert_eq!(silence_score, 0.0);
    }

    #[test]
    fn test_z_score_normalization_clamping() {
        assert_eq!(normalize_z_score(-5.0), 0.0);
        assert_eq!(normalize_z_score(0.0), 0.0);
        assert!((normalize_z_score(5.0) - 0.5).abs() < 1e-6);
        assert_eq!(normalize_z_score(15.0), 1.0);
        assert_eq!(normalize_z_score(f64::NAN), 0.0);
    }

    #[test]
    fn test_multimodal_series_fusion_flow() {
        let vel = vec![
            ChatVelocityPoint::new(0.0, 5, 0.0),
            ChatVelocityPoint::new(5.0, 25, 3.5),
            ChatVelocityPoint::new(10.0, 50, 10.0),
        ];
        let emote = vec![
            EmoteDensityPoint::new(0.0, vec![], 0.0, 10, 0, 0.0),
            EmoteDensityPoint::new(5.0, vec!["KEKW".to_string()], 0.5, 30, 15, 0.5),
            EmoteDensityPoint::new(10.0, vec!["tutel".to_string()], 0.8, 50, 40, 0.8),
        ];
        let audio = vec![
            AudioRmsPoint::new(0.0, 0.1, false),
            AudioRmsPoint::new(5.0, 0.4, false),
            AudioRmsPoint::new(10.0, 0.6, true),
        ];

        let fused = fuse_multimodal_series(&vel, &emote, &audio, 15.0);
        assert!(fused.is_ok());
        if let Ok(points) = fused {
            assert_ne!(points, []);
            // Point at 10.0s should have highest score
            let p10 = points
                .iter()
                .find(|p| (p.timestamp_sec - 10.0).abs() < 1e-3);
            assert!(p10.is_some());
            if let Some(p) = p10 {
                assert!((p.score - 89.0).abs() < 1e-2);
                assert!(p.is_peak);
            }
        }
    }
}
