//! Tauri IPC commands for subtitle export (.ass, .srt) and project metadata persistence.

use std::path::Path;
use serde::{Deserialize, Serialize};
use tauri::command;

use crate::error::AppError;
use crate::export::{
    export_ass_file, export_ass_project_file, export_capcut_draft_folder, export_srt_file,
    export_srt_project_file, export_subtitle_project_json, CapCutExportResult, SubtitleProject,
};
use crate::stt::transcript_parser::TranscriptSegmentsContainer;

/// Options for CapCut PC Draft export command.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct CapCutExportOptions {
    pub video_file_path: Option<String>,
    pub custom_draft_root: Option<String>,
}

/// Export transcript segments into an Advanced SubStation Alpha (`.ass`) file with UTF-8 BOM.
///
/// # Errors
/// Returns [`AppError::Export`] if creating parent directories or writing the `.ass` file fails.
#[command]
pub async fn export_subtitles_ass(
    container: TranscriptSegmentsContainer,
    output_path: String,
) -> Result<String, AppError> {
    let path = Path::new(&output_path);
    export_ass_file(&container, path)?;
    Ok(output_path)
}

/// Export transcript segments into a standard SubRip (`.srt`) subtitle file with UTF-8 BOM.
///
/// # Errors
/// Returns [`AppError::Export`] if creating parent directories or writing the `.srt` file fails.
#[command]
pub async fn export_subtitles_srt(
    container: TranscriptSegmentsContainer,
    output_path: String,
) -> Result<String, AppError> {
    let path = Path::new(&output_path);
    export_srt_file(&container, path)?;
    Ok(output_path)
}

/// Export and serialize a full [`SubtitleProject`] adhering to Schema 5.6 JSON format.
///
/// # Errors
/// Returns [`AppError::Export`] if serialization or file write fails.
#[command]
pub async fn export_subtitle_project(
    project: SubtitleProject,
    output_path: String,
) -> Result<String, AppError> {
    let path = Path::new(&output_path);
    export_subtitle_project_json(&project, path)?;
    Ok(output_path)
}

/// Export an existing [`SubtitleProject`] directly into an Advanced SubStation Alpha (`.ass`) file with UTF-8 BOM.
///
/// # Errors
/// Returns [`AppError::Export`] if directory creation or file write fails.
#[command]
pub async fn export_subtitles_project_ass(
    project: SubtitleProject,
    output_path: String,
) -> Result<String, AppError> {
    let path = Path::new(&output_path);
    export_ass_project_file(&project, path)?;
    Ok(output_path)
}

/// Export an existing [`SubtitleProject`] directly into a SubRip (`.srt`) file with UTF-8 BOM.
///
/// # Errors
/// Returns [`AppError::Export`] if directory creation or file write fails.
#[command]
pub async fn export_subtitles_project_srt(
    project: SubtitleProject,
    output_path: String,
) -> Result<String, AppError> {
    let path = Path::new(&output_path);
    export_srt_project_file(&project, path)?;
    Ok(output_path)
}

/// Export full CapCut PC Draft project folder with Dual Export Safeguard (subtitle.ass & subtitle.srt).
///
/// Executes on a background blocking thread (`tokio::task::spawn_blocking`) to keep async loop responsive.
///
/// # Errors
/// Returns [`AppError::Export`] with canonical error codes (`ERR_EXPORT_CAPCUT_NOT_FOUND`, `ERR_EXPORT_INVALID_LAYOUT`, `ERR_EXPORT_FILE_WRITE_FAILED`).
#[command]
pub async fn export_capcut_draft(
    project: SubtitleProject,
    options: Option<CapCutExportOptions>,
) -> Result<CapCutExportResult, AppError> {
    tokio::task::spawn_blocking(move || {
        let (v_path, custom_root) = match &options {
            Some(opt) => (
                opt.video_file_path.as_deref().or(project.video_file_path.as_deref()),
                opt.custom_draft_root.as_deref().map(Path::new),
            ),
            None => (project.video_file_path.as_deref(), None),
        };
        export_capcut_draft_folder(&project, v_path, custom_root)
    })
    .await
    .map_err(|e| AppError::Internal(format!("CapCut export task panicked: {e}")))?
}
