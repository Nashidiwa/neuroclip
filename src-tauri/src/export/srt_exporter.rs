//! SubRip (`.srt`) international subtitle exporter engine.
//!
//! Provides sequential 1-based indexing, millisecond-precision comma-separated timecodes,
//! and UTF-8 with BOM file serialization for international compatibility.

use std::fmt::Write;
use std::path::{Path, PathBuf};

use crate::error::AppError;
use crate::export::{write_file_with_bom, SubtitleProject};
use crate::stt::transcript_parser::{TranscriptSegment, TranscriptSegmentsContainer};
use crate::translation::cps_optimizer::effective_segment_text;

/// Format time in seconds to standard SRT timecode `HH:MM:SS,mmm` (milliseconds, 3 digits).
///
/// Clamps NaN, infinite, or negative seconds safely to `"00:00:00,000"`.
#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
#[must_use]
pub fn format_srt_timecode(seconds: f64) -> String {
    let safe_sec = if seconds.is_finite() && seconds >= 0.0 {
        seconds
    } else {
        0.0
    };

    let total_millis = (safe_sec * 1000.0).round() as u64;
    let millis = total_millis % 1000;
    let total_secs = total_millis / 1000;
    let s = total_secs % 60;
    let total_mins = total_secs / 60;
    let m = total_mins % 60;
    let h = total_mins / 60;

    format!("{h:02}:{m:02}:{s:02},{millis:03}")
}

/// Generate a standard SubRip (`.srt`) subtitle document from a segment slice.
///
/// Outputs sequential 1-based index numbers and comma-separated millisecond timestamps.
#[must_use]
pub fn generate_srt_script_from_segments(segments: &[TranscriptSegment]) -> String {
    let mut out = String::with_capacity(4096);

    for (idx, seg) in segments.iter().enumerate() {
        let seq = idx + 1;
        let start_tc = format_srt_timecode(seg.start_sec);
        let end_tc = format_srt_timecode(seg.end_sec);
        let raw_text = effective_segment_text(seg);
        let clean_text = raw_text.trim();

        let _ = writeln!(out, "{seq}");
        let _ = writeln!(out, "{start_tc} --> {end_tc}");
        let _ = writeln!(out, "{clean_text}\n");
    }

    out
}

/// Generate a standard SubRip (`.srt`) subtitle document from a segment container.
#[must_use]
pub fn generate_srt_script(container: &TranscriptSegmentsContainer) -> String {
    generate_srt_script_from_segments(&container.segments)
}

/// Generate a standard SubRip (`.srt`) subtitle document from a [`SubtitleProject`].
#[must_use]
pub fn generate_srt_script_from_project(project: &SubtitleProject) -> String {
    generate_srt_script_from_segments(&project.segments)
}

/// Export transcript segments into a standard SubRip (`.srt`) file with UTF-8 BOM.
///
/// # Errors
/// Returns [`AppError::Export`] if directory creation or file writing fails.
pub fn export_srt_file(
    container: &TranscriptSegmentsContainer,
    output_path: &Path,
) -> Result<PathBuf, AppError> {
    let srt_content = generate_srt_script(container);
    write_file_with_bom(output_path, &srt_content)?;
    Ok(output_path.to_path_buf())
}

/// Export a [`SubtitleProject`] into a standard SubRip (`.srt`) file with UTF-8 BOM.
///
/// # Errors
/// Returns [`AppError::Export`] if directory creation or file writing fails.
pub fn export_srt_project_file(
    project: &SubtitleProject,
    output_path: &Path,
) -> Result<PathBuf, AppError> {
    let srt_content = generate_srt_script_from_project(project);
    write_file_with_bom(output_path, &srt_content)?;
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
    use crate::stt::transcript_parser::{
        TranscriptSegment, TranscriptSegmentsContainer, WordTiming,
    };
    use chrono::Utc;
    use std::time::Instant;

    fn sample_container() -> TranscriptSegmentsContainer {
        TranscriptSegmentsContainer {
            clip_id: "clip_sample_srt".to_string(),
            segments: vec![
                TranscriptSegment {
                    segment_id: "seg_0001".to_string(),
                    start_sec: 83.45,
                    end_sec: 85.80,
                    speaker_id: "neuro".to_string(),
                    text_en: "Testing SRT timecodes.".to_string(),
                    text_vi: Some("Kiểm tra timecode SRT.".to_string()),
                    confidence: 0.97,
                    cps_score: 11.0,
                    is_flagged_for_review: false,
                    words: vec![WordTiming {
                        word: "Testing".to_string(),
                        start_sec: 83.45,
                        end_sec: 84.50,
                        probability: 0.99,
                    }],
                },
                TranscriptSegment {
                    segment_id: "seg_0002".to_string(),
                    start_sec: 86.10,
                    end_sec: 89.25,
                    speaker_id: "vedal".to_string(),
                    text_en: "Second block Vedal response.".to_string(),
                    text_vi: None,
                    confidence: 0.93,
                    cps_score: 10.5,
                    is_flagged_for_review: false,
                    words: vec![],
                },
            ],
        }
    }

    #[test]
    fn test_ut_24_3_srt_timecode_format() {
        // UT-24.3: Standard comma-separated millisecond timecode
        let start_tc = format_srt_timecode(83.45);
        let end_tc = format_srt_timecode(85.80);
        let combined = format!("{start_tc} --> {end_tc}");
        assert_eq!(combined, "00:01:23,450 --> 00:01:25,800");

        assert_eq!(format_srt_timecode(0.0), "00:00:00,000");
        assert_eq!(format_srt_timecode(1.25), "00:00:01,250");
        assert_eq!(format_srt_timecode(65.5), "00:01:05,500");
        assert_eq!(format_srt_timecode(3661.05), "01:01:01,050");

        // Clamping invalid seconds
        assert_eq!(format_srt_timecode(-5.0), "00:00:00,000");
        assert_eq!(format_srt_timecode(f64::NAN), "00:00:00,000");
        assert_eq!(format_srt_timecode(f64::INFINITY), "00:00:00,000");
    }

    #[test]
    fn test_srt_sequential_numbering() {
        let container = sample_container();
        let srt = generate_srt_script(&container);

        // Block 1 sequential
        assert!(srt.contains("1\n00:01:23,450 --> 00:01:25,800\nKiểm tra timecode SRT."));
        // Block 2 sequential
        assert!(srt.contains("2\n00:01:26,100 --> 00:01:29,250\nSecond block Vedal response."));
    }

    #[test]
    fn test_it_24_2_srt_bom_disk_export() {
        let container = sample_container();
        let temp_dir = std::env::temp_dir().join(format!(
            "neuroclip_srt_test_{}",
            Utc::now().timestamp_nanos_opt().unwrap_or(0)
        ));
        let output_path = temp_dir.join("subtitles_test.srt");

        let exported_path = export_srt_file(&container, &output_path).unwrap();
        assert_eq!(exported_path, output_path);
        assert!(output_path.exists());

        let raw_bytes = std::fs::read(&output_path).unwrap();
        // Mandatory UTF-8 BOM check at byte offset 0 (0xEF, 0xBB, 0xBF)
        assert_eq!(&raw_bytes[0..3], &[0xEF, 0xBB, 0xBF]);

        let content_str = String::from_utf8(raw_bytes[3..].to_vec()).unwrap();
        assert!(content_str.contains("1\n00:01:23,450 --> 00:01:25,800\nKiểm tra timecode SRT."));

        let _ = std::fs::remove_dir_all(temp_dir);
    }

    #[test]
    fn test_ed_24_2_srt_export_performance_300_segments() {
        let mut segments = Vec::with_capacity(300);
        for i in 0..300 {
            segments.push(TranscriptSegment {
                segment_id: format!("seg_{i:04}"),
                start_sec: (i as f64) * 1.5,
                end_sec: (i as f64) * 1.5 + 1.2,
                speaker_id: "vedal".to_string(),
                text_en: format!("SRT performance sentence {i}"),
                text_vi: Some(format!("Câu phụ đề SRT số {i}")),
                confidence: 0.98,
                cps_score: 13.0,
                is_flagged_for_review: false,
                words: vec![],
            });
        }

        let container = TranscriptSegmentsContainer {
            clip_id: "clip_srt_perf_300".to_string(),
            segments,
        };

        let start = Instant::now();
        let srt_content = generate_srt_script(&container);
        let elapsed = start.elapsed();

        assert_ne!(srt_content, "");
        // Must strictly finish generation within <= 200ms
        assert!(
            elapsed.as_millis() <= 200,
            "SRT generation for 300 segments took {}ms, expected <= 200ms",
            elapsed.as_millis()
        );
    }

    #[test]
    fn test_empty_and_whitespace_text_vi_falls_back_to_text_en() {
        let segments = vec![
            TranscriptSegment {
                segment_id: "seg_empty".to_string(),
                start_sec: 0.0,
                end_sec: 1.5,
                speaker_id: "neuro".to_string(),
                text_en: "English SRT one".to_string(),
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
                text_en: "English SRT two".to_string(),
                text_vi: Some("   \t  ".to_string()),
                confidence: 0.95,
                cps_score: 12.0,
                is_flagged_for_review: false,
                words: vec![],
            },
        ];

        let srt = generate_srt_script_from_segments(&segments);
        assert!(srt.contains("1\n00:00:00,000 --> 00:00:01,500\nEnglish SRT one\n"));
        assert!(srt.contains("2\n00:00:02,000 --> 00:00:03,500\nEnglish SRT two\n"));
    }
}
