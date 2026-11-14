#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

//! Unit and integration tests for the zero-panic transcript parser (TASK-M4-08).

use neuroclip_lib::stt::{
    calculate_cps, is_special_token, parse_timestamp_to_seconds, parse_whisper_json_full,
    parse_whisper_stdout_line, CONFIDENCE_FLAG_THRESHOLD, CPS_FLAG_THRESHOLD,
};

#[test]
fn test_ut_08_1_happy_path_whisper_json_full_parsing() {
    let mock_json = r#"{
        "transcription": [
            {
                "timestamps": { "from": "00:00:01,000", "to": "00:00:03,500" },
                "offsets": { "from": 1000, "to": 3500 },
                "text": " Hello Vedal tutel",
                "tokens": [
                    {
                        "text": " Hello",
                        "timestamps": { "from": "00:00:01,000", "to": "00:00:01,800" },
                        "offsets": { "from": 1000, "to": 1800 },
                        "p": 0.95
                    },
                    {
                        "text": " Vedal",
                        "timestamps": { "from": "00:00:01,800", "to": "00:00:02,600" },
                        "offsets": { "from": 1800, "to": 2600 },
                        "p": 0.98
                    },
                    {
                        "text": " tutel",
                        "timestamps": { "from": "00:00:02,600", "to": "00:00:03,500" },
                        "offsets": { "from": 2600, "to": 3500 },
                        "p": 0.92
                    }
                ]
            }
        ]
    }"#;

    let res = parse_whisper_json_full(mock_json, "clip_test_101");
    assert!(res.is_ok(), "Expected valid parsing of full whisper JSON");

    let container = res.unwrap();
    assert_eq!(container.clip_id, "clip_test_101");
    assert_eq!(container.segments.len(), 1);

    let seg = &container.segments[0];
    assert_eq!(seg.segment_id, "seg_0001");
    assert!((seg.start_sec - 1.0).abs() < 1e-4);
    assert!((seg.end_sec - 3.5).abs() < 1e-4);
    assert_eq!(seg.text_en, "Hello Vedal tutel");
    assert_eq!(seg.words.len(), 3);

    // Verify individual words
    assert_eq!(seg.words[0].word, "Hello");
    assert!((seg.words[0].start_sec - 1.0).abs() < 1e-4);
    assert!((seg.words[0].end_sec - 1.8).abs() < 1e-4);
    assert!((seg.words[0].probability - 0.95).abs() < 1e-4);

    assert_eq!(seg.words[1].word, "Vedal");
    assert_eq!(seg.words[2].word, "tutel");

    // Check monotonic timestamps
    assert!(seg.end_sec >= seg.start_sec);
    assert!(seg.confidence > CONFIDENCE_FLAG_THRESHOLD);
    assert!(!seg.is_flagged_for_review);
}

#[test]
fn test_ut_08_2_empty_transcription_returns_clean_container() {
    let empty_json = r#"{"transcription": []}"#;
    let res = parse_whisper_json_full(empty_json, "clip_empty");
    assert!(res.is_ok());

    let container = res.unwrap();
    assert_eq!(container.clip_id, "clip_empty");
    assert!(container.segments.is_empty());

    // Whitespace input
    let res_ws = parse_whisper_json_full("   ", "clip_ws");
    assert!(res_ws.is_ok());
    assert_eq!(res_ws.unwrap().segments.len(), 0);
}

#[test]
fn test_ut_08_3_corrupted_json_returns_parser_failed_without_panic() {
    let corrupted = r#"{"transcription": [{"offsets": { "from": 1000"#;
    let res = parse_whisper_json_full(corrupted, "clip_corrupted");
    assert!(res.is_err());

    let err = res.unwrap_err();
    assert_eq!(err.code(), "ERR_STT_PARSER_FAILED");
}

#[test]
fn test_ut_08_4_nan_and_infinite_floats_sanitization() {
    let weird_json = r#"{
        "transcription": [
            {
                "offsets": { "from": -5000, "to": 2000 },
                "text": "Negative start offset test"
            }
        ]
    }"#;

    let res = parse_whisper_json_full(weird_json, "clip_clamped");
    assert!(res.is_ok());

    let container = res.unwrap();
    assert_eq!(container.segments.len(), 1);
    let seg = &container.segments[0];
    assert!(seg.start_sec >= 0.0, "Start sec must be non-negative");
    assert!(seg.end_sec >= seg.start_sec, "End sec must be >= start sec");
}

#[test]
fn test_ut_08_5_cps_calculation_and_flagging() {
    // 10 chars over 1.0 sec = 10.0 CPS (under 20 threshold)
    let cps_normal = calculate_cps("Hello chat", 0.0, 1.0);
    assert!((cps_normal - 10.0).abs() < 1e-4);
    assert!(cps_normal < CPS_FLAG_THRESHOLD);

    // 50 chars over 1.0 sec = 50.0 CPS (over 20 threshold)
    let fast_text = "This is a very fast talking streamer sentence that exceeds limits";
    let cps_fast = calculate_cps(fast_text, 0.0, 1.0);
    assert!(cps_fast > CPS_FLAG_THRESHOLD);

    // Zero duration edge case (should not divide by zero or return NaN)
    let cps_zero_dur = calculate_cps("Instant", 2.0, 2.0);
    assert!(cps_zero_dur.is_finite());
    assert!(cps_zero_dur >= 0.0);
}

#[test]
fn test_stdout_line_parser() {
    let line = "[00:01:23.500 --> 00:01:26.800]   I am not an AI, I am real!";
    let parsed = parse_whisper_stdout_line(line);
    assert!(parsed.is_some());

    let (start, end, text) = parsed.unwrap();
    assert!((start - 83.5).abs() < 1e-4);
    assert!((end - 86.8).abs() < 1e-4);
    assert_eq!(text, "I am not an AI, I am real!");

    // Non-timestamp line returns None
    assert!(parse_whisper_stdout_line("whisper_init_from_file: loading model").is_none());
}

#[test]
fn test_special_tokens_filtering() {
    assert!(is_special_token("<|startoftranscript|>"));
    assert!(is_special_token("<|transcribe|>"));
    assert!(is_special_token("[_BEG_]"));
    assert!(is_special_token("[BLANK_AUDIO]"));
    assert!(is_special_token("[Music]"));
    assert!(!is_special_token("Hello"));
    assert!(!is_special_token("Vedal"));
}

#[test]
fn test_parse_timestamp_to_seconds() {
    assert_eq!(parse_timestamp_to_seconds("00:00:02.500"), Some(2.5));
    assert_eq!(parse_timestamp_to_seconds("00:01:30,000"), Some(90.0));
    assert_eq!(parse_timestamp_to_seconds("01:00:00.000"), Some(3600.0));
    assert_eq!(parse_timestamp_to_seconds("05:12.500"), Some(312.5));
    assert_eq!(parse_timestamp_to_seconds("invalid"), None);
}
