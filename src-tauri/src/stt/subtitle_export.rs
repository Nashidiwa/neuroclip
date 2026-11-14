//! Subtitle export engines for Advanced SubStation Alpha (.ass) and SubRip (.srt).
//!
//! Implements Schema 5.6 [`SubtitleProject`] generation and character styling
//! with VTuber color matrices for Vedal, Neuro, Evil, Guest, and Chat.

use std::collections::HashMap;
use std::fmt::Write;
use chrono::Utc;
use serde::{Deserialize, Serialize};

use crate::stt::transcript_parser::{TranscriptSegment, TranscriptSegmentsContainer};

/// Styling attributes for a speaker persona in Schema 5.6 (`subtitle_project.json`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct SpeakerStyle {
    pub color_hex: String,
    pub font_name: String,
    pub font_size: u32,
    pub outline_color_hex: String,
}

/// Subtitle project metadata and segment container adhering to Schema 5.6.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct SubtitleProject {
    pub project_id: String,
    pub project_name: String,
    pub vod_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub video_file_path: Option<String>,
    pub target_layout: String,
    pub segments: Vec<TranscriptSegment>,
    pub speaker_styles: HashMap<String, SpeakerStyle>,
    pub updated_at: String,
}

/// Format time in seconds to ASS timecode `H:MM:SS.cc` (centiseconds, 2 digits).
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

/// Format time in seconds to SRT timecode `HH:MM:SS,mmm` (milliseconds, 3 digits).
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

/// Escape text for ASS subtitles to prevent accidental tag injection.
pub fn escape_ass_text(text: &str) -> String {
    text.replace('{', "(")
        .replace('}', ")")
        .replace('\r', "")
        .replace('\n', "\\N")
}

/// Map speaker role string to canonical ASS style name.
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

/// Generate a complete Advanced SubStation Alpha (.ass v4.00+) subtitle script.
///
/// Features VTuber character styles in `&HAABBGGRR` hexadecimal format and
/// proper text escaping.
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
    out.push_str("Style: Vedal,Arial,54,&H0066CC66,&H000000FF,&H00000000,&H80000000,-1,0,0,0,100,100,0,0,1,3,2,2,20,20,40,1\n");
    out.push_str("Style: Neuro,Arial,54,&H00B6B6FF,&H000000FF,&H00000000,&H80000000,-1,0,0,0,100,100,0,0,1,3,2,2,20,20,40,1\n");
    out.push_str("Style: Evil,Arial,54,&H000000DC,&H000000FF,&H00000000,&H80000000,-1,0,0,0,100,100,0,0,1,3,2,2,20,20,40,1\n");
    out.push_str("Style: Guest,Arial,54,&H00FFCC33,&H000000FF,&H00000000,&H80000000,-1,0,0,0,100,100,0,0,1,3,2,2,20,20,40,1\n");
    out.push_str("Style: Chat,Arial,48,&H00CCCCCC,&H000000FF,&H00000000,&H80000000,0,0,0,0,100,100,0,0,1,2,2,2,20,20,40,1\n\n");

    out.push_str("[Events]\n");
    out.push_str("Format: Layer, Start, End, Style, Name, MarginL, MarginR, MarginV, Effect, Text\n");

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

/// Generate a standard SubRip (.srt) subtitle document.
///
/// Outputs sequential 1-based index numbers and comma-separated millisecond timestamps.
pub fn generate_srt_script(container: &TranscriptSegmentsContainer) -> String {
    let mut out = String::with_capacity(4096);

    for (idx, seg) in container.segments.iter().enumerate() {
        let seq = idx + 1;
        let start_tc = format_srt_timecode(seg.start_sec);
        let end_tc = format_srt_timecode(seg.end_sec);
        let raw_text = seg.text_vi.as_deref().unwrap_or(&seg.text_en);
        let clean_text = raw_text.trim();

        let _ = writeln!(out, "{seq}");
        let _ = writeln!(out, "{start_tc} --> {end_tc}");
        let _ = writeln!(out, "{clean_text}\n");
    }

    out
}

/// Build default speaker style dictionary for Schema 5.6 [`SubtitleProject`].
pub fn default_speaker_styles() -> HashMap<String, SpeakerStyle> {
    let mut map = HashMap::new();
    map.insert(
        "neuro".to_string(),
        SpeakerStyle {
            color_hex: "#FFB6C1".to_string(),
            font_name: "Arial".to_string(),
            font_size: 54,
            outline_color_hex: "#000000".to_string(),
        },
    );
    map.insert(
        "evil".to_string(),
        SpeakerStyle {
            color_hex: "#DC143C".to_string(),
            font_name: "Arial".to_string(),
            font_size: 54,
            outline_color_hex: "#000000".to_string(),
        },
    );
    map.insert(
        "vedal".to_string(),
        SpeakerStyle {
            color_hex: "#66CC66".to_string(),
            font_name: "Arial".to_string(),
            font_size: 54,
            outline_color_hex: "#000000".to_string(),
        },
    );
    map.insert(
        "guest".to_string(),
        SpeakerStyle {
            color_hex: "#33CCFF".to_string(),
            font_name: "Arial".to_string(),
            font_size: 54,
            outline_color_hex: "#000000".to_string(),
        },
    );
    map.insert(
        "chat".to_string(),
        SpeakerStyle {
            color_hex: "#CCCCCC".to_string(),
            font_name: "Arial".to_string(),
            font_size: 48,
            outline_color_hex: "#000000".to_string(),
        },
    );
    map.insert(
        "unknown".to_string(),
        SpeakerStyle {
            color_hex: "#FFFFFF".to_string(),
            font_name: "Arial".to_string(),
            font_size: 52,
            outline_color_hex: "#000000".to_string(),
        },
    );
    map
}

/// Construct a full [`SubtitleProject`] document adhering to Schema 5.6.
pub fn create_subtitle_project(
    container: TranscriptSegmentsContainer,
    project_name: &str,
    vod_id: &str,
    target_layout: Option<&str>,
) -> SubtitleProject {
    let project_id = format!("subproj_{}", container.clip_id);
    let layout = target_layout.unwrap_or("original_16_9").to_string();

    SubtitleProject {
        project_id,
        project_name: project_name.to_string(),
        vod_id: vod_id.to_string(),
        video_file_path: None,
        target_layout: layout,
        segments: container.segments,
        speaker_styles: default_speaker_styles(),
        updated_at: Utc::now().to_rfc3339(),
    }
}
