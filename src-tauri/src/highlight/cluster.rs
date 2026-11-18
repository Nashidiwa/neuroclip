//! Highlight Clustering & Latency Compensation Engine (TASK-M4-12)
//!
//! Complies with `docs/CONVENTIONS.md` (zero-panic invariant, Result<T, AppError>),
//! `docs/ARCHITECTURE.md` §4.3 & §5.4 Schema 5.4, and `docs/TASK_BOARD.md` TASK-M4-12.
//!
//! Enforces:
//! - 4.5s livestream CDN latency compensation + 10.0s pre-roll buffer = 14.5s pre-offset.
//! - Post-roll buffer: 15.0s.
//! - Dynamic padding at stream end (ED-12.3): shifts missing post-roll duration into pre-roll.
//! - Strict boundary invariants: $0.0 \le t_{\text{start}} < t_{\text{end}} \le \text{duration}$.
//! - Distance clustering: merges peaks with gap $\le 30.0\text{s}$ (IT-12.1).
//! - Deduplication / NMS: suppresses clusters with $> 50\%$ duration overlap (AC-12.3).
//! - Ranks top 15–20 clusters according to Schema 5.4.

use serde::{Deserialize, Serialize};

use crate::error::AppError;
use crate::highlight::fusion::FusedScorePoint;

/// Livestream viewer reaction latency compensation in seconds (AC-12.2, ADR-004).
pub const STREAM_LATENCY_COMPENSATION_SEC: f64 = 4.5;

/// Default pre-roll buffer prior to compensated peak in seconds.
pub const DEFAULT_PRE_ROLL_SEC: f64 = 10.0;

/// Total pre-peak offset combining latency compensation and pre-roll (14.5s).
pub const DEFAULT_TOTAL_PRE_OFFSET_SEC: f64 =
    STREAM_LATENCY_COMPENSATION_SEC + DEFAULT_PRE_ROLL_SEC;

/// Default post-roll buffer after peak in seconds.
pub const DEFAULT_POST_ROLL_SEC: f64 = 15.0;

/// Maximum allowable pre-roll buffer when dynamic padding expands near EOF (ED-12.3).
pub const MAX_PRE_ROLL_SEC: f64 = 20.0;

/// Minimum allowable clip duration in seconds.
pub const MIN_CLIP_DURATION_SEC: f64 = 15.0;

/// Maximum temporal gap between peaks to merge them into a single cluster (IT-12.1).
pub const CLUSTER_MERGE_DISTANCE_SEC: f64 = 30.0;

/// Maximum duration overlap ratio allowed between two clusters before NMS suppression (AC-12.3).
pub const MAX_OVERLAP_RATIO: f64 = 0.50;

/// Default maximum number of top clusters to retain and output (Schema 5.4).
pub const DEFAULT_MAX_CLUSTERS: usize = 20;

/// Highlight cluster entity strictly conforming to Schema 5.4 in ARCHITECTURE.md.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct HighlightCluster {
    /// Unique cluster identifier, formatted as "cluster_<index>".
    pub cluster_id: String,
    /// Start timecode in seconds with latency compensation & pre-roll applied.
    pub start_sec: f64,
    /// End timecode in seconds with post-roll applied.
    pub end_sec: f64,
    /// Peak timecode of highest interest in seconds.
    pub peak_sec: f64,
    /// Normalized composite score in range [0.0, 100.0].
    pub score: f64,
    /// Human-readable explanation of why this moment is significant.
    pub primary_reason: String,
    /// Optional Z-score at focal peak if chat was active.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub chat_z_score: Option<f64>,
    /// Optional peak RMS energy in this cluster window.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub audio_rms_peak: Option<f64>,
    /// Dominant cultural Swarm emotes occurring in this cluster window.
    #[serde(default)]
    pub top_emotes: Vec<String>,
    /// Whether user bookmarked this cluster (default false).
    #[serde(default)]
    pub is_bookmarked: bool,
}

impl HighlightCluster {
    /// Construct a new HighlightCluster.
    #[must_use]
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        cluster_id: String,
        start_sec: f64,
        end_sec: f64,
        peak_sec: f64,
        score: f64,
        primary_reason: String,
        chat_z_score: Option<f64>,
        audio_rms_peak: Option<f64>,
        top_emotes: Vec<String>,
        is_bookmarked: bool,
    ) -> Self {
        Self {
            cluster_id,
            start_sec,
            end_sec,
            peak_sec,
            score,
            primary_reason,
            chat_z_score,
            audio_rms_peak,
            top_emotes,
            is_bookmarked,
        }
    }

    /// Duration of the highlight cluster in seconds.
    #[must_use]
    pub fn duration_sec(&self) -> f64 {
        (self.end_sec - self.start_sec).max(0.0)
    }
}

/// Container object conforming to Schema 5.4 document format on disk and IPC wire.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct HighlightClustersContainer {
    /// Unique identifier for the VOD.
    pub vod_id: String,
    /// Ranked highlight clusters.
    pub clusters: Vec<HighlightCluster>,
    /// Optional total number of clusters found.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub total_clusters: Option<usize>,
    /// Optional ISO 8601 generation timestamp.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub generated_at: Option<String>,
}

impl HighlightClustersContainer {
    /// Construct a new HighlightClustersContainer.
    #[must_use]
    pub fn new(vod_id: String, clusters: Vec<HighlightCluster>) -> Self {
        let count = clusters.len();
        Self {
            vod_id,
            clusters,
            total_clusters: Some(count),
            generated_at: None,
        }
    }
}

/// Calculate clip bounds for a peak or cluster window with 4.5s latency compensation,
/// 10.0s pre-roll, 15.0s post-roll, zero-clamping (ED-12.1), and dynamic padding at EOF (ED-12.3).
///
/// Invariant guaranteed: $0.0 \le t_{\text{start}} < t_{\text{end}} \le \text{vod\_duration\_sec}$.
#[must_use]
pub fn calculate_clip_bounds(
    earliest_peak_sec: f64,
    latest_peak_sec: f64,
    vod_duration_sec: f64,
    is_audio_only: bool,
) -> (f64, f64) {
    let duration = if vod_duration_sec.is_finite() && vod_duration_sec > 0.0 {
        vod_duration_sec
    } else {
        0.0
    };

    let latency = if is_audio_only {
        0.0
    } else {
        STREAM_LATENCY_COMPENSATION_SEC
    };

    let initial_pre_roll = DEFAULT_PRE_ROLL_SEC;
    let target_post_roll = DEFAULT_POST_ROLL_SEC;

    // 1. Initial end clamp
    let end_sec = (latest_peak_sec + target_post_roll).min(duration);

    // 2. Base start clamp
    let base_start = (earliest_peak_sec - latency - initial_pre_roll).max(0.0);
    let mut start_sec = base_start;

    // 3. Dynamic padding at stream end (ED-12.3):
    // If the clip duration falls below MIN_CLIP_DURATION_SEC (15.0s) due to EOF clamping,
    // shift missing post-roll duration into pre-roll buffer (up to MAX_PRE_ROLL_SEC 20.0s).
    if end_sec - start_sec < MIN_CLIP_DURATION_SEC && duration >= MIN_CLIP_DURATION_SEC {
        let actual_post_roll = (end_sec - latest_peak_sec).max(0.0);
        let missing_post_roll = (target_post_roll - actual_post_roll).max(0.0);
        let needed_extension = MIN_CLIP_DURATION_SEC - (end_sec - start_sec);
        let extension = missing_post_roll
            .max(needed_extension)
            .min(MAX_PRE_ROLL_SEC - initial_pre_roll);
        start_sec = (earliest_peak_sec - latency - (initial_pre_roll + extension)).max(0.0);

        if end_sec - start_sec < MIN_CLIP_DURATION_SEC {
            start_sec = (end_sec - MIN_CLIP_DURATION_SEC).max(0.0);
        }
    }

    // 4. Invariant check: 0.0 <= start < end <= duration
    if start_sec >= end_sec && duration > 0.0 {
        start_sec = (end_sec - 1.0).max(0.0);
    }

    // Round to 1 decimal place
    let rounded_start = (start_sec * 10.0).round() / 10.0;
    let rounded_end = (end_sec * 10.0).round() / 10.0;

    (rounded_start, rounded_end)
}

/// Compute duration overlap ratio between two time intervals:
/// $$\text{overlap\_ratio} = \frac{\max(0.0, \min(end_A, end_B) - \max(start_A, start_B))}{\min(D_A, D_B)}$$
#[must_use]
pub fn compute_overlap_ratio(start_a: f64, end_a: f64, start_b: f64, end_b: f64) -> f64 {
    let dur_a = (end_a - start_a).max(0.0);
    let dur_b = (end_b - start_b).max(0.0);
    let min_dur = dur_a.min(dur_b);

    if min_dur <= 0.0 {
        return 0.0;
    }

    let overlap_start = start_a.max(start_b);
    let overlap_end = end_a.min(end_b);
    let overlap = (overlap_end - overlap_start).max(0.0);

    overlap / min_dur
}

/// Heuristically determine human-readable primary reason for a highlight cluster in Vietnamese.
#[must_use]
pub fn determine_primary_reason(
    top_emotes: &[String],
    chat_z_score: Option<f64>,
    audio_rms_peak: Option<f64>,
    is_audio_only: bool,
) -> String {
    // 1. Emote-based culture indicators
    for emote in top_emotes {
        let upper = emote.to_ascii_uppercase();
        if upper.contains("KEKW")
            || upper.contains("LUL")
            || upper.contains("OMEGALUL")
            || upper.contains("💀")
            || upper.contains("LMAO")
        {
            return "Spam KEKW / LUL bùng nổ".to_string();
        }
        if upper.contains("TUTEL")
            || upper.contains("GYMBAG")
            || upper.contains("VEDAL")
            || upper.contains("NEURO")
            || upper.contains("EVIL")
            || upper.contains("HEART")
        {
            return format!("Văn hóa Swarm bùng nổ ({emote})");
        }
    }

    // 2. Chat Z-score reaction
    if let Some(z) = chat_z_score {
        if z >= 3.0 {
            return format!("Phản ứng chat bùng nổ (Z: {z:.1})");
        }
    }

    // 3. Audio RMS peak
    if is_audio_only {
        return "Đỉnh năng lượng âm thanh (Audio Energy Peak)".to_string();
    }

    if let Some(rms) = audio_rms_peak {
        if rms >= 0.70 {
            return "Vedal hét to / Đỉnh năng lượng âm thanh".to_string();
        }
    }

    "Khoảnh khắc cao trào đa phương thức".to_string()
}

/// Cluster candidate fused points within 30s distance, apply deduplication (> 50% overlap),
/// rank by composite score descending, and extract top 15–20 clusters.
#[must_use]
#[allow(clippy::too_many_lines)]
pub fn cluster_and_deduplicate(
    fused_points: &[FusedScorePoint],
    vod_duration_sec: f64,
    max_clusters: usize,
) -> Vec<HighlightCluster> {
    if fused_points.is_empty() || vod_duration_sec <= 0.0 {
        return Vec::new();
    }

    // 1. Filter candidate points
    let mut candidates: Vec<&FusedScorePoint> = fused_points
        .iter()
        .filter(|p| {
            p.score >= 15.0
                || p.is_peak
                || p.chat_z_score.is_some_and(|z| z >= 2.0)
                || p.audio_rms.is_some_and(|rms| rms >= 0.5)
        })
        .collect();

    if candidates.is_empty() {
        // Fallback: take top scoring points > 0
        candidates = fused_points.iter().filter(|p| p.score > 0.0).collect();
    }

    if candidates.is_empty() {
        return Vec::new();
    }

    // Sort candidates chronologically
    candidates.sort_by(|a, b| {
        a.timestamp_sec
            .partial_cmp(&b.timestamp_sec)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    // 2. 30-Second Distance Clustering (IT-12.1)
    let mut raw_groups: Vec<Vec<&FusedScorePoint>> = Vec::new();
    let mut current_group: Vec<&FusedScorePoint> = Vec::new();

    for pt in candidates {
        if let Some(last) = current_group.last() {
            if pt.timestamp_sec - last.timestamp_sec <= CLUSTER_MERGE_DISTANCE_SEC {
                current_group.push(pt);
            } else {
                let finished = std::mem::take(&mut current_group);
                raw_groups.push(finished);
                current_group.push(pt);
            }
        } else {
            current_group.push(pt);
        }
    }
    if !current_group.is_empty() {
        raw_groups.push(current_group);
    }

    // 3. Convert groups into provisional clusters
    let mut provisional_clusters: Vec<HighlightCluster> = Vec::with_capacity(raw_groups.len());

    for group in raw_groups {
        if group.is_empty() {
            continue;
        }

        // Focal peak is point with maximum score (earliest if tied)
        let focal = group.iter().fold(
            group[0],
            |best, item| {
                if item.score > best.score {
                    item
                } else {
                    best
                }
            },
        );

        let earliest_sec = group
            .first()
            .map_or(focal.timestamp_sec, |p| p.timestamp_sec);
        let latest_sec = group
            .last()
            .map_or(focal.timestamp_sec, |p| p.timestamp_sec);

        let is_audio_only = group.iter().all(|p| p.is_audio_only);

        let (start_sec, end_sec) =
            calculate_clip_bounds(earliest_sec, latest_sec, vod_duration_sec, is_audio_only);

        // Aggregate unique top emotes up to 5
        let mut emotes_set = Vec::new();
        for p in &group {
            for e in &p.top_emotes {
                if !emotes_set.contains(e) && emotes_set.len() < 5 {
                    emotes_set.push(e.clone());
                }
            }
        }

        let max_rms = group.iter().filter_map(|p| p.audio_rms).fold(0.0, f64::max);
        let rms_opt = if max_rms > 0.0 { Some(max_rms) } else { None };

        let primary_reason =
            determine_primary_reason(&emotes_set, focal.chat_z_score, rms_opt, is_audio_only);

        provisional_clusters.push(HighlightCluster::new(
            String::new(), // cluster_id will be assigned after NMS ranking
            start_sec,
            end_sec,
            focal.timestamp_sec,
            focal.score,
            primary_reason,
            focal.chat_z_score,
            rms_opt,
            emotes_set,
            false,
        ));
    }

    // 4. Non-Maximum Suppression (NMS) with 50% overlap rule (AC-12.3)
    // Sort provisional clusters by score descending
    provisional_clusters.sort_by(|a, b| {
        b.score
            .partial_cmp(&a.score)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    let mut accepted: Vec<HighlightCluster> = Vec::with_capacity(provisional_clusters.len());

    for cand in provisional_clusters {
        let is_suppressed = accepted.iter().any(|acc| {
            compute_overlap_ratio(acc.start_sec, acc.end_sec, cand.start_sec, cand.end_sec)
                > MAX_OVERLAP_RATIO
        });

        if !is_suppressed {
            accepted.push(cand);
        }
    }

    // 5. Truncate to max_clusters (e.g. top 15-20)
    let limit = if max_clusters > 0 {
        max_clusters
    } else {
        DEFAULT_MAX_CLUSTERS
    };
    if accepted.len() > limit {
        accepted.truncate(limit);
    }

    // 6. Assign final cluster IDs and verify invariants
    for (i, cluster) in accepted.iter_mut().enumerate() {
        cluster.cluster_id = format!("cluster_{}", i + 1);

        // Strict invariant enforcement (AC-12.4)
        if cluster.start_sec < 0.0 {
            cluster.start_sec = 0.0;
        }
        if cluster.end_sec > vod_duration_sec {
            cluster.end_sec = vod_duration_sec;
        }
        if cluster.start_sec >= cluster.end_sec && vod_duration_sec > 0.0 {
            cluster.start_sec = (cluster.end_sec - 1.0).max(0.0);
        }
    }

    accepted
}

/// Generate and package highlight clusters container from fused points.
///
/// # Errors
/// Returns `AppError` if packaging fails or duration is invalid.
pub fn generate_clusters_from_fused_series(
    vod_id: &str,
    fused_points: &[FusedScorePoint],
    vod_duration_sec: f64,
) -> Result<HighlightClustersContainer, AppError> {
    let clusters = cluster_and_deduplicate(fused_points, vod_duration_sec, DEFAULT_MAX_CLUSTERS);
    Ok(HighlightClustersContainer::new(
        vod_id.to_string(),
        clusters,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// UT-12.3: Latency compensation & padding:
    /// peak_sec = 5070.0s (01:24:30.0), duration = 7200.0s
    /// start_sec = 5070.0 - 14.5 = 5055.5s (01:24:15.5)
    /// end_sec = 5070.0 + 15.0 = 5085.0s (01:24:45.0)
    /// length = 29.5s
    #[test]
    fn test_ut_12_3_latency_compensation_and_padding() {
        let peak_sec = 5070.0;
        let duration = 7200.0;
        let (start, end) = calculate_clip_bounds(peak_sec, peak_sec, duration, false);

        assert!(
            (start - 5055.5).abs() < 1e-4,
            "Expected start 5055.5, got {start}"
        );
        assert!(
            (end - 5085.0).abs() < 1e-4,
            "Expected end 5085.0, got {end}"
        );
        let len = end - start;
        assert!(
            (len - 29.5).abs() < 1e-4,
            "Expected length 29.5s, got {len}"
        );
    }

    /// ED-12.1: Start Clamping:
    /// Early peak at 8.0s, duration 3600.0s:
    /// start_sec = max(0.0, 8.0 - 14.5) = 0.0s (no negative timecode)
    /// end_sec = 8.0 + 15.0 = 23.0s
    #[test]
    fn test_ed_12_1_start_clamping() {
        let peak_sec = 8.0;
        let duration = 3600.0;
        let (start, end) = calculate_clip_bounds(peak_sec, peak_sec, duration, false);

        assert_eq!(start, 0.0, "Start must be clamped to 0.0s");
        assert!((end - 23.0).abs() < 1e-4, "Expected end 23.0, got {end}");
        assert!(start >= 0.0);
        assert!(start < end);
    }

    /// ED-12.3: EOF Dynamic Padding:
    /// duration = 10000.0s, peak = 9992.0s
    /// end_sec clamped to 10000.0s
    /// start_sec = 9977.5s, length = 22.5s >= 15.0s
    #[test]
    fn test_ed_12_3_eof_dynamic_padding() {
        let peak_sec = 9992.0;
        let duration = 10000.0;
        let (start, end) = calculate_clip_bounds(peak_sec, peak_sec, duration, false);

        assert_eq!(end, 10000.0, "End must be clamped to EOF 10000.0s");
        assert!(
            (start - 9977.5).abs() < 1e-4,
            "Expected start 9977.5, got {start}"
        );
        let len = end - start;
        assert!(len >= 15.0, "Clip duration {len} must be >= 15.0s");
        assert!(
            (len - 22.5).abs() < 1e-4,
            "Expected clip length 22.5s, got {len}"
        );
    }

    /// IT-12.1: 30-Second Distance Clustering:
    /// Peaks at t1 = 100.0s and t2 = 125.0s (gap = 25s <= 30s) merge into 1 cluster.
    /// Peak at t3 = 200.0s (gap = 75s > 30s) forms separate cluster.
    #[test]
    fn test_it_12_1_clustering_within_30s() {
        let fused_points = vec![
            FusedScorePoint::new(
                100.0,
                80.0,
                Some(5.0),
                Some(0.8),
                Some(0.6),
                vec!["KEKW".to_string()],
                true,
                false,
            ),
            FusedScorePoint::new(
                125.0,
                85.0,
                Some(6.0),
                Some(0.9),
                Some(0.7),
                vec!["tutel".to_string()],
                true,
                false,
            ),
            FusedScorePoint::new(
                200.0,
                75.0,
                Some(4.0),
                Some(0.5),
                Some(0.5),
                vec!["gymbag".to_string()],
                true,
                false,
            ),
        ];

        let clusters = cluster_and_deduplicate(&fused_points, 1000.0, 10);
        assert_eq!(
            clusters.len(),
            2,
            "Peaks within 30s should merge, resulting in exactly 2 clusters"
        );

        // First cluster merges 100s and 125s, focal peak at 125s (score 85.0)
        let c1 = &clusters[0];
        assert_eq!(c1.score, 85.0);
        assert_eq!(c1.peak_sec, 125.0);
        assert!(c1.start_sec <= 100.0 - 14.5 + 0.1);
        assert!(c1.end_sec >= 125.0 + 15.0 - 0.1);

        // Second cluster is at 200s
        let c2 = &clusters[1];
        assert_eq!(c2.score, 75.0);
        assert_eq!(c2.peak_sec, 200.0);
    }

    /// AC-12.3: Overlap Deduplication (> 50% overlap suppressed)
    #[test]
    fn test_ac_12_3_overlap_deduplication() {
        let overlap = compute_overlap_ratio(100.0, 140.0, 110.0, 145.0);
        // Durations: 40s and 35s. Overlap is [110, 140] = 30s.
        // Ratio = 30 / 35 = 0.857 > 0.50
        assert!(overlap > 0.50);

        let low_overlap = compute_overlap_ratio(100.0, 140.0, 135.0, 180.0);
        // Overlap is [135, 140] = 5s. Min dur = 40s. Ratio = 5/40 = 0.125 <= 0.50
        assert!(low_overlap <= 0.50);
    }
}
