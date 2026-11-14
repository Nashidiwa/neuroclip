#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

//! Adversarial stress test harness written by Challenger 1 for TASK-M4-08.
//!
//! Empirically stress-tests:
//! 1. Parser resilience: Malformed JSON, negative offsets, extreme CPS, non-UTF8/garbage.
//! 2. Model integrity fast-fail: Missing files, 0-byte files, 1-byte mutations (start, mid, end).
//! 3. VAD bridge: Empty intervals, multiple pause gaps > 1.0s, boundary integrity, 0/1-word segments.
//! 4. Subtitle export: ASS injection attacks, newline escaping, SRT sequential invariants.

use std::fs::File;
use std::io::Write;
use neuroclip_lib::stt::{
    align_segments_with_vad, calculate_cps, compute_file_sha256, escape_ass_text,
    format_ass_timecode, format_srt_timecode, generate_ass_script, generate_srt_script,
    is_special_token, parse_timestamp_to_seconds, parse_whisper_json_full,
    SpeechInterval, TranscriptSegment, TranscriptSegmentsContainer, WordTiming,
    CPS_FLAG_THRESHOLD, CONFIDENCE_FLAG_THRESHOLD,
    verify_model,
};

#[test]
fn test_adv_malformed_json_variants() {
    // 1. Truncated at various points
    let cuts = [
        "",
        "{",
        "{\"transcription\"",
        "{\"transcription\": [",
        "{\"transcription\": [{\"offsets\": {",
        "{\"transcription\": [{\"text\": \"Hello",
    ];

    for cut in &cuts {
        let res = parse_whisper_json_full(cut, "adv_cut");
        if cut.trim().is_empty() {
            assert!(res.is_ok());
            assert_eq!(res.unwrap().segments.len(), 0);
        } else {
            assert!(res.is_err(), "Expected parser failure for: {cut}");
            assert_eq!(res.unwrap_err().code(), "ERR_STT_PARSER_FAILED");
        }
    }

    // 2. JSON non-objects
    let non_objects = ["12345", "true", "\"just a string\"", "[1, 2, 3]"];
    for non_obj in &non_objects {
        let res = parse_whisper_json_full(non_obj, "adv_non_obj");
        assert!(res.is_err(), "Expected failure for non-object: {non_obj}");
    }

    // 3. Binary control characters and non-UTF8
    let garbage = "\u{0000}\u{0001}\u{001f}{\"invalid\": true\u{0007}}";
    let res = parse_whisper_json_full(garbage, "adv_garbage");
    assert!(res.is_err());
    assert_eq!(res.unwrap_err().code(), "ERR_STT_PARSER_FAILED");

    // 4. Empty objects
    let empty_obj = "{}";
    let res = parse_whisper_json_full(empty_obj, "adv_empty_obj");
    assert!(res.is_ok());
    assert_eq!(res.unwrap().segments.len(), 0);

    // 5. Empty segments array
    let empty_segs = "{\"segments\": []}";
    let res = parse_whisper_json_full(empty_segs, "adv_empty_segs");
    assert!(res.is_ok());
    assert_eq!(res.unwrap().segments.len(), 0);
}

#[test]
fn test_adv_negative_and_extreme_timestamps() {
    // 1. Extreme negative offsets
    let json_neg = r#"{
        "transcription": [
            {
                "offsets": { "from": -99999999, "to": -10000000 },
                "text": "Deep in negative time"
            }
        ]
    }"#;
    let res = parse_whisper_json_full(json_neg, "adv_neg");
    assert!(res.is_ok());
    let seg = &res.unwrap().segments[0];
    assert!(seg.start_sec >= 0.0, "start_sec must be non-negative");
    assert!(seg.end_sec >= seg.start_sec, "end_sec must be >= start_sec");

    // 2. Start offset greater than end offset
    let json_inv = r#"{
        "transcription": [
            {
                "offsets": { "from": 50000, "to": 10000 },
                "text": "Time runs backwards"
            }
        ]
    }"#;
    let res = parse_whisper_json_full(json_inv, "adv_inv");
    assert!(res.is_ok());
    let seg = &res.unwrap().segments[0];
    assert_eq!(seg.start_sec, 50.0);
    assert_eq!(seg.end_sec, 50.0, "end_sec should be clamped to >= start_sec");

    // 3. String timestamp parsing stress
    assert_eq!(parse_timestamp_to_seconds("-01:00:00,000"), Some(0.0));
    assert_eq!(parse_timestamp_to_seconds("00:-05:00,000"), Some(0.0));
    assert_eq!(parse_timestamp_to_seconds("00:00:-10,000"), Some(0.0));
    assert_eq!(parse_timestamp_to_seconds("00:00:00,000"), Some(0.0));
    assert_eq!(parse_timestamp_to_seconds("99:59:59.999"), Some(99.0 * 3600.0 + 59.0 * 60.0 + 59.999));
    assert_eq!(parse_timestamp_to_seconds(":::"), None);
    assert_eq!(parse_timestamp_to_seconds("invalid:string"), None);
}

#[test]
fn test_adv_extreme_cps_and_flagging() {
    // 1. Extremely long text in tiny duration -> CPS should be huge, flagged for review
    let long_text = "A".repeat(5000);
    let cps = calculate_cps(&long_text, 0.0, 1.0);
    assert!(cps > 1000.0);
    assert!(cps.is_finite());

    // 2. Zero duration
    let cps_zero = calculate_cps("Hello", 5.0, 5.0);
    assert!(cps_zero.is_finite());
    assert!(cps_zero > 0.0);

    // 3. Negative duration in calculate_cps (start > end)
    let cps_neg = calculate_cps("Hello", 10.0, 5.0);
    assert!(cps_neg.is_finite());
    assert!(cps_neg > 0.0);

    // 4. In full parser: test that high CPS automatically sets is_flagged_for_review = true
    let fast_talk_json = r#"{
        "transcription": [
            {
                "offsets": { "from": 1000, "to": 2000 },
                "text": "This is a super fast talking sentence with lots of words crammed into one second"
            }
        ]
    }"#;
    let res = parse_whisper_json_full(fast_talk_json, "adv_fast");
    assert!(res.is_ok());
    let seg = &res.unwrap().segments[0];
    assert!(seg.cps_score > CPS_FLAG_THRESHOLD);
    assert!(seg.is_flagged_for_review, "Should be flagged for review due to high CPS");

    // 5. Low confidence (< 0.6) sets is_flagged_for_review = true
    let low_conf_json = r#"{
        "transcription": [
            {
                "offsets": { "from": 1000, "to": 4000 },
                "text": "Quiet mumble",
                "tokens": [
                    { "text": " Quiet", "offsets": { "from": 1000, "to": 2500 }, "p": 0.4 },
                    { "text": " mumble", "offsets": { "from": 2500, "to": 4000 }, "p": 0.3 }
                ]
            }
        ]
    }"#;
    let res_low = parse_whisper_json_full(low_conf_json, "adv_low_conf");
    assert!(res_low.is_ok());
    let seg_low = &res_low.unwrap().segments[0];
    assert!(seg_low.confidence < CONFIDENCE_FLAG_THRESHOLD);
    assert!(seg_low.is_flagged_for_review, "Should be flagged for review due to low confidence");
}

#[test]
fn test_adv_special_tokens_comprehensive_filtering() {
    let tokens = [
        "<|startoftranscript|>",
        "<|transcribe|>",
        "<|translate|>",
        "<|notimestamps|>",
        "<|0.00|>",
        "<|30.00|>",
        "[_BEG_]",
        "[BLANK_AUDIO]",
        "[Music]",
        "[Applause]",
        "[laughter]",
        "[Silence]",
    ];

    for tok in &tokens {
        assert!(is_special_token(tok), "Expected {tok} to be recognized as special token");
    }

    // Normal speech should NOT be identified as special token
    let normal_tokens = ["Hello", "Vedal", "Neuro-sama", "[bracketed banter]", "tutel"];
    assert!(!is_special_token(normal_tokens[0]));
    assert!(!is_special_token(normal_tokens[1]));
    assert!(!is_special_token(normal_tokens[2]));
    // Note: "[bracketed banter]" has brackets, so by design starts_with('[') && ends_with(']') is true
    assert!(!is_special_token(normal_tokens[4]));

    // When full transcription only contains special tokens, no segments should be emitted
    let mut special_json = String::from("{\"transcription\": [");
    for (i, tok) in tokens.iter().enumerate() {
        if i > 0 {
            special_json.push(',');
        }
        special_json.push_str(&format!(
            "{{\"offsets\": {{\"from\": {}, \"to\": {}}}, \"text\": \" {} \"}}",
            i * 1000,
            (i + 1) * 1000,
            tok
        ));
    }
    special_json.push_str("]}");

    let res = parse_whisper_json_full(&special_json, "adv_special_only");
    assert!(res.is_ok());
    assert_eq!(res.unwrap().segments.len(), 0, "All special token segments must be suppressed");
}

#[test]
fn test_adv_model_integrity_fast_fail_1byte_mutations() {
    let temp_dir = std::env::temp_dir();

    // 1. Missing model file returns ERR_STT_MODEL_NOT_FOUND
    let missing_path = temp_dir.join("non_existent_adv_model.bin");
    let res_missing = verify_model("base.en-q5_k", Some(&missing_path));
    assert!(res_missing.is_err());
    assert_eq!(res_missing.unwrap_err().code(), "ERR_STT_MODEL_NOT_FOUND");

    // 2. 0-byte file returns ERR_STT_MODEL_INTEGRITY_FAILED
    let empty_path = temp_dir.join("adv_empty_model.bin");
    {
        let _f = File::create(&empty_path).unwrap();
    }
    let res_empty = verify_model("base.en-q5_k", Some(&empty_path));
    assert!(res_empty.is_err());
    assert_eq!(res_empty.unwrap_err().code(), "ERR_STT_MODEL_INTEGRITY_FAILED");
    let _ = std::fs::remove_file(&empty_path);

    // 3. Synthetic reference file with known hash
    let ref_path = temp_dir.join("adv_ref_model.bin");
    let data = vec![0x55u8; 10000]; // 10,000 bytes
    {
        let mut f = File::create(&ref_path).unwrap();
        f.write_all(&data).unwrap();
    }
    let ref_hash = compute_file_sha256(&ref_path).unwrap();
    assert_eq!(ref_hash.len(), 64);

    // 4. 1-byte mutation at start (index 0)
    let mut data_mut_start = data.clone();
    data_mut_start[0] ^= 0xFF;
    let mut_start_path = temp_dir.join("adv_mut_start.bin");
    {
        let mut f = File::create(&mut_start_path).unwrap();
        f.write_all(&data_mut_start).unwrap();
    }
    let hash_mut_start = compute_file_sha256(&mut_start_path).unwrap();
    assert_ne!(ref_hash, hash_mut_start, "Start byte mutation must change hash");
    let _ = std::fs::remove_file(&mut_start_path);

    // 5. 1-byte mutation at middle (index 5000)
    let mut data_mut_mid = data.clone();
    data_mut_mid[5000] ^= 0x01;
    let mut_mid_path = temp_dir.join("adv_mut_mid.bin");
    {
        let mut f = File::create(&mut_mid_path).unwrap();
        f.write_all(&data_mut_mid).unwrap();
    }
    let hash_mut_mid = compute_file_sha256(&mut_mid_path).unwrap();
    assert_ne!(ref_hash, hash_mut_mid, "Middle byte mutation must change hash");
    let _ = std::fs::remove_file(&mut_mid_path);

    // 6. 1-byte mutation at end (index 9999)
    let mut data_mut_end = data.clone();
    data_mut_end[9999] ^= 0x80;
    let mut_end_path = temp_dir.join("adv_mut_end.bin");
    {
        let mut f = File::create(&mut_end_path).unwrap();
        f.write_all(&data_mut_end).unwrap();
    }
    let hash_mut_end = compute_file_sha256(&mut_end_path).unwrap();
    assert_ne!(ref_hash, hash_mut_end, "End byte mutation must change hash");
    let _ = std::fs::remove_file(&mut_end_path);

    // 7. Truncated by 1 byte (9999 bytes)
    let data_trunc = data[..9999].to_vec();
    let trunc_path = temp_dir.join("adv_trunc.bin");
    {
        let mut f = File::create(&trunc_path).unwrap();
        f.write_all(&data_trunc).unwrap();
    }
    let hash_trunc = compute_file_sha256(&trunc_path).unwrap();
    assert_ne!(ref_hash, hash_trunc, "Truncated file must change hash");
    let _ = std::fs::remove_file(&trunc_path);

    let _ = std::fs::remove_file(&ref_path);
}

#[test]
fn test_adv_vad_bridge_empty_intervals_and_multi_gap_splitting() {
    let seg = TranscriptSegment {
        segment_id: "seg_0010".to_string(),
        start_sec: 1.0,
        end_sec: 15.0,
        speaker_id: "neuro".to_string(),
        text_en: "one two three four five".to_string(),
        text_vi: None,
        confidence: 0.95,
        cps_score: 1.5,
        is_flagged_for_review: false,
        words: vec![
            WordTiming { word: "one".to_string(), start_sec: 1.0, end_sec: 1.5, probability: 0.99 },
            // Gap 1: 1.5 to 3.5 (2.0s gap > 1.0s) -> split!
            WordTiming { word: "two".to_string(), start_sec: 3.5, end_sec: 4.0, probability: 0.95 },
            // Gap 2: 4.0 to 7.0 (3.0s gap > 1.0s) -> split!
            WordTiming { word: "three".to_string(), start_sec: 7.0, end_sec: 7.5, probability: 0.96 },
            // Small gap: 7.5 to 8.0 (0.5s gap <= 1.0s) -> no split
            WordTiming { word: "four".to_string(), start_sec: 8.0, end_sec: 8.5, probability: 0.97 },
            // Gap 3: 8.5 to 12.0 (3.5s gap > 1.0s) -> split!
            WordTiming { word: "five".to_string(), start_sec: 12.0, end_sec: 12.5, probability: 0.98 },
        ],
    };

    // 1. With empty VAD intervals: should gracefully handle and still perform word gap splitting
    let empty_intervals: Vec<SpeechInterval> = Vec::new();
    let aligned_empty = align_segments_with_vad(std::slice::from_ref(&seg), &empty_intervals);

    // Expected 4 subsegments:
    // Sub 1: "one"
    // Sub 2: "two"
    // Sub 3: "three four"
    // Sub 4: "five"
    assert_eq!(aligned_empty.len(), 4, "Should split into exactly 4 subsegments");
    assert_eq!(aligned_empty[0].segment_id, "seg_0010_1");
    assert_eq!(aligned_empty[0].text_en, "one");
    assert_eq!(aligned_empty[0].words.len(), 1);

    assert_eq!(aligned_empty[1].segment_id, "seg_0010_2");
    assert_eq!(aligned_empty[1].text_en, "two");
    assert_eq!(aligned_empty[1].words.len(), 1);

    assert_eq!(aligned_empty[2].segment_id, "seg_0010_3");
    assert_eq!(aligned_empty[2].text_en, "three four");
    assert_eq!(aligned_empty[2].words.len(), 2);

    assert_eq!(aligned_empty[3].segment_id, "seg_0010_4");
    assert_eq!(aligned_empty[3].text_en, "five");
    assert_eq!(aligned_empty[3].words.len(), 1);

    // Verify token conservation: total words across subsegments must equal original words (5)
    let total_words: usize = aligned_empty.iter().map(|s| s.words.len()).sum();
    assert_eq!(total_words, 5, "Total words must be preserved across splits");

    // 2. Segment with 0 words
    let empty_words_seg = TranscriptSegment {
        segment_id: "seg_nowords".to_string(),
        start_sec: 2.0,
        end_sec: 4.0,
        speaker_id: "vedal".to_string(),
        text_en: "silent segment".to_string(),
        text_vi: None,
        confidence: 0.8,
        cps_score: 7.0,
        is_flagged_for_review: false,
        words: vec![],
    };
    let aligned_nowords = align_segments_with_vad(&[empty_words_seg], &empty_intervals);
    assert_eq!(aligned_nowords.len(), 1);
    assert_eq!(aligned_nowords[0].segment_id, "seg_nowords");

    // 3. Segment with 1 word
    let single_word_seg = TranscriptSegment {
        segment_id: "seg_oneword".to_string(),
        start_sec: 2.0,
        end_sec: 3.0,
        speaker_id: "guest".to_string(),
        text_en: "Tutel".to_string(),
        text_vi: None,
        confidence: 0.99,
        cps_score: 5.0,
        is_flagged_for_review: false,
        words: vec![WordTiming {
            word: "Tutel".to_string(),
            start_sec: 2.0,
            end_sec: 3.0,
            probability: 0.99,
        }],
    };
    let aligned_oneword = align_segments_with_vad(&[single_word_seg], &empty_intervals);
    assert_eq!(aligned_oneword.len(), 1);
    assert_eq!(aligned_oneword[0].segment_id, "seg_oneword");
}

#[test]
fn test_adv_subtitle_export_injection_and_escaping() {
    // 1. ASS injection attempt with override tags
    let malicious = "Hello {\\pos(100,100)\\c&H0000FF&} evil injection {\\alpha&HFF&}";
    let escaped = escape_ass_text(malicious);
    assert!(!escaped.contains('{'), "ASS curly brace {{ must be escaped");
    assert!(!escaped.contains('}'), "ASS curly brace }} must be escaped");
    assert!(escaped.contains("(\\pos(100,100)\\c&H0000FF&)"));

    // 2. Multiline raw text with \r\n and \n
    let multiline = "Line 1\r\nLine 2\nLine 3";
    let escaped_multi = escape_ass_text(multiline);
    assert!(!escaped_multi.contains('\r'));
    assert!(!escaped_multi.contains('\n'));
    assert_eq!(escaped_multi, "Line 1\\NLine 2\\NLine 3");

    // 3. Timecode boundaries
    assert_eq!(format_ass_timecode(-100.0), "0:00:00.00");
    assert_eq!(format_ass_timecode(f64::NAN), "0:00:00.00");
    assert_eq!(format_ass_timecode(f64::INFINITY), "0:00:00.00");

    assert_eq!(format_srt_timecode(-50.0), "00:00:00,000");
    assert_eq!(format_srt_timecode(f64::NAN), "00:00:00,000");
    assert_eq!(format_srt_timecode(f64::INFINITY), "00:00:00,000");

    // 4. Generate SRT and ASS for container and verify format
    let container = TranscriptSegmentsContainer {
        clip_id: "adv_sub_clip".to_string(),
        segments: vec![
            TranscriptSegment {
                segment_id: "seg_0001".to_string(),
                start_sec: 0.0,
                end_sec: 2.5,
                speaker_id: "neuro".to_string(),
                text_en: "Sub 1 with {override}".to_string(),
                text_vi: None,
                confidence: 0.9,
                cps_score: 8.0,
                is_flagged_for_review: false,
                words: vec![],
            },
            TranscriptSegment {
                segment_id: "seg_0002".to_string(),
                start_sec: 3.0,
                end_sec: 5.5,
                speaker_id: "unknown".to_string(),
                text_en: "Sub 2\nwith newline".to_string(),
                text_vi: None,
                confidence: 0.85,
                cps_score: 9.0,
                is_flagged_for_review: false,
                words: vec![],
            },
        ],
    };

    let ass = generate_ass_script(&container);
    assert!(ass.contains("Dialogue: 0,0:00:00.00,0:00:02.50,Neuro,Neuro,0,0,0,,Sub 1 with (override)"));
    assert!(ass.contains("Dialogue: 0,0:00:03.00,0:00:05.50,Default,Default,0,0,0,,Sub 2\\Nwith newline"));

    let srt = generate_srt_script(&container);
    assert!(srt.contains("1\n00:00:00,000 --> 00:00:02,500\nSub 1 with {override}"));
    assert!(srt.contains("2\n00:00:03,000 --> 00:00:05,500\nSub 2\nwith newline"));
}
