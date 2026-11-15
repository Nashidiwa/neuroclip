//! Tauri IPC commands for Multimodal Highlight Detection, Clustering & Bookmark Management.
//!
//! Complies with `docs/CONVENTIONS.md` (zero-panic invariant, Result<T, AppError>),
//! `docs/ARCHITECTURE.md` §4.3 & §5.4 Schema 5.4, and `docs/TASK_BOARD.md` TASK-M4-12.

use std::path::PathBuf;

use crate::core::storage::{resolve_cache_dir, resolve_projects_dir};
use crate::error::{AppError, HighlightError, StorageError};
use crate::highlight::cluster::{
    generate_clusters_from_fused_series, HighlightCluster, HighlightClustersContainer,
};
use crate::highlight::emote_filter::compute_emote_density_series_from_file;
use crate::highlight::energy_analyzer::analyze_wav_file;
use crate::highlight::fusion::fuse_multimodal_series;
use crate::highlight::zscore::compute_chat_velocity_series_from_file;
use crate::ingestion::streamer::VodManifest;

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

/// Detect and cluster highlights for a VOD using multimodal fusion.
///
/// Combines Chat Velocity ($Z_{\text{adj}}$), Swarm Emote Density ($E_{\text{ratio}}$),
/// and Audio RMS energy into a composite score, compensates 4.5s livestream delay,
/// merges peaks within 30s, suppresses overlaps > 50%, and writes `highlight_clusters.json`.
///
/// # Errors
/// Returns `AppError` if file reading, audio analysis, fusion processing, or serialization fails.
#[tauri::command]
pub async fn detect_highlights(
    vod_id: String,
    duration_seconds: f64,
    chat_events_path: Option<String>,
    audio_path: Option<String>,
) -> Result<HighlightClustersContainer, AppError> {
    let project_dir = resolve_vod_project_dir(&vod_id);
    std::fs::create_dir_all(&project_dir).map_err(|e| {
        AppError::Storage(StorageError::DirectoryCreationFailed {
            path: project_dir.display().to_string(),
            details: e.to_string(),
        })
    })?;

    // Determine paths for chat and audio
    let resolved_chat_path: Option<PathBuf> = chat_events_path
        .map(PathBuf::from)
        .or_else(|| {
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
        });

    let resolved_audio_path: Option<PathBuf> = audio_path
        .map(PathBuf::from)
        .or_else(|| {
            let p_wav = project_dir.join(format!("{vod_id}.audio.wav"));
            if p_wav.exists() {
                Some(p_wav)
            } else {
                let manifest_path = project_dir.join("vod_manifest.json");
                if manifest_path.exists() {
                    if let Ok(bytes) = std::fs::read(&manifest_path) {
                        if let Ok(manifest) = serde_json::from_slice::<VodManifest>(&bytes) {
                            let p = PathBuf::from(manifest.audio_path);
                            if p.exists() {
                                return Some(p);
                            }
                        }
                    }
                }
                None
            }
        });

    let vod_id_clone = vod_id.clone();
    let container = tokio::task::spawn_blocking(move || -> Result<HighlightClustersContainer, AppError> {
        let duration = if duration_seconds.is_finite() && duration_seconds > 0.0 {
            duration_seconds
        } else {
            0.0
        };

        // 1. Chat Velocity Series (TASK-M4-09)
        let velocity_series = if let Some(ref chat_path) = resolved_chat_path {
            compute_chat_velocity_series_from_file(
                chat_path,
                5.0,
                60.0,
                Some(duration),
            )
            .unwrap_or_default()
        } else {
            Vec::new()
        };

        // 2. Swarm Emote Density Series (TASK-M4-10)
        let emote_series = if let Some(ref chat_path) = resolved_chat_path {
            compute_emote_density_series_from_file(
                chat_path,
                None,
                5.0,
                Some(duration),
            )
            .unwrap_or_default()
        } else {
            Vec::new()
        };

        // 3. Audio RMS Series (TASK-M4-11)
        let audio_series = if let Some(ref audio_path) = resolved_audio_path {
            analyze_wav_file(audio_path)
                .unwrap_or_default()
        } else {
            Vec::new()
        };

        // 4. Defensive check: If all inputs are empty, return NoHighlightsFound error
        if velocity_series.is_empty() && emote_series.is_empty() && audio_series.is_empty() {
            return Err(AppError::Highlight(HighlightError::NoHighlightsFound));
        }

        // 5. Multimodal Fusion (TASK-M4-12)
        let fused_points = fuse_multimodal_series(
            &velocity_series,
            &emote_series,
            &audio_series,
            duration,
        )?;

        // 6. Highlight Clustering & Deduplication
        let clusters_container =
            generate_clusters_from_fused_series(&vod_id_clone, &fused_points, duration)?;

        // 7. Write to highlight_clusters.json adhering 100% to Schema 5.4
        let output_json_path = project_dir.join("highlight_clusters.json");
        let serialized = serde_json::to_vec_pretty(&clusters_container).map_err(|e| {
            AppError::Internal(format!("Failed to serialize highlight_clusters.json: {e}"))
        })?;

        std::fs::write(&output_json_path, serialized).map_err(|e| {
            AppError::Storage(StorageError::CacheCleanupFailed(format!(
                "Failed to write highlight_clusters.json: {e}"
            )))
        })?;

        Ok(clusters_container)
    })
    .await
    .map_err(|e| AppError::Highlight(HighlightError::WorkerPanicked(e.to_string())))??;

    Ok(container)
}

/// Retrieve saved highlight clusters from `highlight_clusters.json` for a VOD.
///
/// # Errors
/// Returns `AppError::Storage` if file is missing or `AppError::Highlight` if JSON parsing fails.
#[tauri::command]
pub async fn get_highlight_clusters(
    vod_id: String,
) -> Result<HighlightClustersContainer, AppError> {
    let project_dir = resolve_vod_project_dir(&vod_id);
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

    let container: HighlightClustersContainer = serde_json::from_slice(&bytes).map_err(|e| {
        AppError::Highlight(HighlightError::ChatParsingFailed(format!(
            "Failed to parse highlight_clusters.json: {e}"
        )))
    })?;

    Ok(container)
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
    let project_dir = resolve_vod_project_dir(&vod_id);
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

    let mut container: HighlightClustersContainer = serde_json::from_slice(&bytes).map_err(|e| {
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
        AppError::Storage(StorageError::CacheCleanupFailed(format!(
            "Failed to write updated highlight_clusters.json: {e}"
        )))
    })?;

    Ok(result)
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
