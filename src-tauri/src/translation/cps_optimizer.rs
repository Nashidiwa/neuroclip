//! Characters Per Second (CPS) Calculator and Time Borrowing Optimization Engine (`TASK-M4-19`, `M4.5`).
//!
//! Implements:
//! - UTF-8 character-based CPS calculation (`chars().count()`).
//! - Readability rating: Green (< 17.0), Yellow (17.0..=20.0), Red (> 20.0).
//! - Non-colliding Time Borrowing algorithm expanding `end_sec` into following silence gaps
//!   by up to 800ms while preserving a minimum 50ms buffer (`AC-19.2`, `ED-19.1`).
//! - Schema 5.5 `transcript_segments.json` compatibility and container optimization.
//! - Pure Rust, zero unwraps, zero expects, clippy pedantic compliant.

#![allow(clippy::cast_precision_loss)]

use serde::{Deserialize, Serialize};

use crate::error::AppError;
use crate::stt::transcript_parser::{
    TranscriptSegment, TranscriptSegmentsContainer, CONFIDENCE_FLAG_THRESHOLD, CPS_FLAG_THRESHOLD,
    MIN_SEGMENT_DURATION_SEC,
};

/// Target reading speed in characters per second (safe green threshold).
pub const CPS_GREEN_THRESHOLD: f64 = 17.0;

/// Maximum allowable reading speed before warning (yellow/red boundary).
pub const CPS_RED_THRESHOLD: f64 = CPS_FLAG_THRESHOLD; // 20.0

/// Default target CPS when performing time borrowing.
pub const DEFAULT_TARGET_CPS: f64 = 17.0;

/// Default maximum time to borrow into following silence (800ms).
pub const DEFAULT_MAX_BORROW_MS: u64 = 800;

/// Minimum safety gap required between consecutive segments (50ms).
pub const DEFAULT_MIN_GAP_MS: u64 = 50;

/// CPS reading speed classification.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CpsRating {
    /// Safe reading speed (< 17.0 CPS)
    Green,
    /// Warning reading speed (17.0 ..= 20.0 CPS)
    Yellow,
    /// Over-speed reading limit (> 20.0 CPS)
    Red,
}

/// Configuration parameters for the Time Borrowing algorithm.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TimeBorrowingConfig {
    /// Maximum time in milliseconds to borrow into following silence (default: 800ms).
    pub max_borrow_ms: u64,
    /// Minimum buffer gap in milliseconds before next segment start (default: 50ms).
    pub min_gap_ms: u64,
    /// Target CPS to reduce towards (default: 17.0).
    pub target_cps: f64,
    /// Threshold CPS above which time borrowing is activated (default: 20.0).
    pub trigger_cps: f64,
    /// Optional upper bound on total clip / VOD duration in seconds.
    pub max_clip_duration_sec: Option<f64>,
}

impl Default for TimeBorrowingConfig {
    fn default() -> Self {
        Self {
            max_borrow_ms: DEFAULT_MAX_BORROW_MS,
            min_gap_ms: DEFAULT_MIN_GAP_MS,
            target_cps: DEFAULT_TARGET_CPS,
            trigger_cps: CPS_RED_THRESHOLD,
            max_clip_duration_sec: None,
        }
    }
}

/// Diagnostic report emitted by the Time Borrowing optimization engine.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct TimeBorrowingReport {
    /// Total number of segments evaluated.
    pub total_segments: usize,
    /// Number of segments with CPS > trigger threshold before optimization.
    pub initial_high_cps_count: usize,
    /// Number of segments whose end_sec was successfully extended.
    pub optimized_count: usize,
    /// Total duration in seconds borrowed across all segments.
    pub total_borrowed_sec: f64,
    /// Number of segments that remain with CPS > trigger threshold after optimization.
    pub remaining_high_cps_count: usize,
}

/// Rate CPS into Green, Yellow, or Red categories.
#[must_use]
pub fn rate_cps(cps: f64) -> CpsRating {
    if cps < CPS_GREEN_THRESHOLD {
        CpsRating::Green
    } else if cps <= CPS_RED_THRESHOLD {
        CpsRating::Yellow
    } else {
        CpsRating::Red
    }
}

/// Round a time value in seconds to millisecond precision (3 decimal places)
/// to eliminate IEEE-754 floating point arithmetic artifacts.
#[must_use]
pub fn round_to_millis(seconds: f64) -> f64 {
    (seconds * 1000.0).round() / 1000.0
}

/// Compute characters per second (CPS) for a given text and duration in seconds.
///
/// Uses Unicode UTF-8 character count (`chars().count()`), trimming surrounding whitespace.
/// Clamps duration to at least [`MIN_SEGMENT_DURATION_SEC`] (0.05s).
#[must_use]
pub fn calculate_cps(text: &str, duration_sec: f64) -> f64 {
    let clean_len = text.trim().chars().count() as f64;
    let duration = duration_sec.max(MIN_SEGMENT_DURATION_SEC);
    let cps = clean_len / duration;
    if cps.is_finite() && cps >= 0.0 {
        cps
    } else {
        0.0
    }
}

/// Compute characters per second (CPS) for start and end timestamp bounds.
#[must_use]
pub fn calculate_segment_cps(text: &str, start_sec: f64, end_sec: f64) -> f64 {
    let duration = (end_sec - start_sec).max(MIN_SEGMENT_DURATION_SEC);
    calculate_cps(text, duration)
}

/// Extract the effective subtitle text for a segment (prefers Vietnamese translation `text_vi` if present).
#[must_use]
pub fn effective_segment_text(segment: &TranscriptSegment) -> &str {
    match segment.text_vi {
        Some(ref s) if !s.trim().is_empty() => s.as_str(),
        _ => segment.text_en.as_str(),
    }
}

/// Calculate the allowable borrowed duration in seconds for an individual segment.
#[must_use]
pub fn calculate_borrow_duration(
    text: &str,
    start_sec: f64,
    end_sec: f64,
    next_start_sec: Option<f64>,
    config: &TimeBorrowingConfig,
) -> f64 {
    let current_duration = (end_sec - start_sec).max(MIN_SEGMENT_DURATION_SEC);
    let current_cps = calculate_cps(text, current_duration);

    // Only borrow if current CPS exceeds trigger threshold
    if current_cps <= config.trigger_cps {
        return 0.0;
    }

    let char_count = text.trim().chars().count() as f64;
    if char_count == 0.0 || config.target_cps <= 0.0 {
        return 0.0;
    }

    let desired_duration = char_count / config.target_cps;
    let needed_extension = (desired_duration - current_duration).max(0.0);
    if needed_extension <= 0.0 {
        return 0.0;
    }

    let max_borrow_sec = config.max_borrow_ms as f64 / 1000.0;
    let min_gap_sec = config.min_gap_ms as f64 / 1000.0;

    let max_gap_extension = match (next_start_sec, config.max_clip_duration_sec) {
        (Some(next_start), _) => (next_start - end_sec - min_gap_sec).max(0.0),
        (None, Some(clip_end)) => (clip_end - end_sec).max(0.0),
        (None, None) => f64::INFINITY,
    };

    let allowed = needed_extension.min(max_borrow_sec).min(max_gap_extension);
    if allowed.is_finite() && allowed > 0.0 {
        round_to_millis(allowed)
    } else {
        0.0
    }
}

/// Optimize CPS across a mutable slice of transcript segments using custom configuration.
pub fn optimize_cps_with_config(
    segments: &mut [TranscriptSegment],
    config: &TimeBorrowingConfig,
) -> TimeBorrowingReport {
    let total_segments = segments.len();
    if total_segments == 0 {
        return TimeBorrowingReport::default();
    }

    let mut initial_high_cps_count = 0;
    let mut optimized_count = 0;
    let mut total_borrowed_sec = 0.0;

    // First pass: update cps_score and count initial high CPS segments
    for seg in segments.iter_mut() {
        let text = effective_segment_text(seg);
        let cps = calculate_segment_cps(text, seg.start_sec, seg.end_sec);
        seg.cps_score = cps;
        if cps > config.trigger_cps {
            initial_high_cps_count += 1;
        }
    }

    // Second pass: sequential time borrowing
    for i in 0..total_segments {
        let next_start = segments.get(i + 1).map(|next_seg| next_seg.start_sec);

        let Some(seg) = segments.get_mut(i) else {
            continue;
        };

        let text = effective_segment_text(seg);
        let borrow_sec =
            calculate_borrow_duration(text, seg.start_sec, seg.end_sec, next_start, config);

        if borrow_sec > 0.0 {
            seg.end_sec = round_to_millis(seg.end_sec + borrow_sec);
            let text = effective_segment_text(seg);
            let new_cps = calculate_segment_cps(text, seg.start_sec, seg.end_sec);
            seg.cps_score = new_cps;

            seg.is_flagged_for_review =
                seg.confidence < CONFIDENCE_FLAG_THRESHOLD || new_cps > config.trigger_cps;

            optimized_count += 1;
            total_borrowed_sec += borrow_sec;
        }
    }

    let remaining_high_cps_count = segments
        .iter()
        .filter(|s| s.cps_score > config.trigger_cps)
        .count();

    TimeBorrowingReport {
        total_segments,
        initial_high_cps_count,
        optimized_count,
        total_borrowed_sec: round_to_millis(total_borrowed_sec),
        remaining_high_cps_count,
    }
}

/// Standard interface adhering to TASK-M4-19 specification:
/// `optimize_cps_time_borrowing(segments: &mut [TranscriptSegment], max_borrow_ms: u64, min_gap_ms: u64, target_cps: f64)`.
pub fn optimize_cps_time_borrowing(
    segments: &mut [TranscriptSegment],
    max_borrow_ms: u64,
    min_gap_ms: u64,
    target_cps: f64,
) -> TimeBorrowingReport {
    let config = TimeBorrowingConfig {
        max_borrow_ms,
        min_gap_ms,
        target_cps,
        trigger_cps: CPS_RED_THRESHOLD,
        max_clip_duration_sec: None,
    };
    optimize_cps_with_config(segments, &config)
}

/// Optimize CPS using project defaults (800ms max borrow, 50ms min gap, 17.0 target CPS).
pub fn optimize_cps_time_borrowing_default(
    segments: &mut [TranscriptSegment],
) -> TimeBorrowingReport {
    let config = TimeBorrowingConfig::default();
    optimize_cps_with_config(segments, &config)
}

/// Optimize entire container adhering to Schema 5.5.
pub fn optimize_container_cps(
    container: &mut TranscriptSegmentsContainer,
    config: &TimeBorrowingConfig,
) -> TimeBorrowingReport {
    optimize_cps_with_config(&mut container.segments, config)
}

/// Serialize a [`TranscriptSegmentsContainer`] to valid Schema 5.5 JSON string.
///
/// # Errors
/// Returns [`AppError::Internal`] if JSON serialization fails.
pub fn serialize_transcript_segments_json(
    container: &TranscriptSegmentsContainer,
) -> Result<String, AppError> {
    serde_json::to_string_pretty(container)
        .map_err(|e| AppError::Internal(format!("Failed to serialize Schema 5.5 JSON: {e}")))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_segment(
        id: &str,
        start_sec: f64,
        end_sec: f64,
        text_en: &str,
        text_vi: Option<&str>,
        confidence: f64,
    ) -> TranscriptSegment {
        let text = text_vi.unwrap_or(text_en);
        let cps_score = calculate_segment_cps(text, start_sec, end_sec);
        TranscriptSegment {
            segment_id: id.to_string(),
            start_sec,
            end_sec,
            speaker_id: "neuro".to_string(),
            text_en: text_en.to_string(),
            text_vi: text_vi.map(ToString::to_string),
            confidence,
            cps_score,
            is_flagged_for_review: confidence < CONFIDENCE_FLAG_THRESHOLD
                || cps_score > CPS_RED_THRESHOLD,
            words: Vec::new(),
        }
    }

    #[test]
    fn test_ut_19_2_cps_math_red_overflow() {
        // 40 Vietnamese characters in 1.5 seconds -> CPS = 40 / 1.5 ≈ 26.67 > 20.0
        // Exact 40 chars: "Một hai ba bốn năm sáu bảy tám chín mười" = 40 chars
        let vi_text_40 = "Một hai ba bốn năm sáu bảy tám chín mười";
        assert_eq!(vi_text_40.chars().count(), 40);

        let cps = calculate_cps(vi_text_40, 1.5);
        assert!((cps - 26.6667).abs() < 0.01, "CPS must be ~26.7, got {cps}");
        assert!(
            cps > CPS_RED_THRESHOLD,
            "CPS must exceed red threshold 20.0"
        );
        assert_eq!(rate_cps(cps), CpsRating::Red);
    }

    #[test]
    fn test_ut_19_3_time_borrowing_calculation() {
        // CPS = 26.7, followed by 1.0s silence gap before next segment (start = 2.5s).
        // Algorithm automatically extends end_sec by 0.8s -> new CPS = 40 / 2.3 ≈ 17.4 <= 20.0.
        let vi_text_40 = "Một hai ba bốn năm sáu bảy tám chín mười";
        let mut segments = vec![
            create_test_segment(
                "seg_0001",
                0.0,
                1.5,
                "Forty character sentence in english here!!",
                Some(vi_text_40),
                0.95,
            ),
            create_test_segment(
                "seg_0002",
                2.5,
                4.0,
                "Next sentence here",
                Some("Câu tiếp theo"),
                0.95,
            ),
        ];

        let report = optimize_cps_time_borrowing(&mut segments, 800, 50, 17.0);

        assert_eq!(report.total_segments, 2);
        assert_eq!(report.initial_high_cps_count, 1);
        assert_eq!(report.optimized_count, 1);
        assert!((report.total_borrowed_sec - 0.8).abs() < 1e-4);

        let seg0 = &segments[0];
        assert!(
            (seg0.end_sec - 2.3).abs() < 1e-4,
            "end_sec must be 2.3s, got {}",
            seg0.end_sec
        );
        assert!(
            (seg0.cps_score - 17.3913).abs() < 0.05,
            "New CPS must be ~17.4, got {}",
            seg0.cps_score
        );
        assert!(
            seg0.cps_score <= CPS_RED_THRESHOLD,
            "New CPS must be <= 20.0"
        );
        assert!(
            !seg0.is_flagged_for_review,
            "Red flag must be cleared when CPS <= 20 and confidence >= 0.6"
        );

        // Distance to next segment must be >= 50ms (2.5 - 2.3 = 0.2s = 200ms >= 50ms)
        assert!(segments[1].start_sec - seg0.end_sec >= 0.050);
    }

    #[test]
    fn test_ed_19_1_collision_avoidance() {
        // Gap to next segment is only 100ms (next_start = 1.6s).
        // Time borrowing must NOT extend by 800ms; it must cap at 1.6 - 1.5 - 0.05 = 50ms!
        let vi_text_40 = "Một hai ba bốn năm sáu bảy tám chín mười";
        let mut segments = vec![
            create_test_segment("seg_0001", 0.0, 1.5, "English text", Some(vi_text_40), 0.95),
            create_test_segment(
                "seg_0002",
                1.6,
                3.0,
                "Next sentence",
                Some("Câu tiếp theo"),
                0.95,
            ),
        ];

        let report = optimize_cps_time_borrowing(&mut segments, 800, 50, 17.0);
        assert_eq!(report.optimized_count, 1);

        let seg0 = &segments[0];
        assert!(
            (seg0.end_sec - 1.55).abs() < 1e-4,
            "end_sec must cap at 1.55s, got {}",
            seg0.end_sec
        );
        assert!(
            (segments[1].start_sec - seg0.end_sec - 0.05).abs() < 1e-4,
            "Exact 50ms buffer preserved"
        );
        assert!(seg0.end_sec <= segments[1].start_sec - 0.050);
    }

    #[test]
    fn test_insufficient_silence_gap_sub_50ms() {
        // Gap is only 30ms (next_start = 1.53s). Less than 50ms buffer.
        // Borrowing must be 0ms.
        let vi_text_40 = "Một hai ba bốn năm sáu bảy tám chín mười";
        let mut segments = vec![
            create_test_segment("seg_0001", 0.0, 1.5, "English text", Some(vi_text_40), 0.95),
            create_test_segment(
                "seg_0002",
                1.53,
                3.0,
                "Next sentence",
                Some("Câu tiếp"),
                0.95,
            ),
        ];

        let report = optimize_cps_time_borrowing(&mut segments, 800, 50, 17.0);
        assert_eq!(report.optimized_count, 0);
        assert!((segments[0].end_sec - 1.5).abs() < 1e-4);
        assert!(segments[0].is_flagged_for_review);
    }

    #[test]
    fn test_last_segment_with_clip_duration_cap() {
        // Last segment in clip with clip duration = 2.0s.
        // Needed: 0.85s. Max borrow: 0.8s. Gap to clip end: 2.0 - 1.5 = 0.5s.
        // Must cap at 0.5s -> end_sec = 2.0s.
        let vi_text_40 = "Một hai ba bốn năm sáu bảy tám chín mười";
        let mut segments = vec![create_test_segment(
            "seg_0001",
            0.0,
            1.5,
            "English text",
            Some(vi_text_40),
            0.95,
        )];

        let config = TimeBorrowingConfig {
            max_borrow_ms: 800,
            min_gap_ms: 50,
            target_cps: 17.0,
            trigger_cps: 20.0,
            max_clip_duration_sec: Some(2.0),
        };

        let report = optimize_cps_with_config(&mut segments, &config);
        assert_eq!(report.optimized_count, 1);
        assert!((segments[0].end_sec - 2.0).abs() < 1e-4);
        assert!(segments[0].end_sec <= 2.0);
    }

    #[test]
    fn test_consecutive_high_cps_segments() {
        // Two consecutive high CPS segments, each followed by silence.
        let text = "Một hai ba bốn năm sáu bảy tám chín mười"; // 40 chars
        let mut segments = vec![
            create_test_segment("seg_0001", 0.0, 1.0, "Fast line 1", Some(text), 0.95), // 40 CPS
            create_test_segment("seg_0002", 2.0, 3.0, "Fast line 2", Some(text), 0.95), // 40 CPS
            create_test_segment(
                "seg_0003",
                5.0,
                6.0,
                "Normal line",
                Some("Bình thường"),
                0.95,
            ),
        ];

        let report = optimize_cps_time_borrowing(&mut segments, 800, 50, 17.0);
        assert_eq!(report.optimized_count, 2);

        // Seg 0 borrowed 800ms -> ends at 1.8s (<= 2.0 - 0.05 = 1.95s)
        assert!((segments[0].end_sec - 1.8).abs() < 1e-4);
        assert!(segments[1].start_sec - segments[0].end_sec >= 0.05);

        // Seg 1 borrowed 800ms -> ends at 3.8s (<= 5.0 - 0.05 = 4.95s)
        assert!((segments[1].end_sec - 3.8).abs() < 1e-4);
        assert!(segments[2].start_sec - segments[1].end_sec >= 0.05);
    }

    #[test]
    fn test_it_19_1_schema_5_5_serialization_roundtrip() {
        let vi_text_40 = "Một hai ba bốn năm sáu bảy tám chín mười";
        let segments = vec![create_test_segment(
            "seg_0001",
            0.0,
            1.5,
            "English text",
            Some(vi_text_40),
            0.95,
        )];
        let mut container = TranscriptSegmentsContainer {
            clip_id: "clip_m4_19_test".to_string(),
            segments,
        };

        optimize_container_cps(&mut container, &TimeBorrowingConfig::default());

        let json_res = serialize_transcript_segments_json(&container);
        assert!(json_res.is_ok(), "Serialization must succeed");
        let json_str = json_res.unwrap_or_default();

        // Validate deserialization roundtrip
        let roundtrip_res: Result<TranscriptSegmentsContainer, _> = serde_json::from_str(&json_str);
        assert!(
            roundtrip_res.is_ok(),
            "Must deserialize back into TranscriptSegmentsContainer"
        );
        let roundtrip = roundtrip_res.unwrap_or(container);

        assert_eq!(roundtrip.clip_id, "clip_m4_19_test");
        assert_eq!(roundtrip.segments.len(), 1);
        assert_eq!(roundtrip.segments[0].segment_id, "seg_0001");
        assert!((roundtrip.segments[0].end_sec - 2.3).abs() < 1e-4);
        assert!((roundtrip.segments[0].cps_score - 17.3913).abs() < 0.05);
    }
}
