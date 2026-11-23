//! Tauri IPC commands for subtitle export (.ass, .srt), Premiere Pro FCP7 XML,
//! CapCut PC Draft generation, and NLE export progress streaming (`export:progress`).

use serde::{Deserialize, Serialize};
use std::path::Path;
use std::time::Instant;
use tauri::{command, Emitter};

use crate::core::storage::validate_user_export_path;
use crate::error::AppError;
use crate::export::capcut_schema::canvas_config_for_layout;
use crate::export::premiere_xml::{build_premiere_xml, write_premiere_xml_bytes};
use crate::export::{
    export_ass_file, export_ass_project_file, export_capcut_draft_folder, export_srt_file,
    export_srt_project_file, export_subtitle_project_json, CapCutExportResult,
    PremiereExportResult, PremiereXmlExportOptions, SubtitleProject,
};
use crate::stt::transcript_parser::TranscriptSegmentsContainer;

/// Canonical Tauri event name for NLE export progress updates (`CONVENTIONS.md` §4.2).
pub const EXPORT_PROGRESS_EVENT: &str = "export:progress";

/// Real-time NLE export progress payload emitted on `export:progress`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportProgressPayload {
    pub project_id: String,
    pub percent: f64,
    pub current_step: String,
}

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
    tokio::task::spawn_blocking(move || {
        let path = Path::new(&output_path);
        validate_user_export_path(path)?;
        export_ass_file(&container, path)?;
        Ok(output_path)
    })
    .await
    .map_err(|e| AppError::Internal(format!("ASS export task panicked: {e}")))?
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
    tokio::task::spawn_blocking(move || {
        let path = Path::new(&output_path);
        validate_user_export_path(path)?;
        export_srt_file(&container, path)?;
        Ok(output_path)
    })
    .await
    .map_err(|e| AppError::Internal(format!("SRT export task panicked: {e}")))?
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
    tokio::task::spawn_blocking(move || {
        let path = Path::new(&output_path);
        validate_user_export_path(path)?;
        export_subtitle_project_json(&project, path)?;
        Ok(output_path)
    })
    .await
    .map_err(|e| AppError::Internal(format!("Subtitle project export task panicked: {e}")))?
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
    tokio::task::spawn_blocking(move || {
        let path = Path::new(&output_path);
        validate_user_export_path(path)?;
        export_ass_project_file(&project, path)?;
        Ok(output_path)
    })
    .await
    .map_err(|e| AppError::Internal(format!("ASS project export task panicked: {e}")))?
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
    tokio::task::spawn_blocking(move || {
        let path = Path::new(&output_path);
        validate_user_export_path(path)?;
        export_srt_project_file(&project, path)?;
        Ok(output_path)
    })
    .await
    .map_err(|e| AppError::Internal(format!("SRT project export task panicked: {e}")))?
}

/// Execute CapCut PC Draft export while streaming monotonic progress steps via `on_progress`.
///
/// # Errors
/// Returns [`AppError::Export`] with canonical error codes (`ERR_EXPORT_CAPCUT_NOT_FOUND`,
/// `ERR_EXPORT_INVALID_LAYOUT`, `ERR_EXPORT_FILE_WRITE_FAILED`).
pub fn execute_capcut_draft_export_with_progress<F>(
    project: &SubtitleProject,
    options: Option<&CapCutExportOptions>,
    mut on_progress: F,
) -> Result<CapCutExportResult, AppError>
where
    F: FnMut(ExportProgressPayload),
{
    on_progress(ExportProgressPayload {
        project_id: project.project_id.clone(),
        percent: 10.0,
        current_step: "Đang kiểm tra bố cục và thư mục CapCut Draft...".to_string(),
    });

    canvas_config_for_layout(&project.target_layout)?;

    on_progress(ExportProgressPayload {
        project_id: project.project_id.clone(),
        percent: 45.0,
        current_step: "Đang tạo dự án CapCut Draft & sao lưu kép (.ass, .srt)...".to_string(),
    });

    let (v_path, custom_root) = match options {
        Some(opt) => (
            opt.video_file_path
                .as_deref()
                .or(project.video_file_path.as_deref()),
            opt.custom_draft_root.as_deref().map(Path::new),
        ),
        None => (project.video_file_path.as_deref(), None),
    };

    if let Some(root) = custom_root {
        validate_user_export_path(root)?;
    }

    let result = export_capcut_draft_folder(project, v_path, custom_root)?;

    on_progress(ExportProgressPayload {
        project_id: project.project_id.clone(),
        percent: 85.0,
        current_step: "Đang hoàn tất tệp metadata CapCut Draft...".to_string(),
    });

    on_progress(ExportProgressPayload {
        project_id: project.project_id.clone(),
        percent: 100.0,
        current_step: "Hoàn tất xuất dự án CapCut PC Draft.".to_string(),
    });

    Ok(result)
}

/// Export full CapCut PC Draft project folder with Dual Export Safeguard (subtitle.ass & subtitle.srt).
///
/// Executes on a background blocking thread (`tokio::task::spawn_blocking`) to keep async loop responsive
/// and streams `export:progress` events to the frontend.
///
/// # Errors
/// Returns [`AppError::Export`] with canonical error codes (`ERR_EXPORT_CAPCUT_NOT_FOUND`, `ERR_EXPORT_INVALID_LAYOUT`, `ERR_EXPORT_FILE_WRITE_FAILED`).
#[command]
pub async fn export_capcut_draft(
    project: SubtitleProject,
    options: Option<CapCutExportOptions>,
    app: tauri::AppHandle,
) -> Result<CapCutExportResult, AppError> {
    tokio::task::spawn_blocking(move || {
        execute_capcut_draft_export_with_progress(&project, options.as_ref(), |payload| {
            let _ = app.emit(EXPORT_PROGRESS_EVENT, payload);
        })
    })
    .await
    .map_err(|e| AppError::Internal(format!("CapCut export task panicked: {e}")))?
}

/// Execute Premiere Pro FCP7 XML (`<xmeml version="4">`) export while streaming monotonic progress steps.
///
/// # Errors
/// Returns [`AppError::Export`] with [`crate::error::ExportError::InvalidLayout`] or
/// [`crate::error::ExportError::FileWriteFailed`].
pub fn execute_premiere_xml_export_with_progress<F>(
    project: &SubtitleProject,
    output_path: &Path,
    options: Option<&PremiereXmlExportOptions>,
    mut on_progress: F,
) -> Result<PremiereExportResult, AppError>
where
    F: FnMut(ExportProgressPayload),
{
    let start_time = Instant::now();

    on_progress(ExportProgressPayload {
        project_id: project.project_id.clone(),
        percent: 10.0,
        current_step: "Đang xác thực bố cục và thông số Premiere XML...".to_string(),
    });

    validate_user_export_path(output_path)?;
    canvas_config_for_layout(&project.target_layout)?;

    on_progress(ExportProgressPayload {
        project_id: project.project_id.clone(),
        percent: 45.0,
        current_step: "Đang dựng timeline FCP7 XML (<xmeml version=\"4\">)...".to_string(),
    });

    let built = build_premiere_xml(project, options)?;

    on_progress(ExportProgressPayload {
        project_id: project.project_id.clone(),
        percent: 80.0,
        current_step: "Đang ghi tệp project_timeline.xml ra ổ đĩa...".to_string(),
    });

    write_premiere_xml_bytes(output_path, &built.xml)?;

    #[allow(clippy::cast_possible_truncation)]
    let duration_ms = start_time.elapsed().as_millis() as u64;

    on_progress(ExportProgressPayload {
        project_id: project.project_id.clone(),
        percent: 100.0,
        current_step: "Hoàn tất xuất Premiere Pro XML.".to_string(),
    });

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

/// Export a [`SubtitleProject`] as an FCP7 XML (`<xmeml version="4">`) timeline compatible with
/// Adobe Premiere Pro 2022–2025 and DaVinci Resolve 18–19, streaming `export:progress` events.
///
/// # Errors
/// Returns [`AppError::Export`] with [`crate::error::ExportError::InvalidLayout`] or
/// [`crate::error::ExportError::FileWriteFailed`].
#[command]
pub async fn export_premiere_xml(
    project: SubtitleProject,
    output_path: String,
    options: Option<PremiereXmlExportOptions>,
    app: tauri::AppHandle,
) -> Result<PremiereExportResult, AppError> {
    tokio::task::spawn_blocking(move || {
        let path = Path::new(&output_path);
        execute_premiere_xml_export_with_progress(&project, path, options.as_ref(), |payload| {
            let _ = app.emit(EXPORT_PROGRESS_EVENT, payload);
        })
    })
    .await
    .map_err(|e| AppError::Internal(format!("Premiere XML export task panicked: {e}")))?
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

    fn make_test_project() -> SubtitleProject {
        let container = TranscriptSegmentsContainer {
            clip_id: "clip_export_cmd_test".to_string(),
            segments: vec![TranscriptSegment {
                segment_id: "seg_01".to_string(),
                start_sec: 1.0,
                end_sec: 3.5,
                speaker_id: "neuro".to_string(),
                text_en: "Hello chat!".to_string(),
                text_vi: Some("Chào chat!".to_string()),
                confidence: 0.99,
                cps_score: 11.0,
                is_flagged_for_review: false,
                words: vec![],
            }],
        };
        let mut proj = create_subtitle_project(
            container,
            "Cmd Progress Test",
            "vod_cmd_test",
            Some("original_16_9"),
        );
        proj.video_file_path = Some(r"C:\Videos\sample.mp4".to_string());
        proj
    }

    #[test]
    fn test_it_26_2_export_premiere_xml_emits_monotonic_progress_and_camel_case_payload() {
        let project = make_test_project();
        let temp_dir = std::env::temp_dir().join(format!(
            "neuroclip_cmd_premiere_{}",
            Utc::now().timestamp_nanos_opt().unwrap_or(0)
        ));
        let xml_path = temp_dir.join("project_timeline.xml");

        let mut events = Vec::new();
        let result =
            execute_premiere_xml_export_with_progress(&project, &xml_path, None, |payload| {
                events.push(payload);
            })
            .unwrap();

        assert!(xml_path.exists());
        assert_eq!(result.width, 1920);
        assert_eq!(result.height, 1080);
        assert_eq!(result.fps, 60);
        assert_eq!(result.marker_count, 1);
        assert_eq!(result.clip_count, 1);

        assert_eq!(events.len(), 4);
        assert!((events[0].percent - 10.0).abs() < f64::EPSILON);
        assert!((events[1].percent - 45.0).abs() < f64::EPSILON);
        assert!((events[2].percent - 80.0).abs() < f64::EPSILON);
        assert!((events[3].percent - 100.0).abs() < f64::EPSILON);
        for ev in &events {
            assert_eq!(ev.project_id, project.project_id);
            assert_ne!(ev.current_step, "");
        }

        // Verify wire JSON uses exact camelCase keys required by CONVENTIONS.md §4.2
        let json = serde_json::to_value(&events[1]).unwrap();
        assert_eq!(json["projectId"], project.project_id);
        assert_eq!(json["percent"], 45.0);
        assert!(json["currentStep"].is_string());
        assert!(json.get("project_id").is_none());
        assert!(json.get("current_step").is_none());

        let _ = std::fs::remove_dir_all(temp_dir);
    }

    #[test]
    fn test_it_26_2_export_capcut_draft_emits_monotonic_progress() {
        let project = make_test_project();
        let temp_dir = std::env::temp_dir().join(format!(
            "neuroclip_cmd_capcut_{}",
            Utc::now().timestamp_nanos_opt().unwrap_or(0)
        ));
        std::fs::create_dir_all(&temp_dir).unwrap();

        let options = CapCutExportOptions {
            video_file_path: Some(r"C:\Videos\sample.mp4".to_string()),
            custom_draft_root: Some(temp_dir.display().to_string()),
        };

        let mut events = Vec::new();
        let result =
            execute_capcut_draft_export_with_progress(&project, Some(&options), |payload| {
                events.push(payload);
            })
            .unwrap();

        assert!(Path::new(&result.draft_content_path).exists());
        assert_eq!(events.len(), 4);
        assert!((events[0].percent - 10.0).abs() < f64::EPSILON);
        assert!((events[1].percent - 45.0).abs() < f64::EPSILON);
        assert!((events[2].percent - 85.0).abs() < f64::EPSILON);
        assert!((events[3].percent - 100.0).abs() < f64::EPSILON);

        let _ = std::fs::remove_dir_all(temp_dir);
    }

    #[test]
    fn test_execute_premiere_xml_invalid_layout_fails_before_45_percent() {
        let mut project = make_test_project();
        project.target_layout = "invalid_layout_1_1".to_string();
        let temp_dir = std::env::temp_dir().join(format!(
            "neuroclip_cmd_invalid_layout_{}",
            Utc::now().timestamp_nanos_opt().unwrap_or(0)
        ));
        let xml_path = temp_dir.join("should_not_exist.xml");

        let mut events = Vec::new();
        let err = execute_premiere_xml_export_with_progress(&project, &xml_path, None, |payload| {
            events.push(payload);
        })
        .unwrap_err();

        assert_eq!(err.code(), "ERR_EXPORT_INVALID_LAYOUT");
        assert_eq!(events.len(), 1);
        assert!((events[0].percent - 10.0).abs() < f64::EPSILON);
        assert!(!xml_path.exists());
    }

    #[test]
    fn test_export_rejects_traversal_and_system_paths() {
        let project = make_test_project();
        let err = execute_premiere_xml_export_with_progress(
            &project,
            Path::new("../escape/timeline.xml"),
            None,
            |_| {},
        )
        .unwrap_err();
        assert_eq!(err.code(), "ERR_EXPORT_FILE_WRITE_FAILED");

        #[cfg(windows)]
        {
            let sys_err = execute_premiere_xml_export_with_progress(
                &project,
                Path::new(r"C:\Windows\System32\timeline.xml"),
                None,
                |_| {},
            )
            .unwrap_err();
            assert_eq!(sys_err.code(), "ERR_EXPORT_FILE_WRITE_FAILED");
        }
    }
}
