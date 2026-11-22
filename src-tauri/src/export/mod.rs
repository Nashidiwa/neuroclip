//! Subtitle and NLE timeline export engine.
//!
//! Provides subtitle exporters for Aegisub Advanced SubStation Alpha (`.ass`) and
//! SubRip (`.srt`) with UTF-8 BOM encoding for 100% Vietnamese diacritic compatibility,
//! alongside Schema 5.6 [`SubtitleProject`] project serialization.

pub mod ass_exporter;
pub mod capcut_draft;
pub mod capcut_schema;
pub mod premiere_xml;
pub mod srt_exporter;

use std::collections::HashMap;
use std::io::Write;
use std::path::{Path, PathBuf};

use chrono::Utc;
use serde::{Deserialize, Serialize};

use crate::error::{AppError, ExportError};
use crate::stt::transcript_parser::{TranscriptSegment, TranscriptSegmentsContainer};

pub use ass_exporter::{
    escape_ass_text, export_ass_file, export_ass_project_file, format_ass_timecode,
    generate_ass_script, generate_ass_script_from_project, hex_to_ass_bgr,
    map_speaker_to_ass_style,
};
pub use capcut_draft::{
    default_capcut_drafts_dir, detect_capcut_drafts_dir, export_capcut_draft_folder,
    generate_project_folder_name, sanitize_project_name, update_root_meta_info, CapCutExportResult,
};
pub use capcut_schema::{
    canvas_config_for_layout, hex_to_normalized_rgb, sec_to_micros, CanvasConfig,
    DraftAudioMaterial, DraftContent, DraftMaterials, DraftMetaInfo, DraftMetaMaterialItem,
    DraftMetaMaterials, DraftSegment, DraftStoreItem, DraftTextMaterial, DraftTrack,
    DraftVideoMaterial, RootMetaInfo, TimeRange, DEFAULT_SUBTITLE_TRANSFORM_X,
    DEFAULT_SUBTITLE_TRANSFORM_Y, MICROS_PER_SEC, MIN_CLIP_DURATION_MICROS,
};
pub use premiere_xml::{
    escape_xml_text, export_premiere_xml_file, generate_premiere_xml, path_to_file_url,
    sec_to_frames, PremiereClipInput, PremiereExportResult, PremiereXmlExportOptions,
    DEFAULT_PREMIERE_FPS, PREMIERE_AUDIO_BIT_DEPTH, PREMIERE_AUDIO_CHANNELS,
    PREMIERE_AUDIO_SAMPLE_RATE,
};
pub use srt_exporter::{
    export_srt_file, export_srt_project_file, format_srt_timecode, generate_srt_script,
    generate_srt_script_from_project, generate_srt_script_from_segments,
};

/// UTF-8 Byte Order Mark string representation (`\u{FEFF}`).
pub const UTF8_BOM: &str = "\u{FEFF}";

/// Raw byte sequence of the UTF-8 Byte Order Mark (`[0xEF, 0xBB, 0xBF]`).
pub const UTF8_BOM_BYTES: &[u8; 3] = &[0xEF, 0xBB, 0xBF];

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

/// Write string content to disk prefixed with the UTF-8 BOM header (`[0xEF, 0xBB, 0xBF]`).
///
/// Ensures parent directories are created automatically if they do not exist.
///
/// # Errors
/// Returns [`AppError::Export`] with [`ExportError::FileWriteFailed`] on I/O failure.
pub fn write_file_with_bom(path: &Path, content: &str) -> Result<(), AppError> {
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() && !parent.exists() {
            std::fs::create_dir_all(parent).map_err(|e| {
                AppError::Export(ExportError::FileWriteFailed {
                    path: parent.display().to_string(),
                    details: e.to_string(),
                })
            })?;
        }
    }

    let mut file = std::fs::File::create(path).map_err(|e| {
        AppError::Export(ExportError::FileWriteFailed {
            path: path.display().to_string(),
            details: e.to_string(),
        })
    })?;

    file.write_all(UTF8_BOM_BYTES).map_err(|e| {
        AppError::Export(ExportError::FileWriteFailed {
            path: path.display().to_string(),
            details: e.to_string(),
        })
    })?;

    file.write_all(content.as_bytes()).map_err(|e| {
        AppError::Export(ExportError::FileWriteFailed {
            path: path.display().to_string(),
            details: e.to_string(),
        })
    })?;

    file.flush().map_err(|e| {
        AppError::Export(ExportError::FileWriteFailed {
            path: path.display().to_string(),
            details: e.to_string(),
        })
    })?;

    Ok(())
}

/// Build default speaker style dictionary for Schema 5.6 [`SubtitleProject`] (PO Option 1 Palette).
#[must_use]
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
            color_hex: "#FF0055".to_string(),
            font_name: "Arial".to_string(),
            font_size: 54,
            outline_color_hex: "#000000".to_string(),
        },
    );
    map.insert(
        "vedal".to_string(),
        SpeakerStyle {
            color_hex: "#98FB98".to_string(),
            font_name: "Arial".to_string(),
            font_size: 54,
            outline_color_hex: "#000000".to_string(),
        },
    );
    map.insert(
        "guest".to_string(),
        SpeakerStyle {
            color_hex: "#FFD700".to_string(),
            font_name: "Arial".to_string(),
            font_size: 54,
            outline_color_hex: "#000000".to_string(),
        },
    );
    map.insert(
        "chat".to_string(),
        SpeakerStyle {
            color_hex: "#87CEEB".to_string(),
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
#[must_use]
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

/// Export a [`SubtitleProject`] as formatted JSON adhering to Schema 5.6.
///
/// # Errors
/// Returns [`AppError::Export`] with [`ExportError::FileWriteFailed`] on I/O or serialization error.
pub fn export_subtitle_project_json(
    project: &SubtitleProject,
    output_path: &Path,
) -> Result<PathBuf, AppError> {
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

    let json_bytes = serde_json::to_vec_pretty(project).map_err(|e| {
        AppError::Export(ExportError::FileWriteFailed {
            path: output_path.display().to_string(),
            details: e.to_string(),
        })
    })?;

    std::fs::write(output_path, json_bytes).map_err(|e| {
        AppError::Export(ExportError::FileWriteFailed {
            path: output_path.display().to_string(),
            details: e.to_string(),
        })
    })?;

    Ok(output_path.to_path_buf())
}

/// Load and parse a [`SubtitleProject`] from disk JSON.
///
/// # Errors
/// Returns [`AppError::Storage`] if file is not found, or [`AppError::Internal`] if parsing fails.
pub fn load_subtitle_project_json(path: &Path) -> Result<SubtitleProject, AppError> {
    let data = std::fs::read_to_string(path).map_err(|_| {
        AppError::Storage(crate::error::StorageError::FileNotFound(
            path.display().to_string(),
        ))
    })?;

    let project: SubtitleProject = serde_json::from_str(&data)
        .map_err(|e| AppError::Internal(format!("Failed to parse subtitle project JSON: {e}")))?;

    Ok(project)
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::cast_precision_loss,
    clippy::cast_lossless
)]
mod tests {
    use super::*;
    use crate::stt::transcript_parser::{
        TranscriptSegment, TranscriptSegmentsContainer, WordTiming,
    };

    fn make_test_container() -> TranscriptSegmentsContainer {
        TranscriptSegmentsContainer {
            clip_id: "clip_m4_24_test".to_string(),
            segments: vec![TranscriptSegment {
                segment_id: "seg_1".to_string(),
                start_sec: 1.0,
                end_sec: 3.5,
                speaker_id: "neuro".to_string(),
                text_en: "Hello world".to_string(),
                text_vi: Some("Chào thế giới".to_string()),
                confidence: 0.98,
                cps_score: 12.0,
                is_flagged_for_review: false,
                words: vec![WordTiming {
                    word: "Hello".to_string(),
                    start_sec: 1.0,
                    end_sec: 2.0,
                    probability: 0.99,
                }],
            }],
        }
    }

    #[test]
    fn test_it_24_1_schema_5_6_subtitle_project_serialization() {
        let container = make_test_container();
        let project = create_subtitle_project(
            container,
            "Neuro Clip Project",
            "vod_test_999",
            Some("preset_center_crop_9_16"),
        );

        let json_str = serde_json::to_string(&project).unwrap();
        assert!(json_str.contains("\"project_id\":\"subproj_clip_m4_24_test\""));
        assert!(json_str.contains("\"project_name\":\"Neuro Clip Project\""));
        assert!(json_str.contains("\"vod_id\":\"vod_test_999\""));
        assert!(json_str.contains("\"target_layout\":\"preset_center_crop_9_16\""));
        assert!(json_str.contains("\"speaker_styles\""));
        assert!(json_str.contains("\"updated_at\""));

        let deserialized: SubtitleProject = serde_json::from_str(&json_str).unwrap();
        assert_eq!(deserialized.project_id, project.project_id);
        assert_eq!(deserialized.segments.len(), 1);
        assert_eq!(deserialized.speaker_styles.len(), 6);
        assert_eq!(deserialized.speaker_styles["neuro"].color_hex, "#FFB6C1");
    }

    #[test]
    fn test_export_and_load_subtitle_project_json() {
        let container = make_test_container();
        let project = create_subtitle_project(container, "Disk Project", "vod_disk_123", None);

        let temp_dir = std::env::temp_dir().join(format!(
            "neuroclip_test_{}",
            Utc::now().timestamp_nanos_opt().unwrap_or(0)
        ));
        let output_path = temp_dir.join("subproject_test.json");

        let exported_path = export_subtitle_project_json(&project, &output_path).unwrap();
        assert_eq!(exported_path, output_path);
        assert!(output_path.exists());

        let loaded = load_subtitle_project_json(&output_path).unwrap();
        assert_eq!(loaded.project_id, project.project_id);
        assert_eq!(loaded.project_name, "Disk Project");
        assert_eq!(loaded.target_layout, "original_16_9");

        let _ = std::fs::remove_dir_all(temp_dir);
    }
}
