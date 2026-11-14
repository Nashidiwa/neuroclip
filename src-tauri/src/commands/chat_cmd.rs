//! Tauri IPC commands for Chat Extraction, Normalization, and Heatmap Analysis.
//!
//! Complies with docs/CONVENTIONS.md (zero-panic invariant, `Result<T, AppError>`)
//! and docs/ARCHITECTURE.md §4.2, §5.3, §6 (ERR-05).

use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::PathBuf;
use tauri::{AppHandle, Emitter};
use tokio_util::sync::CancellationToken;

use crate::core::storage::{resolve_cache_dir, resolve_projects_dir};
use crate::error::{codes, AppError, IngestError, StorageError};
use crate::ingestion::chat_parser::{
    ChatEventsContainer, ChatHeatmapResponse, StreamingChatNormalizer,
};
use crate::ingestion::streamer::{detect_source_type, VodManifest};
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

/// Helper function to resolve the target project directory for a VOD.
fn resolve_vod_project_dir(vod_id: &str) -> PathBuf {
    let projects_dir = resolve_projects_dir();
    let candidate = projects_dir.join(vod_id);
    if candidate.exists() {
        candidate
    } else {
        resolve_cache_dir().join(vod_id)
    }
}

/// Extract and normalize chat replay for a VOD.
///
/// Implements ERR-05 defensive fallback: if chat replay is unavailable,
/// emits `chat:fallback` event and returns `ERR_INGEST_CHAT_UNAVAILABLE`.
#[tauri::command]
pub async fn start_chat_extraction(
    vod_id: String,
    url: String,
    app: AppHandle,
) -> Result<String, AppError> {
    let project_dir = resolve_vod_project_dir(&vod_id);
    std::fs::create_dir_all(&project_dir).map_err(|e| {
        AppError::Storage(StorageError::DirectoryCreationFailed {
            path: project_dir.display().to_string(),
            details: e.to_string(),
        })
    })?;

    let raw_chat_path = project_dir.join(format!("{vod_id}.raw_chat.ndjson"));
    let normalized_json_path = project_dir.join("chat_events.json");
    let heatmap_json_path = project_dir.join("chat_heatmap.json");

    let cancel_token = CancellationToken::new();
    let source_type = detect_source_type(&url);

    // 1. Download raw chat stream based on source platform
    let app_handle_for_progress = app.clone();
    let progress_cb = move |payload: ChatProgressPayload| {
        let _ = app_handle_for_progress.emit("chat:progress", payload);
    };

    let download_res = if source_type == "twitch_vod" {
        fetch_twitch_vod_chat_gql(&vod_id, &raw_chat_path, cancel_token.clone(), progress_cb).await
    } else {
        match fetch_youtube_live_chat_replay(
            &url,
            &vod_id,
            &project_dir,
            cancel_token.clone(),
            progress_cb,
        )
        .await
        {
            Ok(downloaded_path) => {
                // If downloaded path is not raw_chat_path, copy or rename it
                if downloaded_path != raw_chat_path && downloaded_path.exists() {
                    let _ = std::fs::copy(&downloaded_path, &raw_chat_path);
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
                vod_id
            );
            let _ = app.emit(
                "chat:fallback",
                ChatFallbackPayload {
                    task_id: vod_id.clone(),
                    code: codes::ERR_INGEST_CHAT_UNAVAILABLE.to_string(),
                    message: "Chat replay không khả dụng. Đang tự động chuyển sang phân tích highlight bằng Năng lượng Âm thanh.".to_string(),
                    audio_fallback_active: true,
                },
            );
        }
        return Err(err);
    }

    // 2. Normalize raw chat in blocking thread (strict concurrency separation per CONVENTIONS §2.2)
    let normalizer = StreamingChatNormalizer::new(vod_id.clone(), &project_dir);
    let norm_raw_path = raw_chat_path.clone();
    let norm_out_path = normalized_json_path.clone();
    let token_clone = cancel_token.clone();

    let (_total_msgs, ndjson_path) = tokio::task::spawn_blocking(move || {
        normalizer.normalize_file(&norm_raw_path, &norm_out_path, &token_clone)
    })
    .await
    .map_err(|e| AppError::Internal(format!("Chat normalization worker failed: {e}")))??;

    // 3. Compute 10s sliding window density heatmap
    let normalizer_heat = StreamingChatNormalizer::new(vod_id.clone(), &project_dir);
    let heatmap = tokio::task::spawn_blocking(move || {
        normalizer_heat.compute_density_heatmap(&ndjson_path, 10.0, 10.0)
    })
    .await
    .map_err(|e| AppError::Internal(format!("Heatmap computation worker failed: {e}")))??;

    if let Ok(heatmap_str) = serde_json::to_string(&heatmap) {
        let _ = std::fs::write(&heatmap_json_path, heatmap_str);
    }

    // 4. Update vod_manifest.json if it exists in the project directory
    let manifest_path = project_dir.join("vod_manifest.json");
    if manifest_path.exists() {
        if let Ok(manifest_bytes) = std::fs::read(&manifest_path) {
            if let Ok(mut manifest) = serde_json::from_slice::<VodManifest>(&manifest_bytes) {
                manifest.chat_file_path = Some(normalized_json_path.to_string_lossy().to_string());
                if let Ok(updated_bytes) = serde_json::to_vec_pretty(&manifest) {
                    let _ = std::fs::write(&manifest_path, updated_bytes);
                }
            }
        }
    }

    let _ = app.emit(
        "chat:progress",
        ChatProgressPayload {
            task_id: vod_id,
            stage: "completed".to_string(),
            percent: 100.0,
            messages_extracted: heatmap.points.iter().map(|p| p.message_count as u64).sum(),
            current_offset_sec: heatmap.peak_timestamp_sec,
        },
    );

    Ok(normalized_json_path.to_string_lossy().to_string())
}

/// Retrieve paginated chat events adhering to Schema 5.3 contract.
#[tauri::command]
pub async fn get_chat_events(
    vod_id: String,
    limit: Option<usize>,
    offset: Option<usize>,
) -> Result<ChatEventsContainer, AppError> {
    let project_dir = resolve_vod_project_dir(&vod_id);
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
            let line = match line_res {
                Ok(l) => l,
                Err(_) => break,
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
            vod_id,
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

    let mut container: ChatEventsContainer = serde_json::from_slice(&bytes)
        .map_err(|e| AppError::Highlight(crate::error::HighlightError::ChatParsingFailed(e.to_string())))?;

    let total = container.events.len();
    let start = off.min(total);
    let end = (start + lim).min(total);

    container.events = container.events[start..end].to_vec();
    Ok(container)
}

/// Retrieve the 10-second sliding window velocity heatmap for a VOD.
#[tauri::command]
pub async fn get_chat_velocity_heatmap(
    vod_id: String,
    window_sec: Option<f64>,
) -> Result<ChatHeatmapResponse, AppError> {
    let project_dir = resolve_vod_project_dir(&vod_id);
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
        return Err(AppError::Storage(StorageError::FileNotFound(
            format!("No chat data found for VOD {vod_id}")
        )));
    };

    let normalizer = StreamingChatNormalizer::new(vod_id, &project_dir);
    tokio::task::spawn_blocking(move || {
        normalizer.compute_density_heatmap(&source_path, win_sec, win_sec)
    })
    .await
    .map_err(|e| AppError::Internal(format!("Heatmap worker task failed: {e}")))?
}
