//! Tauri IPC commands for VOD ingestion and FFmpeg audio extraction pipeline.
//!
//! Complies with docs/CONVENTIONS.md (zero-panic invariant, Result<T, AppError>).

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Mutex;
use tauri::Emitter;
use tokio_util::sync::CancellationToken;

use crate::core::storage::{check_disk_space, resolve_cache_dir, resolve_projects_dir};
use crate::error::{AppError, StorageError};
use crate::ingestion::cookies::{parse_browser_type, CookieSource};
use crate::ingestion::streamer::{
    compute_audio_energy_stream, detect_source_type, download_audio_stream, generate_vod_manifest,
    probe_vod_metadata, AudioEnergyWindow, IngestOptions, IngestProgressPayload, VodManifest,
    VodMetadata,
};

static ACTIVE_TASKS: Mutex<Option<HashMap<String, CancellationToken>>> = Mutex::new(None);

fn register_task_token(task_id: &str, token: CancellationToken) {
    if let Ok(mut lock) = ACTIVE_TASKS.lock() {
        let map = lock.get_or_insert_with(HashMap::new);
        map.insert(task_id.to_string(), token);
    }
}

fn unregister_task_token(task_id: &str) {
    if let Ok(mut lock) = ACTIVE_TASKS.lock() {
        if let Some(map) = lock.as_mut() {
            map.remove(task_id);
        }
    }
}

fn cancel_task_token(task_id: &str) -> bool {
    if let Ok(mut lock) = ACTIVE_TASKS.lock() {
        if let Some(map) = lock.as_mut() {
            let key = task_id.trim();
            if !key.is_empty() {
                if let Some(token) = map.remove(key) {
                    token.cancel();
                    return true;
                }
            } else if map.len() == 1 {
                // If task_id is empty, cancel the single running active task as fallback
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

fn resolve_cookie_source(options: Option<&IngestOptions>) -> CookieSource {
    let opts = match options {
        Some(o) => o,
        None => return CookieSource::None,
    };

    if let Some(ref path) = opts.cookies_path {
        if !path.trim().is_empty() {
            return CookieSource::File(path.trim().to_string());
        }
    }

    if let Some(ref browser_str) = opts.browser_cookies {
        if let Some(browser) = parse_browser_type(browser_str) {
            return CookieSource::Browser(browser);
        }
    }

    CookieSource::None
}

/// Probe online VOD metadata without downloading media.
///
/// Returns title, duration, uploader, and rejects live streams immediately.
#[tauri::command]
pub async fn probe_vod(
    url: String,
    cookies_path: Option<String>,
    browser_cookies: Option<String>,
) -> Result<VodMetadata, AppError> {
    let opts = IngestOptions {
        audio_only: true,
        cookies_path,
        browser_cookies,
        custom_user_agent: None,
        custom_cache_dir: None,
        task_id: None,
    };
    let cookie_source = resolve_cookie_source(Some(&opts));
    probe_vod_metadata(&url, &cookie_source, None).await
}

/// Start full Audio-First VOD ingestion pipeline.
///
/// Steps:
/// 1. Validate URL & probe metadata (rejects live streams <= 500ms).
/// 2. Pre-flight storage check (requires >= 5GB disk space).
/// 3. Download audio-only stream with yt-dlp anti-bot defense.
/// 4. Generate Schema 5.2 compliant `vod_manifest.json`.
#[tauri::command]
pub async fn start_vod_ingestion(
    url: String,
    options: Option<IngestOptions>,
    app: tauri::AppHandle,
) -> Result<VodManifest, AppError> {
    let opts = options.unwrap_or_default();
    let cookie_source = resolve_cookie_source(Some(&opts));

    // Register cancellation token immediately so early cancellation works reliably
    let cancellation_token = CancellationToken::new();
    let client_task_id = opts.task_id.as_deref().unwrap_or("").trim().to_string();
    let initial_task_id = if !client_task_id.is_empty() {
        client_task_id
    } else {
        format!("task_{}", chrono::Utc::now().timestamp_millis())
    };
    register_task_token(&initial_task_id, cancellation_token.clone());

    // Step 1: Probe metadata (with early cancellation awareness)
    let metadata_res = tokio::select! {
        biased;
        _ = cancellation_token.cancelled() => {
            unregister_task_token(&initial_task_id);
            return Err(AppError::TaskCancelled);
        }
        res = probe_vod_metadata(&url, &cookie_source, opts.custom_user_agent.as_deref()) => res,
    };

    let metadata = match metadata_res {
        Ok(m) => m,
        Err(e) => {
            unregister_task_token(&initial_task_id);
            return Err(e);
        }
    };

    let vod_id = if metadata.id.trim().is_empty() {
        initial_task_id.clone()
    } else {
        metadata.id
    };

    // Register vod_id alias if distinct from initial_task_id
    if vod_id != initial_task_id {
        register_task_token(&vod_id, cancellation_token.clone());
    }

    // Step 2: Resolve workspace and cache paths
    let projects_dir = resolve_projects_dir();
    let project_dir = projects_dir.join(&vod_id);
    if let Err(e) = std::fs::create_dir_all(&project_dir) {
        unregister_task_token(&initial_task_id);
        if vod_id != initial_task_id {
            unregister_task_token(&vod_id);
        }
        return Err(AppError::Storage(StorageError::DirectoryCreationFailed {
            path: project_dir.to_string_lossy().to_string(),
            details: e.to_string(),
        }));
    }

    let cache_dir = match opts.custom_cache_dir.as_deref() {
        Some(dir) if !dir.trim().is_empty() => PathBuf::from(dir.trim()),
        _ => resolve_cache_dir(),
    };
    if let Err(e) = std::fs::create_dir_all(&cache_dir) {
        unregister_task_token(&initial_task_id);
        if vod_id != initial_task_id {
            unregister_task_token(&vod_id);
        }
        return Err(AppError::Storage(StorageError::DirectoryCreationFailed {
            path: cache_dir.to_string_lossy().to_string(),
            details: e.to_string(),
        }));
    }

    // Step 3: Pre-flight disk space check
    if let Err(e) = check_disk_space(Some(&cache_dir)) {
        unregister_task_token(&initial_task_id);
        if vod_id != initial_task_id {
            unregister_task_token(&vod_id);
        }
        return Err(e);
    }

    // Step 4: Download audio stream with 1Hz progress updates
    let app_clone = app.clone();
    let audio_res = download_audio_stream(
        &url,
        &cache_dir,
        &vod_id,
        &cookie_source,
        opts.custom_user_agent.as_deref(),
        move |payload: IngestProgressPayload| {
            let _ = app_clone.emit("ingest:progress", &payload);
        },
        cancellation_token,
    )
    .await;

    unregister_task_token(&initial_task_id);
    if vod_id != initial_task_id {
        unregister_task_token(&vod_id);
    }
    let audio_path = audio_res?;

    // Step 6: Generate Schema 5.2 compliant vod_manifest.json
    let source_type = detect_source_type(&url);
    let duration = metadata.duration.unwrap_or(0.0);
    let streamer = metadata.uploader.or(metadata.channel);

    let manifest = generate_vod_manifest(
        &vod_id,
        source_type,
        Some(&url),
        &metadata.title,
        streamer.as_deref(),
        duration,
        &audio_path,
        &project_dir,
    )?;

    // Emit final 100% progress event
    let _ = app.emit(
        "ingest:progress",
        &IngestProgressPayload {
            task_id: vod_id,
            stage: "complete".to_string(),
            percent: 100.0,
            speed_kbps: 0.0,
            eta_sec: 0,
        },
    );

    Ok(manifest)
}

/// Cancel an ongoing VOD ingestion task.
#[tauri::command]
pub fn cancel_vod_ingestion(task_id: String) -> Result<bool, AppError> {
    let cancelled = cancel_task_token(&task_id);
    Ok(cancelled)
}

/// Retrieve the Schema 5.2 `vod_manifest.json` for a specific project.
#[tauri::command]
pub fn get_vod_manifest(project_id: String) -> Result<VodManifest, AppError> {
    let project_dir = resolve_projects_dir().join(&project_id);
    let manifest_path = project_dir.join("vod_manifest.json");

    if !manifest_path.exists() {
        return Err(AppError::Storage(StorageError::FileNotFound(
            manifest_path.to_string_lossy().to_string(),
        )));
    }

    let bytes = std::fs::read(&manifest_path).map_err(|e| {
        AppError::Storage(StorageError::FileNotFound(format!(
            "Failed to read vod_manifest.json: {e}"
        )))
    })?;

    let manifest: VodManifest = serde_json::from_slice(&bytes).map_err(|e| {
        AppError::Internal(format!("Failed to parse vod_manifest.json: {e}"))
    })?;

    Ok(manifest)
}

/// Stream audio from input source (file or URL) through FFmpeg 16kHz mono pipe,
/// processing samples via `AudioRingBuffer` sliding window to compute RMS energy telemetry.
#[tauri::command]
pub async fn extract_pcm_stream(
    source_path: String,
    window_samples: Option<usize>,
    max_windows: Option<usize>,
    task_id: Option<String>,
) -> Result<Vec<AudioEnergyWindow>, AppError> {
    let cancellation_token = CancellationToken::new();
    let tid = task_id.unwrap_or_else(|| format!("pcm_{}", chrono::Utc::now().timestamp_millis()));
    register_task_token(&tid, cancellation_token.clone());

    let win_samples = window_samples.unwrap_or(8000); // 500ms at 16kHz mono
    let res = compute_audio_energy_stream(
        &source_path,
        win_samples,
        max_windows,
        cancellation_token,
    )
    .await;

    unregister_task_token(&tid);
    res
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ingestion::cookies::BrowserType;

    #[test]
    fn test_resolve_cookie_source() {
        let opts_browser = IngestOptions {
            audio_only: true,
            cookies_path: None,
            browser_cookies: Some("chrome".to_string()),
            custom_user_agent: None,
            custom_cache_dir: None,
            task_id: None,
        };
        assert_eq!(
            resolve_cookie_source(Some(&opts_browser)),
            CookieSource::Browser(BrowserType::Chrome)
        );

        let opts_file = IngestOptions {
            audio_only: true,
            cookies_path: Some("C:\\cookies.txt".to_string()),
            browser_cookies: None,
            custom_user_agent: None,
            custom_cache_dir: None,
            task_id: None,
        };
        assert_eq!(
            resolve_cookie_source(Some(&opts_file)),
            CookieSource::File("C:\\cookies.txt".to_string())
        );

        assert_eq!(resolve_cookie_source(None), CookieSource::None);
    }

    #[test]
    fn test_task_token_lifecycle() {
        let token = CancellationToken::new();
        register_task_token("task_123", token.clone());
        assert!(!token.is_cancelled());

        let cancelled = cancel_task_token("task_123");
        assert!(cancelled);
        assert!(token.is_cancelled());

        // Cancel again returns false
        assert!(!cancel_task_token("task_123"));
    }

    #[test]
    fn test_task_token_empty_id_fallback() {
        let token = CancellationToken::new();
        register_task_token("task_fallback", token.clone());
        assert!(!token.is_cancelled());

        let cancelled = cancel_task_token("");
        assert!(cancelled);
        assert!(token.is_cancelled());
    }
}
