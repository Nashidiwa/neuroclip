#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

//! Unit and Integration Tests for Chat Parser, Schema 5.3 Normalizer, and Heatmap Engine.
//!
//! Complies with docs/CONVENTIONS.md (§7 Testing & QA) and TASK-M4-07 QA checklist.

use std::fs::File;
use std::io::Write;
use std::path::{Path, PathBuf};
use tokio_util::sync::CancellationToken;

use neuroclip_lib::{
    extract_message_emotes, is_neuro_swarm_emote, parse_raw_chat_line, AppError,
    ChatEventsContainer, IngestError, StreamingChatNormalizer,
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

// ---------------------------------------------------------------------------
// UT-07.1: Happy Path - Parsing & Normalization
// ---------------------------------------------------------------------------

#[test]
fn test_parse_normalized_chat_event_line() {
    let raw = r#"{"timestamp_sec":14.25,"author":"VedalFan","message":"KEKW so fast","badges":["subscriber"],"emotes_detected":["KEKW"],"is_swarm_emote":true}"#;
    let event = parse_raw_chat_line(raw);
    assert!(event.is_some());
    let e = event.unwrap();
    assert!((e.timestamp_sec - 14.25).abs() < 1e-4);
    assert_eq!(e.author, "VedalFan");
    assert_eq!(e.message, "KEKW so fast");
    assert_eq!(e.emotes_detected, vec!["KEKW".to_string()]);
    assert!(e.is_swarm_emote);
}

#[test]
fn test_parse_twitch_gql_edge_payload() {
    let raw = r#"{
        "cursor": "cursor_123",
        "node": {
            "contentOffsetSeconds": 45.5,
            "commenter": {
                "displayName": "Gymbag01"
            },
            "message": {
                "fragments": [
                    { "text": "Tutel tutel! " },
                    { "text": "Aware", "emoticon": { "emoticonID": "Aware" } }
                ]
            }
        }
    }"#;

    let event = parse_raw_chat_line(raw);
    assert!(event.is_some());
    let e = event.unwrap();
    assert!((e.timestamp_sec - 45.5).abs() < 1e-4);
    assert_eq!(e.author, "Gymbag01");
    assert_eq!(e.message, "Tutel tutel! Aware");
    assert!(e.is_swarm_emote);
    assert!(e.emotes_detected.contains(&"Tutel".to_string()));
    assert!(e.emotes_detected.contains(&"Aware".to_string()));
}

#[test]
fn test_parse_youtube_live_chat_replay_payload() {
    let raw = r#"{
        "replayChatItemAction": {
            "actions": [
                {
                    "addChatItemAction": {
                        "item": {
                            "liveChatTextMessageRenderer": {
                                "message": {
                                    "runs": [
                                        { "text": "Vedal is a turtle 💀 " },
                                        { "text": "KEKW" }
                                    ]
                                },
                                "authorName": {
                                    "simpleText": "NeuroFanVN"
                                },
                                "videoOffsetTimeMsec": "120500"
                            }
                        }
                    }
                }
            ]
        }
    }"#;

    let event = parse_raw_chat_line(raw);
    assert!(event.is_some());
    let e = event.unwrap();
    assert!((e.timestamp_sec - 120.5).abs() < 1e-4);
    assert_eq!(e.author, "NeuroFanVN");
    assert_eq!(e.message, "Vedal is a turtle 💀 KEKW");
    assert!(e.is_swarm_emote);
    assert!(e.emotes_detected.contains(&"💀".to_string()));
    assert!(e.emotes_detected.contains(&"KEKW".to_string()));
}

#[test]
fn test_swarm_emote_classifier_dictionary() {
    // Assert all core culture emotes are detected
    assert!(is_neuro_swarm_emote("KEKW"));
    assert!(is_neuro_swarm_emote("kekw"));
    assert!(is_neuro_swarm_emote("Tutel"));
    assert!(is_neuro_swarm_emote("Gymbag"));
    assert!(is_neuro_swarm_emote("Aware"));
    assert!(is_neuro_swarm_emote("💀"));
    assert!(is_neuro_swarm_emote("neuroHeart"));
    assert!(is_neuro_swarm_emote("evilHeart"));
    assert!(is_neuro_swarm_emote("Erm"));

    // Assert non-swarm words are not detected
    assert!(!is_neuro_swarm_emote("hello"));
    assert!(!is_neuro_swarm_emote("windows"));
    assert!(!is_neuro_swarm_emote("stream"));

    let (emotes, is_swarm) = extract_message_emotes("Vedal Tutel tutel! Erm... 💀 KEKW");
    assert!(is_swarm);
    assert!(emotes.contains(&"Tutel".to_string()));
    assert!(emotes.contains(&"Erm".to_string()));
    assert!(emotes.contains(&"💀".to_string()));
    assert!(emotes.contains(&"KEKW".to_string()));
}

// ---------------------------------------------------------------------------
// UT-07.2: Heatmap & Density Window Calculation
// ---------------------------------------------------------------------------

#[test]
fn test_10s_sliding_window_density_and_spam_bot_filter() {
    let dir = TestDir::new("density_filter");
    let ndjson_path = dir.path().join("test_chat.ndjson");

    let mut file = File::create(&ndjson_path).unwrap();

    // Bucket 0 (0-10s): Normal activity (10 messages from 10 distinct authors)
    for i in 0..10 {
        let line = format!(
            r#"{{"timestamp_sec":{:.1},"author":"user_{i}","message":"chat message","emotes_detected":[],"is_swarm_emote":false}}"#,
            1.0 + (i as f64) * 0.5
        );
        writeln!(file, "{line}").unwrap();
    }

    // Bucket 1 (10-20s): Spam bot burst (1 rogue author sends 25 messages, 2 regular users send 1 each)
    // Spam bot filter rule: max 3 messages per author per window!
    // Expected effective count = min(25, 3) + 1 + 1 = 5
    for _ in 0..25 {
        writeln!(
            file,
            r#"{{"timestamp_sec":12.5,"author":"spam_bot_9000","message":"BUY CRYPTO NOW","emotes_detected":[],"is_swarm_emote":false}}"#
        )
        .unwrap();
    }
    writeln!(
        file,
        r#"{{"timestamp_sec":13.0,"author":"real_user_1","message":"mods ban bot","emotes_detected":[],"is_swarm_emote":false}}"#
    )
    .unwrap();
    writeln!(
        file,
        r#"{{"timestamp_sec":14.0,"author":"real_user_2","message":"KEKW bot","emotes_detected":["KEKW"],"is_swarm_emote":true}}"#
    )
    .unwrap();

    // Bucket 2 (20-30s): Hype moment (20 distinct users send messages with Swarm emotes)
    // Expected count = 20, which is >= 15 threshold, so Z-score calculation is active!
    for i in 0..20 {
        let line = format!(
            r#"{{"timestamp_sec":{:.1},"author":"hype_user_{i}","message":"Tutel KEKW!","emotes_detected":["Tutel","KEKW"],"is_swarm_emote":true}}"#,
            21.0 + (i as f64) * 0.3
        );
        writeln!(file, "{line}").unwrap();
    }

    drop(file);

    let normalizer = StreamingChatNormalizer::new("test_vod", dir.path());
    let heatmap = normalizer
        .compute_density_heatmap(&ndjson_path, 10.0, 10.0)
        .unwrap();

    assert_eq!(heatmap.total_buckets, 3);

    // Bucket 0
    assert_eq!(heatmap.points[0].message_count, 10);
    assert!((heatmap.points[0].velocity_mps - 1.0).abs() < 1e-4);
    // Noise floor: count 10 < 15 => Z = 0.0 (AC-09.3)
    assert_eq!(heatmap.points[0].z_score, 0.0);

    // Bucket 1: Spam bot clamped to 3! Total = 3 + 1 + 1 = 5
    assert_eq!(heatmap.points[1].message_count, 5);
    assert!((heatmap.points[1].velocity_mps - 0.5).abs() < 1e-4);
    // Noise floor: count 5 < 15 => Z = 0.0
    assert_eq!(heatmap.points[1].z_score, 0.0);

    // Bucket 2: Hype peak! Count = 20 >= 15 => Z > 0.0
    assert_eq!(heatmap.points[2].message_count, 20);
    assert!((heatmap.points[2].velocity_mps - 2.0).abs() < 1e-4);
    assert!(heatmap.points[2].z_score > 0.0);
    assert!(heatmap.points[2].swarm_ratio > 0.9);
    assert!((heatmap.max_velocity_mps - 2.0).abs() < 1e-4);
    assert!((heatmap.peak_timestamp_sec - 20.0).abs() < 1e-4);
}

// ---------------------------------------------------------------------------
// IT-07.1: Schema 5.3 & Two-Pass NDJSON Serialization
// ---------------------------------------------------------------------------

#[test]
fn test_two_pass_streaming_normalization_schema_5_3() {
    let dir = TestDir::new("norm_schema");
    let raw_input = dir.path().join("raw_chat.ndjson");
    let target_json = dir.path().join("chat_events.json");

    let mut raw_file = File::create(&raw_input).unwrap();
    writeln!(
        raw_file,
        r#"{{"contentOffsetSeconds":1.5,"commenter":{{"displayName":"Vedal"}},"message":{{"fragments":[{{"text":"Hello everyone"}}]}}}}"#
    )
    .unwrap();
    writeln!(
        raw_file,
        r#"{{"contentOffsetSeconds":2.0,"commenter":{{"displayName":"Neuro"}},"message":{{"fragments":[{{"text":"Heart Tutel"}}]}}}}"#
    )
    .unwrap();
    drop(raw_file);

    let cancel = CancellationToken::new();
    let normalizer = StreamingChatNormalizer::new("vod_schema_test", dir.path());
    let (total_msgs, ndjson_path) = normalizer
        .normalize_file(&raw_input, &target_json, &cancel)
        .unwrap();

    assert_eq!(total_msgs, 2);
    assert!(target_json.exists());
    assert!(ndjson_path.exists());

    // Verify chat_events.json matches Schema 5.3 strictly
    let content = std::fs::read_to_string(&target_json).unwrap();
    let container: ChatEventsContainer = serde_json::from_str(&content).unwrap();

    assert_eq!(container.vod_id, "vod_schema_test");
    assert_eq!(container.total_messages, 2);
    assert_eq!(container.events.len(), 2);

    assert_eq!(container.events[0].author, "Vedal");
    assert_eq!(container.events[0].message, "Hello everyone");
    assert!(!container.events[0].is_swarm_emote);

    assert_eq!(container.events[1].author, "Neuro");
    assert_eq!(container.events[1].message, "Heart Tutel");
    assert!(container.events[1].is_swarm_emote);
    assert!(container.events[1].emotes_detected.contains(&"Tutel".to_string()));
}

// ---------------------------------------------------------------------------
// IT-07.2: High Throughput Benchmark (10,000 Messages)
// ---------------------------------------------------------------------------

#[test]
fn test_throughput_benchmark_10k_messages() {
    let dir = TestDir::new("benchmark_10k");
    let raw_input = dir.path().join("synthetic_10k.ndjson");
    let target_json = dir.path().join("chat_events.json");

    let mut file = File::create(&raw_input).unwrap();
    for i in 0..10_000 {
        let t = (i as f64) * 0.1;
        let is_swarm = i % 5 == 0;
        let text = if is_swarm { "KEKW Tutel!" } else { "normal chat message" };
        writeln!(
            file,
            r#"{{"timestamp_sec":{t:.1},"author":"user_{i}","message":"{text}","emotes_detected":[],"is_swarm_emote":{is_swarm}}}"#
        )
        .unwrap();
    }
    drop(file);

    let start = std::time::Instant::now();
    let cancel = CancellationToken::new();
    let normalizer = StreamingChatNormalizer::new("benchmark_vod", dir.path());

    let (total_msgs, ndjson_path) = normalizer
        .normalize_file(&raw_input, &target_json, &cancel)
        .unwrap();

    let heatmap = normalizer
        .compute_density_heatmap(&ndjson_path, 10.0, 10.0)
        .unwrap();

    let elapsed = start.elapsed();

    assert_eq!(total_msgs, 10_000);
    assert!(heatmap.total_buckets > 0);
    // 10,000 messages processed in well under 2 seconds on any CPU
    assert!(elapsed.as_secs_f64() < 2.0);
}

// ---------------------------------------------------------------------------
// ED-07.1: Chat Replay Unavailable Fallback (ERR-05)
// ---------------------------------------------------------------------------

#[test]
fn test_err_05_chat_replay_unavailable_recoverable_flag() {
    let err = AppError::Ingestion(IngestError::ChatReplayUnavailable);
    assert!(err.is_recoverable());
    assert_eq!(err.code(), "ERR_INGEST_CHAT_UNAVAILABLE");
}

// ---------------------------------------------------------------------------
// ED-07.2: Malformed Stream Resilience & Unicode
// ---------------------------------------------------------------------------

#[test]
fn test_malformed_lines_and_unicode_resilience() {
    let malformed_inputs = [
        "",
        "   ",
        "NOT JSON AT ALL",
        r#"{"random_field": 123}"#,
        r#"{"contentOffsetSeconds": "not a number"}"#,
        r#"{"null_byte": "\u0000"}"#,
    ];

    for line in &malformed_inputs {
        let parsed = parse_raw_chat_line(line);
        assert!(parsed.is_none());
    }

    // Unicode Vietnamese and emoji preservation
    let vn_line = r#"{"timestamp_sec":10.5,"author":"Nguyễn Văn A","message":"Chào Neuro và Vedal 💖 Tutel!","emotes_detected":["Tutel"],"is_swarm_emote":true}"#;
    let parsed_vn = parse_raw_chat_line(vn_line);
    assert!(parsed_vn.is_some());
    let e = parsed_vn.unwrap();
    assert_eq!(e.author, "Nguyễn Văn A");
    assert_eq!(e.message, "Chào Neuro và Vedal 💖 Tutel!");
    assert!(e.is_swarm_emote);
}
