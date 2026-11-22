//! Subtitle export facade for backward compatibility.
//!
//! Re-exports all subtitle export types and functions from [`crate::export`].
//! Schema 5.6: pub struct SubtitleProject and pub struct SpeakerStyle.

pub use crate::export::{
    create_subtitle_project, default_speaker_styles, escape_ass_text, export_ass_file,
    export_ass_project_file, export_srt_file, export_srt_project_file,
    export_subtitle_project_json, format_ass_timecode, format_srt_timecode, generate_ass_script,
    generate_ass_script_from_project, generate_srt_script, generate_srt_script_from_project,
    generate_srt_script_from_segments, hex_to_ass_bgr, load_subtitle_project_json,
    map_speaker_to_ass_style, write_file_with_bom, SpeakerStyle, SubtitleProject, UTF8_BOM,
    UTF8_BOM_BYTES,
};
