//! Tauri IPC commands for Chat Extraction, Normalization, and Heatmap Analysis.
//!
//! Complies with docs/CONVENTIONS.md (zero-panic invariant, `Result<T, AppError>`)
//! and docs/ARCHITECTURE.md §4.2, §5.3, §6 (ERR-05).

use std::collections::HashMap;
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::sync::Mutex;
use tauri::{AppHandle, Emitter};
use tokio_util::sync::CancellationToken;

use crate::core::storage::{resolve_vod_project_dir, validate_vod_id};
use crate::error::{codes, AppError, IngestError, StorageError};
use crate::ingestion::chat_parser::{
    ChatEventsContainer, ChatHeatmapResponse, StreamingChatNormalizer,
};
use crate::ingestion::streamer::{detect_source_type, validate_http_url, VodManifest};
use crate::ingestion::twitch_gql::{
    extract_twitch_vod_id, fetch_twitch_vod_chat_gql_with_task_id, ChatProgressPayload,
};
use crate::ingestion::youtube_chat::fetch_youtube_live_chat_replay_with_cookies;

/// Resolve the numeric Twitch VOD ID to send to GQL, falling back to extracting it from `clean_url`
/// when `clean_id` is synthetic/non-numeric (e.g., `vod_custom_alias`).
#[must_use]
fn resolve_twitch_gql_vod_id<'a>(clean_id: &'a str, clean_url: &'a str) -> &'a str {
    let from_id = extract_twitch_vod_id(clean_id);
    if !from_id.is_empty() && from_id.chars().all(|c| c.is_ascii_digit()) {
        return from_id;
    }
    let from_url = extract_twitch_vod_id(clean_url);
    if !from_url.is_empty() && from_url.chars().all(|c| c.is_ascii_digit()) {
        return from_url;
    }
    clean_id
}

/// Fallback notification payload emitted to Frontend UI when chat replay is missing.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChatFallbackPayload {
    pub task_id: String,
    pub code: String,
    pub message: String,
    pub audio_fallback_active: bool,
}

static ACTIVE_CHAT_TASKS: Mutex<Option<HashMap<String, CancellationToken>>> = Mutex::new(None);

/// Register an active cancellation token for a chat extraction task.
pub fn register_chat_task_token(task_id: &str, token: CancellationToken) {
    if let Ok(mut lock) = ACTIVE_CHAT_TASKS.lock() {
        let map = lock.get_or_insert_with(HashMap::new);
        map.insert(task_id.to_string(), token);
    }
}

/// Unregister a chat extraction cancellation token when the task finishes.
pub fn unregister_chat_task_token(task_id: &str) {
    if let Ok(mut lock) = ACTIVE_CHAT_TASKS.lock() {
        if let Some(map) = lock.as_mut() {
            map.remove(task_id);
        }
    }
}

/// Cancel a running chat extraction task by ID (or the single active task if task_id is empty).
pub fn cancel_chat_task_token(task_id: &str) -> bool {
    if let Ok(mut lock) = ACTIVE_CHAT_TASKS.lock() {
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
    cookies_path: Option<String>,
    browser_cookies: Option<String>,
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
    register_chat_task_token(&clean_id, cancel_token.clone());
    let source_type = detect_source_type(&clean_url);

    // 1. Download raw chat stream based on source platform
    let app_handle_for_progress = app.clone();
    let progress_cb = move |payload: ChatProgressPayload| {
        let _ = app_handle_for_progress.emit("chat:progress", payload);
    };

    let download_res = if source_type == "twitch_vod" {
        let twitch_query_id = resolve_twitch_gql_vod_id(&clean_id, &clean_url);
        fetch_twitch_vod_chat_gql_with_task_id(
            &clean_id,
            twitch_query_id,
            &raw_chat_path,
            cancel_token.clone(),
            progress_cb,
        )
        .await
    } else {
        match fetch_youtube_live_chat_replay_with_cookies(
            &clean_url,
            &clean_id,
            &project_dir,
            cookies_path.as_deref(),
            browser_cookies.as_deref(),
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
        unregister_chat_task_token(&clean_id);
        if cancel_token.is_cancelled() || matches!(&err, AppError::TaskCancelled) {
            let _ = app.emit(
                "chat:progress",
                ChatProgressPayload {
                    task_id: clean_id.clone(),
                    stage: "cancelled".to_string(),
                    percent: 0.0,
                    messages_extracted: 0,
                    current_offset_sec: 0.0,
                },
            );
            return Err(AppError::TaskCancelled);
        }
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

    if cancel_token.is_cancelled() {
        unregister_chat_task_token(&clean_id);
        let _ = app.emit(
            "chat:progress",
            ChatProgressPayload {
                task_id: clean_id,
                stage: "cancelled".to_string(),
                percent: 0.0,
                messages_extracted: 0,
                current_offset_sec: 0.0,
            },
        );
        return Err(AppError::TaskCancelled);
    }

    // 2. Normalize raw chat in blocking thread (strict concurrency separation per CONVENTIONS §2.2)
    let normalizer = StreamingChatNormalizer::new(clean_id.clone(), &project_dir);
    let norm_raw_path = raw_chat_path.clone();
    let norm_out_path = normalized_json_path.clone();
    let token_clone = cancel_token.clone();

    let norm_res = tokio::task::spawn_blocking(move || {
        normalizer.normalize_file(&norm_raw_path, &norm_out_path, &token_clone)
    })
    .await
    .map_err(|e| AppError::Internal(format!("Chat normalization worker failed: {e}")))?;

    let (total_msgs, ndjson_path) = match norm_res {
        Ok(res) => res,
        Err(e) => {
            unregister_chat_task_token(&clean_id);
            if cancel_token.is_cancelled() || matches!(&e, AppError::TaskCancelled) {
                let _ = app.emit(
                    "chat:progress",
                    ChatProgressPayload {
                        task_id: clean_id,
                        stage: "cancelled".to_string(),
                        percent: 0.0,
                        messages_extracted: 0,
                        current_offset_sec: 0.0,
                    },
                );
                return Err(AppError::TaskCancelled);
            }
            return Err(e);
        }
    };

    if total_msgs == 0 {
        unregister_chat_task_token(&clean_id);
        tracing::warn!(
            "Chat replay produced 0 valid normalized messages for VOD {}. Activating Audio RMS fallback (ERR-05).",
            clean_id
        );
        let _ = app.emit(
            "chat:fallback",
            ChatFallbackPayload {
                task_id: clean_id,
                code: codes::ERR_INGEST_CHAT_UNAVAILABLE.to_string(),
                message: "Chat replay không khả dụng hoặc không có tin nhắn hợp lệ. Đang tự động chuyển sang phân tích highlight bằng Năng lượng Âm thanh.".to_string(),
                audio_fallback_active: true,
            },
        );
        return Err(AppError::Ingestion(IngestError::ChatReplayUnavailable));
    }

    if cancel_token.is_cancelled() {
        unregister_chat_task_token(&clean_id);
        let _ = app.emit(
            "chat:progress",
            ChatProgressPayload {
                task_id: clean_id,
                stage: "cancelled".to_string(),
                percent: 0.0,
                messages_extracted: 0,
                current_offset_sec: 0.0,
            },
        );
        return Err(AppError::TaskCancelled);
    }

    // 3. Compute 10s sliding window density heatmap and persist metadata on blocking pool
    let normalizer_heat = StreamingChatNormalizer::new(clean_id.clone(), &project_dir);
    let norm_path_for_manifest = normalized_json_path.clone();
    let project_dir_for_manifest = project_dir.clone();
    let heatmap_res = tokio::task::spawn_blocking(move || {
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
    .map_err(|e| AppError::Internal(format!("Heatmap computation worker failed: {e}")))?;

    let heatmap = match heatmap_res {
        Ok(h) => h,
        Err(e) => {
            unregister_chat_task_token(&clean_id);
            return Err(e);
        }
    };

    unregister_chat_task_token(&clean_id);

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

/// Cancel a running chat extraction task.
///
/// # Errors
/// Returns `Ok(true)` if task was found and cancelled, `Ok(false)` otherwise.
#[tauri::command]
#[allow(clippy::needless_pass_by_value)]
pub fn cancel_chat_extraction(task_id: String, app: AppHandle) -> Result<bool, AppError> {
    let clean_id = task_id.trim();
    let cancelled = cancel_chat_task_token(clean_id);
    if cancelled {
        let _ = app.emit(
            "chat:progress",
            ChatProgressPayload {
                task_id: clean_id.to_string(),
                stage: "cancelled".to_string(),
                percent: 0.0,
                messages_extracted: 0,
                current_offset_sec: 0.0,
            },
        );
    }
    Ok(cancelled)
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
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;

    static CHAT_TOKEN_TEST_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

    struct ChatTokenTestGuard {
        _lock: std::sync::MutexGuard<'static, ()>,
    }

    impl Drop for ChatTokenTestGuard {
        fn drop(&mut self) {
            if let Ok(mut map_lock) = ACTIVE_CHAT_TASKS.lock() {
                if let Some(map) = map_lock.as_mut() {
                    map.clear();
                }
            }
        }
    }

    fn lock_and_clear_chat_tasks() -> ChatTokenTestGuard {
        let guard = CHAT_TOKEN_TEST_LOCK
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if let Ok(mut map_lock) = ACTIVE_CHAT_TASKS.lock() {
            if let Some(map) = map_lock.as_mut() {
                map.clear();
            }
        }
        ChatTokenTestGuard { _lock: guard }
    }

    #[test]
    fn test_resolve_twitch_gql_vod_id_prefers_numeric_url_when_vod_id_synthetic() {
        assert_eq!(
            resolve_twitch_gql_vod_id(
                "vod_custom_alias",
                "https://www.twitch.tv/vedal987/video/2939581544?t=5s"
            ),
            "2939581544"
        );
        assert_eq!(
            resolve_twitch_gql_vod_id("vod_2939581544", "https://www.twitch.tv/videos/2939581544"),
            "2939581544"
        );
        assert_eq!(
            resolve_twitch_gql_vod_id("vod_custom_alias", "https://www.twitch.tv/videos/"),
            "vod_custom_alias"
        );
    }

    #[test]
    fn test_chat_task_token_lifecycle() {
        let _guard = lock_and_clear_chat_tasks();
        let token = CancellationToken::new();
        let task_id = "vod_test_12345";

        register_chat_task_token(task_id, token.clone());
        assert!(!token.is_cancelled());

        let cancelled = cancel_chat_task_token(task_id);
        assert!(cancelled);
        assert!(token.is_cancelled());

        // Cancelling again should return false (already removed)
        let cancelled_again = cancel_chat_task_token(task_id);
        assert!(!cancelled_again);
    }

    #[test]
    fn test_chat_task_token_empty_id_fallback_single_task() {
        let _guard = lock_and_clear_chat_tasks();
        let token = CancellationToken::new();
        let task_id = "vod_single";

        register_chat_task_token(task_id, token.clone());
        assert!(!token.is_cancelled());

        // Empty task_id should cancel the single registered task
        let cancelled = cancel_chat_task_token("");
        assert!(cancelled);
        assert!(token.is_cancelled());
    }

    #[test]
    fn test_chat_task_token_unregister() {
        let _guard = lock_and_clear_chat_tasks();
        let token = CancellationToken::new();
        let task_id = "vod_unreg";

        register_chat_task_token(task_id, token.clone());
        unregister_chat_task_token(task_id);

        let cancelled = cancel_chat_task_token(task_id);
        assert!(!cancelled);
        assert!(!token.is_cancelled());
    }

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
