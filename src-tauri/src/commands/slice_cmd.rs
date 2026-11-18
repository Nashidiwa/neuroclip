//! Tauri IPC commands for Lossless Range Slicing.
//!
//! Complies with `docs/CONVENTIONS.md` (zero-panic invariant, Result<T, AppError>),
//! `docs/ARCHITECTURE.md` §4.2, `docs/adr/ADR-003`, and `docs/TASK_BOARD.md` TASK-M4-13.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Mutex;
use tauri::Emitter;
use tokio_util::sync::CancellationToken;

use crate::core::storage::{resolve_cache_dir, resolve_projects_dir};
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

/// Resolve the project directory for a given VOD ID.
#[must_use]
pub fn resolve_vod_project_dir(vod_id: &str) -> PathBuf {
    let p = std::path::Path::new(vod_id);
    if p.is_absolute() && p.exists() {
        return p.to_path_buf();
    }
    let projects_dir = resolve_projects_dir();
    let candidate = projects_dir.join(vod_id);
    if candidate.exists() {
        candidate
    } else {
        resolve_cache_dir().join(vod_id)
    }
}

/// Internal slice video segment execution logic, callable without a Tauri AppHandle.
#[allow(
    clippy::too_many_lines,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss
)]
pub async fn slice_video_segment_internal(
    vod_id: &str,
    start_sec: f64,
    end_sec: f64,
    cluster_id: Option<&str>,
    output_filename: Option<&str>,
    custom_input_source: Option<&str>,
    app: Option<&tauri::AppHandle>,
) -> Result<SliceResult, AppError> {
    let project_dir = resolve_vod_project_dir(vod_id);

    let clips_dir = project_dir.join("clips");
    std::fs::create_dir_all(&clips_dir).map_err(|e| {
        AppError::Storage(StorageError::DirectoryCreationFailed {
            path: clips_dir.display().to_string(),
            details: e.to_string(),
        })
    })?;

    let output_path = match output_filename {
        Some(name) => {
            // Guard against path traversal by only taking the filename component
            let file_name = std::path::Path::new(name)
                .file_name()
                .and_then(|f| f.to_str())
                .filter(|s| !s.trim().is_empty());
            match file_name {
                Some(clean_name) => clips_dir.join(clean_name),
                None => match cluster_id {
                    Some(cid) => clips_dir.join(format!("clip_{cid}.mp4")),
                    None => {
                        clips_dir.join(format!("clip_{}_{}.mp4", start_sec as u64, end_sec as u64))
                    }
                },
            }
        }
        None => match cluster_id {
            Some(cid) => clips_dir.join(format!("clip_{cid}.mp4")),
            None => clips_dir.join(format!("clip_{}_{}.mp4", start_sec as u64, end_sec as u64)),
        },
    };

    let manifest_path = project_dir.join("vod_manifest.json");

    let mut loaded_manifest: Option<VodManifest> = None;
    let mut had_cached_stream_url = false;

    let (input_source, is_online, clamped_end_sec) = if let Some(src) = custom_input_source {
        let is_remote = src.starts_with("http://") || src.starts_with("https://");
        (src.to_string(), is_remote, end_sec)
    } else if manifest_path.exists() {
        let bytes = std::fs::read(&manifest_path).map_err(|e| {
            AppError::Storage(StorageError::FileNotFound(format!(
                "Không thể đọc vod_manifest.json tại '{}': {e}",
                manifest_path.display()
            )))
        })?;

        let mut manifest: VodManifest = serde_json::from_slice(&bytes).map_err(|e| {
            AppError::Ingestion(IngestError::ExtractionFailed(format!(
                "Lỗi phân giải vod_manifest.json: {e}"
            )))
        })?;

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
                    let _ = std::fs::write(&manifest_path, updated_bytes);
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
                            let _ = std::fs::write(&manifest_path, updated_bytes);
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

/// Slices a highlight segment from online stream m3u8 or local file without re-encoding.
///
/// Complies with:
/// - AC-13.1: Execution time <= 10s for 90s clip
/// - AC-13.2: Bandwidth saving >= 99%
/// - AC-13.3 & ED-13.1: Zero initial freeze, PTS normalized, drift <= 50ms
///
/// # Errors
/// Returns `AppError` if parameters are invalid, manifest is missing, or slicing fails.
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
    slice_video_segment_internal(
        &vod_id,
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
