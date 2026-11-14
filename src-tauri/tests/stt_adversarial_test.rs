#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

//! Adversarial stress, boundary, and corruption tests for STT pipeline (TASK-M4-08).

use neuroclip_lib::stt::{
    align_segments_with_vad, parse_whisper_json_full, SpeechInterval, TranscriptSegment, WordTiming,
};

#[test]
fn test_ed_08_1_silence_and_non_speech_handling() {
    let silence_json = r#"{
        "transcription": [
            {
                "text": " [BLANK_AUDIO] ",
                "offsets": { "from": 0, "to": 5000 }
            },
            {
                "text": " [Music] ",
                "offsets": { "from": 5000, "to": 10000 }
            }
        ]
    }"#;

    let res = parse_whisper_json_full(silence_json, "clip_silence");
    assert!(res.is_ok());

    let container = res.unwrap();
    // Special tokens should be filtered out, leaving zero segments
    assert_eq!(container.segments.len(), 0);
}

#[test]
fn test_ed_08_3_zero_panic_adversarial_parser_stress() {
    // 1. Completely random binary/corrupted bytes
    let raw_bytes = b"\x00\xFF\xFE\xFD{invalid json \x12\x15";
    let garbage = String::from_utf8_lossy(raw_bytes);
    let res1 = parse_whisper_json_full(&garbage, "clip_garbage");
    assert!(res1.is_err());
    assert_eq!(res1.unwrap_err().code(), "ERR_STT_PARSER_FAILED");

    // 2. Truncated mid-string JSON
    let truncated = r#"{"transcription": [{"text": "Hello wor"#;
    let res2 = parse_whisper_json_full(truncated, "clip_trunc");
    assert!(res2.is_err());

    // 3. Negative timestamps
    let negative_ts = r#"{
        "transcription": [
            {
                "offsets": { "from": -15000, "to": -5000 },
                "text": "Negative times"
            }
        ]
    }"#;
    let res3 = parse_whisper_json_full(negative_ts, "clip_neg");
    assert!(res3.is_ok());
    let seg = &res3.unwrap().segments[0];
    assert_eq!(seg.start_sec, 0.0);
    assert_eq!(seg.end_sec, 0.0);

    // 4. Inverted timestamps (start > end)
    let inverted_ts = r#"{
        "transcription": [
            {
                "offsets": { "from": 8000, "to": 2000 },
                "text": "Inverted times"
            }
        ]
    }"#;
    let res4 = parse_whisper_json_full(inverted_ts, "clip_inv");
    assert!(res4.is_ok());
    let seg4 = &res4.unwrap().segments[0];
    assert!(seg4.end_sec >= seg4.start_sec);

    // 5. Heavy Unicode, Vietnamese diacritics, and Emojis
    let unicode_json = r#"{
        "transcription": [
            {
                "offsets": { "from": 1000, "to": 4000 },
                "text": "Neuro-sama nói: Xin chào Việt Nam! 💀 Tutel 🐢 ❤️ 100%"
            }
        ]
    }"#;
    let res5 = parse_whisper_json_full(unicode_json, "clip_unicode");
    assert!(res5.is_ok());
    assert_eq!(
        res5.unwrap().segments[0].text_en,
        "Neuro-sama nói: Xin chào Việt Nam! 💀 Tutel 🐢 ❤️ 100%"
    );
}

#[test]
fn test_ed_08_4_bounded_memory_under_large_payload() {
    // Generate a massive payload with 1,000 words
    let mut large_json = String::from("{\"transcription\": [");
    for i in 0..100 {
        if i > 0 {
            large_json.push(',');
        }
        large_json.push_str(&format!(
            "{{\"offsets\": {{\"from\": {}, \"to\": {}}}, \"text\": \"Segment number {} containing banter and chat words tutel tutel tutel\"}}",
            i * 5000,
            (i + 1) * 5000,
            i
        ));
    }
    large_json.push_str("]}");

    let res = parse_whisper_json_full(&large_json, "clip_large_100");
    assert!(res.is_ok());
    let container = res.unwrap();
    assert_eq!(container.segments.len(), 100);
}

#[test]
fn test_vad_bridge_snapping_and_word_gap_splitting() {
    let seg = TranscriptSegment {
        segment_id: "seg_0001".to_string(),
        start_sec: 1.15, // close to 1.0 (within 250ms)
        end_sec: 6.85,   // close to 7.0 (within 250ms)
        speaker_id: "vedal".to_string(),
        text_en: "Hello chat ... wait what".to_string(),
        text_vi: None,
        confidence: 0.95,
        cps_score: 5.0,
        is_flagged_for_review: false,
        words: vec![
            WordTiming {
                word: "Hello".to_string(),
                start_sec: 1.15,
                end_sec: 1.80,
                probability: 0.98,
            },
            WordTiming {
                word: "chat".to_string(),
                start_sec: 1.80,
                end_sec: 2.30,
                probability: 0.96,
            },
            // 2.0s gap between 2.30 and 4.30 (> 1.0s) -> triggers split!
            WordTiming {
                word: "wait".to_string(),
                start_sec: 4.30,
                end_sec: 5.00,
                probability: 0.94,
            },
            WordTiming {
                word: "what".to_string(),
                start_sec: 5.00,
                end_sec: 6.85,
                probability: 0.92,
            },
        ],
    };

    let intervals = vec![SpeechInterval {
        start_sec: 1.0,
        end_sec: 7.0,
    }];

    let aligned = align_segments_with_vad(&[seg], &intervals);
    // Should be split into 2 subsegments at the 2.0s pause gap
    assert_eq!(aligned.len(), 2);

    // Subsegment 1
    assert_eq!(aligned[0].segment_id, "seg_0001_1");
    assert_eq!(aligned[0].text_en, "Hello chat");
    assert_eq!(aligned[0].words.len(), 2);

    // Subsegment 2
    assert_eq!(aligned[1].segment_id, "seg_0001_2");
    assert_eq!(aligned[1].text_en, "wait what");
    assert_eq!(aligned[1].words.len(), 2);
}
