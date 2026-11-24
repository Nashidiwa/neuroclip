//! Premiere Pro / DaVinci Resolve FCP7 XML (`<xmeml version="4">`) timeline exporter.
//!
//! Generates standards-compliant Final Cut Pro 7 XML interchange files compatible
//! with Adobe Premiere Pro 2022–2025 and DaVinci Resolve 18–19:
//! - Dynamic canvas resolution resolved via [`canvas_config_for_layout`] (`1920x1080` or `1080x1920`).
//! - Fixed `60fps` sequence timebase (`<timebase>60</timebase>`, `<ntsc>FALSE</ntsc>`).
//! - Synchronized Video `<track>` and Audio `<track>` (`48kHz`, `16-bit`, `2ch`) with
//!   single `<file id="file-N">` definitions, self-closing references, and `<link>` pairs.
//! - RFC-3986 `file://localhost/...` `<pathurl>` encoding via `percent-encoding`.
//! - Sequence `<marker>` elements synchronized to subtitle timestamps (`text_vi` with fallback to `text_en`).
//! - Pure UTF-8 XML output without BOM and strict XML 1.0 character escaping.

use std::collections::HashMap;
use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::time::Instant;

use percent_encoding::{utf8_percent_encode, AsciiSet, CONTROLS};
use serde::{Deserialize, Serialize};

use crate::error::{AppError, ExportError};
use crate::export::capcut_schema::canvas_config_for_layout;
use crate::export::SubtitleProject;

/// Default frame rate for Premiere Pro / DaVinci Resolve FCP7 XML sequences (`60fps`).
pub const DEFAULT_PREMIERE_FPS: u32 = 60;

/// Standard NLE audio sample rate (`48kHz`).
pub const PREMIERE_AUDIO_SAMPLE_RATE: u32 = 48_000;

/// Standard NLE audio sample bit depth (`16-bit`).
pub const PREMIERE_AUDIO_BIT_DEPTH: u32 = 16;

/// Standard stereo channel count (`2ch`).
pub const PREMIERE_AUDIO_CHANNELS: u32 = 2;

/// Maximum safe timeline seconds (~100 days) to guard frame multiplication against overflow.
const MAX_TIMELINE_SECONDS: f64 = 8_640_000.0;

/// Percent-encoding set for individual URL path segments inside `file://localhost/...`.
/// Preserves unreserved characters and common safe punctuation while encoding spaces,
/// `#`, `%`, `?`, `&`, `<`, `>`, `"`, `'`, braces, and non-ASCII UTF-8 bytes.
const PATH_SEGMENT_ENCODE_SET: &AsciiSet = &CONTROLS
    .add(b' ')
    .add(b'"')
    .add(b'#')
    .add(b'%')
    .add(b'&')
    .add(b'\'')
    .add(b'+')
    .add(b'<')
    .add(b'>')
    .add(b'?')
    .add(b'[')
    .add(b']')
    .add(b'^')
    .add(b'`')
    .add(b'{')
    .add(b'|')
    .add(b'}');

/// Individual video clip input for multi-clip Premiere Pro XML timeline assembly.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct PremiereClipInput {
    #[serde(default, alias = "clipId")]
    pub clip_id: Option<String>,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(alias = "filePath")]
    pub file_path: String,
    #[serde(default, alias = "startSec")]
    pub start_sec: Option<f64>,
    #[serde(default, alias = "endSec")]
    pub end_sec: Option<f64>,
    #[serde(default, alias = "durationSec")]
    pub duration_sec: Option<f64>,
}

/// Optional configuration for Premiere Pro FCP7 XML export.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct PremiereXmlExportOptions {
    #[serde(default, alias = "videoFilePath")]
    pub video_file_path: Option<String>,
    #[serde(default)]
    pub clips: Option<Vec<PremiereClipInput>>,
    #[serde(default)]
    pub fps: Option<u32>,
}

/// Result metadata returned after exporting a Premiere Pro FCP7 XML timeline.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PremiereExportResult {
    pub xml_path: String,
    pub sequence_name: String,
    pub width: u32,
    pub height: u32,
    pub fps: u32,
    pub marker_count: usize,
    pub clip_count: usize,
    pub duration_ms: u64,
}

#[derive(Debug, Clone)]
pub(crate) struct PremiereXmlBuildOutput {
    pub xml: String,
    pub sequence_name: String,
    pub width: u32,
    pub height: u32,
    pub fps: u32,
    pub marker_count: usize,
    pub clip_count: usize,
}

#[derive(Debug, Clone)]
struct ResolvedTimelineClip {
    file_url: String,
    name: String,
    seq_start_frames: u64,
    seq_end_frames: u64,
    duration_frames: u64,
    file_duration_frames: u64,
    file_id: usize,
    define_file: bool,
}

#[derive(Debug, Clone)]
struct ResolvedMarker {
    comment: String,
    name: String,
    in_frame: u64,
    out_frame: u64,
}

const fn is_valid_xml_char(ch: char) -> bool {
    let code = ch as u32;
    matches!(
        code,
        0x09 | 0x0A | 0x0D | 0x20..=0xD7FF | 0xE000..=0xFFFD | 0x10000..=0x10_FFFF
    ) && code != 0x7F
}

fn strip_invalid_xml_chars(text: &str) -> String {
    text.chars().filter(|&ch| is_valid_xml_char(ch)).collect()
}

/// Escape text safely for XML 1.0 character data and strip invalid control characters.
///
/// Escapes `&`, `<`, `>`, `"`, and `'` and removes XML-forbidden control codes
/// (`< 0x20` other than `\t`, `\n`, `\r`, plus `0x7F`, `\u{FFFE}`, `\u{FFFF}`).
#[must_use]
pub fn escape_xml_text(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for ch in text.chars() {
        if !is_valid_xml_char(ch) {
            continue;
        }
        match ch {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&apos;"),
            _ => out.push(ch),
        }
    }
    out
}

fn percent_encode_preserving_escapes(segment: &str) -> String {
    let bytes = segment.as_bytes();
    let mut out = String::with_capacity(segment.len());
    let mut chunk_start = 0usize;
    let mut idx = 0usize;

    while idx < bytes.len() {
        if bytes.get(idx) == Some(&b'%')
            && bytes.get(idx + 1).is_some_and(u8::is_ascii_hexdigit)
            && bytes.get(idx + 2).is_some_and(u8::is_ascii_hexdigit)
        {
            if chunk_start < idx {
                if let Some(prefix) = segment.get(chunk_start..idx) {
                    out.push_str(&utf8_percent_encode(prefix, PATH_SEGMENT_ENCODE_SET).to_string());
                }
            }
            if let Some(escape) = segment.get(idx..idx + 3) {
                out.push_str(escape);
            }
            idx += 3;
            chunk_start = idx;
            continue;
        }
        idx += 1;
    }

    if chunk_start < segment.len() {
        if let Some(tail) = segment.get(chunk_start..) {
            out.push_str(&utf8_percent_encode(tail, PATH_SEGMENT_ENCODE_SET).to_string());
        }
    }

    out
}

fn resolve_relative_to_absolute(trimmed: &str) -> String {
    let p = Path::new(trimmed);
    let abs_path = if let Ok(canon) = std::fs::canonicalize(p) {
        canon
    } else {
        let base = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
        let combined = base.join(p);
        let mut components = Vec::new();
        for comp in combined.components() {
            match comp {
                std::path::Component::CurDir => {}
                std::path::Component::ParentDir => {
                    if let Some(std::path::Component::Normal(_)) = components.last() {
                        components.pop();
                    }
                }
                c => components.push(c),
            }
        }
        components.iter().collect::<PathBuf>()
    };
    abs_path.to_string_lossy().to_string()
}

/// Convert a local or UNC filesystem path (Windows or POSIX) into an RFC-3986 `file://` URL.
///
/// Preserves Windows drive letters (`C:`) and `/` path separators while percent-encoding spaces,
/// reserved URI delimiters, and non-ASCII UTF-8 characters. Windows UNC paths
/// (`\\server\share\...`, `\\?\UNC\server\share\...`, `\\.\UNC\server\share\...`) are
/// formatted as `file://server/share/...`.
#[must_use]
pub fn path_to_file_url(path: &str) -> String {
    let trimmed = path.trim();
    if trimmed.is_empty() {
        return String::new();
    }

    if trimmed
        .get(..7)
        .is_some_and(|prefix| prefix.eq_ignore_ascii_case("file://"))
    {
        let rest = trimmed[7..].replace('\\', "/");
        if let Some((authority, path_rest)) = rest.split_once('/') {
            let encoded_authority = if authority.is_empty() {
                String::new()
            } else {
                percent_encode_preserving_escapes(authority)
            };
            let encoded_segments: Vec<String> = path_rest
                .split('/')
                .filter(|seg| !seg.is_empty())
                .map(percent_encode_preserving_escapes)
                .collect();
            if encoded_segments.is_empty() {
                return format!("file://{encoded_authority}/");
            }
            return format!("file://{encoded_authority}/{}", encoded_segments.join("/"));
        }
        return format!("file://{}", percent_encode_preserving_escapes(&rest));
    }

    let is_windows_drive = {
        let b = trimmed.as_bytes();
        b.len() >= 2 && b[0].is_ascii_alphabetic() && b[1] == b':'
    };
    let is_absolute_posix = trimmed.starts_with('/');
    let is_unc_or_device = trimmed.starts_with(r"\\") || trimmed.starts_with("//");

    let resolved_path_str: String;
    let path_to_process = if !is_windows_drive && !is_absolute_posix && !is_unc_or_device {
        resolved_path_str = resolve_relative_to_absolute(trimmed);
        &resolved_path_str
    } else {
        trimmed
    };

    let normalized = path_to_process.replace('\\', "/");

    let unc_rest = if let Some(device_rest) = normalized
        .strip_prefix("//?/")
        .or_else(|| normalized.strip_prefix("//./"))
    {
        if device_rest
            .get(..4)
            .is_some_and(|prefix| prefix.eq_ignore_ascii_case("UNC/"))
        {
            Some(&device_rest[4..])
        } else {
            None
        }
    } else if normalized.starts_with("//") && !normalized.starts_with("///") {
        Some(normalized.trim_start_matches('/'))
    } else {
        None
    };

    if let Some(unc) = unc_rest {
        let mut unc_segments = unc.split('/').filter(|seg| !seg.is_empty());
        let Some(server) = unc_segments.next() else {
            return String::new();
        };
        let encoded_server = utf8_percent_encode(server, PATH_SEGMENT_ENCODE_SET).to_string();
        let encoded_path: Vec<String> = unc_segments
            .map(|seg| utf8_percent_encode(seg, PATH_SEGMENT_ENCODE_SET).to_string())
            .collect();
        if encoded_path.is_empty() {
            return format!("file://{encoded_server}");
        }
        return format!("file://{encoded_server}/{}", encoded_path.join("/"));
    }

    let local_normalized = normalized
        .strip_prefix("//?/")
        .or_else(|| normalized.strip_prefix("//./"))
        .unwrap_or(&normalized);

    let mut encoded_segments = Vec::new();
    for (idx, seg) in local_normalized.split('/').enumerate() {
        if seg.is_empty() {
            continue;
        }
        let is_windows_drive = idx == 0
            && seg.len() == 2
            && seg.as_bytes().first().is_some_and(u8::is_ascii_alphabetic)
            && seg.as_bytes().get(1) == Some(&b':');
        if is_windows_drive {
            encoded_segments.push(seg.to_string());
        } else {
            encoded_segments.push(utf8_percent_encode(seg, PATH_SEGMENT_ENCODE_SET).to_string());
        }
    }

    if encoded_segments.is_empty() {
        return String::new();
    }

    format!("file://localhost/{}", encoded_segments.join("/"))
}

fn canonical_file_url_key(file_url: &str) -> String {
    let is_windows_drive_url = file_url
        .strip_prefix("file://localhost/")
        .or_else(|| file_url.strip_prefix("file:///"))
        .is_some_and(|rest| {
            let bytes = rest.as_bytes();
            bytes.first().is_some_and(u8::is_ascii_alphabetic)
                && bytes.get(1) == Some(&b':')
                && bytes.get(2) == Some(&b'/')
        });

    let is_unc_url = file_url
        .strip_prefix("file://")
        .is_some_and(|rest| !rest.starts_with('/') && !rest.starts_with("localhost/"));

    if is_windows_drive_url || is_unc_url {
        file_url.to_ascii_lowercase()
    } else {
        file_url.to_string()
    }
}

/// Convert seconds into non-negative frame count at the given `fps`.
///
/// Safely clamps `NaN`, infinite, and negative inputs to `0`.
#[must_use]
pub fn sec_to_frames(sec: f64, fps: u32) -> u64 {
    if !sec.is_finite() || sec <= 0.0 || fps == 0 {
        return 0;
    }
    let clamped_sec = sec.min(MAX_TIMELINE_SECONDS);
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let frames = (clamped_sec * f64::from(fps)).round() as u64;
    frames
}

fn deterministic_sequence_uuid(project_id: &str) -> String {
    // Stable FNV-1a 128-bit style deterministic UUID v4 formatting from project_id
    let mut h1: u64 = 0xcbf2_9ce4_8422_2325;
    let mut h2: u64 = 0xaf63_bd4c_8601_b7df;
    for byte in project_id.as_bytes() {
        h1 ^= u64::from(*byte);
        h1 = h1.wrapping_mul(0x0100_0000_01b3);
        h2 = h2.wrapping_mul(0x0100_0000_01b3) ^ u64::from(*byte);
    }
    #[allow(clippy::cast_possible_truncation)]
    let part1 = (h1 >> 32) as u32;
    #[allow(clippy::cast_possible_truncation)]
    let part2 = ((h1 >> 16) & 0xFFFF) as u16;
    #[allow(clippy::cast_possible_truncation)]
    let part3 = ((h1 & 0x0FFF) as u16) | 0x4000;
    #[allow(clippy::cast_possible_truncation)]
    let part4 = (((h2 >> 48) & 0x3FFF) as u16) | 0x8000;
    let part5 = h2 & 0x0000_FFFF_FFFF_FFFF;
    format!("{part1:08x}-{part2:04x}-{part3:04x}-{part4:04x}-{part5:012x}")
}

fn derive_clip_name(explicit_name: Option<&str>, file_path: &str, fallback_idx: usize) -> String {
    if let Some(name) = explicit_name.map(str::trim).filter(|s| !s.is_empty()) {
        return name.to_string();
    }
    Path::new(file_path)
        .file_name()
        .map(|f| f.to_string_lossy().to_string())
        .filter(|s| !s.trim().is_empty())
        .unwrap_or_else(|| format!("clip_{fallback_idx}.mp4"))
}

fn truncate_chars(text: &str, max_chars: usize) -> String {
    let mut iter = text.chars();
    let collected: String = iter.by_ref().take(max_chars).collect();
    if iter.next().is_some() {
        format!("{collected}...")
    } else {
        collected
    }
}

fn collect_raw_clips(
    project: &SubtitleProject,
    options: Option<&PremiereXmlExportOptions>,
    default_duration_sec: f64,
) -> Vec<PremiereClipInput> {
    let mut raw_clips: Vec<PremiereClipInput> = options
        .and_then(|opt| opt.clips.as_ref())
        .map(|clips| {
            clips
                .iter()
                .filter(|clip| !clip.file_path.trim().is_empty())
                .cloned()
                .collect()
        })
        .unwrap_or_default();

    if raw_clips.is_empty() {
        let single_video = options
            .and_then(|opt| opt.video_file_path.as_deref())
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .or_else(|| {
                project
                    .video_file_path
                    .as_deref()
                    .map(str::trim)
                    .filter(|s| !s.is_empty())
            });

        if let Some(v_path) = single_video {
            raw_clips.push(PremiereClipInput {
                clip_id: Some("clip-1".to_string()),
                name: None,
                file_path: v_path.to_string(),
                start_sec: Some(0.0),
                end_sec: Some(default_duration_sec),
                duration_sec: Some(default_duration_sec),
            });
        }
    }

    raw_clips
}

fn resolve_timeline_clips(
    project: &SubtitleProject,
    options: Option<&PremiereXmlExportOptions>,
    fps: u32,
    max_seg_end_sec: f64,
) -> Vec<ResolvedTimelineClip> {
    let default_duration_sec = max_seg_end_sec.max(1.0);
    let raw_clips = collect_raw_clips(project, options, default_duration_sec);

    let mut resolved = Vec::with_capacity(raw_clips.len());
    let mut file_ids: HashMap<String, usize> = HashMap::new();
    let mut max_file_durations: HashMap<usize, u64> = HashMap::new();
    let mut next_file_id: usize = 1;
    let mut cursor_frames: u64 = 0;

    for (idx, clip) in raw_clips.into_iter().enumerate() {
        let file_url = path_to_file_url(&clip.file_path);
        if file_url.is_empty() {
            continue;
        }

        let canonical_key = canonical_file_url_key(&file_url);
        let (file_id, define_file) = if let Some(&existing_id) = file_ids.get(&canonical_key) {
            (existing_id, false)
        } else {
            let assigned = next_file_id;
            next_file_id += 1;
            file_ids.insert(canonical_key, assigned);
            (assigned, true)
        };

        let explicit_start = clip.start_sec.filter(|s| s.is_finite() && *s >= 0.0);
        let explicit_end = clip.end_sec.filter(|e| e.is_finite() && *e >= 0.0);
        let explicit_dur = clip.duration_sec.filter(|d| d.is_finite() && *d > 0.0);

        let (seq_start_frames, duration_frames) = match (explicit_start, explicit_end, explicit_dur)
        {
            (Some(s), Some(e), None) if e > s => {
                let start_f = sec_to_frames(s, fps);
                let dur_f = sec_to_frames(e - s, fps).max(1);
                (start_f, dur_f)
            }
            (Some(s), Some(_e), None) => {
                // Explicit start & end with end <= start: clamp to minimum 1 frame at start_f
                let start_f = sec_to_frames(s, fps);
                (start_f, 1)
            }
            (Some(s), _, Some(d)) => {
                let start_f = sec_to_frames(s, fps);
                let dur_f = sec_to_frames(d, fps).max(1);
                (start_f, dur_f)
            }
            (None, _, Some(d)) => {
                let start_f = cursor_frames;
                let dur_f = sec_to_frames(d, fps).max(1);
                (start_f, dur_f)
            }
            _ => {
                let start_f = cursor_frames;
                let dur_f = sec_to_frames(default_duration_sec, fps).max(1);
                (start_f, dur_f)
            }
        };

        let seq_end_frames = seq_start_frames.saturating_add(duration_frames);
        cursor_frames = cursor_frames.max(seq_end_frames);
        max_file_durations
            .entry(file_id)
            .and_modify(|max_d| *max_d = (*max_d).max(duration_frames))
            .or_insert(duration_frames);

        let clip_name = derive_clip_name(clip.name.as_deref(), &clip.file_path, idx + 1);

        resolved.push(ResolvedTimelineClip {
            file_url,
            name: clip_name,
            seq_start_frames,
            seq_end_frames,
            duration_frames,
            file_duration_frames: duration_frames,
            file_id,
            define_file,
        });
    }

    for clip in &mut resolved {
        if let Some(&max_dur) = max_file_durations.get(&clip.file_id) {
            clip.file_duration_frames = max_dur;
        }
    }

    resolved
}

fn resolve_markers(project: &SubtitleProject, fps: u32) -> Vec<ResolvedMarker> {
    let mut markers = Vec::with_capacity(project.segments.len());
    for seg in &project.segments {
        let vi_candidate = seg
            .text_vi
            .as_deref()
            .map(strip_invalid_xml_chars)
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty());
        let en_candidate = strip_invalid_xml_chars(seg.text_en.trim())
            .trim()
            .to_string();
        let subtitle_text = vi_candidate.unwrap_or(en_candidate);
        if subtitle_text.is_empty() {
            continue;
        }

        let in_frame = sec_to_frames(seg.start_sec, fps);
        let raw_out = sec_to_frames(seg.end_sec, fps);
        let out_frame = if raw_out > in_frame {
            raw_out
        } else {
            in_frame.saturating_add(1)
        };

        let speaker_clean = strip_invalid_xml_chars(&seg.speaker_id);
        let speaker_label = if speaker_clean.trim().is_empty() {
            "UNKNOWN".to_string()
        } else {
            speaker_clean.trim().to_ascii_uppercase()
        };
        let marker_name = format!("[{speaker_label}] {}", truncate_chars(&subtitle_text, 60));

        markers.push(ResolvedMarker {
            comment: subtitle_text,
            name: marker_name,
            in_frame,
            out_frame,
        });
    }
    markers
}

#[allow(clippy::too_many_lines)]
pub(crate) fn build_premiere_xml(
    project: &SubtitleProject,
    options: Option<&PremiereXmlExportOptions>,
) -> Result<PremiereXmlBuildOutput, AppError> {
    let canvas = canvas_config_for_layout(&project.target_layout)?;
    let fps = options
        .and_then(|opt| opt.fps)
        .filter(|&f| f > 0)
        .unwrap_or(DEFAULT_PREMIERE_FPS);

    let max_seg_end_sec = project
        .segments
        .iter()
        .filter_map(|seg| {
            if seg.end_sec.is_finite() && seg.end_sec > 0.0 {
                Some(seg.end_sec)
            } else {
                None
            }
        })
        .fold(0.0_f64, f64::max);

    let clips = resolve_timeline_clips(project, options, fps, max_seg_end_sec);
    let markers = resolve_markers(project, fps);
    let max_clip_end_frames = clips
        .iter()
        .map(|clip| clip.seq_end_frames)
        .max()
        .unwrap_or(0);
    let max_marker_out_frames = markers.iter().map(|m| m.out_frame).max().unwrap_or(0);
    let sequence_duration_frames = max_clip_end_frames.max(max_marker_out_frames).max(1);

    let sequence_name = if project.project_name.trim().is_empty() {
        if project.project_id.trim().is_empty() {
            "NeuroClip Sequence".to_string()
        } else {
            project.project_id.trim().to_string()
        }
    } else {
        project.project_name.trim().to_string()
    };

    let escaped_seq_name = escape_xml_text(&sequence_name);
    let seq_uuid = deterministic_sequence_uuid(&project.project_id);

    let mut xml = String::with_capacity(8192 + project.segments.len() * 256);
    let _ = writeln!(xml, "<?xml version=\"1.0\" encoding=\"UTF-8\"?>");
    let _ = writeln!(xml, "<!DOCTYPE xmeml>");
    let _ = writeln!(xml, "<xmeml version=\"4\">");
    let _ = writeln!(xml, "  <sequence id=\"sequence-1\">");
    let _ = writeln!(xml, "    <uuid>{seq_uuid}</uuid>");
    let _ = writeln!(xml, "    <duration>{sequence_duration_frames}</duration>");
    let _ = writeln!(xml, "    <rate>");
    let _ = writeln!(xml, "      <timebase>{fps}</timebase>");
    let _ = writeln!(xml, "      <ntsc>FALSE</ntsc>");
    let _ = writeln!(xml, "    </rate>");
    let _ = writeln!(xml, "    <name>{escaped_seq_name}</name>");
    let _ = writeln!(xml, "    <media>");

    // Video section
    let _ = writeln!(xml, "      <video>");
    let _ = writeln!(xml, "        <format>");
    let _ = writeln!(xml, "          <samplecharacteristics>");
    let _ = writeln!(xml, "            <rate>");
    let _ = writeln!(xml, "              <timebase>{fps}</timebase>");
    let _ = writeln!(xml, "              <ntsc>FALSE</ntsc>");
    let _ = writeln!(xml, "            </rate>");
    let _ = writeln!(xml, "            <width>{}</width>", canvas.width);
    let _ = writeln!(xml, "            <height>{}</height>", canvas.height);
    let _ = writeln!(xml, "            <anamorphic>FALSE</anamorphic>");
    let _ = writeln!(
        xml,
        "            <pixelaspectratio>square</pixelaspectratio>"
    );
    let _ = writeln!(xml, "            <fielddominance>none</fielddominance>");
    let _ = writeln!(xml, "          </samplecharacteristics>");
    let _ = writeln!(xml, "        </format>");
    let _ = writeln!(xml, "        <track>");

    for (idx, clip) in clips.iter().enumerate() {
        let clip_index = idx + 1;
        let v_id = format!("clipitem-v-{clip_index}");
        let a_id = format!("clipitem-a-{clip_index}");
        let escaped_clip_name = escape_xml_text(&clip.name);
        let escaped_file_url = escape_xml_text(&clip.file_url);
        let _ = writeln!(xml, "          <clipitem id=\"{v_id}\">");
        let _ = writeln!(xml, "            <name>{escaped_clip_name}</name>");
        let _ = writeln!(xml, "            <enabled>TRUE</enabled>");
        let _ = writeln!(
            xml,
            "            <duration>{}</duration>",
            clip.duration_frames
        );
        let _ = writeln!(xml, "            <rate>");
        let _ = writeln!(xml, "              <timebase>{fps}</timebase>");
        let _ = writeln!(xml, "              <ntsc>FALSE</ntsc>");
        let _ = writeln!(xml, "            </rate>");
        let _ = writeln!(xml, "            <start>{}</start>", clip.seq_start_frames);
        let _ = writeln!(xml, "            <end>{}</end>", clip.seq_end_frames);
        let _ = writeln!(xml, "            <in>0</in>");
        let _ = writeln!(xml, "            <out>{}</out>", clip.duration_frames);
        if clip.define_file {
            let _ = writeln!(xml, "            <file id=\"file-{}\">", clip.file_id);
            let _ = writeln!(xml, "              <name>{escaped_clip_name}</name>");
            let _ = writeln!(xml, "              <pathurl>{escaped_file_url}</pathurl>");
            let _ = writeln!(xml, "              <rate>");
            let _ = writeln!(xml, "                <timebase>{fps}</timebase>");
            let _ = writeln!(xml, "                <ntsc>FALSE</ntsc>");
            let _ = writeln!(xml, "              </rate>");
            let _ = writeln!(
                xml,
                "              <duration>{}</duration>",
                clip.file_duration_frames
            );
            let _ = writeln!(xml, "              <media>");
            let _ = writeln!(xml, "                <video>");
            let _ = writeln!(xml, "                  <samplecharacteristics>");
            let _ = writeln!(xml, "                    <rate>");
            let _ = writeln!(xml, "                      <timebase>{fps}</timebase>");
            let _ = writeln!(xml, "                      <ntsc>FALSE</ntsc>");
            let _ = writeln!(xml, "                    </rate>");
            let _ = writeln!(xml, "                    <width>{}</width>", canvas.width);
            let _ = writeln!(
                xml,
                "                    <height>{}</height>",
                canvas.height
            );
            let _ = writeln!(xml, "                  </samplecharacteristics>");
            let _ = writeln!(xml, "                </video>");
            let _ = writeln!(xml, "                <audio>");
            let _ = writeln!(xml, "                  <samplecharacteristics>");
            let _ = writeln!(
                xml,
                "                    <depth>{PREMIERE_AUDIO_BIT_DEPTH}</depth>"
            );
            let _ = writeln!(
                xml,
                "                    <samplerate>{PREMIERE_AUDIO_SAMPLE_RATE}</samplerate>"
            );
            let _ = writeln!(xml, "                  </samplecharacteristics>");
            let _ = writeln!(
                xml,
                "                  <channelcount>{PREMIERE_AUDIO_CHANNELS}</channelcount>"
            );
            let _ = writeln!(xml, "                </audio>");
            let _ = writeln!(xml, "              </media>");
            let _ = writeln!(xml, "            </file>");
        } else {
            let _ = writeln!(xml, "            <file id=\"file-{}\"/>", clip.file_id);
        }
        let _ = writeln!(xml, "            <link>");
        let _ = writeln!(xml, "              <linkclipref>{v_id}</linkclipref>");
        let _ = writeln!(xml, "              <mediatype>video</mediatype>");
        let _ = writeln!(xml, "              <trackindex>1</trackindex>");
        let _ = writeln!(xml, "              <clipindex>{clip_index}</clipindex>");
        let _ = writeln!(xml, "            </link>");
        let _ = writeln!(xml, "            <link>");
        let _ = writeln!(xml, "              <linkclipref>{a_id}</linkclipref>");
        let _ = writeln!(xml, "              <mediatype>audio</mediatype>");
        let _ = writeln!(xml, "              <trackindex>1</trackindex>");
        let _ = writeln!(xml, "              <clipindex>{clip_index}</clipindex>");
        let _ = writeln!(xml, "            </link>");
        let _ = writeln!(xml, "          </clipitem>");
    }

    let _ = writeln!(xml, "        </track>");
    let _ = writeln!(xml, "      </video>");

    // Audio section
    let _ = writeln!(xml, "      <audio>");
    let _ = writeln!(
        xml,
        "        <numOutputChannels>{PREMIERE_AUDIO_CHANNELS}</numOutputChannels>"
    );
    let _ = writeln!(xml, "        <format>");
    let _ = writeln!(xml, "          <samplecharacteristics>");
    let _ = writeln!(xml, "            <depth>{PREMIERE_AUDIO_BIT_DEPTH}</depth>");
    let _ = writeln!(
        xml,
        "            <samplerate>{PREMIERE_AUDIO_SAMPLE_RATE}</samplerate>"
    );
    let _ = writeln!(xml, "          </samplecharacteristics>");
    let _ = writeln!(xml, "        </format>");
    let _ = writeln!(xml, "        <track>");

    for (idx, clip) in clips.iter().enumerate() {
        let clip_index = idx + 1;
        let v_id = format!("clipitem-v-{clip_index}");
        let a_id = format!("clipitem-a-{clip_index}");
        let escaped_clip_name = escape_xml_text(&clip.name);
        let _ = writeln!(xml, "          <clipitem id=\"{a_id}\">");
        let _ = writeln!(xml, "            <name>{escaped_clip_name}</name>");
        let _ = writeln!(xml, "            <enabled>TRUE</enabled>");
        let _ = writeln!(
            xml,
            "            <duration>{}</duration>",
            clip.duration_frames
        );
        let _ = writeln!(xml, "            <rate>");
        let _ = writeln!(xml, "              <timebase>{fps}</timebase>");
        let _ = writeln!(xml, "              <ntsc>FALSE</ntsc>");
        let _ = writeln!(xml, "            </rate>");
        let _ = writeln!(xml, "            <start>{}</start>", clip.seq_start_frames);
        let _ = writeln!(xml, "            <end>{}</end>", clip.seq_end_frames);
        let _ = writeln!(xml, "            <in>0</in>");
        let _ = writeln!(xml, "            <out>{}</out>", clip.duration_frames);
        let _ = writeln!(xml, "            <file id=\"file-{}\"/>", clip.file_id);
        let _ = writeln!(xml, "            <sourcetrack>");
        let _ = writeln!(xml, "              <mediatype>audio</mediatype>");
        let _ = writeln!(xml, "              <trackindex>1</trackindex>");
        let _ = writeln!(xml, "            </sourcetrack>");
        let _ = writeln!(xml, "            <link>");
        let _ = writeln!(xml, "              <linkclipref>{v_id}</linkclipref>");
        let _ = writeln!(xml, "              <mediatype>video</mediatype>");
        let _ = writeln!(xml, "              <trackindex>1</trackindex>");
        let _ = writeln!(xml, "              <clipindex>{clip_index}</clipindex>");
        let _ = writeln!(xml, "            </link>");
        let _ = writeln!(xml, "            <link>");
        let _ = writeln!(xml, "              <linkclipref>{a_id}</linkclipref>");
        let _ = writeln!(xml, "              <mediatype>audio</mediatype>");
        let _ = writeln!(xml, "              <trackindex>1</trackindex>");
        let _ = writeln!(xml, "              <clipindex>{clip_index}</clipindex>");
        let _ = writeln!(xml, "            </link>");
        let _ = writeln!(xml, "          </clipitem>");
    }

    let _ = writeln!(xml, "        </track>");
    let _ = writeln!(xml, "      </audio>");
    let _ = writeln!(xml, "    </media>");

    // Sequence markers for Vietnamese subtitles (fallback to English)
    let marker_count = markers.len();
    for marker in &markers {
        let _ = writeln!(xml, "    <marker>");
        let _ = writeln!(
            xml,
            "      <comment>{}</comment>",
            escape_xml_text(&marker.comment)
        );
        let _ = writeln!(xml, "      <name>{}</name>", escape_xml_text(&marker.name));
        let _ = writeln!(xml, "      <in>{}</in>", marker.in_frame);
        let _ = writeln!(xml, "      <out>{}</out>", marker.out_frame);
        let _ = writeln!(xml, "    </marker>");
    }

    let _ = writeln!(xml, "  </sequence>");
    let _ = writeln!(xml, "</xmeml>");

    Ok(PremiereXmlBuildOutput {
        xml,
        sequence_name,
        width: canvas.width,
        height: canvas.height,
        fps,
        marker_count,
        clip_count: clips.len(),
    })
}

/// Generate a complete FCP7 XML (`<xmeml version="4">`) document from a [`SubtitleProject`].
///
/// # Errors
/// Returns [`AppError::Export`] with [`ExportError::InvalidLayout`] if `project.target_layout` is unsupported.
pub fn generate_premiere_xml(
    project: &SubtitleProject,
    options: Option<&PremiereXmlExportOptions>,
) -> Result<String, AppError> {
    build_premiere_xml(project, options).map(|built| built.xml)
}

pub(crate) fn write_premiere_xml_bytes(output_path: &Path, xml: &str) -> Result<(), AppError> {
    if output_path.as_os_str().is_empty() {
        return Err(AppError::Export(ExportError::FileWriteFailed {
            path: String::new(),
            details: "Output path cannot be empty".to_string(),
        }));
    }

    if let Some(parent) = output_path.parent() {
        if !parent.as_os_str().is_empty() && !parent.exists() {
            std::fs::create_dir_all(parent).map_err(|e| {
                AppError::Export(ExportError::FileWriteFailed {
                    path: parent.display().to_string(),
                    details: e.to_string(),
                })
            })?;
        }
    }

    std::fs::write(output_path, xml.as_bytes()).map_err(|e| {
        AppError::Export(ExportError::FileWriteFailed {
            path: output_path.display().to_string(),
            details: e.to_string(),
        })
    })
}

/// Generate and write an FCP7 XML (`<xmeml version="4">`) file to `output_path` in UTF-8 without BOM.
///
/// # Errors
/// Returns [`AppError::Export`] with [`ExportError::InvalidLayout`] if `project.target_layout` is unsupported,
/// or [`ExportError::FileWriteFailed`] if directory creation or writing fails.
pub fn export_premiere_xml_file(
    project: &SubtitleProject,
    output_path: &Path,
    options: Option<&PremiereXmlExportOptions>,
) -> Result<PremiereExportResult, AppError> {
    let start_time = Instant::now();
    let built = build_premiere_xml(project, options)?;
    write_premiere_xml_bytes(output_path, &built.xml)?;

    #[allow(clippy::cast_possible_truncation)]
    let duration_ms = start_time.elapsed().as_millis() as u64;

    Ok(PremiereExportResult {
        xml_path: output_path.display().to_string(),
        sequence_name: built.sequence_name,
        width: built.width,
        height: built.height,
        fps: built.fps,
        marker_count: built.marker_count,
        clip_count: built.clip_count,
        duration_ms,
    })
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
    use crate::stt::transcript_parser::{TranscriptSegment, TranscriptSegmentsContainer};
    use chrono::Utc;

    fn make_segment(
        id: &str,
        start_sec: f64,
        end_sec: f64,
        speaker: &str,
        text_en: &str,
        text_vi: Option<&str>,
    ) -> TranscriptSegment {
        TranscriptSegment {
            segment_id: id.to_string(),
            start_sec,
            end_sec,
            speaker_id: speaker.to_string(),
            text_en: text_en.to_string(),
            text_vi: text_vi.map(ToString::to_string),
            confidence: 0.95,
            cps_score: 12.0,
            is_flagged_for_review: false,
            words: vec![],
        }
    }

    fn make_project(layout: &str, video_path: Option<&str>) -> SubtitleProject {
        let container = TranscriptSegmentsContainer {
            clip_id: "clip_m4_26_ut".to_string(),
            segments: vec![
                make_segment(
                    "seg_1",
                    1.0,
                    3.5,
                    "neuro",
                    "Heart <3 Vedal & Evil",
                    Some("Trái tim <3 Vedal & Evil \"đỉnh nóc\" 'kịch trần'"),
                ),
                make_segment("seg_2", 4.0, 6.25, "vedal", "Fallback English line", None),
                make_segment(
                    "seg_3",
                    7.0,
                    8.5,
                    "evil",
                    "Whitespace Vi fallback",
                    Some("   "),
                ),
            ],
        };
        let mut proj = create_subtitle_project(
            container,
            "Vedal & Neuro <Special> Clip",
            "vod_m4_26",
            Some(layout),
        );
        proj.video_file_path = video_path.map(ToString::to_string);
        proj
    }

    #[test]
    fn test_ut_26_1_xml_schema_compliance_16_9_and_9_16() {
        // 16:9 layout -> 1920x1080, 60fps
        let proj_16_9 = make_project("original_16_9", Some(r"C:\Users\Neuro Clip\clip #1.mp4"));
        let xml_16_9 = generate_premiere_xml(&proj_16_9, None).unwrap();

        assert!(xml_16_9.starts_with("<?xml version=\"1.0\" encoding=\"UTF-8\"?>"));
        assert!(!xml_16_9.starts_with('\u{FEFF}'));
        assert!(xml_16_9.contains("<!DOCTYPE xmeml>"));
        assert!(xml_16_9.contains("<xmeml version=\"4\">"));
        assert!(xml_16_9.contains("<timebase>60</timebase>"));
        assert!(xml_16_9.contains("<ntsc>FALSE</ntsc>"));
        assert!(xml_16_9.contains("<width>1920</width>"));
        assert!(xml_16_9.contains("<height>1080</height>"));
        assert!(xml_16_9.contains("<depth>16</depth>"));
        assert!(xml_16_9.contains("<samplerate>48000</samplerate>"));
        assert!(xml_16_9.contains("<numOutputChannels>2</numOutputChannels>"));
        assert!(xml_16_9
            .contains("<pathurl>file://localhost/C:/Users/Neuro%20Clip/clip%20%231.mp4</pathurl>"));

        // Single <file id="file-1"> definition and self-closing reference in audio track
        assert_eq!(xml_16_9.matches("<file id=\"file-1\">").count(), 1);
        assert_eq!(xml_16_9.matches("<file id=\"file-1\"/>").count(), 1);
        assert!(xml_16_9.contains("<linkclipref>clipitem-v-1</linkclipref>"));
        assert!(xml_16_9.contains("<linkclipref>clipitem-a-1</linkclipref>"));

        // Escaped sequence name and Vietnamese markers + fallbacks
        assert!(xml_16_9.contains("<name>Vedal &amp; Neuro &lt;Special&gt; Clip</name>"));
        assert!(xml_16_9.contains(
            "<comment>Trái tim &lt;3 Vedal &amp; Evil &quot;đỉnh nóc&quot; &apos;kịch trần&apos;</comment>"
        ));
        assert!(xml_16_9.contains("<comment>Fallback English line</comment>"));
        assert!(xml_16_9.contains("<comment>Whitespace Vi fallback</comment>"));
        assert!(xml_16_9.contains("<in>60</in>"));
        assert!(xml_16_9.contains("<out>210</out>"));

        // 9:16 layouts -> 1080x1920
        let proj_crop = make_project("preset_center_crop_9_16", Some(r"D:\clip.mp4"));
        let xml_crop = generate_premiere_xml(&proj_crop, None).unwrap();
        assert!(xml_crop.contains("<width>1080</width>"));
        assert!(xml_crop.contains("<height>1920</height>"));

        let proj_stack = make_project("preset_two_stack_9_16", Some(r"D:\clip.mp4"));
        let xml_stack = generate_premiere_xml(&proj_stack, None).unwrap();
        assert!(xml_stack.contains("<width>1080</width>"));
        assert!(xml_stack.contains("<height>1920</height>"));
    }

    #[test]
    fn test_invalid_layout_returns_err_export_invalid_layout() {
        let proj = make_project("square_1_1_invalid", None);
        let err = generate_premiere_xml(&proj, None).unwrap_err();
        assert_eq!(err.code(), "ERR_EXPORT_INVALID_LAYOUT");
    }

    #[test]
    fn test_escape_xml_text_and_control_char_stripping() {
        let raw = "A & B < C > D \"E\" 'F'\u{0000}\u{0007}\u{001F}\nLine2\tTab";
        let escaped = escape_xml_text(raw);
        assert_eq!(
            escaped,
            "A &amp; B &lt; C &gt; D &quot;E&quot; &apos;F&apos;\nLine2\tTab"
        );
    }

    #[test]
    fn test_path_to_file_url_windows_and_posix() {
        assert_eq!(
            path_to_file_url(r"C:\Users\Taine\My Clip #1 (Việt).mp4"),
            "file://localhost/C:/Users/Taine/My%20Clip%20%231%20(Vi%E1%BB%87t).mp4"
        );
        assert_eq!(
            path_to_file_url(r"\\?\D:\Neuro\slice_01.mp4"),
            "file://localhost/D:/Neuro/slice_01.mp4"
        );
        assert_eq!(
            path_to_file_url("/mnt/data/my clip.mp4"),
            "file://localhost/mnt/data/my%20clip.mp4"
        );
        assert_eq!(
            path_to_file_url("file://localhost/C:/already/encoded.mp4"),
            "file://localhost/C:/already/encoded.mp4"
        );
        assert_eq!(path_to_file_url("   "), "");
    }

    #[test]
    fn test_path_to_file_url_relative_path_normalization() {
        let url_dot = path_to_file_url("./clips/clip_1.mp4");
        assert!(
            url_dot.starts_with("file://localhost/"),
            "URL must start with file://localhost/, got: {url_dot}"
        );
        assert!(
            !url_dot.contains("/./") && !url_dot.contains("/."),
            "URL must not contain relative dot segments, got: {url_dot}"
        );
        assert!(
            url_dot.ends_with("/clips/clip_1.mp4"),
            "URL must terminate with resolved filename, got: {url_dot}"
        );

        let url_dotdot = path_to_file_url(r"..\test\clip.mp4");
        assert!(
            url_dotdot.starts_with("file://localhost/"),
            "URL must start with file://localhost/, got: {url_dotdot}"
        );
        assert!(
            !url_dotdot.contains("/../") && !url_dotdot.contains(".."),
            "URL must not contain parent dotdot segments, got: {url_dotdot}"
        );
        assert!(
            url_dotdot.ends_with("/test/clip.mp4"),
            "URL must terminate with resolved filename, got: {url_dotdot}"
        );
    }

    #[test]
    fn test_sec_to_frames_safety() {
        assert_eq!(sec_to_frames(0.0, 60), 0);
        assert_eq!(sec_to_frames(1.5, 60), 90);
        assert_eq!(sec_to_frames(-5.0, 60), 0);
        assert_eq!(sec_to_frames(f64::NAN, 60), 0);
        assert_eq!(sec_to_frames(f64::INFINITY, 60), 0);
        assert_eq!(sec_to_frames(10.0, 0), 0);
    }

    #[test]
    fn test_multi_clip_and_subtitle_only_fallback() {
        let proj = make_project("original_16_9", None);

        // Subtitle-only fallback (no video path, no clips)
        let xml_no_video = generate_premiere_xml(&proj, None).unwrap();
        assert!(xml_no_video.contains("<track>\n        </track>"));
        assert_eq!(xml_no_video.matches("<marker>").count(), 3);

        // Multi-clip assembly (including duplicate source path reuse)
        let options = PremiereXmlExportOptions {
            video_file_path: None,
            clips: Some(vec![
                PremiereClipInput {
                    clip_id: Some("c1".to_string()),
                    name: Some("Part 1".to_string()),
                    file_path: r"C:\Clips\part1.mp4".to_string(),
                    start_sec: None,
                    end_sec: None,
                    duration_sec: Some(4.0),
                },
                PremiereClipInput {
                    clip_id: Some("c2".to_string()),
                    name: Some("Part 1 Repeat".to_string()),
                    file_path: r"C:\Clips\part1.mp4".to_string(),
                    start_sec: None,
                    end_sec: None,
                    duration_sec: Some(5.0),
                },
            ]),
            fps: None,
        };

        let built = build_premiere_xml(&proj, Some(&options)).unwrap();
        assert_eq!(built.clip_count, 2);
        assert_eq!(built.marker_count, 3);
        assert_eq!(built.xml.matches("<file id=\"file-1\">").count(), 1);
        // 1 in second video clipitem + 2 in audio clipitems = 3 self-closing refs
        assert_eq!(built.xml.matches("<file id=\"file-1\"/>").count(), 3);
        assert!(built.xml.contains("<start>240</start>"));
        assert!(built.xml.contains("<end>540</end>"));
    }

    #[test]
    fn test_ed_26_1_performance_sla_300_segments_under_100ms() {
        let mut segments = Vec::with_capacity(300);
        for i in 0..300 {
            let start = f64::from(i) * 1.5;
            segments.push(make_segment(
                &format!("seg_{i}"),
                start,
                start + 1.2,
                if i % 2 == 0 { "neuro" } else { "vedal" },
                &format!("English line {i}"),
                Some(&format!(
                    "Phụ đề tiếng Việt số {i} có dấu ể ễ ợ ứ đ & <tag>"
                )),
            ));
        }

        let container = TranscriptSegmentsContainer {
            clip_id: "clip_perf_300".to_string(),
            segments,
        };
        let mut proj = create_subtitle_project(
            container,
            "Perf 300 Project",
            "vod_perf",
            Some("original_16_9"),
        );
        proj.video_file_path = Some(r"C:\Videos\highlight_300.mp4".to_string());

        let temp_dir = std::env::temp_dir().join(format!(
            "neuroclip_premiere_perf_{}",
            Utc::now().timestamp_nanos_opt().unwrap_or(0)
        ));
        let output_path = temp_dir.join("project_timeline.xml");

        let started = Instant::now();
        let result = export_premiere_xml_file(&proj, &output_path, None).unwrap();
        let elapsed = started.elapsed();

        assert!(
            elapsed.as_millis() <= 100,
            "ED-26.1 SLA violated: XML export took {}ms (limit: 100ms)",
            elapsed.as_millis()
        );
        assert_eq!(result.marker_count, 300);
        assert_eq!(result.clip_count, 1);
        assert_eq!(result.fps, 60);

        let raw_bytes = std::fs::read(&output_path).unwrap();
        // Ensure no UTF-8 BOM
        assert!(!raw_bytes.starts_with(&[0xEF, 0xBB, 0xBF]));
        assert!(raw_bytes.starts_with(b"<?xml"));

        let _ = std::fs::remove_dir_all(temp_dir);
    }

    #[test]
    fn test_path_to_file_url_unc_and_pre_prefixed_urls() {
        assert_eq!(
            path_to_file_url(r"\\server\share\clip.mp4"),
            "file://server/share/clip.mp4"
        );
        assert_eq!(
            path_to_file_url(r"\\?\UNC\server\my share\clip #1.mp4"),
            "file://server/my%20share/clip%20%231.mp4"
        );
        assert_eq!(
            path_to_file_url(r"\\.\UNC\nas01\neuro_clips\part 2.mp4"),
            "file://nas01/neuro_clips/part%202.mp4"
        );
        assert_eq!(
            path_to_file_url("file://localhost/C:/my clip %20 100% #1.mp4"),
            "file://localhost/C:/my%20clip%20%20%20100%25%20%231.mp4"
        );
    }

    #[test]
    fn test_sequence_duration_covers_inverted_and_max_clamped_markers() {
        let container_inverted = TranscriptSegmentsContainer {
            clip_id: "clip_inverted".to_string(),
            segments: vec![
                make_segment("s1", 0.0, 5.0, "neuro", "Normal", None),
                make_segment("s2", 10.0, 2.0, "vedal", "Inverted", None),
                make_segment("s3", 5.0, 5.0, "evil", "Zero len", None),
            ],
        };
        let proj_inv = create_subtitle_project(
            container_inverted,
            "Inverted Markers",
            "vod_inv",
            Some("original_16_9"),
        );
        let xml_inv = generate_premiere_xml(&proj_inv, None).unwrap();
        assert!(xml_inv.contains("<duration>601</duration>"));
        assert!(xml_inv.contains("<in>600</in>\n      <out>601</out>"));

        let container_max = TranscriptSegmentsContainer {
            clip_id: "clip_max".to_string(),
            segments: vec![make_segment(
                "s_max",
                9_000_000.0,
                9_000_005.0,
                "neuro",
                "Over max",
                None,
            )],
        };
        let proj_max = create_subtitle_project(
            container_max,
            "Over Max Markers",
            "vod_max",
            Some("original_16_9"),
        );
        let xml_max = generate_premiere_xml(&proj_max, None).unwrap();
        assert!(xml_max.contains("<duration>518400001</duration>"));
        assert!(xml_max.contains("<in>518400000</in>\n      <out>518400001</out>"));
    }

    #[test]
    fn test_multi_clip_mixed_slash_case_dedup_and_max_file_duration() {
        let proj = make_project("original_16_9", None);
        let options = PremiereXmlExportOptions {
            video_file_path: None,
            clips: Some(vec![
                PremiereClipInput {
                    clip_id: Some("c1".to_string()),
                    name: Some("Short".to_string()),
                    file_path: r"C:\Clips\part1.mp4".to_string(),
                    start_sec: Some(0.0),
                    end_sec: Some(4.0),
                    duration_sec: None,
                },
                PremiereClipInput {
                    clip_id: Some("c2".to_string()),
                    name: Some("Longer".to_string()),
                    file_path: "c:/clips/PART1.mp4".to_string(),
                    start_sec: Some(4.0),
                    end_sec: Some(14.0),
                    duration_sec: None,
                },
            ]),
            fps: None,
        };

        let xml = generate_premiere_xml(&proj, Some(&options)).unwrap();
        assert_eq!(xml.matches("<file id=\"file-1\">").count(), 1);
        assert_eq!(xml.matches("<file id=\"file-2\">").count(), 0);
        assert_eq!(xml.matches("<file id=\"file-1\"/>").count(), 3);
        // Master <file id="file-1"> must carry max duration (10s * 60fps = 600), not 240
        assert!(xml.contains("              <duration>600</duration>\n              <media>"));
    }

    #[test]
    fn test_marker_name_strips_control_chars_before_truncation() {
        let payload = format!("{}VISIBLE_AFTER_60_NULS", "\u{0000}".repeat(60));
        let container = TranscriptSegmentsContainer {
            clip_id: "clip_ctrl".to_string(),
            segments: vec![make_segment(
                "s_ctrl",
                1.0,
                2.0,
                "vedal",
                "fallback",
                Some(&payload),
            )],
        };
        let proj = create_subtitle_project(
            container,
            "Control Chars",
            "vod_ctrl",
            Some("original_16_9"),
        );
        let xml = generate_premiere_xml(&proj, None).unwrap();
        assert!(xml.contains("<name>[VEDAL] VISIBLE_AFTER_60_NULS</name>"));
        assert!(xml.contains("<comment>VISIBLE_AFTER_60_NULS</comment>"));
    }

    #[test]
    fn test_resolve_timeline_clips_clamps_non_positive_end_range_at_explicit_start() {
        let container = TranscriptSegmentsContainer {
            clip_id: "clip_range_clamp".to_string(),
            segments: vec![make_segment("s1", 0.0, 2.0, "neuro", "hi", None)],
        };
        let proj =
            create_subtitle_project(container, "Range Clamp", "vod_clamp", Some("original_16_9"));
        let options = PremiereXmlExportOptions {
            video_file_path: None,
            clips: Some(vec![
                PremiereClipInput {
                    clip_id: Some("c1".to_string()),
                    name: Some("First".to_string()),
                    file_path: "C:/clips/first.mp4".to_string(),
                    start_sec: Some(0.0),
                    end_sec: Some(2.0),
                    duration_sec: None,
                },
                PremiereClipInput {
                    clip_id: Some("c2".to_string()),
                    name: Some("Inverted".to_string()),
                    file_path: "C:/clips/second.mp4".to_string(),
                    start_sec: Some(5.0),
                    end_sec: Some(3.0),
                    duration_sec: None,
                },
            ]),
            fps: Some(60),
        };

        let xml = generate_premiere_xml(&proj, Some(&options)).unwrap();
        // Second clip must start at 5.0s (300 frames) and clamp to 1 frame (end=301), not cursor_frames (120)
        assert!(xml.contains("<start>300</start>\n            <end>301</end>"));
    }
}
