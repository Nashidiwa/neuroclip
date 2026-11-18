#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

//! Integration, Performance & Adversarial Test Suite for TASK-M4-10:
//! Động Cơ Trọng Số Emote Đặc Thù Văn Hóa Neuro Swarm (Emote Swarm Ratio Filter).
//!
//! Complies with `docs/CONVENTIONS.md` (§7 Testing & QA) and `docs/TASK_BOARD.md` §TASK-M4-10.

use std::fs::File;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::Instant;

use neuroclip_lib::{
    calculate_emote_ratio, compute_chat_velocity_series_from_events, compute_emote_density_series,
    compute_emote_density_series_default, compute_emote_density_series_from_file,
    default_swarm_dictionary, extract_message_tokens, extract_swarm_emotes, is_swarm_emote,
    ChatEvent, ChatEventsContainer, EmoteDensityPoint, SwarmDictionary, SwarmEmoteCategory,
    DEFAULT_BUCKET_WIDTH_SEC, SPAM_BOT_MAX_MESSAGES_PER_AUTHOR,
};

struct TestDir {
    path: PathBuf,
}

impl TestDir {
    fn new(name: &str) -> Self {
        let path = std::env::temp_dir().join(format!(
            "neuroclip_emote_test_{name}_{}_{}",
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
// UT-10.1: Happy Path Swarm Emote Extraction
// ---------------------------------------------------------------------------

#[test]
fn test_ut_10_1_happy_path_swarm_match() {
    let msg = "KEKW look at that tutel 💀";
    let detected = extract_swarm_emotes(msg);

    assert_eq!(
        detected,
        vec!["KEKW".to_string(), "tutel".to_string(), "💀".to_string()],
        "Must extract exactly ['KEKW', 'tutel', '💀'] per UT-10.1"
    );

    let dict = default_swarm_dictionary();
    let (tokens, total, emotes, weighted, canonical) = extract_message_tokens(msg, &dict);
    assert_eq!(tokens, detected);
    assert_eq!(total, 6);
    assert_eq!(emotes, 3);
    assert!(weighted > 0.0);
    assert_eq!(canonical, vec!["KEKW", "Tutel", "💀"]);
}

// ---------------------------------------------------------------------------
// UT-10.2: Token Boundaries, Case Insensitivity & Unicode Emotes
// ---------------------------------------------------------------------------

#[test]
fn test_ut_10_2_case_insensitivity_and_token_boundaries() {
    // Case-insensitivity tests
    assert!(is_swarm_emote("kekw"));
    assert!(is_swarm_emote("KEKW"));
    assert!(is_swarm_emote("Kekw"));
    assert!(is_swarm_emote("TUTEL"));
    assert!(is_swarm_emote("tutel"));
    assert!(is_swarm_emote("Tutel"));
    assert!(is_swarm_emote("ERM"));
    assert!(is_swarm_emote("erm"));
    assert!(is_swarm_emote("Erm"));
    assert!(is_swarm_emote("SOOP"));
    assert!(is_swarm_emote("soop"));
    assert!(is_swarm_emote("PogBones"));
    assert!(is_swarm_emote("pogbones"));

    // Unicode skull and heart emoticon
    assert!(is_swarm_emote("💀"));
    assert!(is_swarm_emote("<3"));

    // Outer punctuation trimming
    assert!(is_swarm_emote("(tutel)!"));
    assert!(is_swarm_emote("Erm..."));
    assert!(is_swarm_emote("KEKW,"));
    assert!(is_swarm_emote("<3!"));
    assert!(is_swarm_emote("(<3)"));
    assert!(is_swarm_emote("SOOP!"));

    // Strict word boundary checks: negative non-matches
    assert!(!is_swarm_emote("medieval"), "medieval must not match evil");
    assert!(!is_swarm_emote("talking"), "talking must not match king");
    assert!(!is_swarm_emote("pogo"), "pogo must not match pog");
    assert!(
        !is_swarm_emote("neurological"),
        "neurological must not match neuro"
    );
    assert!(!is_swarm_emote("channel"), "channel must not match anny");
    assert!(!is_swarm_emote("hello"), "plain words must not match");
    assert!(!is_swarm_emote("world"), "plain words must not match");
}

// ---------------------------------------------------------------------------
// UT-10.3: Emote Ratio Calculation ($E_{\text{ratio}}(t) \in [0.0, 1.0]$)
// ---------------------------------------------------------------------------

#[test]
fn test_ut_10_3_ratio_math_and_clamp() {
    // 50 total tokens, 25 swarm emotes => 25 / 50 = 0.50 (50%)
    let ratio = calculate_emote_ratio(25, 50);
    assert!(
        (ratio - 0.50).abs() < 1e-6,
        "Expected E_ratio=0.50, got {ratio}"
    );

    // 0 tokens => 0.0 (no division by zero)
    assert_eq!(calculate_emote_ratio(0, 0), 0.0);

    // Clamping invariants
    assert_eq!(calculate_emote_ratio(0, 50), 0.0);
    assert_eq!(calculate_emote_ratio(50, 50), 1.0);
    assert_eq!(calculate_emote_ratio(100, 50), 1.0);
}

// ---------------------------------------------------------------------------
// IT-10.1: Alignment with ChatVelocityPoint Series (5.0s Resolution)
// ---------------------------------------------------------------------------

#[test]
fn test_it_10_1_alignment_with_chat_velocity_series() {
    let mut events = Vec::new();
    for i in 0..120 {
        let ts = (i as f64) * 1.5; // spans 0.0 to 178.5s
        let author = format!("user_{}", i % 15);
        let msg = if i % 3 == 0 {
            "KEKW look at that tutel 💀"
        } else if i % 3 == 1 {
            "just chatting here"
        } else {
            "Erm SOOP <3"
        };
        events.push(create_event(ts, &author, msg));
    }

    let vod_duration_sec = Some(200.0);
    let velocity_series =
        compute_chat_velocity_series_from_events(&events, 5.0, 60.0, vod_duration_sec)
            .expect("Velocity computation must succeed");
    let emote_series = compute_emote_density_series(&events, None, 5.0, vod_duration_sec)
        .expect("Emote series must succeed");

    // Must have identical length
    assert_eq!(
        velocity_series.len(),
        emote_series.len(),
        "Velocity and Emote series must have 1:1 identical bucket counts"
    );

    // Must have identical timestamps for every single bucket
    for (idx, (vel, emo)) in velocity_series.iter().zip(emote_series.iter()).enumerate() {
        assert!(
            (vel.timestamp_sec - emo.timestamp_sec).abs() < 1e-6,
            "Bucket {idx} timestamp mismatch: velocity={}s vs emote={}s",
            vel.timestamp_sec,
            emo.timestamp_sec
        );
        assert!(
            (emo.timestamp_sec - (idx as f64) * 5.0).abs() < 1e-6,
            "Bucket {idx} must be exact multiple of 5.0s"
        );
        assert!(
            emo.e_ratio >= 0.0 && emo.e_ratio <= 1.0,
            "Bucket {idx} e_ratio must be in [0.0, 1.0]"
        );
    }
}

// ---------------------------------------------------------------------------
// ED-10.1: Throughput SLA Benchmark (500,000 Messages <= 1.0s)
// ---------------------------------------------------------------------------

#[test]
fn test_ed_10_1_performance_benchmark_500k_messages() {
    let count = 500_000;
    let mut events = Vec::with_capacity(count);

    for i in 0..count {
        let ts = ((i % 5760) as f64) * 5.0 + 0.05 * ((i % 80) as f64);
        let author = format!("viewer_{}", i % 500);
        let msg = match i % 8 {
            0 => "KEKW",
            1 => "hello",
            2 => "tutel 💀",
            3 => "SOOP",
            4 => "Erm <3",
            5 => "PogBones",
            6 => "lmao",
            _ => "Aware",
        };
        events.push(create_event(ts, &author, msg));
    }

    let start = Instant::now();
    let series =
        compute_emote_density_series(&events, None, DEFAULT_BUCKET_WIDTH_SEC, Some(28800.0))
            .expect("500k benchmark must succeed");
    let elapsed = start.elapsed();
    let elapsed_sec = elapsed.as_secs_f64();

    let total_tokens_scanned: u32 = series.iter().map(|p| p.total_tokens).sum();
    let tokens_per_sec = f64::from(total_tokens_scanned) / elapsed_sec;

    println!(
        "\n=======================================================\n\
         ED-10.1 Performance Benchmark:\n\
         Processed {} messages into {} buckets in {:.2}ms\n\
         Total Tokens Scanned: {}\n\
         Throughput: {:.0} tokens/sec\n\
         SLA Target: <= 1.0s (>= 500,000 tokens/s)\n\
         =======================================================",
        count,
        series.len(),
        elapsed_sec * 1000.0,
        total_tokens_scanned,
        tokens_per_sec
    );

    assert!(
        elapsed_sec <= 1.0,
        "Benchmark exceeded 1.0s SLA: took {elapsed_sec:.4}s"
    );
    assert!(
        tokens_per_sec >= 500_000.0,
        "Throughput below 500,000 tokens/s SLA: got {tokens_per_sec:.0} tokens/s"
    );
}

// ---------------------------------------------------------------------------
// ED-10.2: Spam Bot Neutralization (SPAM_BOT_MAX_MESSAGES_PER_AUTHOR = 3)
// ---------------------------------------------------------------------------

#[test]
fn test_spambot_author_cap_neutralization() {
    let mut events = Vec::new();
    // Spammer posts 50 messages of "KEKW TUTEL 💀" in bucket 0 [0.0, 5.0)
    for _ in 0..50 {
        events.push(create_event(2.0, "malicious_bot", "KEKW TUTEL 💀"));
    }
    // Genuine user posts 1 normal message
    events.push(create_event(3.0, "normal_user", "hello world"));

    let series = compute_emote_density_series(&events, None, 5.0, None).expect("Must succeed");
    assert_eq!(series.len(), 1);

    // Bot is capped to 3 messages: 3 * 3 = 9 tokens (all 9 emotes)
    // Normal user: 1 message with 2 tokens (0 emotes)
    // Total tokens: 9 + 2 = 11 tokens. Emotes: 9.
    assert_eq!(series[0].total_tokens, 11);
    assert_eq!(series[0].emote_count, 9);
    let expected_ratio = 9.0 / 11.0;
    assert!((series[0].e_ratio - expected_ratio).abs() < 1e-6);
}

// ---------------------------------------------------------------------------
// Cultural Weights Quantification & Top Emotes Sorting
// ---------------------------------------------------------------------------

#[test]
fn test_cultural_weights_quantification_and_top_emotes() {
    let dict = default_swarm_dictionary();

    // Bucket with high-lore cultural emotes: Tutel (2.0), SOOP (2.0), Erm (2.0)
    let lore_events = vec![
        create_event(1.0, "user1", "Tutel Tutel SOOP"),
        create_event(2.0, "user2", "Erm Erm <3"),
    ];

    let lore_series =
        compute_emote_density_series(&lore_events, Some(&dict), 5.0, None).expect("Must succeed");
    assert_eq!(lore_series.len(), 1);

    // Total tokens: 6 (all emotes). Weights: 2.0*6 = 12.0. Weighted score = 12.0 / 6 = 2.0
    assert_eq!(lore_series[0].total_tokens, 6);
    assert_eq!(lore_series[0].emote_count, 6);
    assert!((lore_series[0].weighted_score - 2.0).abs() < 1e-6);

    // Bucket with generic humor emotes: LMAO (1.0), LOL (1.0), XD (1.0)
    let generic_events = vec![
        create_event(1.0, "user1", "LMAO LOL XD"),
        create_event(2.0, "user2", "lol lmao xd"),
    ];

    let generic_series = compute_emote_density_series(&generic_events, Some(&dict), 5.0, None)
        .expect("Must succeed");
    assert_eq!(generic_series.len(), 1);

    // Total tokens: 6 (all emotes). Weights: 1.0*6 = 6.0. Weighted score = 6.0 / 6 = 1.0
    assert_eq!(generic_series[0].total_tokens, 6);
    assert_eq!(generic_series[0].emote_count, 6);
    assert!((generic_series[0].weighted_score - 1.0).abs() < 1e-6);

    // Culture score must clearly distinguish lore hype vs generic laughs
    assert!(lore_series[0].weighted_score > generic_series[0].weighted_score * 1.8);
}

// ---------------------------------------------------------------------------
// File I/O: NDJSON and JSON Chat Log Parsing
// ---------------------------------------------------------------------------

#[test]
fn test_file_loading_ndjson_and_json() {
    let test_dir = TestDir::new("file_io");

    // 1. NDJSON test
    let ndjson_path = test_dir.path().join("chat.ndjson");
    {
        let mut f = File::create(&ndjson_path).expect("Must create ndjson");
        writeln!(
            f,
            r#"{{"timestamp_sec": 1.0, "author": "u1", "message": "KEKW tutel"}}"#
        )
        .expect("Must write");
        writeln!(
            f,
            r#"{{"timestamp_sec": 6.0, "author": "u2", "message": "SOOP 💀"}}"#
        )
        .expect("Must write");
    }

    let ndjson_series = compute_emote_density_series_from_file(&ndjson_path, None, 5.0, Some(15.0))
        .expect("NDJSON parsing must succeed");
    assert_eq!(ndjson_series.len(), 3); // [0,5), [5,10), [10,15)
    assert_eq!(ndjson_series[0].top_emotes.len(), 2);
    assert_eq!(ndjson_series[1].top_emotes.len(), 2);
    assert_eq!(ndjson_series[2].top_emotes.len(), 0);

    // 2. JSON container test
    let json_path = test_dir.path().join("chat.json");
    {
        let container = ChatEventsContainer {
            vod_id: "vod_test".to_string(),
            total_messages: 2,
            events: vec![
                create_event(2.0, "u1", "Tutel Tutel"),
                create_event(7.0, "u2", "Aware Clueless"),
            ],
        };
        let data = serde_json::to_string(&container).expect("Must serialize container");
        let mut f = File::create(&json_path).expect("Must create json");
        f.write_all(data.as_bytes()).expect("Must write json");
    }

    let json_series = compute_emote_density_series_from_file(&json_path, None, 5.0, Some(10.0))
        .expect("JSON parsing must succeed");
    assert_eq!(json_series.len(), 2);
    assert_eq!(json_series[0].top_emotes, vec!["Tutel".to_string()]);
    assert_eq!(
        json_series[1].top_emotes,
        vec!["Aware".to_string(), "Clueless".to_string()]
    );
}

// ---------------------------------------------------------------------------
// Adversarial Edge Cases: Malformed, Empty, and Extreme Inputs
// ---------------------------------------------------------------------------

#[test]
fn test_adversarial_edge_cases() {
    // 1. Empty chat events with VOD duration padding
    let empty_series =
        compute_emote_density_series(&[], None, 5.0, Some(20.0)).expect("Must succeed");
    assert_eq!(empty_series.len(), 4); // 0s, 5s, 10s, 15s
    for p in &empty_series {
        assert_eq!(p.e_ratio, 0.0);
        assert_eq!(p.total_tokens, 0);
        assert_eq!(p.top_emotes.len(), 0);
    }

    // 2. Empty chat events without duration
    let zero_series = compute_emote_density_series(&[], None, 5.0, None).expect("Must succeed");
    assert_eq!(zero_series.len(), 0);
    let def_series = compute_emote_density_series_default(&[], None).expect("Must succeed");
    assert_eq!(def_series.len(), 0);
    assert_eq!(SPAM_BOT_MAX_MESSAGES_PER_AUTHOR, 3);

    let pt = EmoteDensityPoint::new(0.0, vec!["KEKW".to_string()], 1.0, 1, 1, 1.5);
    assert_eq!(pt.swarm_count(), 1);
    assert_eq!(SwarmEmoteCategory::Humor, SwarmEmoteCategory::Humor);
    let custom_dict = SwarmDictionary::build_default_dictionary();
    assert!(custom_dict.contains("KEKW"));

    // 3. Negative and NaN timestamps
    let dirty_events = vec![
        create_event(-10.0, "bad", "KEKW"),
        create_event(f64::NAN, "bad", "Tutel"),
        create_event(f64::INFINITY, "bad", "SOOP"),
        create_event(1.0, "good", "KEKW tutel"),
    ];
    let dirty_series =
        compute_emote_density_series(&dirty_events, None, 5.0, None).expect("Must succeed");
    assert_eq!(dirty_series.len(), 1);
    assert_eq!(dirty_series[0].emote_count, 2);

    // 4. Invalid bucket width errors
    assert!(compute_emote_density_series(&[], None, 0.0, None).is_err());
    assert!(compute_emote_density_series(&[], None, -5.0, None).is_err());
    assert!(compute_emote_density_series(&[], None, f64::NAN, None).is_err());
}
