#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

//! Integration and Adversarial Test Suite for TASK-M4-09:
//! Adjusted Chat Velocity Z-Score Algorithm with Message Floor.
//!
//! Complies with `docs/CONVENTIONS.md` (§7 Testing & QA) and `docs/TASK_BOARD.md` §TASK-M4-09.

use std::fs::File;
use std::io::Write;
use std::path::{Path, PathBuf};

use neuroclip_lib::{
    bin_chat_events_default, calculate_adjusted_zscore, compute_chat_velocity_series,
    compute_chat_velocity_series_default, compute_chat_velocity_series_from_events,
    compute_chat_velocity_series_from_file, compute_zscore, ChatBucket, ChatEvent,
    ChatEventsContainer, DEFAULT_BASELINE_WINDOW_SEC, DEFAULT_BUCKET_WIDTH_SEC,
    MESSAGE_FLOOR_THRESHOLD, SMOOTHING_EPSILON,
};

struct TestDir {
    path: PathBuf,
}

impl TestDir {
    fn new(name: &str) -> Self {
        let path = std::env::temp_dir().join(format!(
            "neuroclip_test_{name}_{}_{}",
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

fn create_event(ts: f64, author: &str, msg: &str) -> ChatEvent {
    ChatEvent {
        timestamp_sec: ts,
        author: author.to_string(),
        message: msg.to_string(),
        badges: Vec::new(),
        emotes_detected: Vec::new(),
        is_swarm_emote: false,
    }
}

// ---------------------------------------------------------------------------
// UT-09.1: Message Floor Threshold ($X(t) < 15 \implies Z_{\text{adj}}(t) = 0.0$)
// ---------------------------------------------------------------------------

#[test]
fn test_ut_09_1_message_floor_strict_cutoff() {
    // Exactly 14 messages (under floor) -> 0.0
    for mu in [0.0, 1.0, 5.0, 10.0, 50.0] {
        for sigma in [0.0, 0.1, 1.0, 5.0, 10.0] {
            let z = calculate_adjusted_zscore(
                14,
                mu,
                sigma,
                SMOOTHING_EPSILON,
                MESSAGE_FLOOR_THRESHOLD,
            );
            assert_eq!(
                z, 0.0,
                "Count 14 must return 0.0 for mu={mu}, sigma={sigma}"
            );
        }
    }

    // Zero messages -> 0.0
    assert_eq!(compute_zscore(0, 0.0, 0.0), 0.0);
    assert_eq!(compute_zscore(1, 0.0, 0.0), 0.0);
    assert_eq!(compute_zscore(14, 0.0, 0.0), 0.0);

    // Exactly 15 messages (at floor) -> evaluated normally
    let z_15 = compute_zscore(15, 10.0, 0.0);
    // (15 - 10) / (0.0 + 0.5) = 5.0 / 0.5 = 10.0
    assert!((z_15 - 10.0).abs() < 1e-6);
}

// ---------------------------------------------------------------------------
// UT-09.2: Smoothing Constant $\epsilon = 0.5$ with Zero Variance Background
// ---------------------------------------------------------------------------

#[test]
fn test_ut_09_2_zero_variance_smoothing() {
    // $X = 45$, $\mu = 10.0$, $\sigma = 0.0 \implies Z = \frac{45 - 10}{0.0 + 0.5} = 70.0$
    let z = compute_zscore(45, 10.0, 0.0);
    assert!(
        (z - 70.0).abs() < 1e-6,
        "UT-09.2 formula check failed: expected 70.0, got {z}"
    );

    // Also verify via full sliding window calculation
    let mut buckets = Vec::new();
    for i in 0..12 {
        buckets.push(ChatBucket::new(i, (i as f64) * 5.0, 10, 10));
    }
    buckets.push(ChatBucket::new(12, 60.0, 45, 45));

    let series = compute_chat_velocity_series_default(&buckets).unwrap();
    assert_eq!(series.len(), 13);
    assert_eq!(series[12].count, 45);
    assert!(
        (series[12].z_score - 70.0).abs() < 1e-6,
        "UT-09.2 sliding series check failed: expected 70.0, got {}",
        series[12].z_score
    );
}

// ---------------------------------------------------------------------------
// UT-09.3: Spam Bot Author Rate Limiting
// ---------------------------------------------------------------------------

#[test]
fn test_ut_09_3_single_author_spambot_eliminated() {
    // Spambot sends 20 messages in bucket 0
    let mut events = Vec::new();
    for i in 0..20 {
        events.push(create_event(
            0.5 + (i as f64) * 0.1,
            "spambot_single",
            "SPAM SPAM SPAM",
        ));
    }

    let buckets = bin_chat_events_default(&events, None).unwrap();
    assert_eq!(buckets.len(), 1);
    assert_eq!(buckets[0].raw_count, 20);
    assert_eq!(buckets[0].effective_count, 3);

    let series = compute_chat_velocity_series_default(&buckets).unwrap();
    assert_eq!(series[0].count, 3);
    assert_eq!(series[0].z_score, 0.0);
}

#[test]
fn test_ut_09_3_multiple_authors_organic_vs_spam() {
    let mut events = Vec::new();

    // 1 Spambot sends 50 messages in 5s (capped at 3)
    for _ in 0..50 {
        events.push(create_event(2.0, "annoying_bot", "spam"));
    }

    // 4 Organic users send 2 messages each (4 * 2 = 8)
    for u in 1..=4 {
        events.push(create_event(
            2.1,
            &format!("viewer_{u}"),
            "normal message 1",
        ));
        events.push(create_event(
            2.2,
            &format!("viewer_{u}"),
            "normal message 2",
        ));
    }

    let buckets = bin_chat_events_default(&events, None).unwrap();
    // Raw count = 50 + 8 = 58
    assert_eq!(buckets[0].raw_count, 58);
    // Effective count = 3 + 8 = 11 < 15
    assert_eq!(buckets[0].effective_count, 11);

    let series = compute_chat_velocity_series_default(&buckets).unwrap();
    assert_eq!(series[0].count, 11);
    assert_eq!(series[0].z_score, 0.0);
}

// ---------------------------------------------------------------------------
// IT-09.1: Full Continuous 5-Second Series Across VOD
// ---------------------------------------------------------------------------

#[test]
fn test_it_09_1_continuous_timeline_and_gaps() {
    let vod_dur = 600.0; // 10 minutes = 120 buckets
    let events = vec![
        // Event at minute 0 (bucket 0)
        create_event(1.0, "user_a", "start"),
        // Event at minute 5 (bucket 60, t=300.0s)
        create_event(301.0, "user_b", "middle"),
        // Event at minute 9:55 (bucket 119, t=595.0s)
        create_event(596.0, "user_c", "end"),
    ];

    let series = compute_chat_velocity_series_from_events(
        &events,
        DEFAULT_BUCKET_WIDTH_SEC,
        DEFAULT_BASELINE_WINDOW_SEC,
        Some(vod_dur),
    )
    .unwrap();

    assert_eq!(series.len(), 120);

    for (i, pt) in series.iter().enumerate() {
        assert_eq!(pt.timestamp_sec, (i as f64) * 5.0);
        if i == 0 || i == 60 || i == 119 {
            assert_eq!(pt.count, 1);
        } else {
            assert_eq!(pt.count, 0);
        }
        assert_eq!(pt.z_score, 0.0);
    }
}

// ---------------------------------------------------------------------------
// ED-09.1: 500,000 Messages Benchmark SLA ($\le 1.2\text{s}$)
// ---------------------------------------------------------------------------

#[test]
fn test_ed_09_1_benchmark_500k_messages() {
    const COUNT: usize = 500_000;
    let mut events = Vec::with_capacity(COUNT);

    for i in 0..COUNT {
        let ts = ((i % 5760) as f64) * 5.0 + 0.05 * ((i % 80) as f64);
        let author = format!("user_{}", i % 1000);
        events.push(create_event(ts, &author, "Neuro Swarm!"));
    }

    let start = std::time::Instant::now();
    let buckets = bin_chat_events_default(&events, Some(28800.0)).unwrap();
    let series = compute_chat_velocity_series_default(&buckets).unwrap();
    let duration = start.elapsed();

    println!("ED-09.1: 500k messages processed in {duration:?}");
    assert_eq!(series.len(), 5760);
    assert!(
        duration.as_millis() <= 1200,
        "Benchmark exceeded 1.2s SLA: took {duration:?}"
    );
}

// ---------------------------------------------------------------------------
// File Ingestion: JSON and NDJSON Formats
// ---------------------------------------------------------------------------

#[test]
fn test_compute_from_json_and_ndjson_files() {
    let test_dir = TestDir::new("highlight_files");

    // 1. NDJSON test file
    let ndjson_path = test_dir.path().join("chat.ndjson");
    {
        let mut file = File::create(&ndjson_path).unwrap();
        for i in 0..15 {
            let line = format!(
                r#"{{"timestamp_sec":{},"author":"viewer_{}","message":"chat message","emotes_detected":[]}}"#,
                1.0 + (i as f64) * 0.1,
                i
            );
            writeln!(file, "{line}").unwrap();
        }
    }

    let ndjson_series = compute_chat_velocity_series_from_file(
        &ndjson_path,
        DEFAULT_BUCKET_WIDTH_SEC,
        DEFAULT_BASELINE_WINDOW_SEC,
        Some(10.0),
    )
    .unwrap();

    assert_eq!(ndjson_series.len(), 2);
    assert_eq!(ndjson_series[0].count, 15);
    assert!(ndjson_series[0].z_score > 0.0);

    // 2. JSON container test file
    let json_path = test_dir.path().join("chat.json");
    {
        let events = vec![
            create_event(1.0, "u1", "m1"),
            create_event(2.0, "u2", "m2"),
            create_event(6.0, "u3", "m3"),
        ];
        let container = ChatEventsContainer {
            vod_id: "vod_test_123".to_string(),
            total_messages: 3,
            events,
        };
        let mut file = File::create(&json_path).unwrap();
        serde_json::to_writer(&mut file, &container).unwrap();
    }

    let json_series = compute_chat_velocity_series_from_file(
        &json_path,
        DEFAULT_BUCKET_WIDTH_SEC,
        DEFAULT_BASELINE_WINDOW_SEC,
        Some(15.0),
    )
    .unwrap();

    assert_eq!(json_series.len(), 3);
    assert_eq!(json_series[0].count, 2);
    assert_eq!(json_series[1].count, 1);
    assert_eq!(json_series[2].count, 0);

    // 3. Non-existent file error handling
    let missing_path = test_dir.path().join("does_not_exist.json");
    let err = compute_chat_velocity_series_from_file(
        &missing_path,
        DEFAULT_BUCKET_WIDTH_SEC,
        DEFAULT_BASELINE_WINDOW_SEC,
        None,
    );
    assert!(err.is_err());
}

// ---------------------------------------------------------------------------
// Adversarial Edge Cases
// ---------------------------------------------------------------------------

#[test]
fn test_adversarial_dirty_data() {
    let events = vec![
        create_event(-100.0, "negative_user", "bad"),
        create_event(f64::NAN, "nan_user", "bad"),
        create_event(f64::INFINITY, "inf_user", "bad"),
        create_event(f64::NEG_INFINITY, "neg_inf", "bad"),
        create_event(99_999_999.0, "far_future", "bad"),
        create_event(2.5, "valid_user", "good"),
    ];

    let buckets = bin_chat_events_default(&events, None).unwrap();
    assert_eq!(buckets.len(), 1);
    assert_eq!(buckets[0].raw_count, 1);
    assert_eq!(buckets[0].effective_count, 1);
}

#[test]
fn test_adversarial_empty_stream() {
    let events: Vec<ChatEvent> = Vec::new();

    // Empty with no duration
    let empty_series = compute_chat_velocity_series_from_events(
        &events,
        DEFAULT_BUCKET_WIDTH_SEC,
        DEFAULT_BASELINE_WINDOW_SEC,
        None,
    )
    .unwrap();
    assert!(empty_series.is_empty());

    // Empty with 60s duration -> 12 buckets of zeroes
    let padded_series = compute_chat_velocity_series_from_events(
        &events,
        DEFAULT_BUCKET_WIDTH_SEC,
        DEFAULT_BASELINE_WINDOW_SEC,
        Some(60.0),
    )
    .unwrap();
    assert_eq!(padded_series.len(), 12);
    for pt in padded_series {
        assert_eq!(pt.count, 0);
        assert_eq!(pt.z_score, 0.0);
    }
}

#[test]
fn test_adversarial_invalid_config_parameters() {
    let buckets = vec![ChatBucket::new(0, 0.0, 10, 10)];

    // Invalid bucket widths
    assert!(compute_chat_velocity_series(&buckets, -5.0, 60.0).is_err());
    assert!(compute_chat_velocity_series(&buckets, 0.0, 60.0).is_err());
    assert!(compute_chat_velocity_series(&buckets, f64::NAN, 60.0).is_err());

    // Invalid baseline windows
    assert!(compute_chat_velocity_series(&buckets, 5.0, -60.0).is_err());
    assert!(compute_chat_velocity_series(&buckets, 5.0, 0.0).is_err());
    assert!(compute_chat_velocity_series(&buckets, 5.0, f64::INFINITY).is_err());
}
