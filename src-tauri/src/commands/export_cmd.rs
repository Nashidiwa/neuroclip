//! Tauri IPC commands for subtitle export (.ass, .srt) and project metadata persistence.

use std::path::Path;
use tauri::command;

use crate::error::AppError;
use crate::export::{
    export_ass_file, export_ass_project_file, export_srt_file, export_srt_project_file,
    export_subtitle_project_json, SubtitleProject,
};
use crate::stt::transcript_parser::TranscriptSegmentsContainer;

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
