#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

//! Tests for Advanced SubStation Alpha (.ass) and SubRip (.srt) subtitle generators (TASK-M4-08).

use neuroclip_lib::stt::{
    create_subtitle_project, escape_ass_text, format_ass_timecode, format_srt_timecode,
    generate_ass_script, generate_srt_script, map_speaker_to_ass_style,
    TranscriptSegment, TranscriptSegmentsContainer, WordTiming,
};

fn create_sample_container() -> TranscriptSegmentsContainer {
    TranscriptSegmentsContainer {
        clip_id: "clip_sample_123".to_string(),
        segments: vec![
            TranscriptSegment {
                segment_id: "seg_0001".to_string(),
                start_sec: 1.25,
                end_sec: 3.80,
                speaker_id: "neuro".to_string(),
                text_en: "Hello chat! I love tutel {heart}.".to_string(),
                text_vi: Some("Chào chat! Mình thích tutel (trái tim).".to_string()),
                confidence: 0.96,
                cps_score: 13.5,
                is_flagged_for_review: false,
                words: vec![
                    WordTiming {
                        word: "Hello".to_string(),
                        start_sec: 1.25,
                        end_sec: 1.80,
                        probability: 0.98,
                    },
                    WordTiming {
                        word: "chat!".to_string(),
                        start_sec: 1.80,
                        end_sec: 2.30,
                        probability: 0.95,
                    },
                ],
            },
            TranscriptSegment {
                segment_id: "seg_0002".to_string(),
                start_sec: 4.10,
                end_sec: 6.50,
                speaker_id: "vedal".to_string(),
                text_en: "Vedal: Stop saying that.".to_string(),
                text_vi: None,
                confidence: 0.92,
                cps_score: 10.0,
                is_flagged_for_review: false,
                words: vec![],
            },
            TranscriptSegment {
                segment_id: "seg_0003".to_string(),
                start_sec: 7.00,
                end_sec: 9.20,
                speaker_id: "evil".to_string(),
                text_en: "Evil: I will destroy the world.".to_string(),
                text_vi: Some("Evil: Ta sẽ tiêu diệt thế giới.".to_string()),
                confidence: 0.88,
                cps_score: 14.1,
                is_flagged_for_review: false,
                words: vec![],
            },
        ],
    }
}

#[test]
fn test_ut_08_10_ass_script_generation_and_speaker_styles() {
    let container = create_sample_container();
    let ass = generate_ass_script(&container);

    // Verify Script Info headers
    assert!(ass.contains("[Script Info]"));
    assert!(ass.contains("ScriptType: v4.00+"));
    assert!(ass.contains("PlayResX: 1920"));
    assert!(ass.contains("PlayResY: 1080"));

    // Verify V4+ Styles table with VTuber personas
    assert!(ass.contains("[V4+ Styles]"));
    assert!(ass.contains("Style: Neuro,Arial,54,&H00B6B6FF")); // Pastel Pink
    assert!(ass.contains("Style: Vedal,Arial,54,&H0066CC66")); // Turtle Green
    assert!(ass.contains("Style: Evil,Arial,54,&H000000DC"));  // Crimson
    assert!(ass.contains("Style: Guest,Arial,54,&H00FFCC33")); // Sky Blue
    assert!(ass.contains("Style: Chat,Arial,48,&H00CCCCCC"));  // Light Grey
    assert!(ass.contains("Style: Default,Arial,52,&H00FFFFFF"));// White

    // Verify Events dialogue entries
    assert!(ass.contains("[Events]"));
    assert!(ass.contains("Dialogue: 0,0:00:01.25,0:00:03.80,Neuro,Neuro,0,0,0,,Chào chat! Mình thích tutel (trái tim)."));
    assert!(ass.contains("Dialogue: 0,0:00:04.10,0:00:06.50,Vedal,Vedal,0,0,0,,Vedal: Stop saying that."));
    assert!(ass.contains("Dialogue: 0,0:00:07.00,0:00:09.20,Evil,Evil,0,0,0,,Evil: Ta sẽ tiêu diệt thế giới."));
}

#[test]
fn test_ut_08_11_srt_script_generation_sequential_numbering() {
    let container = create_sample_container();
    let srt = generate_srt_script(&container);

    // Block 1
    assert!(srt.contains("1\n00:00:01,250 --> 00:00:03,800\nChào chat! Mình thích tutel (trái tim)."));

    // Block 2
    assert!(srt.contains("2\n00:00:04,100 --> 00:00:06,500\nVedal: Stop saying that."));

    // Block 3
    assert!(srt.contains("3\n00:00:07,000 --> 00:00:09,200\nEvil: Ta sẽ tiêu diệt thế giới."));
}

#[test]
fn test_ut_08_12_vietnamese_diacritics_and_special_character_escaping() {
    assert_eq!(escape_ass_text("Normal text"), "Normal text");
    assert_eq!(escape_ass_text("Text with {tag} here"), "Text with (tag) here");
    assert_eq!(escape_ass_text("Line 1\nLine 2"), "Line 1\\NLine 2");

    let vietnamese_sample = "Đồng hồ sinh học của Neuro-sama: tiếng Việt có dấu 100%";
    assert_eq!(escape_ass_text(vietnamese_sample), vietnamese_sample);
}

#[test]
fn test_timecode_formatting_functions() {
    // ASS: H:MM:SS.cc
    assert_eq!(format_ass_timecode(0.0), "0:00:00.00");
    assert_eq!(format_ass_timecode(1.25), "0:00:01.25");
    assert_eq!(format_ass_timecode(65.5), "0:01:05.50");
    assert_eq!(format_ass_timecode(3661.12), "1:01:01.12");

    // Negative or NaN clamps to 0:00:00.00
    assert_eq!(format_ass_timecode(-5.0), "0:00:00.00");
    assert_eq!(format_ass_timecode(f64::NAN), "0:00:00.00");

    // SRT: HH:MM:SS,mmm
    assert_eq!(format_srt_timecode(0.0), "00:00:00,000");
    assert_eq!(format_srt_timecode(1.25), "00:00:01,250");
    assert_eq!(format_srt_timecode(65.5), "00:01:05,500");
    assert_eq!(format_srt_timecode(3661.05), "01:01:01,050");
}

#[test]
fn test_speaker_style_mapping() {
    assert_eq!(map_speaker_to_ass_style("vedal"), "Vedal");
    assert_eq!(map_speaker_to_ass_style("neuro"), "Neuro");
    assert_eq!(map_speaker_to_ass_style("neuro-sama"), "Neuro");
    assert_eq!(map_speaker_to_ass_style("evil"), "Evil");
    assert_eq!(map_speaker_to_ass_style("guest"), "Guest");
    assert_eq!(map_speaker_to_ass_style("chat"), "Chat");
    assert_eq!(map_speaker_to_ass_style("random"), "Default");
}

#[test]
fn test_schema_5_6_subtitle_project_creation() {
    let container = create_sample_container();
    let project = create_subtitle_project(
        container,
        "Vedal and Neuro Banter",
        "vod_vedal_12345",
        Some("preset_center_crop_9_16"),
    );

    assert_eq!(project.project_id, "subproj_clip_sample_123");
    assert_eq!(project.project_name, "Vedal and Neuro Banter");
    assert_eq!(project.vod_id, "vod_vedal_12345");
    assert_eq!(project.target_layout, "preset_center_crop_9_16");
    assert_eq!(project.segments.len(), 3);
    assert!(project.speaker_styles.contains_key("neuro"));
    assert!(project.speaker_styles.contains_key("vedal"));
    assert!(project.speaker_styles.contains_key("evil"));
}
