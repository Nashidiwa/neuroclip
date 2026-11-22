//! Integration tests for Vietnamese Smart Line Wrapping and CPS Time Borrowing (`TASK-M4-19`, `M4.5`).
//!
//! Verifies:
//! - Pure Rust line wrapping with compound word preservation (AC-19.1, UT-19.1).
//! - Non-colliding Time Borrowing algorithm (AC-19.2, UT-19.2, UT-19.3, ED-19.1).
//! - Schema 5.5 `transcript_segments.json` compatibility and roundtrip serialization (AC-19.3, IT-19.1).

use neuroclip_lib::stt::transcript_parser::{
    TranscriptSegment, TranscriptSegmentsContainer, CONFIDENCE_FLAG_THRESHOLD, CPS_FLAG_THRESHOLD,
};
use neuroclip_lib::translation::cps_optimizer::{
    calculate_segment_cps, optimize_container_cps, optimize_cps_time_borrowing,
    serialize_transcript_segments_json, TimeBorrowingConfig, CPS_RED_THRESHOLD,
};
use neuroclip_lib::translation::line_wrapper::{
    wrap_vietnamese_subtitle, wrap_vietnamese_text, DEFAULT_MAX_CHARS_PER_LINE,
    VERTICAL_9_16_MAX_CHARS_PER_LINE,
};

fn create_segment(
    id: &str,
    start_sec: f64,
    end_sec: f64,
    speaker: &str,
    text_en: &str,
    text_vi: Option<&str>,
    confidence: f64,
) -> TranscriptSegment {
    let effective = text_vi.unwrap_or(text_en);
    let cps = calculate_segment_cps(effective, start_sec, end_sec);
    TranscriptSegment {
        segment_id: id.to_string(),
        start_sec,
        end_sec,
        speaker_id: speaker.to_string(),
        text_en: text_en.to_string(),
        text_vi: text_vi.map(ToString::to_string),
        confidence,
        cps_score: cps,
        is_flagged_for_review: confidence < CONFIDENCE_FLAG_THRESHOLD || cps > CPS_FLAG_THRESHOLD,
        words: Vec::new(),
    }
}

#[test]
fn test_it_19_1_end_to_end_line_wrapper_and_cps_pipeline() {
    // 1. Create a dialogue with rapid speech containing compounds
    let text_50 = "Tôi đã mua một ổ bánh mì rất ngon cho bố Vedal ăn."; // 50 chars
    assert_eq!(text_50.chars().count(), 50);

    // Initial segment: 50 chars in 1.8s -> CPS = 50 / 1.8 ≈ 27.78 > 20
    let mut segment_1 = create_segment(
        "seg_001",
        1.0,
        2.8,
        "neuro",
        "I bought a very delicious banh mi for father Vedal to eat.",
        Some(text_50),
        0.95,
    );

    // Second segment: starts at 4.0s (silence gap = 4.0 - 2.8 = 1.2s > 850ms)
    // Duration: 2.5s -> 38 chars / 2.5s = 15.2 CPS <= 17.0 (comfortable green speed)
    let segment_2 = create_segment(
        "seg_002",
        4.0,
        6.5,
        "vedal",
        "Thank you Neuro, that is very sweet of you.",
        Some("Cảm ơn Neuro, con thật là ngoan ngoãn."),
        0.98,
    );

    // Step A: Wrap Vietnamese lines
    if let Some(ref vi) = segment_1.text_vi {
        let wrapped = wrap_vietnamese_subtitle(vi, DEFAULT_MAX_CHARS_PER_LINE, 2);
        segment_1.text_vi = Some(wrapped);
    }

    let wrapped_vi = segment_1.text_vi.as_deref().unwrap_or("");
    let lines: Vec<&str> = wrapped_vi.lines().collect();
    assert_eq!(lines.len(), 2, "Must wrap into 2 lines");
    for line in &lines {
        assert!(
            line.chars().count() <= DEFAULT_MAX_CHARS_PER_LINE,
            "Line exceeds max chars: '{line}' ({})",
            line.chars().count()
        );
    }
    assert!(!lines[0].ends_with("bánh"), "Must not split 'bánh mì'");
    assert!(!lines[0].ends_with("bố"), "Must not split 'bố Vedal'");

    // Step B: Build Container and run Time Borrowing
    let mut container = TranscriptSegmentsContainer {
        clip_id: "clip_m4_19_e2e".to_string(),
        segments: vec![segment_1, segment_2],
    };

    let report = optimize_container_cps(&mut container, &TimeBorrowingConfig::default());

    assert_eq!(report.total_segments, 2);
    assert_eq!(report.initial_high_cps_count, 1);
    assert_eq!(report.optimized_count, 1);
    assert!(
        (report.total_borrowed_sec - 0.8).abs() < 1e-4,
        "Should borrow max 800ms"
    );

    let seg1 = &container.segments[0];
    let seg2 = &container.segments[1];

    // end_sec moved from 2.8 to 3.6 (2.8 + 0.8)
    assert!((seg1.end_sec - 3.6).abs() < 1e-4);
    // Gap before seg2 is 4.0 - 3.6 = 0.4s = 400ms >= 50ms buffer
    assert!(seg2.start_sec - seg1.end_sec >= 0.050);

    // Recalculated CPS: 50 / (3.6 - 1.0) = 50 / 2.6 ≈ 19.23 <= 20.0 (Green/Yellow safe zone)
    assert!((seg1.cps_score - 19.2308).abs() < 0.05);
    assert!(seg1.cps_score <= CPS_RED_THRESHOLD);
    assert!(
        !seg1.is_flagged_for_review,
        "Flag must be cleared after successful time borrowing"
    );

    // Step C: Schema 5.5 JSON serialization roundtrip
    let json_res = serialize_transcript_segments_json(&container);
    assert!(json_res.is_ok(), "Schema 5.5 serialization must succeed");
    let json_str = json_res.unwrap_or_default();
    assert!(json_str.contains("\"clip_id\": \"clip_m4_19_e2e\""));
    assert!(json_str.contains("\"cps_score\":"));

    let roundtrip_res: Result<TranscriptSegmentsContainer, _> = serde_json::from_str(&json_str);
    assert!(
        roundtrip_res.is_ok(),
        "Schema 5.5 deserialization must succeed"
    );
    let roundtrip = roundtrip_res.unwrap_or(container);
    assert_eq!(roundtrip.clip_id, "clip_m4_19_e2e");
    assert_eq!(roundtrip.segments.len(), 2);
    assert_eq!(roundtrip.segments[0].segment_id, "seg_001");
    assert!((roundtrip.segments[0].end_sec - 3.6).abs() < 1e-4);
}

#[test]
fn test_vertical_9_16_smart_wrapping_preserves_compounds() {
    let text = "Cụ Rùa đang lập trình trí tuệ nhân tạo cùng bé Neuro.";
    let lines = wrap_vietnamese_text(text, VERTICAL_9_16_MAX_CHARS_PER_LINE, 2);

    for line in &lines {
        assert!(
            line.chars().count() <= VERTICAL_9_16_MAX_CHARS_PER_LINE,
            "Line '{line}' exceeds 26 chars"
        );
        assert!(!line.ends_with("Cụ"));
        assert!(!line.ends_with("trí"));
        assert!(!line.ends_with("trí tuệ"));
        assert!(!line.ends_with("bé"));
    }
}

#[test]
fn test_ed_19_1_tight_gap_prevents_overlap() {
    // Gap is only 70ms: 1.57 - 1.50 = 70ms. Buffer is 50ms.
    // Max allowable borrow is 70ms - 50ms = 20ms.
    let text_40 = "Một hai ba bốn năm sáu bảy tám chín mười";
    let mut segments = vec![
        create_segment("seg_a", 0.0, 1.50, "neuro", "text", Some(text_40), 0.95),
        create_segment("seg_b", 1.57, 3.00, "vedal", "text", Some("Trả lời"), 0.95),
    ];

    let report = optimize_cps_time_borrowing(&mut segments, 800, 50, 17.0);
    assert_eq!(report.optimized_count, 1);

    let seg_a = &segments[0];
    let seg_b = &segments[1];

    // end_sec should be 1.52 (1.50 + 0.02)
    assert!((seg_a.end_sec - 1.52).abs() < 1e-4);
    // Gap: 1.57 - 1.52 = 0.05s = 50ms buffer preserved
    assert!((seg_b.start_sec - seg_a.end_sec - 0.05).abs() < 1e-4);
    assert!(seg_a.end_sec <= seg_b.start_sec - 0.050);
}
