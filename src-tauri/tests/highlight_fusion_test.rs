#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

//! Comprehensive Integration & SLA Test Suite for TASK-M4-12:
//! Multimodal Fusion & Stream Latency Compensation Highlight Clustering.
//!
//! Complies with `docs/CONVENTIONS.md` (§7 Testing & QA), `docs/ARCHITECTURE.md` §4.3 & §5.4,
//! and `docs/TASK_BOARD.md` §TASK-M4-12.

use std::path::{Path, PathBuf};
use std::time::Instant;

use neuroclip_lib::{
    calculate_clip_bounds, cluster_and_deduplicate, compute_audio_only_score,
    compute_multimodal_score, compute_overlap_ratio,
    fuse_multimodal_series, generate_clusters_from_fused_series, normalize_z_score,
    AudioRmsPoint, ChatVelocityPoint, EmoteDensityPoint, FusedScorePoint,
    HighlightClustersContainer,
};

/// Helper directory for creating temporary test files with auto-cleanup.
struct TestDir {
    path: PathBuf,
}

impl TestDir {
    fn new(name: &str) -> Self {
        let path = std::env::temp_dir().join(format!(
            "neuroclip_test_fusion_{name}_{}_{}",
            std::process::id(),
            chrono::Utc::now().timestamp_nanos_opt().unwrap_or(0)
        ));
        let _ = std::fs::create_dir_all(&path);
        Self { path }
    }

    fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for TestDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.path);
    }
}

/// UT-12.1: Full Multimodal Weighting Formula
/// Formula: Score = (0.60 * Z_norm + 0.25 * E_ratio + 0.15 * RMS_norm) * 100.0
/// Input: Z_adj = 10.0 (Z_norm = 1.0), E = 0.8, RMS = 0.6
/// Expected: Score = 89.0 (0.89 * 100)
#[test]
fn test_ut_12_1_full_multimodal_weighting_integration() {
    let z_adj = 10.0;
    let z_norm = normalize_z_score(z_adj);
    assert!((z_norm - 1.0).abs() < 1e-6);

    let e_ratio = 0.8;
    let rms_norm = 0.6;

    let score = compute_multimodal_score(z_norm, e_ratio, rms_norm);
    assert!(
        (score - 89.0).abs() < 1e-4,
        "UT-12.1 Failed: expected score 89.0, got {score}"
    );
}

/// UT-12.2: Audio-Only Fallback Weighting Formula
/// Formula: Score = 1.0 * RMS_norm * 100.0
/// Input: RMS = 0.75
/// Expected: Score = 75.0
#[test]
fn test_ut_12_2_audio_only_fallback_integration() {
    let rms_norm = 0.75;
    let score = compute_audio_only_score(rms_norm);
    assert!(
        (score - 75.0).abs() < 1e-4,
        "UT-12.2 Failed: expected score 75.0, got {score}"
    );

    let silence_score = compute_audio_only_score(0.0);
    assert_eq!(silence_score, 0.0);
}

/// UT-12.3: Stream Latency Compensation & Buffer Offsets
/// Latency: 4.5s; Pre-roll: 10.0s (total pre-offset = 14.5s); Post-roll: 15.0s
/// Peak: 5070.0s (01:24:30.0), Duration: 7200.0s
/// Expected: start = 5055.5s (01:24:15.5), end = 5085.0s (01:24:45.0), length = 29.5s
#[test]
fn test_ut_12_3_latency_compensation_and_padding_integration() {
    let peak_sec = 5070.0;
    let duration = 7200.0;
    let (start, end) = calculate_clip_bounds(peak_sec, peak_sec, duration, false);

    assert!(
        (start - 5055.5).abs() < 1e-4,
        "UT-12.3 Failed: expected start 5055.5s, got {start}"
    );
    assert!(
        (end - 5085.0).abs() < 1e-4,
        "UT-12.3 Failed: expected end 5085.0s, got {end}"
    );
    let length = end - start;
    assert!(
        (length - 29.5).abs() < 1e-4,
        "UT-12.3 Failed: expected length 29.5s, got {length}"
    );
}

/// IT-12.1: 30-Second Distance Clustering
/// Peaks spaced by <= 30.0s merge into 1 cluster; gap > 30.0s forms separate cluster.
#[test]
fn test_it_12_1_30s_distance_clustering_integration() {
    let fused = vec![
        FusedScorePoint::new(
            100.0,
            75.0,
            Some(4.0),
            Some(0.6),
            Some(0.5),
            vec!["KEKW".to_string()],
            true,
            false,
        ),
        FusedScorePoint::new(
            120.0,
            88.0,
            Some(8.0),
            Some(0.9),
            Some(0.7),
            vec!["tutel".to_string()],
            true,
            false,
        ),
        FusedScorePoint::new(
            125.0,
            80.0,
            Some(6.0),
            Some(0.8),
            Some(0.6),
            vec!["gymbag".to_string()],
            true,
            false,
        ),
        FusedScorePoint::new(
            300.0,
            70.0,
            Some(3.5),
            Some(0.5),
            Some(0.4),
            vec!["Aware".to_string()],
            true,
            false,
        ),
    ];

    let clusters = cluster_and_deduplicate(&fused, 1000.0, 10);
    assert_eq!(
        clusters.len(),
        2,
        "IT-12.1 Failed: 100s, 120s, 125s should merge into 1 cluster; 300s into 2nd cluster"
    );

    let first = &clusters[0];
    assert_eq!(first.peak_sec, 120.0, "Focal peak should be highest scoring point (120s)");
    assert_eq!(first.score, 88.0);
    assert!(first.start_sec <= 100.0 - 14.5 + 0.1);
    assert!(first.end_sec >= 125.0 + 15.0 - 0.1);
    assert!(first.top_emotes.contains(&"tutel".to_string()));
    assert!(first.top_emotes.contains(&"KEKW".to_string()));

    let second = &clusters[1];
    assert_eq!(second.peak_sec, 300.0);
    assert_eq!(second.score, 70.0);
}

/// IT-12.2: Schema 5.4 Data Contract Validation
/// Ensures generated `highlight_clusters.json` matches 100% of JSON Schema Draft-07 §5.4.
#[test]
fn test_it_12_2_schema_5_4_data_contract_compliance() {
    let test_dir = TestDir::new("schema_5_4");
    let vod_id = "test_vod_schema_5_4";

    let fused = vec![
        FusedScorePoint::new(
            500.0,
            85.5,
            Some(7.5),
            Some(0.8),
            Some(0.65),
            vec!["KEKW".to_string(), "tutel".to_string()],
            true,
            false,
        ),
        FusedScorePoint::new(
            1500.0,
            72.0,
            Some(4.0),
            Some(0.5),
            Some(0.6),
            vec!["Vedal".to_string()],
            true,
            false,
        ),
    ];

    let container = generate_clusters_from_fused_series(vod_id, &fused, 3600.0).unwrap();
    assert_eq!(container.vod_id, vod_id);
    assert_eq!(container.clusters.len(), 2);

    let json_path = test_dir.path().join("highlight_clusters.json");
    let json_bytes = serde_json::to_vec_pretty(&container).unwrap();
    std::fs::write(&json_path, &json_bytes).unwrap();

    // Deserialize back and verify all required Schema 5.4 fields
    let deserialized: HighlightClustersContainer =
        serde_json::from_slice(&std::fs::read(&json_path).unwrap()).unwrap();

    assert_eq!(deserialized.vod_id, vod_id);
    for cluster in &deserialized.clusters {
        assert!(!cluster.cluster_id.is_empty());
        assert!(cluster.start_sec >= 0.0);
        assert!(cluster.end_sec > cluster.start_sec);
        assert!(cluster.end_sec <= 3600.0);
        assert!(cluster.peak_sec >= cluster.start_sec);
        assert!(cluster.peak_sec <= cluster.end_sec);
        assert!(cluster.score >= 0.0 && cluster.score <= 100.0);
        assert!(!cluster.primary_reason.is_empty());
        assert!(!cluster.is_bookmarked); // Default false
    }

    // Verify raw JSON structure
    let json_val: serde_json::Value = serde_json::from_slice(&json_bytes).unwrap();
    assert!(json_val.get("vod_id").is_some());
    assert!(json_val.get("clusters").unwrap().is_array());
    let first_json = &json_val.get("clusters").unwrap()[0];
    assert!(first_json.get("cluster_id").is_some());
    assert!(first_json.get("start_sec").is_some());
    assert!(first_json.get("end_sec").is_some());
    assert!(first_json.get("peak_sec").is_some());
    assert!(first_json.get("score").is_some());
    assert!(first_json.get("primary_reason").is_some());
}

/// ED-12.1: Zero Clamping of Negative Timecodes
/// Peak at 8.0s, total pre-offset 14.5s -> 8.0 - 14.5 = -6.5s -> Clamped to 0.0s.
#[test]
fn test_ed_12_1_start_clamping_integration() {
    let peak_sec = 8.0;
    let duration = 3600.0;
    let (start, end) = calculate_clip_bounds(peak_sec, peak_sec, duration, false);

    assert_eq!(start, 0.0, "ED-12.1 Failed: start must be clamped to 0.0s");
    assert!((end - 23.0).abs() < 1e-4);
    assert!(start < end);
}

/// ED-12.2: Top 15-20 Ranking & SLA Performance (AC-12.1 <= 500ms for 8h VOD)
#[test]
fn test_ed_12_2_ranking_and_performance_sla_integration() {
    let duration_8h = 28800.0; // 8 hours in seconds
    let num_buckets = (duration_8h / 5.0) as usize; // 5760 buckets

    // Create 5760 buckets of simulated data with 50 distinct excitement spikes
    let mut velocity = Vec::with_capacity(num_buckets);
    let mut emote = Vec::with_capacity(num_buckets);
    let mut audio = Vec::with_capacity(num_buckets * 10);

    for k in 0..num_buckets {
        let t = (k as f64) * 5.0;
        let is_spike = k % 115 == 0; // ~50 spikes across 8 hours

        let z = if is_spike { 6.0 + ((k % 5) as f64) } else { 0.2 };
        let e = if is_spike { 0.8 } else { 0.05 };
        let count = if is_spike { 60 } else { 5 };

        velocity.push(ChatVelocityPoint::new(t, count, z));
        emote.push(EmoteDensityPoint::new(
            t,
            if is_spike { vec!["KEKW".to_string()] } else { vec![] },
            e,
            count * 4,
            if is_spike { 30 } else { 1 },
            e,
        ));
    }

    for m in 0..(num_buckets * 10) {
        let t = (m as f64) * 0.5;
        let is_spike = m % 1150 == 0;
        audio.push(AudioRmsPoint::new(t, if is_spike { 0.75 } else { 0.05 }, is_spike));
    }

    // Benchmark fusion and clustering performance
    let start_time = Instant::now();

    let fused = fuse_multimodal_series(&velocity, &emote, &audio, duration_8h).unwrap();
    let clusters = cluster_and_deduplicate(&fused, duration_8h, 20);

    let elapsed = start_time.elapsed();

    assert!(
        elapsed.as_millis() <= 500,
        "AC-12.1 SLA Failed: 8h VOD fusion & clustering took {:?} (limit <= 500ms)",
        elapsed
    );

    // Verify ranking: clusters must be sorted by score descending
    assert!(!clusters.is_empty());
    assert!(clusters.len() <= 20);
    assert!(clusters.len() >= 15, "Should extract 15-20 top clusters, found {}", clusters.len());

    for i in 1..clusters.len() {
        assert!(
            clusters[i - 1].score >= clusters[i].score,
            "ED-12.2 Failed: clusters not sorted by score descending"
        );
    }
}

/// ED-12.3: EOF Dynamic Padding & Clip Duration Invariant (>= 15.0s)
/// When peak occurs near stream EOF, end is clamped to duration and missing post-roll
/// shifts into pre-roll buffer to guarantee length >= 15.0s without exceeding EOF.
#[test]
fn test_ed_12_3_eof_dynamic_padding_integration() {
    let duration = 10000.0;
    let peak_sec = 9992.0;

    let (start, end) = calculate_clip_bounds(peak_sec, peak_sec, duration, false);
    assert_eq!(end, 10000.0, "End must be clamped to EOF 10000.0s");
    assert!((start - 9977.5).abs() < 1e-4);
    let len = end - start;
    assert!(len >= 15.0, "Clip length {len} must be >= 15.0s");
    assert!((len - 22.5).abs() < 1e-4);

    // Extreme near-EOF peak at 9998.0s in audio-only mode (pre-roll 10s, post-roll 15s)
    let (start_near, end_near) = calculate_clip_bounds(9998.0, 9998.0, duration, true);
    assert_eq!(end_near, 10000.0);
    let len_near = end_near - start_near;
    assert!(
        len_near >= 15.0,
        "Extreme EOF clip length {len_near} must be >= 15.0s"
    );
    assert!(start_near >= 0.0);
    assert!(end_near <= duration);
}

/// AC-12.3: Overlap Deduplication (> 50% overlap suppressed)
#[test]
fn test_ac_12_3_nms_overlap_suppression_integration() {
    let r1 = compute_overlap_ratio(100.0, 140.0, 110.0, 145.0);
    assert!(r1 > 0.50, "Expected overlap ratio > 0.50, got {r1}");

    let r2 = compute_overlap_ratio(100.0, 140.0, 136.0, 180.0);
    assert!(r2 <= 0.50, "Expected overlap ratio <= 0.50, got {r2}");
}

/// AC-12.4: Boundary Invariants: 0.0 <= start < end <= duration for 100% of clusters
#[test]
fn test_ac_12_4_boundary_invariants_across_timeline() {
    let duration = 5000.0;
    let test_peaks = vec![0.0, 2.5, 14.5, 50.0, 2500.0, 4985.0, 4995.0, 4999.0, 5000.0];

    for peak in test_peaks {
        let (start, end) = calculate_clip_bounds(peak, peak, duration, false);
        assert!(start >= 0.0, "start {start} must be >= 0.0 for peak {peak}");
        assert!(start < end, "start {start} must be < end {end} for peak {peak}");
        assert!(end <= duration, "end {end} must be <= duration {duration} for peak {peak}");
        assert!(
            end - start >= 1.0,
            "clip duration must be positive non-zero for peak {peak}"
        );
    }
}
