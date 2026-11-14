#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

//! Adversarial Challenge Test Harness for TASK-M4-07
//! (Soft Fallback, WindowsJobObjectGuard Process Lifecycle, and Edge Conditions).
//!
//! Complies with docs/CONVENTIONS.md (§7 Testing & QA) and Teamwork Challenger protocol.

use std::fs::File;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};
use tokio_util::sync::CancellationToken;

use neuroclip_lib::{
    extract_message_emotes, parse_raw_chat_line, AppError,
    ChatEventsContainer, StreamingChatNormalizer,
};
use neuroclip_lib::ingestion::streamer::{assign_child_to_job_guard, WindowsJobObjectGuard};
use neuroclip_lib::ingestion::twitch_gql::fetch_twitch_vod_chat_gql;
use neuroclip_lib::ingestion::youtube_chat::fetch_youtube_live_chat_replay;

struct AdversarialTestDir {
    path: PathBuf,
}

impl AdversarialTestDir {
    fn new(suffix: &str) -> Self {
        let path = std::env::temp_dir().join(format!(
            "neuroclip_adv_{suffix}_{}_{}",
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

impl Drop for AdversarialTestDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.path);
    }
}

// ===========================================================================
// DIMENSION 1: ERR-05 SOFT FALLBACK & LATENCY VERIFICATION (<= 200ms)
// ===========================================================================

static ENV_MUTEX: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

#[tokio::test]
async fn test_adv_err_05_twitch_gql_unavailable_latency_under_200ms() {
    let _lock = ENV_MUTEX.lock().await;
    std::env::set_var("NEUROCLIP_MOCK_CHAT_UNAVAILABLE", "1");

    let dir = AdversarialTestDir::new("err05_twitch");
    let out_file = dir.path().join("chat.ndjson");
    let cancel = CancellationToken::new();

    let start = Instant::now();
    let result = fetch_twitch_vod_chat_gql("vod_999999", &out_file, cancel, |_| {}).await;
    let elapsed = start.elapsed();

    std::env::remove_var("NEUROCLIP_MOCK_CHAT_UNAVAILABLE");

    assert!(result.is_err(), "Expected error for unavailable chat");
    let err = result.unwrap_err();

    // Verify error code matches ERR_INGEST_CHAT_UNAVAILABLE
    assert_eq!(
        err.code(),
        "ERR_INGEST_CHAT_UNAVAILABLE",
        "Error code must be canonical ERR_INGEST_CHAT_UNAVAILABLE"
    );

    // Verify error is recoverable (soft fallback flag)
    assert!(
        err.is_recoverable(),
        "ERR_INGEST_CHAT_UNAVAILABLE must be marked recoverable for audio fallback"
    );

    // Empirically assert SLA: <= 200ms
    assert!(
        elapsed.as_millis() <= 200,
        "ERR-05 soft fallback took {}ms, which exceeds 200ms SLA",
        elapsed.as_millis()
    );
}

#[tokio::test]
async fn test_adv_err_05_youtube_unavailable_latency_under_200ms() {
    let _lock = ENV_MUTEX.lock().await;
    std::env::set_var("NEUROCLIP_MOCK_CHAT_UNAVAILABLE", "1");

    let dir = AdversarialTestDir::new("err05_yt");
    let cancel = CancellationToken::new();

    let start = Instant::now();
    let result = fetch_youtube_live_chat_replay(
        "https://www.youtube.com/watch?v=missing_chat_vod",
        "vod_yt_missing",
        dir.path(),
        cancel,
        |_| {},
    )
    .await;
    let elapsed = start.elapsed();

    std::env::remove_var("NEUROCLIP_MOCK_CHAT_UNAVAILABLE");

    assert!(result.is_err(), "Expected error for unavailable chat");
    let err = result.unwrap_err();

    assert_eq!(err.code(), "ERR_INGEST_CHAT_UNAVAILABLE");
    assert!(err.is_recoverable());
    assert!(
        elapsed.as_millis() <= 200,
        "YouTube ERR-05 fallback took {}ms, exceeding 200ms SLA",
        elapsed.as_millis()
    );
}

#[tokio::test]
async fn test_adv_err_05_twitch_gql_empty_mock_file_returns_chat_unavailable() {
    let _lock = ENV_MUTEX.lock().await;
    let dir = AdversarialTestDir::new("empty_twitch_file");
    let mock_file = dir.path().join("empty_gql.ndjson");
    File::create(&mock_file).unwrap(); // 0 bytes

    std::env::set_var("NEUROCLIP_MOCK_TWITCH_CHAT_FILE", mock_file.to_str().unwrap());

    let out_file = dir.path().join("out.ndjson");
    let cancel = CancellationToken::new();

    let start = Instant::now();
    let result = fetch_twitch_vod_chat_gql("vod_empty", &out_file, cancel, |_| {}).await;
    let elapsed = start.elapsed();

    std::env::remove_var("NEUROCLIP_MOCK_TWITCH_CHAT_FILE");

    // Line count 0 should return ChatReplayUnavailable or error, not panic
    if let Ok(count) = result {
        assert_eq!(count, 0, "Empty chat should yield 0 messages");
    }
    assert!(elapsed.as_millis() <= 200);
}

// ===========================================================================
// DIMENSION 2: WINDOWS JOB OBJECT & PROCESS LIFECYCLE (ZOMBIE PREVENTION)
// ===========================================================================

#[cfg(windows)]
#[tokio::test]
async fn test_adv_windows_job_object_guard_lifecycle_and_zombie_termination() {
    use tokio::process::Command;

    // 1. Initialize WindowsJobObjectGuard
    let guard = WindowsJobObjectGuard::new();
    assert!(guard.is_ok(), "WindowsJobObjectGuard::new() must succeed on Windows");
    let guard = guard.unwrap();

    // 2. Spawn a long-running child process that would otherwise outlive the test
    // Use powershell Start-Sleep 30
    let mut cmd = Command::new("powershell.exe");
    cmd.args(["-NoProfile", "-NonInteractive", "-Command", "Start-Sleep -Seconds 30"]);
    cmd.stdout(std::process::Stdio::null());
    cmd.stderr(std::process::Stdio::null());

    let child = cmd.spawn();
    assert!(child.is_ok(), "Failed to spawn test child process");
    let mut child = child.unwrap();

    let pid = child.id().expect("Child must have a valid PID");
    assert!(pid > 0);

    // 3. Assign child process to Job Object
    let assign_res = guard.assign_child_process(&child);
    assert!(
        assign_res.is_ok(),
        "assign_child_process must succeed: {:?}",
        assign_res.err()
    );

    // 4. Verify process is initially running
    let status_before = child.try_wait().unwrap();
    assert!(
        status_before.is_none(),
        "Child process should still be running"
    );

    // 5. Drop the Job Object guard!
    // Dropping guard invokes CloseHandle on the job object.
    // Because JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE was set, Windows kernel
    // MUST automatically terminate all processes assigned to the job.
    drop(guard);

    // Give Windows kernel up to 500ms to terminate the job
    let mut terminated = false;
    for _ in 0..10 {
        tokio::time::sleep(Duration::from_millis(50)).await;
        if let Ok(Some(_status)) = child.try_wait() {
            terminated = true;
            break;
        }
    }

    assert!(
        terminated,
        "Child process (PID {pid}) was NOT terminated after dropping WindowsJobObjectGuard! Potential zombie child!"
    );
}

#[tokio::test]
async fn test_adv_assign_child_to_job_guard_helper_does_not_panic() {
    use tokio::process::Command;

    let mut cmd = Command::new("cmd.exe");
    cmd.args(["/c", "echo hello"]);
    cmd.stdout(std::process::Stdio::null());

    if let Ok(child) = cmd.spawn() {
        // Must not panic or crash
        assign_child_to_job_guard(&child);
    }
}

// ===========================================================================
// DIMENSION 3: ADVERSARIAL EDGE CONDITIONS IN NORMALIZER & HEATMAP
// ===========================================================================

#[test]
fn test_adv_negative_timestamps_stream_resilience() {
    let dir = AdversarialTestDir::new("neg_ts");
    let input_path = dir.path().join("negative_ts.ndjson");
    let out_json_path = dir.path().join("chat_events.json");

    let mut file = File::create(&input_path).unwrap();
    // Line 1: Negative timestamp from pre-roll (-15.0s)
    writeln!(
        file,
        r#"{{"timestamp_sec":-15.0,"author":"PreRollUser","message":"Stream starting soon","emotes_detected":[],"is_swarm_emote":false}}"#
    )
    .unwrap();
    // Line 2: Negative timestamp (-0.5s)
    writeln!(
        file,
        r#"{{"timestamp_sec":-0.5,"author":"EarlyBird","message":"Tutel KEKW","emotes_detected":["Tutel","KEKW"],"is_swarm_emote":true}}"#
    )
    .unwrap();
    // Line 3: Valid timestamp (5.0s)
    writeln!(
        file,
        r#"{{"timestamp_sec":5.0,"author":"NormalUser","message":"Hello stream","emotes_detected":[],"is_swarm_emote":false}}"#
    )
    .unwrap();
    drop(file);

    let cancel = CancellationToken::new();
    let normalizer = StreamingChatNormalizer::new("vod_neg_ts", dir.path());

    // Normalize file must not crash on negative timestamps
    let norm_res = normalizer.normalize_file(&input_path, &out_json_path, &cancel);
    assert!(norm_res.is_ok(), "normalize_file failed on negative timestamps: {:?}", norm_res.err());

    let (total_msgs, ndjson_path) = norm_res.unwrap();
    // parse_raw_chat_line filters event.timestamp_sec >= 0.0 for ChatEvent
    // So only 1 valid event should be recognized
    assert_eq!(total_msgs, 1, "Only non-negative timestamps should be parsed");

    // Heatmap calculation must not panic or crash
    let heatmap_res = normalizer.compute_density_heatmap(&ndjson_path, 10.0, 10.0);
    assert!(heatmap_res.is_ok(), "compute_density_heatmap failed on negative ts");
    let heatmap = heatmap_res.unwrap();
    assert_eq!(heatmap.total_buckets, 1);
    assert_eq!(heatmap.points[0].message_count, 1);
}

#[test]
fn test_adv_all_negative_timestamps_file_empty_heatmap() {
    let dir = AdversarialTestDir::new("all_neg_ts");
    let input_path = dir.path().join("all_negative.ndjson");
    let out_json_path = dir.path().join("chat_events.json");

    let mut file = File::create(&input_path).unwrap();
    for i in 1..=10 {
        writeln!(
            file,
            r#"{{"timestamp_sec":-{i}.0,"author":"user_{i}","message":"negative time","emotes_detected":[],"is_swarm_emote":false}}"#
        )
        .unwrap();
    }
    drop(file);

    let cancel = CancellationToken::new();
    let normalizer = StreamingChatNormalizer::new("vod_all_neg", dir.path());

    let (total_msgs, ndjson_path) = normalizer
        .normalize_file(&input_path, &out_json_path, &cancel)
        .unwrap();

    assert_eq!(total_msgs, 0, "All negative timestamps should result in 0 valid messages");

    let heatmap = normalizer
        .compute_density_heatmap(&ndjson_path, 10.0, 10.0)
        .unwrap();

    assert_eq!(heatmap.total_buckets, 0);
    assert_eq!(heatmap.points.len(), 0);
    assert_eq!(heatmap.max_velocity_mps, 0.0);
    assert_eq!(heatmap.peak_timestamp_sec, 0.0);
}

#[test]
fn test_adv_empty_chat_replays_zero_byte_and_whitespace() {
    let dir = AdversarialTestDir::new("empty_chat");
    let empty_file = dir.path().join("empty.ndjson");
    File::create(&empty_file).unwrap(); // 0 bytes

    let out_json_path = dir.path().join("chat_events.json");
    let cancel = CancellationToken::new();
    let normalizer = StreamingChatNormalizer::new("vod_empty", dir.path());

    let (total, ndjson_path) = normalizer
        .normalize_file(&empty_file, &out_json_path, &cancel)
        .unwrap();

    assert_eq!(total, 0);
    assert!(out_json_path.exists());

    // Verify chat_events.json valid Schema 5.3 structure even for 0 messages
    let content = std::fs::read_to_string(&out_json_path).unwrap();
    let container: ChatEventsContainer = serde_json::from_str(&content).unwrap();
    assert_eq!(container.vod_id, "vod_empty");
    assert_eq!(container.total_messages, 0);
    assert_eq!(container.events.len(), 0);

    // Heatmap on empty chat
    let heatmap = normalizer
        .compute_density_heatmap(&ndjson_path, 10.0, 10.0)
        .unwrap();
    assert_eq!(heatmap.total_buckets, 0);
    assert_eq!(heatmap.points.len(), 0);
    assert_eq!(heatmap.max_velocity_mps, 0.0);
}

#[test]
fn test_adv_all_swarm_emote_chat_stream() {
    let dir = AdversarialTestDir::new("all_swarm");
    let ndjson_path = dir.path().join("all_swarm.ndjson");

    let mut file = File::create(&ndjson_path).unwrap();
    // Swarm emote messages
    let swarm_phrases = [
        "KEKW",
        "Tutel",
        "Gymbag",
        "neuroHeart evilHeart",
        "💀 💀 💀",
        "Vedal is Tutel KEKW",
        "Evil neuroHeart",
        "Aware monkaS monkaW",
    ];

    // Baseline: 6 quiet buckets (t = 0..60s) with 2 regular messages each
    for bucket in 0..6 {
        for m in 0..2 {
            let t = (bucket as f64) * 10.0 + (m as f64) * 2.0;
            writeln!(
                file,
                r#"{{"timestamp_sec":{t:.1},"author":"chatter_{bucket}_{m}","message":"chatting","emotes_detected":[],"is_swarm_emote":false}}"#
            )
            .unwrap();
        }
    }

    // Hype burst at t = 70s: 25 pure swarm emote messages
    for (i, phrase) in swarm_phrases.iter().cycle().take(25).enumerate() {
        let (emotes, is_swarm) = extract_message_emotes(phrase);
        let event = neuroclip_lib::ChatEvent {
            timestamp_sec: 70.0 + (i as f64) * 0.1,
            author: format!("swarm_fan_{i}"),
            message: phrase.to_string(),
            badges: vec![],
            emotes_detected: emotes,
            is_swarm_emote: is_swarm,
        };
        writeln!(file, "{}", serde_json::to_string(&event).unwrap()).unwrap();
    }
    drop(file);

    let normalizer = StreamingChatNormalizer::new("vod_swarm", dir.path());
    let heatmap = normalizer
        .compute_density_heatmap(&ndjson_path, 10.0, 10.0)
        .unwrap();

    assert!(heatmap.total_buckets >= 8);

    // Hype bucket at index 7 (t = 70.0s)
    let pt = &heatmap.points[7];
    assert_eq!(pt.message_count, 25);
    assert_eq!(pt.swarm_count, 25);
    // Swarm ratio must be exactly 1.0 (100% swarm)
    assert!(
        (pt.swarm_ratio - 1.0).abs() < 1e-4,
        "Expected swarm_ratio = 1.0, got {}",
        pt.swarm_ratio
    );
    // Message count >= 15 with quiet baseline triggers Z-score spike and is_peak!
    assert!(pt.z_score >= 2.0, "Expected z_score >= 2.0, got {}", pt.z_score);
    assert!(pt.is_peak, "Expected is_peak == true for 25 messages after quiet baseline");
    // No NaN or Inf
    assert!(!pt.z_score.is_nan());
    assert!(!pt.z_score.is_infinite());
}

#[test]
fn test_adv_single_message_stream_noise_floor_clamping() {
    let dir = AdversarialTestDir::new("single_msg");
    let ndjson_path = dir.path().join("single.ndjson");

    let mut file = File::create(&ndjson_path).unwrap();
    // Exactly 1 message at timestamp 0.0
    writeln!(
        file,
        r#"{{"timestamp_sec":0.0,"author":"LoneUser","message":"First!","emotes_detected":[],"is_swarm_emote":false}}"#
    )
    .unwrap();
    drop(file);

    let normalizer = StreamingChatNormalizer::new("vod_single", dir.path());
    let heatmap = normalizer
        .compute_density_heatmap(&ndjson_path, 10.0, 10.0)
        .unwrap();

    assert_eq!(heatmap.total_buckets, 1);
    let pt = &heatmap.points[0];
    assert_eq!(pt.message_count, 1);
    // Noise floor: count 1 < 15 => z_score MUST be 0.0
    assert_eq!(
        pt.z_score, 0.0,
        "Noise floor clamp failed: count 1 must yield z_score = 0.0"
    );
    assert!(!pt.is_peak);
    assert_eq!(pt.swarm_count, 0);
    assert_eq!(pt.swarm_ratio, 0.0);
}

#[test]
fn test_adv_spam_bot_flooding_1000_messages_clamped_to_3() {
    let dir = AdversarialTestDir::new("bot_flooding");
    let ndjson_path = dir.path().join("flooding.ndjson");

    let mut file = File::create(&ndjson_path).unwrap();
    // 1 rogue author floods 1,000 identical spam messages in the same 10s window (at timestamp 5.0)
    for _ in 0..1000 {
        writeln!(
            file,
            r#"{{"timestamp_sec":5.0,"author":"spammer_99","message":"BUY CRYPTO TOKEN","emotes_detected":[],"is_swarm_emote":false}}"#
        )
        .unwrap();
    }
    // 2 legitimate users send messages
    writeln!(
        file,
        r#"{{"timestamp_sec":6.0,"author":"real_user_a","message":"hello","emotes_detected":[],"is_swarm_emote":false}}"#
    )
    .unwrap();
    writeln!(
        file,
        r#"{{"timestamp_sec":7.0,"author":"real_user_b","message":"tutel","emotes_detected":["Tutel"],"is_swarm_emote":true}}"#
    )
    .unwrap();
    drop(file);

    let normalizer = StreamingChatNormalizer::new("vod_flood", dir.path());
    let heatmap = normalizer
        .compute_density_heatmap(&ndjson_path, 10.0, 10.0)
        .unwrap();

    assert_eq!(heatmap.total_buckets, 1);
    let pt = &heatmap.points[0];
    // Spammer must be clamped to 3. Real users: 2. Total = 5!
    assert_eq!(
        pt.message_count, 5,
        "Spam bot defense failed: expected 5 effective messages, got {}",
        pt.message_count
    );
    assert!((pt.velocity_mps - 0.5).abs() < 1e-4);
    // 5 < 15 threshold => z_score = 0.0
    assert_eq!(pt.z_score, 0.0);
}

#[test]
fn test_adv_rapid_cancel_pre_cancelled_token() {
    let dir = AdversarialTestDir::new("rapid_cancel");
    let raw_input = dir.path().join("synthetic.ndjson");
    let target_json = dir.path().join("out.json");

    let mut file = File::create(&raw_input).unwrap();
    for i in 0..2000 {
        writeln!(
            file,
            r#"{{"timestamp_sec":{:.1},"author":"u_{i}","message":"msg","emotes_detected":[],"is_swarm_emote":false}}"#,
            (i as f64) * 0.1
        )
        .unwrap();
    }
    drop(file);

    // Cancel token is cancelled BEFORE execution begins
    let cancel_token = CancellationToken::new();
    cancel_token.cancel();

    let normalizer = StreamingChatNormalizer::new("vod_cancel", dir.path());
    let result = normalizer.normalize_file(&raw_input, &target_json, &cancel_token);

    assert!(
        result.is_err(),
        "Expected TaskCancelled error when token is pre-cancelled"
    );
    match result.unwrap_err() {
        AppError::TaskCancelled => {} // Correct!
        other => panic!("Expected AppError::TaskCancelled, got: {other:?}"),
    }
}

#[test]
fn test_adv_unicode_vietnamese_and_emoji_stress() {
    let test_messages = [
        "Chào buổi tối cả nhà! Neuro dễ thương quá 💖 Tutel!",
        "Vedal không chịu cho Neuro chơi game 💀 KEKW 😂",
        "Bộ lọc AI bị lủng rồi: ôi trời ơi! 🎭",
        "Đang xem stream cùng 100,000 gymbags 🐢🐢🐢",
        "Cảm ơn bạn đã donate: Nguyễn Thị Bích Loan - 500.000 VNĐ",
    ];

    for msg in &test_messages {
        let (emotes, is_swarm) = extract_message_emotes(msg);
        let raw = format!(
            r#"{{"timestamp_sec":15.5,"author":"Người Dùng VN","message":"{msg}","emotes_detected":{:?},"is_swarm_emote":{is_swarm}}}"#,
            emotes
        );
        let parsed = parse_raw_chat_line(&raw);
        assert!(parsed.is_some(), "Failed parsing Vietnamese unicode message: {msg}");
        let event = parsed.unwrap();
        assert_eq!(event.author, "Người Dùng VN");
        assert_eq!(event.message, *msg);
    }
}
