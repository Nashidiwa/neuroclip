#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::cast_precision_loss,
    clippy::cast_lossless
)]

//! Integration Tests for Premiere Pro FCP7 XML (`<xmeml version="4">`) Exporter & `export:progress`
//! (`TASK-M4-26`, Milestone M4.7).
//!
//! Covers:
//! - UT-26.1: FCP7 XML `<xmeml version="4">` schema compliance (`1920x1080` / `1080x1920`, `60fps`,
//!   synchronized video/audio tracks, single `<file>` definitions, `<marker>` Vietnamese subtitles).
//! - IT-26.1: Multi-clip assembly, single-clip, subtitle-only fallback, RFC-3986 `file://localhost/...`
//!   media links, XML 1.0 character escaping, and `ERR_EXPORT_FILE_WRITE_FAILED` / `ERR_EXPORT_INVALID_LAYOUT`.
//! - IT-26.2: `export:progress` backend callback stream (`{ projectId, percent, currentStep }`).
//! - ED-26.1: Performance SLA benchmark (300+ segments XML generation + disk write <= 100ms).

use chrono::Utc;
use std::path::PathBuf;
use std::time::Instant;

use neuroclip_lib::commands::export_cmd::{
    execute_capcut_draft_export_with_progress, execute_premiere_xml_export_with_progress,
    CapCutExportOptions,
};
use neuroclip_lib::export::{
    export_premiere_xml_file, generate_premiere_xml, path_to_file_url, PremiereClipInput,
    PremiereXmlExportOptions,
};
use neuroclip_lib::stt::{
    create_subtitle_project, TranscriptSegment, TranscriptSegmentsContainer, WordTiming,
};

fn create_temp_test_dir(label: &str) -> PathBuf {
    let nanos = Utc::now().timestamp_nanos_opt().unwrap_or(0);
    let dir = std::env::temp_dir().join(format!("neuroclip_premiere_it_{label}_{nanos}"));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn create_sample_container() -> TranscriptSegmentsContainer {
    TranscriptSegmentsContainer {
        clip_id: "clip_m4_26_integration".to_string(),
        segments: vec![
            TranscriptSegment {
                segment_id: "seg_0001".to_string(),
                start_sec: 1.5,
                end_sec: 4.0,
                speaker_id: "neuro".to_string(),
                text_en: "Heart <3 Vedal & Chat!".to_string(),
                text_vi: Some("Trái tim <3 Vedal & Chat! \"Tuyệt vời\"".to_string()),
                confidence: 0.98,
                cps_score: 13.5,
                is_flagged_for_review: false,
                words: vec![WordTiming {
                    word: "Heart".to_string(),
                    start_sec: 1.5,
                    end_sec: 2.0,
                    probability: 0.99,
                }],
            },
            TranscriptSegment {
                segment_id: "seg_0002".to_string(),
                start_sec: 4.5,
                end_sec: 7.25,
                speaker_id: "vedal".to_string(),
                text_en: "That is actually pretty cool.".to_string(),
                text_vi: Some("Nghe cũng khá ngầu đấy chứ.".to_string()),
                confidence: 0.94,
                cps_score: 11.2,
                is_flagged_for_review: false,
                words: vec![],
            },
            TranscriptSegment {
                segment_id: "seg_0003".to_string(),
                start_sec: 8.0,
                end_sec: 10.0,
                speaker_id: "evil".to_string(),
                text_en: "Fallback English when text_vi is blank".to_string(),
                text_vi: Some("   ".to_string()),
                confidence: 0.91,
                cps_score: 12.0,
                is_flagged_for_review: false,
                words: vec![],
            },
        ],
    }
}

/// Lightweight XML tag stack validator ensuring balanced opening and closing tags
/// and no unescaped `<` or bare `&` inside XML text nodes.
fn assert_well_formed_xmeml(xml: &str) {
    assert!(xml.starts_with("<?xml version=\"1.0\" encoding=\"UTF-8\"?>"));
    assert!(
        !xml.starts_with('\u{FEFF}'),
        "XML must not contain UTF-8 BOM"
    );
    assert!(xml.contains("<!DOCTYPE xmeml>"));

    let mut stack: Vec<String> = Vec::new();
    let mut chars = xml.char_indices().peekable();
    while let Some((idx, ch)) = chars.next() {
        if ch == '<' {
            let rest = &xml[idx..];
            if rest.starts_with("<?") || rest.starts_with("<!") {
                for (_, c) in chars.by_ref() {
                    if c == '>' {
                        break;
                    }
                }
                continue;
            }
            let mut tag_buf = String::new();
            let mut closed = false;
            for (_, c) in chars.by_ref() {
                if c == '>' {
                    closed = true;
                    break;
                }
                tag_buf.push(c);
            }
            assert!(closed, "Unclosed XML tag at byte {idx}");
            let trimmed = tag_buf.trim();
            if let Some(closing_name) = trimmed.strip_prefix('/') {
                let expected = stack
                    .pop()
                    .unwrap_or_else(|| panic!("Unexpected closing tag </{closing_name}>"));
                assert_eq!(expected, closing_name.trim(), "Mismatched XML closing tag");
            } else if trimmed.ends_with('/') {
                // Self-closing tag such as <file id="file-1"/>
            } else {
                let tag_name = trimmed.split_whitespace().next().unwrap_or("");
                assert!(!tag_name.is_empty(), "Empty XML tag name at byte {idx}");
                stack.push(tag_name.to_string());
            }
        } else if ch == '&' {
            let rest = &xml[idx..];
            let valid_entity = rest.starts_with("&amp;")
                || rest.starts_with("&lt;")
                || rest.starts_with("&gt;")
                || rest.starts_with("&quot;")
                || rest.starts_with("&apos;");
            assert!(valid_entity, "Unescaped ampersand in XML near: {rest:.30}");
        }
    }

    assert!(stack.is_empty(), "Unclosed XML tags remaining: {stack:?}");
}

#[test]
fn test_ut_26_1_and_it_26_1_fcp7_xml_structure_and_media_links() {
    let temp_dir = create_temp_test_dir("ut26_1");
    let xml_path = temp_dir.join("nested").join("project_timeline.xml");

    let mut project = create_subtitle_project(
        create_sample_container(),
        "Neuro & Vedal <Clip #1>",
        "vod_m4_26_it",
        Some("preset_center_crop_9_16"),
    );
    project.video_file_path = Some(r"C:\NeuroClip\clips\Highlight #1 (Việt).mp4".to_string());

    let result = export_premiere_xml_file(&project, &xml_path, None).unwrap();
    assert!(xml_path.exists());
    assert_eq!(result.width, 1080);
    assert_eq!(result.height, 1920);
    assert_eq!(result.fps, 60);
    assert_eq!(result.marker_count, 3);
    assert_eq!(result.clip_count, 1);

    let xml_content = std::fs::read_to_string(&xml_path).unwrap();
    assert_well_formed_xmeml(&xml_content);

    assert!(xml_content.contains("<xmeml version=\"4\">"));
    assert!(xml_content.contains("<width>1080</width>"));
    assert!(xml_content.contains("<height>1920</height>"));
    assert!(xml_content.contains("<timebase>60</timebase>"));
    assert!(xml_content.contains("<ntsc>FALSE</ntsc>"));
    assert!(xml_content.contains(
        "<pathurl>file://localhost/C:/NeuroClip/clips/Highlight%20%231%20(Vi%E1%BB%87t).mp4</pathurl>"
    ));
    assert!(xml_content
        .contains("<comment>Trái tim &lt;3 Vedal &amp; Chat! &quot;Tuyệt vời&quot;</comment>"));
    assert!(xml_content.contains("<comment>Nghe cũng khá ngầu đấy chứ.</comment>"));
    assert!(xml_content.contains("<comment>Fallback English when text_vi is blank</comment>"));

    let _ = std::fs::remove_dir_all(temp_dir);
}

#[test]
fn test_it_26_1_multi_clip_and_error_contracts() {
    let project = create_subtitle_project(
        create_sample_container(),
        "Multi Clip Sequence",
        "vod_multi",
        Some("original_16_9"),
    );

    let options = PremiereXmlExportOptions {
        video_file_path: None,
        clips: Some(vec![
            PremiereClipInput {
                clip_id: Some("slice_1".to_string()),
                name: Some("Intro Slice".to_string()),
                file_path: r"D:\VODs\slice 1.mp4".to_string(),
                start_sec: Some(0.0),
                end_sec: Some(5.0),
                duration_sec: None,
            },
            PremiereClipInput {
                clip_id: Some("slice_2".to_string()),
                name: Some("Climax Slice".to_string()),
                file_path: r"D:\VODs\slice 2.mp4".to_string(),
                start_sec: Some(5.0),
                end_sec: Some(12.0),
                duration_sec: None,
            },
        ]),
        fps: None,
    };

    let xml = generate_premiere_xml(&project, Some(&options)).unwrap();
    assert_well_formed_xmeml(&xml);
    assert!(xml.contains("<width>1920</width>"));
    assert!(xml.contains("<height>1080</height>"));
    assert!(xml.contains(&format!(
        "<pathurl>{}</pathurl>",
        path_to_file_url(r"D:\VODs\slice 1.mp4")
    )));
    assert!(xml.contains(&format!(
        "<pathurl>{}</pathurl>",
        path_to_file_url(r"D:\VODs\slice 2.mp4")
    )));
    assert_eq!(xml.matches("<file id=\"file-1\">").count(), 1);
    assert_eq!(xml.matches("<file id=\"file-1\"/>").count(), 1);
    assert_eq!(xml.matches("<file id=\"file-2\">").count(), 1);
    assert_eq!(xml.matches("<file id=\"file-2\"/>").count(), 1);

    // Invalid layout -> ERR_EXPORT_INVALID_LAYOUT
    let mut bad_layout_proj = project.clone();
    bad_layout_proj.target_layout = "vertical_4k_unsupported".to_string();
    let err_layout = generate_premiere_xml(&bad_layout_proj, None).unwrap_err();
    assert_eq!(err_layout.code(), "ERR_EXPORT_INVALID_LAYOUT");

    // Empty / invalid output path -> ERR_EXPORT_FILE_WRITE_FAILED
    let err_write =
        export_premiere_xml_file(&project, PathBuf::from("").as_path(), None).unwrap_err();
    assert_eq!(err_write.code(), "ERR_EXPORT_FILE_WRITE_FAILED");
}

#[test]
fn test_it_26_2_export_progress_events_for_premiere_and_capcut() {
    let temp_dir = create_temp_test_dir("it26_2");
    let xml_path = temp_dir.join("timeline.xml");

    let mut project = create_subtitle_project(
        create_sample_container(),
        "Progress Stream Project",
        "vod_progress",
        Some("original_16_9"),
    );
    project.video_file_path = Some(r"C:\Videos\clip_slice.mp4".to_string());

    let mut premiere_events = Vec::new();
    let premiere_res = execute_premiere_xml_export_with_progress(&project, &xml_path, None, |ev| {
        premiere_events.push(ev);
    })
    .unwrap();

    assert!(xml_path.exists());
    assert_eq!(premiere_res.fps, 60);
    assert!(premiere_events.len() >= 3);
    assert_eq!(premiere_events.last().map(|e| e.percent), Some(100.0));
    for window in premiere_events.windows(2) {
        assert!(window[1].percent >= window[0].percent);
    }

    let mut capcut_events = Vec::new();
    let capcut_opts = CapCutExportOptions {
        video_file_path: project.video_file_path.clone(),
        custom_draft_root: Some(temp_dir.display().to_string()),
    };
    let capcut_res =
        execute_capcut_draft_export_with_progress(&project, Some(&capcut_opts), |ev| {
            capcut_events.push(ev);
        })
        .unwrap();

    assert!(PathBuf::from(&capcut_res.draft_content_path).exists());
    assert!(capcut_events.len() >= 3);
    assert_eq!(capcut_events.last().map(|e| e.percent), Some(100.0));

    let _ = std::fs::remove_dir_all(temp_dir);
}

#[test]
fn test_ed_26_1_300_segments_export_within_100ms_sla() {
    let temp_dir = create_temp_test_dir("ed26_1");
    let xml_path = temp_dir.join("perf_300.xml");

    let segments = (0..300)
        .map(|idx| TranscriptSegment {
            segment_id: format!("seg_{idx:04}"),
            start_sec: f64::from(idx) * 2.0,
            end_sec: f64::from(idx) * 2.0 + 1.6,
            speaker_id: if idx % 3 == 0 {
                "neuro".to_string()
            } else if idx % 3 == 1 {
                "vedal".to_string()
            } else {
                "evil".to_string()
            },
            text_en: format!("Highlight line {idx}"),
            text_vi: Some(format!("Dòng phụ đề {idx}: ể ễ ợ ứ đ & <tag>")),
            confidence: 0.96,
            cps_score: 12.0,
            is_flagged_for_review: false,
            words: vec![],
        })
        .collect();

    let container = TranscriptSegmentsContainer {
        clip_id: "clip_ed_26_1".to_string(),
        segments,
    };
    let mut project = create_subtitle_project(
        container,
        "ED26.1 Benchmark",
        "vod_bench",
        Some("preset_two_stack_9_16"),
    );
    project.video_file_path = Some(r"C:\Videos\bench_clip.mp4".to_string());

    let started = Instant::now();
    let res = export_premiere_xml_file(&project, &xml_path, None).unwrap();
    let elapsed = started.elapsed();

    assert!(
        elapsed.as_millis() <= 100,
        "ED-26.1 SLA exceeded: {}ms > 100ms",
        elapsed.as_millis()
    );
    assert_eq!(res.marker_count, 300);

    let _ = std::fs::remove_dir_all(temp_dir);
}

#[test]
fn test_regression_unc_dedup_bounds_and_early_layout_validation() {
    let temp_dir = create_temp_test_dir("reg_m4_26_it2");
    let xml_path = temp_dir.join("unc_and_dedup.xml");

    let mut container = create_sample_container();
    container.segments.push(TranscriptSegment {
        segment_id: "seg_inverted_ctrl".to_string(),
        start_sec: 15.0,
        end_sec: 5.0,
        speaker_id: "vedal".to_string(),
        text_en: "Fallback".to_string(),
        text_vi: Some(format!("{}VISIBLE_TEXT", "\u{0000}".repeat(60))),
        confidence: 0.9,
        cps_score: 10.0,
        is_flagged_for_review: false,
        words: vec![],
    });

    let project = create_subtitle_project(
        container,
        "Regression IT2",
        "vod_reg_it2",
        Some("original_16_9"),
    );

    let options = PremiereXmlExportOptions {
        video_file_path: None,
        clips: Some(vec![
            PremiereClipInput {
                clip_id: Some("unc_1".to_string()),
                name: Some("UNC Short".to_string()),
                file_path: r"\\?\UNC\nas-01\Neuro Share\clip #1.mp4".to_string(),
                start_sec: Some(0.0),
                end_sec: Some(4.0),
                duration_sec: None,
            },
            PremiereClipInput {
                clip_id: Some("unc_2".to_string()),
                name: Some("UNC Longer".to_string()),
                file_path: r"\\NAS-01\neuro share\clip #1.mp4".to_string(),
                start_sec: Some(4.0),
                end_sec: Some(12.0),
                duration_sec: None,
            },
        ]),
        fps: None,
    };

    let res = export_premiere_xml_file(&project, &xml_path, Some(&options)).unwrap();
    assert_eq!(res.clip_count, 2);
    let xml = std::fs::read_to_string(&xml_path).unwrap();
    assert_well_formed_xmeml(&xml);
    assert!(xml.contains("<pathurl>file://nas-01/Neuro%20Share/clip%20%231.mp4</pathurl>"));
    assert_eq!(xml.matches("<file id=\"file-1\">").count(), 1);
    assert_eq!(xml.matches("<file id=\"file-2\">").count(), 0);
    // Max clip duration = 8s * 60fps = 480; sequence duration = max(720, 901) = 901
    assert!(xml.contains("              <duration>480</duration>\n              <media>"));
    assert!(xml.contains("    <duration>901</duration>"));
    assert!(xml.contains("<name>[VEDAL] VISIBLE_TEXT</name>"));

    let mut bad_proj = project;
    bad_proj.target_layout = "bad_layout".to_string();
    let mut events = Vec::new();
    let err = execute_premiere_xml_export_with_progress(
        &bad_proj,
        &temp_dir.join("never.xml"),
        None,
        |ev| events.push(ev),
    )
    .unwrap_err();
    assert_eq!(err.code(), "ERR_EXPORT_INVALID_LAYOUT");
    assert_eq!(events.len(), 1);
    assert!((events[0].percent - 10.0).abs() < f64::EPSILON);

    let _ = std::fs::remove_dir_all(temp_dir);
}
