//! Advanced SubStation Alpha (`.ass` v4.00+) subtitle exporter engine.
//!
//! Provides color-coded VTuber styling (Vedal, Neuro, Evil, Guest, Chat),
//! BGR hex conversion, tag escaping, and UTF-8 with BOM file serialization.

use std::fmt::Write;
use std::path::{Path, PathBuf};

use crate::error::AppError;
use crate::export::{write_file_with_bom, SubtitleProject};
use crate::stt::transcript_parser::TranscriptSegmentsContainer;

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

/// Generate a complete Advanced SubStation Alpha (`.ass` v4.00+) subtitle script from a segment container.
///
/// Features VTuber character styles in `&HAABBGGRR` hexadecimal format and
/// proper text escaping.
#[must_use]
pub fn generate_ass_script(container: &TranscriptSegmentsContainer) -> String {
    let mut out = String::with_capacity(4096);

    out.push_str("[Script Info]\n");
    out.push_str("Title: NeuroClip Subtitle Project\n");
    out.push_str("ScriptType: v4.00+\n");
    out.push_str("WrapStyle: 0\n");
    out.push_str("ScaledBorderAndShadow: yes\n");
    out.push_str("YCbCr Matrix: TV.601\n");
    out.push_str("PlayResX: 1920\n");
    out.push_str("PlayResY: 1080\n\n");

    out.push_str("[V4+ Styles]\n");
    out.push_str("Format: Name, Fontname, Fontsize, PrimaryColour, SecondaryColour, OutlineColour, BackColour, Bold, Italic, Underline, StrikeOut, ScaleX, ScaleY, Spacing, Angle, BorderStyle, Outline, Shadow, Alignment, MarginL, MarginR, MarginV, Encoding\n");
    out.push_str("Style: Default,Arial,52,&H00FFFFFF,&H000000FF,&H00000000,&H80000000,-1,0,0,0,100,100,0,0,1,3,2,2,20,20,40,1\n");
    out.push_str("Style: Vedal,Arial,54,&H0098FB98,&H000000FF,&H00000000,&H80000000,-1,0,0,0,100,100,0,0,1,3,2,2,20,20,40,1\n");
    out.push_str("Style: Neuro,Arial,54,&H00C1B6FF,&H000000FF,&H00000000,&H80000000,-1,0,0,0,100,100,0,0,1,3,2,2,20,20,40,1\n");
    out.push_str("Style: Evil,Arial,54,&H005500FF,&H000000FF,&H00000000,&H80000000,-1,0,0,0,100,100,0,0,1,3,2,2,20,20,40,1\n");
    out.push_str("Style: Guest,Arial,54,&H0000D7FF,&H000000FF,&H00000000,&H80000000,-1,0,0,0,100,100,0,0,1,3,2,2,20,20,40,1\n");
    out.push_str("Style: Chat,Arial,48,&H00EBCE87,&H000000FF,&H00000000,&H80000000,0,0,0,0,100,100,0,0,1,2,2,2,20,20,40,1\n\n");

    out.push_str("[Events]\n");
    out.push_str(
        "Format: Layer, Start, End, Style, Name, MarginL, MarginR, MarginV, Effect, Text\n",
    );

    for seg in &container.segments {
        let start_tc = format_ass_timecode(seg.start_sec);
        let end_tc = format_ass_timecode(seg.end_sec);
        let style = map_speaker_to_ass_style(&seg.speaker_id);
        let raw_text = seg.text_vi.as_deref().unwrap_or(&seg.text_en);
        let escaped_text = escape_ass_text(raw_text);

        let _ = writeln!(
            out,
            "Dialogue: 0,{start_tc},{end_tc},{style},{style},0,0,0,,{escaped_text}"
        );
    }

    out
}

/// Generate a complete Advanced SubStation Alpha (`.ass` v4.00+) subtitle script from a [`SubtitleProject`].
///
/// Renders custom `speaker_styles` configured on the project.
#[must_use]
pub fn generate_ass_script_from_project(project: &SubtitleProject) -> String {
    let mut out = String::with_capacity(4096);

    out.push_str("[Script Info]\n");
    let _ = writeln!(out, "Title: {}", project.project_name);
    out.push_str("ScriptType: v4.00+\n");
    out.push_str("WrapStyle: 0\n");
    out.push_str("ScaledBorderAndShadow: yes\n");
    out.push_str("YCbCr Matrix: TV.601\n");
    out.push_str("PlayResX: 1920\n");
    out.push_str("PlayResY: 1080\n\n");

    out.push_str("[V4+ Styles]\n");
    out.push_str("Format: Name, Fontname, Fontsize, PrimaryColour, SecondaryColour, OutlineColour, BackColour, Bold, Italic, Underline, StrikeOut, ScaleX, ScaleY, Spacing, Angle, BorderStyle, Outline, Shadow, Alignment, MarginL, MarginR, MarginV, Encoding\n");

    // Standard ordering for consistency
    let standard_keys = ["Default", "Vedal", "Neuro", "Evil", "Guest", "Chat"];
    for style_name in standard_keys {
        let lookup_key = style_name.to_ascii_lowercase();
        if let Some(style) = project.speaker_styles.get(&lookup_key) {
            let primary_bgr = hex_to_ass_bgr(&style.color_hex);
            let outline_bgr = hex_to_ass_bgr(&style.outline_color_hex);
            let _ = writeln!(
                out,
                "Style: {style_name},{},{},{primary_bgr},&H000000FF,{outline_bgr},&H80000000,-1,0,0,0,100,100,0,0,1,3,2,2,20,20,40,1",
                style.font_name, style.font_size
            );
        } else {
            // Fallback for missing style
            let _ = writeln!(
                out,
                "Style: {style_name},Arial,52,&H00FFFFFF,&H000000FF,&H00000000,&H80000000,-1,0,0,0,100,100,0,0,1,3,2,2,20,20,40,1"
            );
        }
    }

    out.push_str("\n[Events]\n");
    out.push_str(
        "Format: Layer, Start, End, Style, Name, MarginL, MarginR, MarginV, Effect, Text\n",
    );

    for seg in &project.segments {
        let start_tc = format_ass_timecode(seg.start_sec);
        let end_tc = format_ass_timecode(seg.end_sec);
        let style = map_speaker_to_ass_style(&seg.speaker_id);
        let raw_text = seg.text_vi.as_deref().unwrap_or(&seg.text_en);
        let escaped_text = escape_ass_text(raw_text);

        let _ = writeln!(
            out,
            "Dialogue: 0,{start_tc},{end_tc},{style},{style},0,0,0,,{escaped_text}"
        );
    }

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
    use crate::stt::transcript_parser::{TranscriptSegment, TranscriptSegmentsContainer, WordTiming};
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
                    words: vec![
                        WordTiming {
                            word: "Hello".to_string(),
                            start_sec: 1.25,
                            end_sec: 1.80,
                            probability: 0.98,
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
        assert!(ass.contains("Dialogue: 0,0:00:04.10,0:00:06.50,Vedal,Vedal,0,0,0,,Vedal: Stop saying that."));
        assert!(ass.contains("Dialogue: 0,0:00:07.00,0:00:09.20,Evil,Evil,0,0,0,,Evil: Ta sẽ tiêu diệt thế giới."));
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
                speaker_id: if i % 2 == 0 { "neuro".to_string() } else { "vedal".to_string() },
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

        let temp_dir = std::env::temp_dir().join(format!("neuroclip_ass_test_{}", Utc::now().timestamp_nanos_opt().unwrap_or(0)));
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
        assert_eq!(escape_ass_text("Text with {tag} here"), "Text with (tag) here");
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
}
