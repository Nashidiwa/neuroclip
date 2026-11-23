//! Tauri IPC commands for Lossless Range Slicing.
//!
//! Complies with `docs/CONVENTIONS.md` (zero-panic invariant, Result<T, AppError>),
//! `docs/ARCHITECTURE.md` §4.2, `docs/adr/ADR-003`, and `docs/TASK_BOARD.md` TASK-M4-13.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use tauri::Emitter;
use tokio_util::sync::CancellationToken;

use crate::core::storage::{
    resolve_vod_project_dir as core_resolve_vod_project_dir, validate_vod_id,
};
use crate::error::{AppError, IngestError, StorageError};
use crate::ingestion::cookies::CookieSource;
use crate::ingestion::range_slicer::{execute_range_slice, RangeSliceRequest, SliceResult};
use crate::ingestion::streamer::{resolve_video_stream_url, VodManifest};

static ACTIVE_SLICE_TASKS: Mutex<Option<HashMap<String, CancellationToken>>> = Mutex::new(None);

/// Register an active cancellation token for a slice task.
pub fn register_slice_task_token(task_id: &str, token: CancellationToken) {
    if let Ok(mut lock) = ACTIVE_SLICE_TASKS.lock() {
        let map = lock.get_or_insert_with(HashMap::new);
        map.insert(task_id.to_string(), token);
    }
}

/// Unregister a slice cancellation token when the task finishes.
pub fn unregister_slice_task_token(task_id: &str) {
    if let Ok(mut lock) = ACTIVE_SLICE_TASKS.lock() {
        if let Some(map) = lock.as_mut() {
            map.remove(task_id);
        }
    }
}

/// Cancel a running slice task by ID (or the single active task if task_id is empty).
pub fn cancel_slice_task_token(task_id: &str) -> bool {
    if let Ok(mut lock) = ACTIVE_SLICE_TASKS.lock() {
        if let Some(map) = lock.as_mut() {
            let key = task_id.trim();
            if !key.is_empty() {
                if let Some(token) = map.remove(key) {
                    token.cancel();
                    return true;
                }
            } else if map.len() == 1 {
                if let Some(first_key) = map.keys().next().cloned() {
                    if let Some(token) = map.remove(&first_key) {
                        token.cancel();
                        return true;
                    }
                }
            }
        }
    }
    false
}

/// Resolve and validate the project directory for a given VOD ID (`projects/<vod_id>` or `cache/<vod_id>`).
///
/// # Errors
/// Returns [`AppError::Storage`] if `vod_id` fails path-traversal validation.
pub fn resolve_vod_project_dir(vod_id: &str) -> Result<PathBuf, AppError> {
    core_resolve_vod_project_dir(vod_id)
}

#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
fn resolve_clip_output_path(
    clips_dir: &Path,
    start_sec: f64,
    end_sec: f64,
    cluster_id: Option<&str>,
    output_filename: Option<&str>,
) -> PathBuf {
    let fallback_name = || {
        let safe_cluster = cluster_id
            .and_then(|cid| Path::new(cid).file_name())
            .and_then(|f| f.to_str())
            .map(str::trim)
            .filter(|s| !s.is_empty() && !s.contains(".."));
        match safe_cluster {
            Some(cid) => format!("clip_{cid}.mp4"),
            None => format!("clip_{}_{}.mp4", start_sec as u64, end_sec as u64),
        }
    };

    match output_filename {
        Some(name) => {
            // Guard against path traversal by only taking the filename component
            let clean_name = Path::new(name)
                .file_name()
                .and_then(|f| f.to_str())
                .filter(|s| !s.trim().is_empty());
            match clean_name {
                Some(file_name) => clips_dir.join(file_name),
                None => clips_dir.join(fallback_name()),
            }
        }
        None => clips_dir.join(fallback_name()),
    }
}

struct SliceTarget<'a> {
    project_dir: &'a Path,
    vod_id: &'a str,
}

/// Internal slice video segment execution logic targeting an explicit `project_dir`.
#[allow(clippy::too_many_lines)]
async fn slice_video_segment_in_dir(
    target: SliceTarget<'_>,
    start_sec: f64,
    end_sec: f64,
    cluster_id: Option<&str>,
    output_filename: Option<&str>,
    custom_input_source: Option<&str>,
    app: Option<&tauri::AppHandle>,
) -> Result<SliceResult, AppError> {
    let project_dir = target.project_dir;
    let vod_id = target.vod_id;
    let clips_dir = project_dir.join("clips");
    let manifest_path = project_dir.join("vod_manifest.json");
    let has_custom_input = custom_input_source.is_some();

    let clips_dir_for_prep = clips_dir.clone();
    let manifest_path_for_prep = manifest_path.clone();
    let disk_manifest = tokio::task::spawn_blocking(move || {
        std::fs::create_dir_all(&clips_dir_for_prep).map_err(|e| {
            AppError::Storage(StorageError::DirectoryCreationFailed {
                path: clips_dir_for_prep.display().to_string(),
                details: e.to_string(),
            })
        })?;

        if !has_custom_input && manifest_path_for_prep.exists() {
            let bytes = std::fs::read(&manifest_path_for_prep).map_err(|e| {
                AppError::Storage(StorageError::FileNotFound(format!(
                    "Không thể đọc vod_manifest.json tại '{}': {e}",
                    manifest_path_for_prep.display()
                )))
            })?;

            let manifest: VodManifest = serde_json::from_slice(&bytes).map_err(|e| {
                AppError::Ingestion(IngestError::ExtractionFailed(format!(
                    "Lỗi phân giải vod_manifest.json: {e}"
                )))
            })?;
            Ok::<_, AppError>(Some(manifest))
        } else {
            Ok::<_, AppError>(None)
        }
    })
    .await
    .map_err(|e| AppError::Internal(format!("Slice directory preparation task panicked: {e}")))??;

    let output_path =
        resolve_clip_output_path(&clips_dir, start_sec, end_sec, cluster_id, output_filename);

    let mut loaded_manifest: Option<VodManifest> = None;
    let mut had_cached_stream_url = false;

    let (input_source, is_online, clamped_end_sec) = if let Some(src) = custom_input_source {
        let is_remote = src.starts_with("http://") || src.starts_with("https://");
        (src.to_string(), is_remote, end_sec)
    } else if let Some(mut manifest) = disk_manifest {
        // Clamp end_sec to manifest duration if manifest duration is positive and finite
        let clamped = if manifest.duration_seconds > 0.0 && end_sec > manifest.duration_seconds {
            manifest.duration_seconds
        } else {
            end_sec
        };

        let (src, is_remote) = if manifest.source_type == "local_file" {
            let path = manifest
                .local_video_path
                .clone()
                .filter(|p| !p.trim().is_empty())
                .ok_or_else(|| {
                    AppError::Ingestion(IngestError::ExtractionFailed(
                        "Không tìm thấy local_video_path trong vod_manifest.json".to_string(),
                    ))
                })?;
            (path, false)
        } else {
            let cached_stream = manifest
                .video_stream_url
                .as_deref()
                .map(str::trim)
                .filter(|u| u.starts_with("http://") || u.starts_with("https://"))
                .filter(|u| manifest.original_url.as_deref().map(str::trim) != Some(*u))
                .map(ToString::to_string);

            let stream_url = if let Some(url) = cached_stream {
                had_cached_stream_url = true;
                url
            } else if let Some(orig_url) = manifest
                .original_url
                .as_deref()
                .map(str::trim)
                .filter(|u| !u.is_empty())
            {
                let resolved =
                    resolve_video_stream_url(orig_url, &CookieSource::None, None).await?;
                manifest.video_stream_url = Some(resolved.clone());
                if let Ok(updated_bytes) = serde_json::to_vec_pretty(&manifest) {
                    let write_path = manifest_path.clone();
                    let _ = tokio::task::spawn_blocking(move || {
                        let _ = std::fs::write(&write_path, updated_bytes);
                    })
                    .await;
                }
                resolved
            } else {
                return Err(AppError::Ingestion(IngestError::ExtractionFailed(
                    "Không tìm thấy video_stream_url hoặc original_url hợp lệ trong vod_manifest.json"
                        .to_string(),
                )));
            };

            (stream_url, true)
        };

        loaded_manifest = Some(manifest);
        (src, is_remote, clamped)
    } else {
        return Err(AppError::Storage(StorageError::FileNotFound(format!(
            "Không tìm thấy tệp tin vod_manifest.json tại '{}'",
            manifest_path.display()
        ))));
    };

    if !start_sec.is_finite()
        || !clamped_end_sec.is_finite()
        || start_sec < 0.0
        || start_sec >= clamped_end_sec
    {
        return Err(AppError::Ingestion(IngestError::InvalidTimeRange {
            start_sec,
            end_sec: clamped_end_sec,
        }));
    }

    let task_id = format!("slice_{vod_id}_{}", cluster_id.unwrap_or("segment"));
    let token = CancellationToken::new();
    register_slice_task_token(&task_id, token.clone());

    if let Some(app_handle) = app {
        let _ = app_handle.emit(
            "slice:progress",
            serde_json::json!({
                "taskId": task_id,
                "stage": "starting",
                "percent": 0.0,
            }),
        );
    }

    let request = RangeSliceRequest {
        input_source,
        start_sec,
        end_sec: clamped_end_sec,
        output_path: output_path.clone(),
        is_online,
        http_headers: None,
        use_duration_flag: false,
        task_id: Some(task_id.clone()),
    };

    let mut slice_result = execute_range_slice(request, Some(token.clone())).await;

    // If a cached online stream URL failed (e.g. expired CDN token), lazily re-resolve once via yt-dlp -g
    if is_online && had_cached_stream_url {
        if let Err(AppError::Ingestion(IngestError::SliceFailed(_))) = slice_result {
            if let Some(mut manifest) = loaded_manifest {
                if let Some(orig_url) = manifest
                    .original_url
                    .as_deref()
                    .map(str::trim)
                    .filter(|u| !u.is_empty())
                {
                    if let Ok(refreshed_url) =
                        resolve_video_stream_url(orig_url, &CookieSource::None, None).await
                    {
                        manifest.video_stream_url = Some(refreshed_url.clone());
                        if let Ok(updated_bytes) = serde_json::to_vec_pretty(&manifest) {
                            let write_path = manifest_path.clone();
                            let _ = tokio::task::spawn_blocking(move || {
                                let _ = std::fs::write(&write_path, updated_bytes);
                            })
                            .await;
                        }
                        let retry_request = RangeSliceRequest {
                            input_source: refreshed_url,
                            start_sec,
                            end_sec: clamped_end_sec,
                            output_path,
                            is_online: true,
                            http_headers: None,
                            use_duration_flag: false,
                            task_id: Some(task_id.clone()),
                        };
                        slice_result = execute_range_slice(retry_request, Some(token)).await;
                    }
                }
            }
        }
    }

    unregister_slice_task_token(&task_id);

    if let Some(app_handle) = app {
        if slice_result.is_ok() {
            let _ = app_handle.emit(
                "slice:progress",
                serde_json::json!({
                    "taskId": task_id,
                    "stage": "completed",
                    "percent": 100.0,
                }),
            );
        }
    }

    slice_result
}

/// Internal slice video segment execution logic, callable without a Tauri AppHandle.
///
/// Accepts existing absolute project directories for integration tests while strictly validating
/// all non-absolute VOD identifiers via [`crate::core::storage::resolve_vod_project_dir`].
pub async fn slice_video_segment_internal(
    vod_id: &str,
    start_sec: f64,
    end_sec: f64,
    cluster_id: Option<&str>,
    output_filename: Option<&str>,
    custom_input_source: Option<&str>,
    app: Option<&tauri::AppHandle>,
) -> Result<SliceResult, AppError> {
    let vod_id_owned = vod_id.to_string();
    let project_dir = tokio::task::spawn_blocking(move || {
        let candidate = Path::new(&vod_id_owned);
        if candidate.is_absolute() && candidate.exists() {
            Ok(candidate.to_path_buf())
        } else {
            core_resolve_vod_project_dir(&vod_id_owned)
        }
    })
    .await
    .map_err(|e| AppError::Internal(format!("Slice project dir resolution panicked: {e}")))??;

    slice_video_segment_in_dir(
        SliceTarget {
            project_dir: &project_dir,
            vod_id,
        },
        start_sec,
        end_sec,
        cluster_id,
        output_filename,
        custom_input_source,
        app,
    )
    .await
}

/// Slices a highlight segment from online stream m3u8 or local file without re-encoding.
///
/// Complies with:
/// - AC-13.1: Execution time <= 10s for 90s clip
/// - AC-13.2: Bandwidth saving >= 99%
/// - AC-13.3 & ED-13.1: Zero initial freeze, PTS normalized, drift <= 50ms
///
/// # Errors
/// Returns `AppError` if parameters are invalid, `vod_id` fails traversal validation, manifest is missing, or slicing fails.
#[tauri::command]
pub async fn slice_video_segment(
    vod_id: String,
    start_sec: f64,
    end_sec: f64,
    cluster_id: Option<String>,
    output_filename: Option<String>,
    custom_input_source: Option<String>,
    app: tauri::AppHandle,
) -> Result<SliceResult, AppError> {
    let clean_id = validate_vod_id(&vod_id)
        .map_err(AppError::Storage)?
        .to_string();
    let clean_id_for_dir = clean_id.clone();
    let project_dir =
        tokio::task::spawn_blocking(move || core_resolve_vod_project_dir(&clean_id_for_dir))
            .await
            .map_err(|e| {
                AppError::Internal(format!("Slice project dir resolution panicked: {e}"))
            })??;
    slice_video_segment_in_dir(
        SliceTarget {
            project_dir: &project_dir,
            vod_id: &clean_id,
        },
        start_sec,
        end_sec,
        cluster_id.as_deref(),
        output_filename.as_deref(),
        custom_input_source.as_deref(),
        Some(&app),
    )
    .await
}

/// Cancel a running video segment slicing task.
///
/// # Errors
/// Always returns `Ok(true)` if found and cancelled, or `Ok(false)` if not active.
#[tauri::command]
// Tauri command IPC deserializes owned String arguments from JS invocations.
#[allow(clippy::needless_pass_by_value)]
pub fn cancel_slice_video_segment(task_id: String) -> Result<bool, AppError> {
    Ok(cancel_slice_task_token(&task_id))
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;

    #[test]
    fn test_resolve_vod_project_dir_rejects_traversal_and_absolute_paths() {
        for bad in [
            "",
            "   ",
            "..",
            "../outside",
            "..\\outside",
            "vod/sub",
            "vod\\sub",
            "C:\\Windows\\System32",
            "D:vod_123",
            "\\\\server\\share\\vod",
        ] {
            let res = resolve_vod_project_dir(bad);
            assert!(
                matches!(res, Err(AppError::Storage(_))),
                "Expected Storage error for invalid vod_id {bad:?}, got {res:?}"
            );
        }
    }

    #[tokio::test]
    async fn test_slice_video_segment_internal_rejects_relative_traversal() {
        let res = slice_video_segment_internal(
            "../../outside_dir",
            0.0,
            10.0,
            None,
            None,
            Some("D:/Videos/test.mp4"),
            None,
        )
        .await;
        assert!(
            matches!(res, Err(AppError::Storage(_))),
            "Expected Storage error for traversal vod_id, got {res:?}"
        );
    }

    #[test]
    fn test_resolve_clip_output_path_sanitizes_cluster_id_and_filename() {
        let clips_dir = Path::new("C:\\NeuroClip\\projects\\vod_1\\clips");
        let path_from_cluster =
            resolve_clip_output_path(clips_dir, 10.0, 20.0, Some("../../evil_cluster"), None);
        assert_eq!(path_from_cluster.parent(), Some(clips_dir));
        assert_eq!(
            path_from_cluster.file_name().and_then(|s| s.to_str()),
            Some("clip_evil_cluster.mp4")
        );

        let path_from_filename =
            resolve_clip_output_path(clips_dir, 10.0, 20.0, None, Some("..\\..\\escaped.mp4"));
        assert_eq!(path_from_filename.parent(), Some(clips_dir));
        assert_eq!(
            path_from_filename.file_name().and_then(|s| s.to_str()),
            Some("escaped.mp4")
        );
    }
}
