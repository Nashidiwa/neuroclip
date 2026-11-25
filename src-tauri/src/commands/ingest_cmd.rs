//! Tauri IPC commands for VOD ingestion and FFmpeg audio extraction pipeline.
//!
//! Complies with docs/CONVENTIONS.md (zero-panic invariant, Result<T, AppError>).

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Mutex;
use tauri::Emitter;
use tokio_util::sync::CancellationToken;

use crate::core::storage::{
    check_disk_space, resolve_cache_dir, resolve_projects_dir, validate_vod_id,
};
use crate::error::{AppError, IngestError, StorageError};
use crate::ingestion::cookies::{parse_browser_type, CookieSource};
use crate::ingestion::local_importer::import_local_video_file;
use crate::ingestion::streamer::{
    compute_audio_energy_stream, detect_source_type, download_audio_stream,
    generate_vod_manifest_with_streams, probe_media_duration_fallback, probe_vod_metadata,
    AudioEnergyWindow, IngestOptions, IngestProgressPayload, VodManifest, VodMetadata,
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
    let Some(opts) = options else {
        return CookieSource::None;
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
    run_vod_ingestion_pipeline(url, options, move |payload: IngestProgressPayload| {
        let _ = app.emit("ingest:progress", &payload);
    })
    .await
}

#[allow(clippy::too_many_lines)]
async fn run_vod_ingestion_pipeline<F>(
    url: String,
    options: Option<IngestOptions>,
    emit_progress: F,
) -> Result<VodManifest, AppError>
where
    F: Fn(IngestProgressPayload) + Send + Sync + Clone + 'static,
{
    let opts = options.unwrap_or_default();
    let cookie_source = resolve_cookie_source(Some(&opts));

    // Register cancellation token immediately so early cancellation works reliably
    let cancellation_token = CancellationToken::new();
    let client_task_id = opts.task_id.as_deref().unwrap_or("").trim().to_string();
    let initial_task_id = if client_task_id.is_empty() {
        format!("task_{}", chrono::Utc::now().timestamp_millis())
    } else {
        client_task_id
    };
    register_task_token(&initial_task_id, cancellation_token.clone());

    // Step 1: Probe metadata (with early cancellation awareness)
    let metadata_res = tokio::select! {
        biased;
        () = cancellation_token.cancelled() => {
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

    let raw_vod_id = if metadata.id.trim().is_empty() {
        initial_task_id.clone()
    } else {
        metadata.id
    };

    let vod_id = match validate_vod_id(&raw_vod_id) {
        Ok(clean_id) => clean_id.to_string(),
        Err(e) => {
            unregister_task_token(&initial_task_id);
            return Err(AppError::Storage(e));
        }
    };

    // Register vod_id alias if distinct from initial_task_id
    if vod_id != initial_task_id {
        register_task_token(&vod_id, cancellation_token.clone());
    }

    // Step 2 & 3: Resolve workspace/cache paths, create directories, and run disk preflight on blocking pool
    let vod_id_for_fs = vod_id.clone();
    let custom_cache_dir = opts.custom_cache_dir.clone();
    let prep_res = tokio::task::spawn_blocking(move || {
        let projects_root = resolve_projects_dir();
        let project_dir = projects_root.join(&vod_id_for_fs);
        if let Err(e) = std::fs::create_dir_all(&project_dir) {
            return Err(AppError::Storage(StorageError::DirectoryCreationFailed {
                path: project_dir.to_string_lossy().to_string(),
                details: e.to_string(),
            }));
        }

        let cache_dir = match custom_cache_dir.as_deref() {
            Some(dir) if !dir.trim().is_empty() => PathBuf::from(dir.trim()),
            _ => resolve_cache_dir(),
        };
        if let Err(e) = std::fs::create_dir_all(&cache_dir) {
            return Err(AppError::Storage(StorageError::DirectoryCreationFailed {
                path: cache_dir.to_string_lossy().to_string(),
                details: e.to_string(),
            }));
        }

        check_disk_space(Some(&cache_dir))?;
        Ok((project_dir, cache_dir))
    })
    .await
    .map_err(|e| AppError::Internal(format!("Ingestion storage prep task panicked: {e}")));

    let (project_dir, cache_dir) = match prep_res {
        Ok(Ok(paths)) => paths,
        Ok(Err(e)) | Err(e) => {
            unregister_task_token(&initial_task_id);
            if vod_id != initial_task_id {
                unregister_task_token(&vod_id);
            }
            return Err(e);
        }
    };

    // Step 4: Download audio stream with 1Hz progress updates keyed by initial_task_id
    let progress_task_id = initial_task_id.clone();
    let emit_for_download = emit_progress.clone();
    let probe_cancel_token = cancellation_token.clone();
    let audio_res = download_audio_stream(
        &url,
        &cache_dir,
        &vod_id,
        &cookie_source,
        opts.custom_user_agent.as_deref(),
        move |mut payload: IngestProgressPayload| {
            payload.task_id.clone_from(&progress_task_id);
            emit_for_download(payload);
        },
        cancellation_token,
    )
    .await;

    unregister_task_token(&initial_task_id);
    if vod_id != initial_task_id {
        unregister_task_token(&vod_id);
    }
    let audio_path = audio_res?;

    // Step 6: Generate Schema 5.2 compliant vod_manifest.json on blocking threadpool
    let source_type = detect_source_type(&url).to_string();
    let mut duration = metadata
        .duration
        .filter(|d| d.is_finite() && *d > 0.0)
        .unwrap_or(0.0);
    if duration <= 0.0 && std::env::var("NEUROCLIP_MOCK_INGEST_RUN").is_err() {
        let probed = probe_media_duration_fallback(&audio_path, &probe_cancel_token).await;
        if probed.is_finite() && probed > 0.0 {
            duration = probed;
        }
    }
    let streamer = metadata.uploader.or(metadata.channel);
    let title = metadata.title.clone();
    let video_stream_url = metadata.video_stream_url.clone();
    let vod_id_manifest = vod_id.clone();

    let manifest = tokio::task::spawn_blocking(move || {
        generate_vod_manifest_with_streams(
            &vod_id_manifest,
            &source_type,
            Some(&url),
            &title,
            streamer.as_deref(),
            duration,
            &audio_path,
            &project_dir,
            video_stream_url.as_deref(),
            None,
        )
    })
    .await
    .map_err(|e| AppError::Internal(format!("Manifest write task panicked: {e}")))??;

    // Emit final 100% progress event keyed by initial_task_id
    emit_progress(IngestProgressPayload {
        task_id: initial_task_id,
        stage: "complete".to_string(),
        percent: 100.0,
        speed_kbps: 0.0,
        eta_sec: 0,
    });

    Ok(manifest)
}

/// Import a local video file completely offline (`TASK-M4-07`).
///
/// Validates extension, probes duration via `ffprobe`, extracts 16kHz mono PCM WAV,
/// and writes Schema 5.2 `vod_manifest.json` (`source_type: "local_file"`).
///
/// # Errors
/// Returns `AppError` if the file is missing/unsupported, storage is insufficient, or `FFmpeg` fails.
#[tauri::command]
pub async fn import_local_video(
    file_path: String,
    custom_cache_dir: Option<String>,
    task_id: Option<String>,
    app: tauri::AppHandle,
) -> Result<VodManifest, AppError> {
    let cancellation_token = CancellationToken::new();
    let tid = task_id
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map_or_else(
            || format!("local_{}", chrono::Utc::now().timestamp_millis()),
            ToString::to_string,
        );
    register_task_token(&tid, cancellation_token.clone());

    let (projects_root, cache_root) = tokio::task::spawn_blocking(move || {
        let projects_root = resolve_projects_dir();
        let cache_root = match custom_cache_dir.as_deref() {
            Some(dir) if !dir.trim().is_empty() => PathBuf::from(dir.trim()),
            _ => resolve_cache_dir(),
        };
        (projects_root, cache_root)
    })
    .await
    .map_err(|e| AppError::Internal(format!("Local import path resolution panicked: {e}")))?;

    let app_clone = app.clone();
    let res = import_local_video_file(
        std::path::Path::new(file_path.trim()),
        &projects_root,
        &cache_root,
        Some(&tid),
        move |payload: IngestProgressPayload| {
            let _ = app_clone.emit("ingest:progress", &payload);
        },
        cancellation_token,
    )
    .await;

    unregister_task_token(&tid);
    res
}

/// Cancel an ongoing VOD ingestion task.
// Owned `String` matches the Tauri IPC command deserialization contract.
#[allow(clippy::needless_pass_by_value)]
#[tauri::command]
pub fn cancel_vod_ingestion(task_id: String) -> Result<bool, AppError> {
    let cancelled = cancel_task_token(&task_id);
    Ok(cancelled)
}

/// Retrieve the Schema 5.2 `vod_manifest.json` for a specific project.
// Owned `String` matches the Tauri IPC command deserialization contract.
#[allow(clippy::needless_pass_by_value)]
#[tauri::command]
pub fn get_vod_manifest(project_id: String) -> Result<VodManifest, AppError> {
    let clean_id = validate_vod_id(&project_id).map_err(AppError::Storage)?;
    let project_dir = resolve_projects_dir().join(clean_id);
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

    let manifest: VodManifest = serde_json::from_slice(&bytes)
        .map_err(|e| AppError::Internal(format!("Failed to parse vod_manifest.json: {e}")))?;

    Ok(manifest)
}

/// Validate `source_path` for `extract_pcm_stream`: reject empty, leading whitespace or `-`,
/// control/`\0` characters; allow `http(s)://` URL or an existing local file.
fn validate_pcm_source_path(source_path: &str) -> Result<String, AppError> {
    if source_path.is_empty() || source_path.trim().is_empty() {
        return Err(AppError::Ingestion(IngestError::InvalidUrl(
            "Source path cannot be empty".to_string(),
        )));
    }
    if source_path.chars().any(|c| c.is_control() || c == '\0') {
        return Err(AppError::Ingestion(IngestError::InvalidUrl(
            "Source path contains control or NUL characters".to_string(),
        )));
    }
    if source_path.chars().next().is_some_and(char::is_whitespace) || source_path.starts_with('-') {
        return Err(AppError::Ingestion(IngestError::InvalidUrl(
            "Source path must not start with whitespace or '-'".to_string(),
        )));
    }
    let trimmed = source_path.trim();
    if trimmed.starts_with("http://") || trimmed.starts_with("https://") {
        return Ok(trimmed.to_string());
    }
    let local_path = std::path::Path::new(trimmed);
    if !local_path.exists() || !local_path.is_file() {
        return Err(AppError::Storage(StorageError::FileNotFound(
            trimmed.to_string(),
        )));
    }
    Ok(trimmed.to_string())
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
    let validated_source =
        tokio::task::spawn_blocking(move || validate_pcm_source_path(&source_path))
            .await
            .map_err(|e| {
                AppError::Internal(format!("Source path validation task panicked: {e}"))
            })??;

    let cancellation_token = CancellationToken::new();
    let tid = task_id.unwrap_or_else(|| format!("pcm_{}", chrono::Utc::now().timestamp_millis()));
    register_task_token(&tid, cancellation_token.clone());

    let win_samples = window_samples.unwrap_or(8000); // 500ms at 16kHz mono
    let res = compute_audio_energy_stream(
        &validated_source,
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

    static TASK_TOKEN_TEST_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

    #[test]
    fn test_task_token_lifecycle() {
        let _guard = TASK_TOKEN_TEST_LOCK
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if let Ok(mut map_lock) = ACTIVE_TASKS.lock() {
            if let Some(map) = map_lock.as_mut() {
                map.clear();
            }
        }

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
        let _guard = TASK_TOKEN_TEST_LOCK
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if let Ok(mut map_lock) = ACTIVE_TASKS.lock() {
            if let Some(map) = map_lock.as_mut() {
                map.clear();
            }
        }

        let token = CancellationToken::new();
        register_task_token("task_fallback", token.clone());
        assert!(!token.is_cancelled());

        let cancelled = cancel_task_token("");
        assert!(cancelled);
        assert!(token.is_cancelled());
    }

    #[test]
    fn test_get_vod_manifest_rejects_traversal_and_absolute_paths() {
        for bad in [
            "",
            "   ",
            "..",
            "../outside",
            "..\\outside",
            "proj/sub",
            "proj\\sub",
            "C:\\Windows\\System32",
            "D:vod_123",
        ] {
            let res = get_vod_manifest(bad.to_string());
            assert!(
                matches!(res, Err(AppError::Storage(_))),
                "Expected Storage error for invalid project_id {bad:?}, got {res:?}"
            );
        }
    }

    #[tokio::test]
    async fn test_extract_pcm_stream_rejects_invalid_source_paths() {
        for bad in [
            "",
            "   ",
            " C:\\video.mp4",
            "--flag",
            "-i",
            "video\0.mp4",
            "video\n.mp4",
        ] {
            let res = extract_pcm_stream(bad.to_string(), None, None, None).await;
            assert!(
                matches!(res, Err(AppError::Ingestion(IngestError::InvalidUrl(_)))),
                "Expected InvalidUrl for {bad:?}, got {res:?}"
            );
        }

        let missing = extract_pcm_stream(
            "C:\\definitely_missing_neuroclip_audio_98765.wav".to_string(),
            None,
            None,
            None,
        )
        .await;
        assert!(matches!(
            missing,
            Err(AppError::Storage(StorageError::FileNotFound(_)))
        ));
    }

    #[tokio::test]
    async fn test_start_vod_ingestion_pipeline_emits_initial_task_id_across_all_stages() {
        use crate::core::storage::test_env::{ScopedEnvVar, ENV_LOCK};
        use std::sync::{Arc, Mutex as StdMutex};

        let _env_guard = ENV_LOCK.lock().await;
        let nanos = chrono::Utc::now().timestamp_nanos_opt().unwrap_or(0);
        let temp_root = std::env::temp_dir().join(format!(
            "neuroclip_ingest_taskid_{}_{nanos}",
            std::process::id()
        ));
        let projects_dir = temp_root.join("projects");
        let cache_dir = temp_root.join("cache");
        std::fs::create_dir_all(&projects_dir).unwrap();
        std::fs::create_dir_all(&cache_dir).unwrap();

        let _proj_env =
            ScopedEnvVar::set("NEUROCLIP_PROJECTS_DIR", &projects_dir.to_string_lossy());
        let _cache_env = ScopedEnvVar::set("NEUROCLIP_CACHE_DIR", &cache_dir.to_string_lossy());
        let _disk_env = ScopedEnvVar::set("NEUROCLIP_MOCK_DISK_FREE_MB", "20480");
        let _meta_env = ScopedEnvVar::set(
            "NEUROCLIP_MOCK_YTDLP_METADATA",
            r#"{"id":"dQw4w9WgXcQ","title":"Contract Test VOD","duration":120.0,"uploader":"Tester"}"#,
        );
        let _ingest_env = ScopedEnvVar::set("NEUROCLIP_MOCK_INGEST_RUN", "1");

        let emitted: Arc<StdMutex<Vec<IngestProgressPayload>>> =
            Arc::new(StdMutex::new(Vec::new()));
        let emitted_clone = Arc::clone(&emitted);

        let opts = IngestOptions {
            audio_only: true,
            cookies_path: None,
            browser_cookies: None,
            custom_user_agent: None,
            custom_cache_dir: None,
            task_id: Some("task_client_1795000_abc123".to_string()),
        };

        let manifest = run_vod_ingestion_pipeline(
            "https://www.youtube.com/watch?v=dQw4w9WgXcQ".to_string(),
            Some(opts),
            move |payload| {
                if let Ok(mut guard) = emitted_clone.lock() {
                    guard.push(payload);
                }
            },
        )
        .await
        .expect("Ingestion pipeline must succeed in mock mode");

        // Manifest and storage paths retain the validated VOD ID
        assert_eq!(manifest.vod_id, "dQw4w9WgXcQ");
        assert!(projects_dir
            .join("dQw4w9WgXcQ")
            .join("vod_manifest.json")
            .exists());
        assert!(cache_dir.join("dQw4w9WgXcQ_audio.m4a").exists());

        // Every progress payload (downloading_audio + complete) must carry initial_task_id
        let events = emitted.lock().unwrap().clone();
        assert!(
            events.len() >= 2,
            "Expected downloading_audio and complete payloads, got {events:?}"
        );
        assert!(
            events
                .iter()
                .all(|e| e.task_id == "task_client_1795000_abc123"),
            "Every ingest:progress payload must carry caller task_id, got: {events:?}"
        );
        assert_eq!(events[0].stage, "downloading_audio");
        let last = events.last().unwrap();
        assert_eq!(last.stage, "complete");
        assert!((last.percent - 100.0).abs() < 1e-4);

        let _ = std::fs::remove_dir_all(&temp_root);
    }

    #[cfg(windows)]
    #[tokio::test]
    async fn test_start_vod_ingestion_pipeline_live_ytdlp_stdout_emits_initial_task_id() {
        use crate::core::storage::test_env::{ScopedEnvVar, ENV_LOCK};
        use std::sync::{Arc, Mutex as StdMutex};

        let _env_guard = ENV_LOCK.lock().await;
        let nanos = chrono::Utc::now().timestamp_nanos_opt().unwrap_or(0);
        let temp_root = std::env::temp_dir().join(format!(
            "neuroclip_ingest_ytdlp_taskid_{}_{nanos}",
            std::process::id()
        ));
        let projects_dir = temp_root.join("projects");
        let cache_dir = temp_root.join("cache");
        std::fs::create_dir_all(&projects_dir).unwrap();
        std::fs::create_dir_all(&cache_dir).unwrap();

        let out_m4a = cache_dir.join("yt_vid_789_audio.m4a");
        let ytdlp_cmd = temp_root.join("mock_ytdlp_progress.cmd");
        std::fs::write(
            &ytdlp_cmd,
            format!(
                "@echo off\r\n\
                 echo [download]  100.0%% of 12.00MiB at 4.50MiB/s ETA 00:00\r\n\
                 echo MOCK_AUDIO> \"{}\"\r\n\
                 exit /b 0\r\n",
                out_m4a.display()
            ),
        )
        .unwrap();

        let _proj_env =
            ScopedEnvVar::set("NEUROCLIP_PROJECTS_DIR", &projects_dir.to_string_lossy());
        let _cache_env = ScopedEnvVar::set("NEUROCLIP_CACHE_DIR", &cache_dir.to_string_lossy());
        let _disk_env = ScopedEnvVar::set("NEUROCLIP_MOCK_DISK_FREE_MB", "20480");
        let _meta_env = ScopedEnvVar::set(
            "NEUROCLIP_MOCK_YTDLP_METADATA",
            r#"{"id":"yt_vid_789","title":"Live Stdout Contract VOD","duration":60.0}"#,
        );
        let _ingest_remove = ScopedEnvVar::remove("NEUROCLIP_MOCK_INGEST_RUN");
        let _ytdlp_env = ScopedEnvVar::set("NEUROCLIP_YTDLP_PATH", &ytdlp_cmd.to_string_lossy());

        let emitted: Arc<StdMutex<Vec<IngestProgressPayload>>> =
            Arc::new(StdMutex::new(Vec::new()));
        let emitted_clone = Arc::clone(&emitted);

        let opts = IngestOptions {
            audio_only: true,
            cookies_path: None,
            browser_cookies: None,
            custom_user_agent: None,
            custom_cache_dir: None,
            task_id: Some("task_live_stdout_456".to_string()),
        };

        let manifest = run_vod_ingestion_pipeline(
            "https://www.youtube.com/watch?v=yt_vid_789".to_string(),
            Some(opts),
            move |payload| {
                if let Ok(mut guard) = emitted_clone.lock() {
                    guard.push(payload);
                }
            },
        )
        .await
        .expect("Ingestion pipeline with mock yt-dlp stdout must succeed");

        assert_eq!(manifest.vod_id, "yt_vid_789");
        let events = emitted.lock().unwrap().clone();
        assert_eq!(events.len(), 2, "Expected 2 events, got {events:?}");
        assert_eq!(events[0].task_id, "task_live_stdout_456");
        assert_eq!(events[0].stage, "downloading_audio");
        assert_eq!(events[1].task_id, "task_live_stdout_456");
        assert_eq!(events[1].stage, "complete");

        let _ = std::fs::remove_dir_all(&temp_root);
    }
}
