//! Advanced SubStation Alpha (`.ass` v4.00+) subtitle exporter engine.
//!
//! Provides color-coded VTuber styling (Vedal, Neuro, Evil, Guest, Chat),
//! BGR hex conversion, tag escaping, and UTF-8 with BOM file serialization.

use std::collections::HashMap;
use std::fmt::Write;
use std::path::{Path, PathBuf};

use crate::error::AppError;
use crate::export::capcut_schema::canvas_config_for_layout;
use crate::export::{write_file_with_bom, SpeakerStyle, SubtitleProject};
use crate::stt::transcript_parser::{TranscriptSegment, TranscriptSegmentsContainer};
use crate::translation::cps_optimizer::effective_segment_text;

/// Convert a 6-character hex RGB color string (e.g. `"#98FB98"`) to Aegisub ASS BGR color
/// format with full opacity (`"&H0098FB98&"`).
///
/// Handles optional leading `#`, case insensitivity, and falls back safely to white
/// (`"&H00FFFFFF&"`) on invalid hex strings without panicking.
#[must_use]
pub fn hex_to_ass_bgr(hex: &str) -> String {
    let clean = hex.trim().trim_start_matches('#');
    if clean.len() != 6 {
        return "&H00FFFFFF&".to_string();
    }

    let r = &clean[0..2];
    let g = &clean[2..4];
    let b = &clean[4..6];

    if u8::from_str_radix(r, 16).is_ok()
        && u8::from_str_radix(g, 16).is_ok()
        && u8::from_str_radix(b, 16).is_ok()
    {
        format!(
            "&H00{}{}{}&",
            b.to_ascii_uppercase(),
            g.to_ascii_uppercase(),
            r.to_ascii_uppercase()
        )
    } else {
        "&H00FFFFFF&".to_string()
    }
}

/// Format time in seconds to ASS timecode `H:MM:SS.cc` (centiseconds, 2 digits).
///
/// Clamps NaN, infinite, or negative seconds safely to `"0:00:00.00"`.
#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
#[must_use]
pub fn format_ass_timecode(seconds: f64) -> String {
    let safe_sec = if seconds.is_finite() && seconds >= 0.0 {
        seconds
    } else {
        0.0
    };

    let total_centis = (safe_sec * 100.0).round() as u64;
    let centis = total_centis % 100;
    let total_secs = total_centis / 100;
    let s = total_secs % 60;
    let total_mins = total_secs / 60;
    let m = total_mins % 60;
    let h = total_mins / 60;

    format!("{h}:{m:02}:{s:02}.{centis:02}")
}

/// Escape text for ASS subtitles to prevent accidental tag injection.
///
/// Replaces `{` with `(`, `}` with `)`, strips `\r`, and converts `\n` to `\N`.
#[must_use]
pub fn escape_ass_text(text: &str) -> String {
    text.replace('{', "(")
        .replace('}', ")")
        .replace('\r', "")
        .replace('\n', "\\N")
}

/// Map speaker role string to canonical ASS style name.
#[must_use]
pub fn map_speaker_to_ass_style(speaker_id: &str) -> &'static str {
    match speaker_id.to_ascii_lowercase().as_str() {
        "vedal" => "Vedal",
        "neuro" | "neuro-sama" => "Neuro",
        "evil" | "evil neuro" => "Evil",
        "guest" => "Guest",
        "chat" => "Chat",
        _ => "Default",
    }
}

struct AssStyleDefaults {
    name: &'static str,
    lookup_keys: &'static [&'static str],
    font_name: &'static str,
    font_size: u32,
    primary_bgr: &'static str,
    outline_bgr: &'static str,
    bold: i8,
    outline: &'static str,
}

const STANDARD_ASS_STYLES: [AssStyleDefaults; 6] = [
    AssStyleDefaults {
        name: "Default",
        lookup_keys: &["default", "unknown"],
        font_name: "Arial",
        font_size: 52,
        primary_bgr: "&H00FFFFFF",
        outline_bgr: "&H00000000",
        bold: -1,
        outline: "3",
    },
    AssStyleDefaults {
        name: "Vedal",
        lookup_keys: &["vedal"],
        font_name: "Arial",
        font_size: 54,
        primary_bgr: "&H0098FB98",
        outline_bgr: "&H00000000",
        bold: -1,
        outline: "2.5",
    },
    AssStyleDefaults {
        name: "Neuro",
        lookup_keys: &["neuro"],
        font_name: "Arial",
        font_size: 54,
        primary_bgr: "&H00C1B6FF",
        outline_bgr: "&H00000000",
        bold: -1,
        outline: "3",
    },
    AssStyleDefaults {
        name: "Evil",
        lookup_keys: &["evil"],
        font_name: "Arial",
        font_size: 54,
        primary_bgr: "&H005500FF",
        outline_bgr: "&H00000000",
        bold: -1,
        outline: "3",
    },
    AssStyleDefaults {
        name: "Guest",
        lookup_keys: &["guest"],
        font_name: "Arial",
        font_size: 54,
        primary_bgr: "&H0000D7FF",
        outline_bgr: "&H00000000",
        bold: -1,
        outline: "3",
    },
    AssStyleDefaults {
        name: "Chat",
        lookup_keys: &["chat"],
        font_name: "Arial",
        font_size: 48,
        primary_bgr: "&H00EBCE87",
        outline_bgr: "&H00000000",
        bold: 0,
        outline: "2",
    },
];

fn append_ass_script_info(out: &mut String, title: &str, play_res_x: u32, play_res_y: u32) {
    out.push_str("[Script Info]\n");
    let _ = writeln!(out, "Title: {title}");
    out.push_str("ScriptType: v4.00+\n");
    out.push_str("WrapStyle: 0\n");
    out.push_str("ScaledBorderAndShadow: yes\n");
    out.push_str("YCbCr Matrix: TV.601\n");
    let _ = writeln!(out, "PlayResX: {play_res_x}");
    let _ = writeln!(out, "PlayResY: {play_res_y}\n");
}

fn find_speaker_style<'a, S: std::hash::BuildHasher>(
    speaker_styles: Option<&'a HashMap<String, SpeakerStyle, S>>,
    lookup_keys: &[&str],
) -> Option<&'a SpeakerStyle> {
    let styles = speaker_styles?;
    for key in lookup_keys {
        if let Some(style) = styles.get(*key) {
            return Some(style);
        }
    }
    None
}

fn append_v4_styles_section<S: std::hash::BuildHasher>(
    out: &mut String,
    speaker_styles: Option<&HashMap<String, SpeakerStyle, S>>,
) {
    out.push_str("[V4+ Styles]\n");
    out.push_str("Format: Name, Fontname, Fontsize, PrimaryColour, SecondaryColour, OutlineColour, BackColour, Bold, Italic, Underline, StrikeOut, ScaleX, ScaleY, Spacing, Angle, BorderStyle, Outline, Shadow, Alignment, MarginL, MarginR, MarginV, Encoding\n");

    for spec in &STANDARD_ASS_STYLES {
        if let Some(style) = find_speaker_style(speaker_styles, spec.lookup_keys) {
            let primary_bgr = hex_to_ass_bgr(&style.color_hex);
            let outline_bgr = hex_to_ass_bgr(&style.outline_color_hex);
            let primary_col = primary_bgr.trim_end_matches('&');
            let outline_col = outline_bgr.trim_end_matches('&');
            let _ = writeln!(
                out,
                "Style: {},{},{},{primary_col},&H000000FF,{outline_col},&H80000000,{},0,0,0,100,100,0,0,1,{},2,2,20,20,40,1",
                spec.name, style.font_name, style.font_size, spec.bold, spec.outline
            );
        } else {
            let _ = writeln!(
                out,
                "Style: {},{},{},{},&H000000FF,{},&H80000000,{},0,0,0,100,100,0,0,1,{},2,2,20,20,40,1",
                spec.name,
                spec.font_name,
                spec.font_size,
                spec.primary_bgr,
                spec.outline_bgr,
                spec.bold,
                spec.outline
            );
        }
    }

    out.push('\n');
}

fn append_events_section(out: &mut String, segments: &[TranscriptSegment]) {
    out.push_str("[Events]\n");
    out.push_str(
        "Format: Layer, Start, End, Style, Name, MarginL, MarginR, MarginV, Effect, Text\n",
    );

    for seg in segments {
        let start_tc = format_ass_timecode(seg.start_sec);
        let end_tc = format_ass_timecode(seg.end_sec);
        let style = map_speaker_to_ass_style(&seg.speaker_id);
        let raw_text = effective_segment_text(seg).trim();
        let escaped_text = escape_ass_text(raw_text);

        let _ = writeln!(
            out,
            "Dialogue: 0,{start_tc},{end_tc},{style},{style},0,0,0,,{escaped_text}"
        );
    }
}

/// Generate a complete Advanced SubStation Alpha (`.ass` v4.00+) subtitle script from a segment container.
///
/// Features VTuber character styles in `&HAABBGGRR` hexadecimal format and
/// proper text escaping.
#[must_use]
pub fn generate_ass_script(container: &TranscriptSegmentsContainer) -> String {
    let mut out = String::with_capacity(4096);

    append_ass_script_info(&mut out, "NeuroClip Subtitle Project", 1920, 1080);
    append_v4_styles_section(&mut out, None::<&HashMap<String, SpeakerStyle>>);
    append_events_section(&mut out, &container.segments);

    out
}

/// Generate a complete Advanced SubStation Alpha (`.ass` v4.00+) subtitle script from a [`SubtitleProject`].
///
/// Resolves `PlayResX`/`PlayResY` from `project.target_layout` and renders custom
/// `speaker_styles` configured on the project.
#[must_use]
pub fn generate_ass_script_from_project(project: &SubtitleProject) -> String {
    let mut out = String::with_capacity(4096);

    let (play_res_x, play_res_y) = canvas_config_for_layout(&project.target_layout)
        .map_or((1920, 1080), |cfg| (cfg.width, cfg.height));

    append_ass_script_info(&mut out, &project.project_name, play_res_x, play_res_y);
    append_v4_styles_section(&mut out, Some(&project.speaker_styles));
    append_events_section(&mut out, &project.segments);

    out
}

/// Export transcript segments into an Advanced SubStation Alpha (`.ass`) file with UTF-8 BOM.
///
/// # Errors
/// Returns [`AppError::Export`] if directory creation or file writing fails.
pub fn export_ass_file(
    container: &TranscriptSegmentsContainer,
    output_path: &Path,
) -> Result<PathBuf, AppError> {
    let ass_content = generate_ass_script(container);
    write_file_with_bom(output_path, &ass_content)?;
    Ok(output_path.to_path_buf())
}

/// Export a [`SubtitleProject`] into an Advanced SubStation Alpha (`.ass`) file with UTF-8 BOM.
///
/// # Errors
/// Returns [`AppError::Export`] if directory creation or file writing fails.
pub fn export_ass_project_file(
    project: &SubtitleProject,
    output_path: &Path,
) -> Result<PathBuf, AppError> {
    let ass_content = generate_ass_script_from_project(project);
    write_file_with_bom(output_path, &ass_content)?;
    Ok(output_path.to_path_buf())
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::cast_precision_loss,
    clippy::cast_lossless
)]
pub mod tests {
    use super::*;
    use crate::export::create_subtitle_project;
    use crate::stt::transcript_parser::{
        TranscriptSegment, TranscriptSegmentsContainer, WordTiming,
    };
    use chrono::Utc;
    use std::time::Instant;

    fn sample_container() -> TranscriptSegmentsContainer {
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
                    words: vec![WordTiming {
                        word: "Hello".to_string(),
                        start_sec: 1.25,
                        end_sec: 1.80,
                        probability: 0.98,
                    }],
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
    fn test_ut_24_1_ass_color_conversion() {
        // PO Option 1 Palette exact conversions
        assert_eq!(hex_to_ass_bgr("#98FB98"), "&H0098FB98&"); // Vedal Mint Green
        assert_eq!(hex_to_ass_bgr("#FFB6C1"), "&H00C1B6FF&"); // Neuro Pastel Pink
        assert_eq!(hex_to_ass_bgr("#FF0055"), "&H005500FF&"); // Evil Neon Crimson
        assert_eq!(hex_to_ass_bgr("#FFD700"), "&H0000D7FF&"); // Guest Gold
        assert_eq!(hex_to_ass_bgr("#87CEEB"), "&H00EBCE87&"); // Chat Sky Blue
        assert_eq!(hex_to_ass_bgr("#FFFFFF"), "&H00FFFFFF&"); // Default / Unknown White

        // Edge cases: without '#', lowercase, invalid length, invalid hex chars
        assert_eq!(hex_to_ass_bgr("98FB98"), "&H0098FB98&");
        assert_eq!(hex_to_ass_bgr("#ffb6c1"), "&H00C1B6FF&");
        assert_eq!(hex_to_ass_bgr("#ffd700"), "&H0000D7FF&");
        assert_eq!(hex_to_ass_bgr(""), "&H00FFFFFF&");
        assert_eq!(hex_to_ass_bgr("12345"), "&H00FFFFFF&");
        assert_eq!(hex_to_ass_bgr("XYZ123"), "&H00FFFFFF&");
    }

    #[test]
    fn test_ut_24_2_ass_header_v4_styles() {
        let container = sample_container();
        let ass = generate_ass_script(&container);

        // Verify Script Info headers
        assert!(ass.contains("[Script Info]"));
        assert!(ass.contains("ScriptType: v4.00+"));
        assert!(ass.contains("PlayResX: 1920"));
        assert!(ass.contains("PlayResY: 1080"));

        // Verify V4+ Styles table with VTuber personas (PO Option 1 Palette in &HAABBGGRR)
        assert!(ass.contains("[V4+ Styles]"));
        assert!(ass.contains("Style: Neuro,Arial,54,&H00C1B6FF")); // Pastel Pink (#FFB6C1)
        assert!(ass.contains("Style: Vedal,Arial,54,&H0098FB98")); // Mint Green (#98FB98)
        assert!(ass.contains("Style: Evil,Arial,54,&H005500FF")); // Neon Crimson (#FF0055)
        assert!(ass.contains("Style: Guest,Arial,54,&H0000D7FF")); // Gold (#FFD700)
        assert!(ass.contains("Style: Chat,Arial,48,&H00EBCE87")); // Sky Blue (#87CEEB)
        assert!(ass.contains("Style: Default,Arial,52,&H00FFFFFF")); // White

        // Verify Events dialogue entries
        assert!(ass.contains("[Events]"));
        assert!(ass.contains("Dialogue: 0,0:00:01.25,0:00:03.80,Neuro,Neuro,0,0,0,,Chào chat! Mình thích tutel (trái tim)."));
        assert!(ass.contains(
            "Dialogue: 0,0:00:04.10,0:00:06.50,Vedal,Vedal,0,0,0,,Vedal: Stop saying that."
        ));
        assert!(ass.contains(
            "Dialogue: 0,0:00:07.00,0:00:09.20,Evil,Evil,0,0,0,,Evil: Ta sẽ tiêu diệt thế giới."
        ));
    }

    #[test]
    fn test_ed_24_1_vietnamese_diacritics_bom() {
        let mut segments = Vec::with_capacity(300);
        let vietnamese_sentences = [
            "Đồng hồ sinh học: ể, ễ, ợ, ứ, đ",
            "Trí tuệ nhân tạo Neuro-sama và Vedal",
            "Chào buổi sáng mọi người và các bạn tutel",
            "Hôm nay chúng ta sẽ lập trình hệ thống phụ đề",
            "Thử thách phụ đề tiếng Việt với đầy đủ dấu câu",
        ];

        for i in 0..300 {
            let sentence = vietnamese_sentences[i % vietnamese_sentences.len()];
            segments.push(TranscriptSegment {
                segment_id: format!("seg_{i:04}"),
                start_sec: (i as f64) * 2.0,
                end_sec: (i as f64) * 2.0 + 1.8,
                speaker_id: if i % 2 == 0 {
                    "neuro".to_string()
                } else {
                    "vedal".to_string()
                },
                text_en: format!("Sentence number {i}"),
                text_vi: Some(format!("{sentence} #{i}")),
                confidence: 0.95,
                cps_score: 14.0,
                is_flagged_for_review: false,
                words: vec![],
            });
        }

        let container = TranscriptSegmentsContainer {
            clip_id: "clip_vn_300".to_string(),
            segments,
        };

        let temp_dir = std::env::temp_dir().join(format!(
            "neuroclip_ass_test_{}",
            Utc::now().timestamp_nanos_opt().unwrap_or(0)
        ));
        let output_path = temp_dir.join("subtitles_vn_test.ass");

        let exported_path = export_ass_file(&container, &output_path).unwrap();
        assert_eq!(exported_path, output_path);
        assert!(output_path.exists());

        let raw_bytes = std::fs::read(&output_path).unwrap();
        // Mandatory UTF-8 BOM check at byte offset 0 (0xEF, 0xBB, 0xBF)
        assert_eq!(&raw_bytes[0..3], &[0xEF, 0xBB, 0xBF]);

        let content_str = String::from_utf8(raw_bytes[3..].to_vec()).unwrap();
        assert!(content_str.contains("Đồng hồ sinh học: ể, ễ, ợ, ứ, đ"));
        assert!(content_str.contains("Dialogue: 0,"));

        let _ = std::fs::remove_dir_all(temp_dir);
    }

    #[test]
    fn test_ed_24_2_ass_export_performance_300_segments() {
        let mut segments = Vec::with_capacity(300);
        for i in 0..300 {
            segments.push(TranscriptSegment {
                segment_id: format!("seg_{i:04}"),
                start_sec: (i as f64) * 1.5,
                end_sec: (i as f64) * 1.5 + 1.2,
                speaker_id: "neuro".to_string(),
                text_en: format!("Performance test sentence {i}"),
                text_vi: Some(format!("Câu kiểm thử hiệu năng số {i}")),
                confidence: 0.99,
                cps_score: 12.0,
                is_flagged_for_review: false,
                words: vec![],
            });
        }

        let container = TranscriptSegmentsContainer {
            clip_id: "clip_perf_300".to_string(),
            segments,
        };

        let start = Instant::now();
        let ass_content = generate_ass_script(&container);
        let elapsed = start.elapsed();

        assert_ne!(ass_content, "");
        // Must strictly finish generation within <= 200ms
        assert!(
            elapsed.as_millis() <= 200,
            "ASS generation for 300 segments took {}ms, expected <= 200ms",
            elapsed.as_millis()
        );
    }

    #[test]
    fn test_ass_escaping_and_timecode_clamping() {
        assert_eq!(escape_ass_text("Normal text"), "Normal text");
        assert_eq!(
            escape_ass_text("Text with {tag} here"),
            "Text with (tag) here"
        );
        assert_eq!(escape_ass_text("Line 1\r\nLine 2"), "Line 1\\NLine 2");

        assert_eq!(format_ass_timecode(0.0), "0:00:00.00");
        assert_eq!(format_ass_timecode(1.25), "0:00:01.25");
        assert_eq!(format_ass_timecode(65.5), "0:01:05.50");
        assert_eq!(format_ass_timecode(3661.12), "1:01:01.12");

        // Negative or NaN clamps to 0:00:00.00
        assert_eq!(format_ass_timecode(-5.0), "0:00:00.00");
        assert_eq!(format_ass_timecode(f64::NAN), "0:00:00.00");
        assert_eq!(format_ass_timecode(f64::INFINITY), "0:00:00.00");

        assert_eq!(map_speaker_to_ass_style("vedal"), "Vedal");
        assert_eq!(map_speaker_to_ass_style("neuro"), "Neuro");
        assert_eq!(map_speaker_to_ass_style("neuro-sama"), "Neuro");
        assert_eq!(map_speaker_to_ass_style("evil"), "Evil");
        assert_eq!(map_speaker_to_ass_style("guest"), "Guest");
        assert_eq!(map_speaker_to_ass_style("chat"), "Chat");
        assert_eq!(map_speaker_to_ass_style("other"), "Default");
    }

    #[test]
    fn test_generate_ass_script_from_project() {
        let container = sample_container();
        let project = create_subtitle_project(container, "Project Test", "vod_1", None);
        let ass = generate_ass_script_from_project(&project);

        assert!(ass.contains("Title: Project Test"));
        assert!(ass.contains("[V4+ Styles]"));
        assert!(ass.contains("Style: Neuro,"));
        assert!(ass.contains("Style: Vedal,"));
        assert!(ass.contains("Dialogue: 0,"));
    }

    #[test]
    fn test_patch_p1_02_vedal_and_chat_exact_styles_in_both_generators() {
        let container = sample_container();
        let ass_container = generate_ass_script(&container);

        let project = create_subtitle_project(
            container,
            "NeuroClip Subtitle Project",
            "vod_1",
            Some("original_16_9"),
        );
        let ass_project = generate_ass_script_from_project(&project);

        let expected_lines = [
            "Style: Default,Arial,52,&H00FFFFFF,&H000000FF,&H00000000,&H80000000,-1,0,0,0,100,100,0,0,1,3,2,2,20,20,40,1",
            "Style: Vedal,Arial,54,&H0098FB98,&H000000FF,&H00000000,&H80000000,-1,0,0,0,100,100,0,0,1,2.5,2,2,20,20,40,1",
            "Style: Neuro,Arial,54,&H00C1B6FF,&H000000FF,&H00000000,&H80000000,-1,0,0,0,100,100,0,0,1,3,2,2,20,20,40,1",
            "Style: Evil,Arial,54,&H005500FF,&H000000FF,&H00000000,&H80000000,-1,0,0,0,100,100,0,0,1,3,2,2,20,20,40,1",
            "Style: Guest,Arial,54,&H0000D7FF,&H000000FF,&H00000000,&H80000000,-1,0,0,0,100,100,0,0,1,3,2,2,20,20,40,1",
            "Style: Chat,Arial,48,&H00EBCE87,&H000000FF,&H00000000,&H80000000,0,0,0,0,100,100,0,0,1,2,2,2,20,20,40,1",
        ];

        for line in expected_lines {
            assert!(
                ass_container.contains(line),
                "generate_ass_script missing: {line}"
            );
            assert!(
                ass_project.contains(line),
                "generate_ass_script_from_project missing: {line}"
            );
        }

        // With identical title and original_16_9 default styles, both generators match byte-for-byte
        assert_eq!(ass_container, ass_project);
    }

    #[test]
    fn test_patch_p1_02_project_playres_from_target_layout() {
        let cases = [
            (
                Some("preset_center_crop_9_16"),
                "PlayResX: 1080",
                "PlayResY: 1920",
            ),
            (
                Some("preset_two_stack_9_16"),
                "PlayResX: 1080",
                "PlayResY: 1920",
            ),
            (Some("original_16_9"), "PlayResX: 1920", "PlayResY: 1080"),
            (None, "PlayResX: 1920", "PlayResY: 1080"),
            (
                Some("unknown_or_empty_layout"),
                "PlayResX: 1920",
                "PlayResY: 1080",
            ),
        ];

        for (layout, expected_x, expected_y) in cases {
            let project =
                create_subtitle_project(sample_container(), "Layout Test", "vod_layout", layout);
            let ass = generate_ass_script_from_project(&project);
            assert!(
                ass.contains(expected_x),
                "Layout {layout:?} expected {expected_x}\n{ass}"
            );
            assert!(
                ass.contains(expected_y),
                "Layout {layout:?} expected {expected_y}\n{ass}"
            );
        }
    }

    #[test]
    fn test_patch_p1_02_project_missing_and_custom_speaker_styles() {
        let mut project = create_subtitle_project(
            sample_container(),
            "Custom Styles",
            "vod_custom",
            Some("preset_center_crop_9_16"),
        );

        // Empty speaker_styles falls back to canonical VTuber defaults (including Vedal 2.5 & Chat 0/2)
        project.speaker_styles.clear();
        let fallback_ass = generate_ass_script_from_project(&project);
        assert!(fallback_ass.contains(
            "Style: Vedal,Arial,54,&H0098FB98,&H000000FF,&H00000000,&H80000000,-1,0,0,0,100,100,0,0,1,2.5,2,2,20,20,40,1"
        ));
        assert!(fallback_ass.contains(
            "Style: Chat,Arial,48,&H00EBCE87,&H000000FF,&H00000000,&H80000000,0,0,0,0,100,100,0,0,1,2,2,2,20,20,40,1"
        ));

        // Custom speaker_styles (including "unknown" fallback for Default) preserve font/size/colors and role Bold/Outline
        project.speaker_styles.insert(
            "vedal".to_string(),
            SpeakerStyle {
                color_hex: "#112233".to_string(),
                font_name: "Inter".to_string(),
                font_size: 60,
                outline_color_hex: "#445566".to_string(),
            },
        );
        project.speaker_styles.insert(
            "chat".to_string(),
            SpeakerStyle {
                color_hex: "#AABBCC".to_string(),
                font_name: "Roboto".to_string(),
                font_size: 44,
                outline_color_hex: "#101010".to_string(),
            },
        );
        project.speaker_styles.insert(
            "unknown".to_string(),
            SpeakerStyle {
                color_hex: "#EEEEEE".to_string(),
                font_name: "Consolas".to_string(),
                font_size: 50,
                outline_color_hex: "#010101".to_string(),
            },
        );

        let custom_ass = generate_ass_script_from_project(&project);
        assert!(custom_ass.contains(
            "Style: Vedal,Inter,60,&H00332211,&H000000FF,&H00665544,&H80000000,-1,0,0,0,100,100,0,0,1,2.5,2,2,20,20,40,1"
        ));
        assert!(custom_ass.contains(
            "Style: Chat,Roboto,44,&H00CCBBAA,&H000000FF,&H00101010,&H80000000,0,0,0,0,100,100,0,0,1,2,2,2,20,20,40,1"
        ));
        assert!(custom_ass.contains(
            "Style: Default,Consolas,50,&H00EEEEEE,&H000000FF,&H00010101,&H80000000,-1,0,0,0,100,100,0,0,1,3,2,2,20,20,40,1"
        ));
    }

    #[test]
    fn test_empty_and_whitespace_text_vi_falls_back_to_text_en() {
        let container = TranscriptSegmentsContainer {
            clip_id: "clip_empty_vi".to_string(),
            segments: vec![
                TranscriptSegment {
                    segment_id: "seg_empty".to_string(),
                    start_sec: 0.0,
                    end_sec: 1.5,
                    speaker_id: "neuro".to_string(),
                    text_en: "English fallback one".to_string(),
                    text_vi: Some(String::new()),
                    confidence: 0.95,
                    cps_score: 12.0,
                    is_flagged_for_review: false,
                    words: vec![],
                },
                TranscriptSegment {
                    segment_id: "seg_ws".to_string(),
                    start_sec: 2.0,
                    end_sec: 3.5,
                    speaker_id: "vedal".to_string(),
                    text_en: "English fallback two".to_string(),
                    text_vi: Some("   \t\n ".to_string()),
                    confidence: 0.95,
                    cps_score: 12.0,
                    is_flagged_for_review: false,
                    words: vec![],
                },
            ],
        };

        let ass = generate_ass_script(&container);
        assert!(ass
            .contains("Dialogue: 0,0:00:00.00,0:00:01.50,Neuro,Neuro,0,0,0,,English fallback one"));
        assert!(ass
            .contains("Dialogue: 0,0:00:02.00,0:00:03.50,Vedal,Vedal,0,0,0,,English fallback two"));
    }

    #[test]
    fn test_ass_trims_leading_and_trailing_whitespace() {
        let container = TranscriptSegmentsContainer {
            clip_id: "clip_trim_ass".to_string(),
            segments: vec![TranscriptSegment {
                segment_id: "seg_trim".to_string(),
                start_sec: 0.0,
                end_sec: 1.5,
                speaker_id: "neuro".to_string(),
                text_en: "  Trimmed English  ".to_string(),
                text_vi: None,
                confidence: 0.95,
                cps_score: 12.0,
                is_flagged_for_review: false,
                words: vec![],
            }],
        };

        let ass = generate_ass_script(&container);
        assert!(
            ass.contains("Dialogue: 0,0:00:00.00,0:00:01.50,Neuro,Neuro,0,0,0,,Trimmed English")
        );
    }
}
