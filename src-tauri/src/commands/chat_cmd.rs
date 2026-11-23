//! Tauri IPC commands for Chat Extraction, Normalization, and Heatmap Analysis.
//!
//! Complies with docs/CONVENTIONS.md (zero-panic invariant, `Result<T, AppError>`)
//! and docs/ARCHITECTURE.md §4.2, §5.3, §6 (ERR-05).

use std::fs::File;
use std::io::{BufRead, BufReader};
use tauri::{AppHandle, Emitter};
use tokio_util::sync::CancellationToken;

use crate::core::storage::{resolve_vod_project_dir, validate_vod_id};
use crate::error::{codes, AppError, IngestError, StorageError};
use crate::ingestion::chat_parser::{
    ChatEventsContainer, ChatHeatmapResponse, StreamingChatNormalizer,
};
use crate::ingestion::streamer::{detect_source_type, validate_http_url, VodManifest};
use crate::ingestion::twitch_gql::{fetch_twitch_vod_chat_gql, ChatProgressPayload};
use crate::ingestion::youtube_chat::fetch_youtube_live_chat_replay;

/// Fallback notification payload emitted to Frontend UI when chat replay is missing.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChatFallbackPayload {
    pub task_id: String,
    pub code: String,
    pub message: String,
    pub audio_fallback_active: bool,
}

/// Extract and normalize chat replay for a VOD.
///
/// Implements ERR-05 defensive fallback: if chat replay is unavailable,
/// emits `chat:fallback` event and returns `ERR_INGEST_CHAT_UNAVAILABLE`.
///
/// # Errors
/// Returns [`AppError::Storage`] if `vod_id` fails path validation or directory creation fails,
/// or [`AppError::Ingestion`] / [`AppError::Internal`] if chat extraction or normalization fails.
#[tauri::command]
#[allow(clippy::too_many_lines)]
pub async fn start_chat_extraction(
    vod_id: String,
    url: String,
    app: AppHandle,
) -> Result<String, AppError> {
    let clean_url = validate_http_url(&url)?.to_string();
    let clean_id = validate_vod_id(&vod_id)
        .map_err(AppError::Storage)?
        .to_string();
    let vod_id_for_dir = clean_id.clone();
    let project_dir = tokio::task::spawn_blocking(move || {
        let dir = resolve_vod_project_dir(&vod_id_for_dir)?;
        std::fs::create_dir_all(&dir).map_err(|e| {
            AppError::Storage(StorageError::DirectoryCreationFailed {
                path: dir.display().to_string(),
                details: e.to_string(),
            })
        })?;
        Ok::<_, AppError>(dir)
    })
    .await
    .map_err(|e| AppError::Internal(format!("Project directory task panicked: {e}")))??;

    let raw_chat_path = project_dir.join(format!("{clean_id}.raw_chat.ndjson"));
    let normalized_json_path = project_dir.join("chat_events.json");
    let heatmap_json_path = project_dir.join("chat_heatmap.json");

    let cancel_token = CancellationToken::new();
    let source_type = detect_source_type(&clean_url);

    // 1. Download raw chat stream based on source platform
    let app_handle_for_progress = app.clone();
    let progress_cb = move |payload: ChatProgressPayload| {
        let _ = app_handle_for_progress.emit("chat:progress", payload);
    };

    let download_res = if source_type == "twitch_vod" {
        fetch_twitch_vod_chat_gql(&clean_id, &raw_chat_path, cancel_token.clone(), progress_cb)
            .await
    } else {
        match fetch_youtube_live_chat_replay(
            &clean_url,
            &clean_id,
            &project_dir,
            cancel_token.clone(),
            progress_cb,
        )
        .await
        {
            Ok(downloaded_path) => {
                // If downloaded path is not raw_chat_path, copy it on blocking pool
                if downloaded_path != raw_chat_path {
                    let raw_dest = raw_chat_path.clone();
                    let _ = tokio::task::spawn_blocking(move || {
                        if downloaded_path.exists() {
                            let _ = std::fs::copy(&downloaded_path, &raw_dest);
                        }
                    })
                    .await;
                }
                Ok(1)
            }
            Err(e) => Err(e),
        }
    };

    if let Err(err) = download_res {
        if let AppError::Ingestion(IngestError::ChatReplayUnavailable) = &err {
            tracing::warn!(
                "Chat replay unavailable for VOD {}. Activating Audio RMS fallback (ERR-05).",
                clean_id
            );
            let _ = app.emit(
                "chat:fallback",
                ChatFallbackPayload {
                    task_id: clean_id.clone(),
                    code: codes::ERR_INGEST_CHAT_UNAVAILABLE.to_string(),
                    message: "Chat replay không khả dụng. Đang tự động chuyển sang phân tích highlight bằng Năng lượng Âm thanh.".to_string(),
                    audio_fallback_active: true,
                },
            );
        }
        return Err(err);
    }

    // 2. Normalize raw chat in blocking thread (strict concurrency separation per CONVENTIONS §2.2)
    let normalizer = StreamingChatNormalizer::new(clean_id.clone(), &project_dir);
    let norm_raw_path = raw_chat_path.clone();
    let norm_out_path = normalized_json_path.clone();
    let token_clone = cancel_token.clone();

    let (_total_msgs, ndjson_path) = tokio::task::spawn_blocking(move || {
        normalizer.normalize_file(&norm_raw_path, &norm_out_path, &token_clone)
    })
    .await
    .map_err(|e| AppError::Internal(format!("Chat normalization worker failed: {e}")))??;

    // 3. Compute 10s sliding window density heatmap and persist metadata on blocking pool
    let normalizer_heat = StreamingChatNormalizer::new(clean_id.clone(), &project_dir);
    let norm_path_for_manifest = normalized_json_path.clone();
    let project_dir_for_manifest = project_dir.clone();
    let heatmap = tokio::task::spawn_blocking(move || {
        let heatmap = normalizer_heat.compute_density_heatmap(&ndjson_path, 10.0, 10.0)?;

        if let Ok(heatmap_str) = serde_json::to_string(&heatmap) {
            let _ = std::fs::write(&heatmap_json_path, heatmap_str);
        }

        // 4. Update vod_manifest.json if it exists in the project directory
        let manifest_path = project_dir_for_manifest.join("vod_manifest.json");
        if manifest_path.exists() {
            if let Ok(manifest_bytes) = std::fs::read(&manifest_path) {
                if let Ok(mut manifest) = serde_json::from_slice::<VodManifest>(&manifest_bytes) {
                    manifest.chat_file_path =
                        Some(norm_path_for_manifest.to_string_lossy().to_string());
                    if let Ok(updated_bytes) = serde_json::to_vec_pretty(&manifest) {
                        let _ = std::fs::write(&manifest_path, updated_bytes);
                    }
                }
            }
        }

        Ok::<_, AppError>(heatmap)
    })
    .await
    .map_err(|e| AppError::Internal(format!("Heatmap computation worker failed: {e}")))??;

    let _ = app.emit(
        "chat:progress",
        ChatProgressPayload {
            task_id: clean_id,
            stage: "completed".to_string(),
            percent: 100.0,
            messages_extracted: heatmap
                .points
                .iter()
                .map(|p| u64::from(p.message_count))
                .sum(),
            current_offset_sec: heatmap.peak_timestamp_sec,
        },
    );

    Ok(normalized_json_path.to_string_lossy().to_string())
}

/// Retrieve paginated chat events adhering to Schema 5.3 contract.
///
/// # Errors
/// Returns [`AppError::Storage`] if `vod_id` is invalid or chat files are missing,
/// or [`AppError::Highlight`] if JSON deserialization fails.
#[tauri::command]
pub async fn get_chat_events(
    vod_id: String,
    limit: Option<usize>,
    offset: Option<usize>,
) -> Result<ChatEventsContainer, AppError> {
    tokio::task::spawn_blocking(move || {
        let clean_id = validate_vod_id(&vod_id)
            .map_err(AppError::Storage)?
            .to_string();
        let project_dir = resolve_vod_project_dir(&clean_id)?;
        let json_path = project_dir.join("chat_events.json");
        let ndjson_path = project_dir.join("chat_events.ndjson");

        let lim = limit.unwrap_or(500);
        let off = offset.unwrap_or(0);

        // If ndjson exists, stream slice directly to avoid large heap deserialization
        if ndjson_path.exists() {
            let file = File::open(&ndjson_path).map_err(|e| {
                AppError::Storage(StorageError::FileNotFound(format!(
                    "Failed to open chat_events.ndjson: {e}"
                )))
            })?;
            let reader = BufReader::with_capacity(64 * 1024, file);

            let mut events = Vec::new();
            let mut total_count: u64 = 0;

            for line_res in reader.lines() {
                let Ok(line) = line_res else {
                    break;
                };
                let trimmed = line.trim();
                if trimmed.is_empty() {
                    continue;
                }

                if total_count >= (off as u64) && events.len() < lim {
                    if let Ok(event) = serde_json::from_str(trimmed) {
                        events.push(event);
                    }
                }
                total_count = total_count.saturating_add(1);
            }

            return Ok(ChatEventsContainer {
                vod_id: clean_id,
                total_messages: total_count,
                events,
            });
        }

        if !json_path.exists() {
            return Err(AppError::Storage(StorageError::FileNotFound(
                json_path.display().to_string(),
            )));
        }

        let bytes = std::fs::read(&json_path).map_err(|e| {
            AppError::Storage(StorageError::FileNotFound(format!(
                "Failed reading chat_events.json: {e}"
            )))
        })?;

        let mut container: ChatEventsContainer = serde_json::from_slice(&bytes).map_err(|e| {
            AppError::Highlight(crate::error::HighlightError::ChatParsingFailed(
                e.to_string(),
            ))
        })?;

        let total = container.events.len();
        let start = off.min(total);
        let end = (start + lim).min(total);

        container.events = container.events[start..end].to_vec();
        Ok(container)
    })
    .await
    .map_err(|e| AppError::Internal(format!("Chat events reader task panicked: {e}")))?
}

/// Retrieve the 10-second sliding window velocity heatmap for a VOD.
///
/// # Errors
/// Returns [`AppError::Storage`] if `vod_id` is invalid or chat files are missing,
/// or [`AppError::Internal`] if the heatmap computation worker fails.
#[tauri::command]
pub async fn get_chat_velocity_heatmap(
    vod_id: String,
    window_sec: Option<f64>,
) -> Result<ChatHeatmapResponse, AppError> {
    tokio::task::spawn_blocking(move || {
        let clean_id = validate_vod_id(&vod_id)
            .map_err(AppError::Storage)?
            .to_string();
        let project_dir = resolve_vod_project_dir(&clean_id)?;
        let heatmap_json_path = project_dir.join("chat_heatmap.json");
        let ndjson_path = project_dir.join("chat_events.ndjson");
        let json_path = project_dir.join("chat_events.json");

        let win_sec = window_sec.unwrap_or(10.0);

        // If default window and cached heatmap exists, load it
        if (win_sec - 10.0).abs() < 1e-4 && heatmap_json_path.exists() {
            if let Ok(bytes) = std::fs::read(&heatmap_json_path) {
                if let Ok(cached) = serde_json::from_slice::<ChatHeatmapResponse>(&bytes) {
                    return Ok(cached);
                }
            }
        }

        let source_path = if ndjson_path.exists() {
            ndjson_path
        } else if json_path.exists() {
            json_path
        } else {
            return Err(AppError::Storage(StorageError::FileNotFound(format!(
                "No chat data found for VOD {clean_id}"
            ))));
        };

        let normalizer = StreamingChatNormalizer::new(clean_id, &project_dir);
        normalizer.compute_density_heatmap(&source_path, win_sec, win_sec)
    })
    .await
    .map_err(|e| AppError::Internal(format!("Heatmap worker task failed: {e}")))?
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_chat_commands_reject_invalid_vod_id() {
        for bad in ["../escape", "C:\\escape", "vod/sub", "vod\0id", ""] {
            let events_res = get_chat_events(bad.to_string(), None, None).await;
            assert!(
                matches!(
                    events_res,
                    Err(AppError::Storage(
                        StorageError::DirectoryCreationFailed { .. }
                    ))
                ),
                "get_chat_events must reject {bad:?}, got {events_res:?}"
            );

            let heatmap_res = get_chat_velocity_heatmap(bad.to_string(), None).await;
            assert!(
                matches!(
                    heatmap_res,
                    Err(AppError::Storage(
                        StorageError::DirectoryCreationFailed { .. }
                    ))
                ),
                "get_chat_velocity_heatmap must reject {bad:?}, got {heatmap_res:?}"
            );
        }
    }
}
