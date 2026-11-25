//! Tauri IPC commands for Multimodal Highlight Detection, Clustering & Bookmark Management.
//!
//! Complies with `docs/CONVENTIONS.md` (zero-panic invariant, `Result<T, AppError>`),
//! `docs/ARCHITECTURE.md` §4.3 & §5.4 Schema 5.4, and `docs/TASK_BOARD.md` TASK-M4-12.

use std::path::{Path, PathBuf};

use crate::core::storage::{
    ensure_path_within_roots, resolve_cache_dir, resolve_projects_dir, resolve_vod_project_dir,
    validate_vod_id,
};
use crate::error::{AppError, HighlightError, StorageError};
use crate::highlight::audio_rms::{DEFAULT_BLOCK_DURATION_SEC, DEFAULT_SAMPLE_RATE};
use crate::highlight::cluster::{
    generate_clusters_from_fused_series, HighlightCluster, HighlightClustersContainer,
};
use crate::highlight::emote_filter::compute_emote_density_series_from_file;
use crate::highlight::energy_analyzer::{
    analyze_wav_file, detect_energy_peaks_default, process_pcm_reader, AudioRmsPoint,
};
use crate::highlight::fusion::fuse_multimodal_series;
use crate::highlight::zscore::compute_chat_velocity_series_from_file;
use crate::ingestion::streamer::VodManifest;

struct StdChildGuard(Option<std::process::Child>);

impl Drop for StdChildGuard {
    fn drop(&mut self) {
        if let Some(mut child) = self.0.take() {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}

/// Analyze an audio file (WAV or compressed formats like `.aac`/`.m4a`/`.opus`/`.webm`)
/// into 500ms `AudioRmsPoint` blocks using bounded-memory streaming.
pub fn analyze_audio_file_for_highlights(audio_path: &Path) -> Vec<AudioRmsPoint> {
    if let Ok(points) = analyze_wav_file(audio_path) {
        if !points.is_empty() {
            return points;
        }
    }

    if !audio_path.is_file() {
        return Vec::new();
    }

    let ffmpeg_bin = crate::ingestion::range_slicer::resolve_ffmpeg_binary();
    let mut cmd = std::process::Command::new(&ffmpeg_bin);
    cmd.args(["-hide_banner", "-loglevel", "error", "-nostdin", "-i"])
        .arg(audio_path.as_os_str())
        .args([
            "-vn", "-sn", "-dn", "-f", "s16le", "-ac", "1", "-ar", "16000", "pipe:1",
        ])
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null());

    let Ok(mut child) = cmd.spawn() else {
        return Vec::new();
    };
    let Some(stdout) = child.stdout.take() else {
        let _ = child.kill();
        let _ = child.wait();
        return Vec::new();
    };
    let mut guard = StdChildGuard(Some(child));

    let blocks_res = process_pcm_reader(stdout, DEFAULT_SAMPLE_RATE, DEFAULT_BLOCK_DURATION_SEC);
    let status_ok = guard
        .0
        .take()
        .is_some_and(|mut c| c.wait().is_ok_and(|s| s.success()));

    if !status_ok {
        return Vec::new();
    }

    match blocks_res {
        Ok(blocks) if !blocks.is_empty() => {
            detect_energy_peaks_default(&blocks).unwrap_or_default()
        }
        _ => Vec::new(),
    }
}

fn run_highlight_detection_blocking(
    vod_id: &str,
    duration_seconds: f64,
    project_dir: &Path,
    resolved_chat_path: Option<&Path>,
    resolved_audio_path: Option<&Path>,
) -> Result<HighlightClustersContainer, AppError> {
    let duration = if duration_seconds.is_finite() && duration_seconds > 0.0 {
        duration_seconds
    } else {
        0.0
    };

    // 1. Chat Velocity Series (TASK-M4-09)
    let velocity_series = if let Some(chat_path) = resolved_chat_path {
        compute_chat_velocity_series_from_file(chat_path, 5.0, 60.0, Some(duration))
            .unwrap_or_default()
    } else {
        Vec::new()
    };

    // 2. Swarm Emote Density Series (TASK-M4-10)
    let emote_series = if let Some(chat_path) = resolved_chat_path {
        compute_emote_density_series_from_file(chat_path, None, 5.0, Some(duration))
            .unwrap_or_default()
    } else {
        Vec::new()
    };

    // 3. Audio RMS Series (TASK-M4-11; supports both WAV and compressed audio via streaming FFmpeg)
    let audio_series = if let Some(audio_path) = resolved_audio_path {
        analyze_audio_file_for_highlights(audio_path)
    } else {
        Vec::new()
    };

    // 4. Defensive check: If all inputs are empty, return NoHighlightsFound error
    if velocity_series.is_empty() && emote_series.is_empty() && audio_series.is_empty() {
        return Err(AppError::Highlight(HighlightError::NoHighlightsFound));
    }

    // 5. Multimodal Fusion (TASK-M4-12)
    let fused_points =
        fuse_multimodal_series(&velocity_series, &emote_series, &audio_series, duration)?;

    // 6. Highlight Clustering & Deduplication
    let clusters_container = generate_clusters_from_fused_series(vod_id, &fused_points, duration)?;

    // 7. Write to highlight_clusters.json adhering 100% to Schema 5.4
    let output_json_path = project_dir.join("highlight_clusters.json");
    let serialized = serde_json::to_vec_pretty(&clusters_container).map_err(|e| {
        AppError::Internal(format!("Failed to serialize highlight_clusters.json: {e}"))
    })?;

    std::fs::write(&output_json_path, serialized).map_err(|e| {
        AppError::Internal(format!(
            "Failed to write highlight_clusters.json at '{}': {e}",
            output_json_path.display()
        ))
    })?;

    Ok(clusters_container)
}

/// Detect and cluster highlights for a VOD using multimodal fusion.
///
/// Combines Chat Velocity ($Z_{\text{adj}}$), Swarm Emote Density ($E_{\text{ratio}}$),
/// and Audio RMS energy into a composite score, compensates 4.5s livestream delay,
/// merges peaks within 30s, suppresses overlaps > 50%, and writes `highlight_clusters.json`.
///
/// # Errors
/// Returns `AppError` if `vod_id` is invalid, file reading, audio analysis, fusion processing, or serialization fails.
#[tauri::command]
pub async fn detect_highlights(
    vod_id: String,
    duration_seconds: f64,
    chat_events_path: Option<String>,
    audio_path: Option<String>,
) -> Result<HighlightClustersContainer, AppError> {
    tokio::task::spawn_blocking(move || {
        let clean_id = validate_vod_id(&vod_id)
            .map_err(AppError::Storage)?
            .to_string();
        let project_dir = resolve_vod_project_dir(&clean_id)?;
        std::fs::create_dir_all(&project_dir).map_err(|e| {
            AppError::Storage(StorageError::DirectoryCreationFailed {
                path: project_dir.display().to_string(),
                details: e.to_string(),
            })
        })?;

        let projects_vod_dir = resolve_projects_dir().join(&clean_id);
        let cache_root = resolve_cache_dir();
        let allowed_roots = [
            project_dir.as_path(),
            projects_vod_dir.as_path(),
            cache_root.as_path(),
        ];

        // Determine paths for chat and audio (strictly contained in VOD workspace or cache)
        let resolved_chat_path: Option<PathBuf> = if let Some(raw_path) = chat_events_path
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
        {
            Some(ensure_path_within_roots(
                Path::new(raw_path),
                &allowed_roots,
            )?)
        } else {
            let p_json = project_dir.join("chat_events.json");
            if p_json.exists() {
                Some(p_json)
            } else {
                let p_ndjson = project_dir.join("chat_events.ndjson");
                if p_ndjson.exists() {
                    Some(p_ndjson)
                } else {
                    None
                }
            }
        };

        let resolved_audio_path: Option<PathBuf> = if let Some(raw_path) = audio_path
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
        {
            Some(ensure_path_within_roots(
                Path::new(raw_path),
                &allowed_roots,
            )?)
        } else {
            let p_wav = project_dir.join(format!("{clean_id}.audio.wav"));
            if p_wav.exists() {
                Some(p_wav)
            } else {
                let manifest_path = project_dir.join("vod_manifest.json");
                let mut manifest_audio: Option<PathBuf> = None;
                if manifest_path.exists() {
                    if let Ok(bytes) = std::fs::read(&manifest_path) {
                        if let Ok(manifest) = serde_json::from_slice::<VodManifest>(&bytes) {
                            let p = PathBuf::from(manifest.audio_path);
                            if p.exists() {
                                manifest_audio = Some(p);
                            }
                        }
                    }
                }
                manifest_audio
            }
        };

        run_highlight_detection_blocking(
            &clean_id,
            duration_seconds,
            &project_dir,
            resolved_chat_path.as_deref(),
            resolved_audio_path.as_deref(),
        )
    })
    .await
    .map_err(|e| AppError::Highlight(HighlightError::WorkerPanicked(e.to_string())))?
}

/// Retrieve saved highlight clusters from `highlight_clusters.json` for a VOD.
///
/// # Errors
/// Returns `AppError::Storage` if `vod_id` is invalid or file is missing, or `AppError::Highlight` if JSON parsing fails.
#[tauri::command]
pub async fn get_highlight_clusters(
    vod_id: String,
) -> Result<HighlightClustersContainer, AppError> {
    tokio::task::spawn_blocking(move || {
        let project_dir = resolve_vod_project_dir(&vod_id)?;
        let output_json_path = project_dir.join("highlight_clusters.json");

        if !output_json_path.exists() {
            return Err(AppError::Storage(StorageError::FileNotFound(
                output_json_path.display().to_string(),
            )));
        }

        let bytes = std::fs::read(&output_json_path).map_err(|e| {
            AppError::Storage(StorageError::FileNotFound(format!(
                "Failed to read highlight_clusters.json: {e}"
            )))
        })?;

        let container: HighlightClustersContainer =
            serde_json::from_slice(&bytes).map_err(|e| {
                AppError::Highlight(HighlightError::ChatParsingFailed(format!(
                    "Failed to parse highlight_clusters.json: {e}"
                )))
            })?;

        Ok(container)
    })
    .await
    .map_err(|e| AppError::Highlight(HighlightError::WorkerPanicked(e.to_string())))?
}

/// Toggle or set bookmark status on a specific highlight cluster.
///
/// # Errors
/// Returns `AppError::Storage` if file is missing or cluster ID is not found.
#[tauri::command]
pub async fn toggle_highlight_bookmark(
    vod_id: String,
    cluster_id: String,
    bookmarked: Option<bool>,
) -> Result<HighlightCluster, AppError> {
    tokio::task::spawn_blocking(move || {
        let project_dir = resolve_vod_project_dir(&vod_id)?;
        let output_json_path = project_dir.join("highlight_clusters.json");

        if !output_json_path.exists() {
            return Err(AppError::Storage(StorageError::FileNotFound(
                output_json_path.display().to_string(),
            )));
        }

        let bytes = std::fs::read(&output_json_path).map_err(|e| {
            AppError::Storage(StorageError::FileNotFound(format!(
                "Failed to read highlight_clusters.json: {e}"
            )))
        })?;

        let mut container: HighlightClustersContainer =
            serde_json::from_slice(&bytes).map_err(|e| {
                AppError::Highlight(HighlightError::ChatParsingFailed(format!(
                    "Failed to parse highlight_clusters.json: {e}"
                )))
            })?;

        let mut updated_cluster: Option<HighlightCluster> = None;

        for cluster in &mut container.clusters {
            if cluster.cluster_id == cluster_id {
                let next_state = bookmarked.unwrap_or(!cluster.is_bookmarked);
                cluster.is_bookmarked = next_state;
                updated_cluster = Some(cluster.clone());
                break;
            }
        }

        let result = updated_cluster.ok_or_else(|| {
            AppError::Storage(StorageError::FileNotFound(format!(
                "Cluster with ID '{cluster_id}' not found in {vod_id}"
            )))
        })?;

        // Persist updated container back to disk
        let serialized = serde_json::to_vec_pretty(&container).map_err(|e| {
            AppError::Internal(format!("Failed to serialize highlight_clusters.json: {e}"))
        })?;

        std::fs::write(&output_json_path, serialized).map_err(|e| {
            AppError::Internal(format!(
                "Failed to write updated highlight_clusters.json at '{}': {e}",
                output_json_path.display()
            ))
        })?;

        Ok(result)
    })
    .await
    .map_err(|e| AppError::Highlight(HighlightError::WorkerPanicked(e.to_string())))?
}

/// Alias command matching frontend IPC call `toggle_cluster_bookmark`.
///
/// # Errors
/// Returns `AppError` if underlying bookmark update fails.
#[tauri::command]
pub async fn toggle_cluster_bookmark(
    vod_id: String,
    cluster_id: String,
    bookmarked: Option<bool>,
) -> Result<HighlightCluster, AppError> {
    toggle_highlight_bookmark(vod_id, cluster_id, bookmarked).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_highlight_commands_reject_invalid_vod_id() {
        for bad in ["../escape", "C:\\escape", "vod/sub", "vod\0id", ""] {
            let detect_res = detect_highlights(bad.to_string(), 60.0, None, None).await;
            assert!(
                matches!(
                    detect_res,
                    Err(AppError::Storage(
                        StorageError::DirectoryCreationFailed { .. }
                    ))
                ),
                "detect_highlights must reject {bad:?}, got {detect_res:?}"
            );

            let get_res = get_highlight_clusters(bad.to_string()).await;
            assert!(
                matches!(
                    get_res,
                    Err(AppError::Storage(
                        StorageError::DirectoryCreationFailed { .. }
                    ))
                ),
                "get_highlight_clusters must reject {bad:?}, got {get_res:?}"
            );

            let toggle_res =
                toggle_highlight_bookmark(bad.to_string(), "c1".to_string(), None).await;
            assert!(
                matches!(
                    toggle_res,
                    Err(AppError::Storage(
                        StorageError::DirectoryCreationFailed { .. }
                    ))
                ),
                "toggle_highlight_bookmark must reject {bad:?}, got {toggle_res:?}"
            );
        }
    }

    #[tokio::test]
    async fn test_detect_highlights_rejects_paths_outside_workspace() {
        let _env_guard = crate::core::storage::test_env::ENV_LOCK.lock().await;
        let root =
            std::env::temp_dir().join(format!("neuroclip_hl_containment_{}", std::process::id()));
        let projects_root = root.join("projects");
        let cache_root = root.join("cache");
        let outside_dir = root.join("outside");
        let _ = std::fs::create_dir_all(&projects_root);
        let _ = std::fs::create_dir_all(&cache_root);
        let _ = std::fs::create_dir_all(&outside_dir);
        let _p_env = crate::core::storage::test_env::ScopedEnvVar::set(
            "NEUROCLIP_PROJECTS_DIR",
            &projects_root.to_string_lossy(),
        );
        let _c_env = crate::core::storage::test_env::ScopedEnvVar::set(
            "NEUROCLIP_CACHE_DIR",
            &cache_root.to_string_lossy(),
        );

        let outside_chat = outside_dir.join("secret.json");
        let outside_audio = outside_dir.join("secret.wav");

        let chat_res = detect_highlights(
            "vod_containment_test".to_string(),
            60.0,
            Some(outside_chat.to_string_lossy().to_string()),
            None,
        )
        .await;
        assert!(matches!(
            chat_res,
            Err(AppError::Storage(
                StorageError::DirectoryCreationFailed { .. }
            ))
        ));

        let audio_res = detect_highlights(
            "vod_containment_test".to_string(),
            60.0,
            None,
            Some(outside_audio.to_string_lossy().to_string()),
        )
        .await;
        assert!(matches!(
            audio_res,
            Err(AppError::Storage(
                StorageError::DirectoryCreationFailed { .. }
            ))
        ));

        let traversal_res = detect_highlights(
            "vod_containment_test".to_string(),
            60.0,
            Some("../../etc/passwd".to_string()),
            None,
        )
        .await;
        assert!(matches!(
            traversal_res,
            Err(AppError::Storage(
                StorageError::DirectoryCreationFailed { .. }
            ))
        ));

        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn test_analyze_audio_file_for_highlights_wav_and_missing() {
        let missing = PathBuf::from("C:\\definitely_missing_neuroclip_highlight_audio_12345.aac");
        assert_eq!(analyze_audio_file_for_highlights(&missing), Vec::new());

        let root = std::env::temp_dir().join(format!(
            "neuroclip_hl_audio_test_{}_{}",
            std::process::id(),
            chrono::Utc::now().timestamp_nanos_opt().unwrap_or(0)
        ));
        let _ = std::fs::create_dir_all(&root);
        let wav_path = root.join("sample.wav");

        // 1.0 second of 16kHz 16-bit mono PCM (32,000 bytes -> 2 blocks of 500ms)
        let sample_count: usize = 16_000;
        let data_len = u32::try_from(sample_count * 2).unwrap();
        let mut bytes = Vec::with_capacity(44 + data_len as usize);
        bytes.extend_from_slice(b"RIFF");
        bytes.extend_from_slice(&(36 + data_len).to_le_bytes());
        bytes.extend_from_slice(b"WAVEfmt ");
        bytes.extend_from_slice(&16u32.to_le_bytes());
        bytes.extend_from_slice(&1u16.to_le_bytes());
        bytes.extend_from_slice(&1u16.to_le_bytes());
        bytes.extend_from_slice(&16_000u32.to_le_bytes());
        bytes.extend_from_slice(&32_000u32.to_le_bytes());
        bytes.extend_from_slice(&2u16.to_le_bytes());
        bytes.extend_from_slice(&16u16.to_le_bytes());
        bytes.extend_from_slice(b"data");
        bytes.extend_from_slice(&data_len.to_le_bytes());
        for _ in 0..sample_count {
            bytes.extend_from_slice(&1000i16.to_le_bytes());
        }
        std::fs::write(&wav_path, &bytes).unwrap();

        let points = analyze_audio_file_for_highlights(&wav_path);
        assert_eq!(points.len(), 2);

        let _ = std::fs::remove_dir_all(&root);
    }
}
