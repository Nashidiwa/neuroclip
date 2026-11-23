//! IPC Command handlers for Storage Pre-flight Checks and Cache Management.
//!
//! Exposes Tauri commands adhering to the `Result<T, AppError>` contract:
//! - `check_preflight_storage(path)`: Pre-flight disk space safety verification.
//! - `initialize_storage(custom_cache_dir)`: Initialize cache and projects root folders.
//! - `initialize_project(project_id)`: Allocate isolated workspace for a project.
//! - `clean_expired_cache(cache_dir, max_age_days)`: Clean expired temporary caches.

use crate::core::storage::{
    clean_expired_cache as core_clean_expired_cache, ensure_path_within_roots,
    init_project_directory, init_storage_directories, query_disk_status, resolve_cache_dir,
    CacheCleanupStats, DiskStatus, StoragePaths, DEFAULT_CACHE_MAX_AGE_DAYS,
};
use crate::error::AppError;
use std::path::PathBuf;

/// Perform pre-flight storage check on default or custom cache path.
///
/// Execution speed SLA <= 50ms (Win32 `GetDiskFreeSpaceExW`).
///
/// # Errors
/// Returns [`AppError::Storage`] if disk space cannot be queried.
#[tauri::command]
pub async fn check_preflight_storage(path: Option<String>) -> Result<DiskStatus, AppError> {
    tokio::task::spawn_blocking(move || {
        let p = path.as_deref().map(PathBuf::from);
        query_disk_status(p.as_deref())
    })
    .await
    .map_err(|e| AppError::Internal(format!("Preflight storage check task panicked: {e}")))?
}

/// Initialize root cache and projects storage directories.
///
/// # Errors
/// Returns [`AppError::Storage`] if creating the cache or projects directory fails.
#[tauri::command]
pub async fn initialize_storage(
    custom_cache_dir: Option<String>,
) -> Result<StoragePaths, AppError> {
    tokio::task::spawn_blocking(move || {
        let p = custom_cache_dir.as_deref().map(PathBuf::from);
        init_storage_directories(p.as_deref())
    })
    .await
    .map_err(|e| AppError::Internal(format!("Initialize storage task panicked: {e}")))?
}

/// Initialize a specific project workspace directory (`%LOCALAPPDATA%/NeuroClip/projects/<project_id>/`).
///
/// # Errors
/// Returns [`AppError::Storage`] if `project_id` is invalid or creating the directory fails.
#[tauri::command]
pub async fn initialize_project(project_id: String) -> Result<String, AppError> {
    tokio::task::spawn_blocking(move || {
        let path = init_project_directory(&project_id)?;
        Ok(path.to_string_lossy().to_string())
    })
    .await
    .map_err(|e| AppError::Internal(format!("Initialize project task panicked: {e}")))?
}

/// Clean expired `.tmp` and `.wav` files older than `max_age_days` (default 7 days).
///
/// Strictly restricts cleanup to `resolve_cache_dir()` (or a subdirectory inside it).
///
/// # Errors
/// Returns [`AppError::Storage`] if `cache_dir` escapes `resolve_cache_dir()` or pruning fails.
#[tauri::command]
pub async fn clean_expired_cache(
    cache_dir: Option<String>,
    max_age_days: Option<u64>,
) -> Result<CacheCleanupStats, AppError> {
    tokio::task::spawn_blocking(move || {
        let days = max_age_days.unwrap_or(DEFAULT_CACHE_MAX_AGE_DAYS);
        let allowed_root = resolve_cache_dir();
        let dir = match cache_dir {
            Some(d) if !d.trim().is_empty() => {
                ensure_path_within_roots(std::path::Path::new(d.trim()), &[allowed_root.as_path()])?
            }
            _ => allowed_root,
        };
        core_clean_expired_cache(&dir, days)
    })
    .await
    .map_err(|e| AppError::Internal(format!("Cache cleanup task panicked: {e}")))?
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::storage::test_env::{ScopedEnvVar, ENV_LOCK};
    use crate::error::StorageError;

    #[tokio::test]
    async fn test_check_preflight_storage_ipc() {
        let _env_guard = ENV_LOCK.lock().await;
        let _mock_env = ScopedEnvVar::set("NEUROCLIP_MOCK_DISK_FREE_MB", "20480");
        let result = check_preflight_storage(None).await;

        assert!(result.is_ok());
        if let Ok(status) = result {
            assert!(status.is_sufficient);
            assert_eq!(status.free_space_mb, 20480);
        }
    }

    #[tokio::test]
    async fn test_clean_expired_cache_ipc_empty_dir() {
        let _env_guard = ENV_LOCK.lock().await;
        let temp_dir =
            std::env::temp_dir().join(format!("neuroclip_test_clean_ipc_{}", std::process::id()));
        let _ = std::fs::create_dir_all(&temp_dir);
        let _cache_env = ScopedEnvVar::set("NEUROCLIP_CACHE_DIR", &temp_dir.to_string_lossy());

        let sub_cache = temp_dir.join("vod_sub");
        let _ = std::fs::create_dir_all(&sub_cache);

        let result =
            clean_expired_cache(Some(sub_cache.to_string_lossy().to_string()), Some(7)).await;
        assert!(result.is_ok());
        if let Ok(stats) = result {
            assert_eq!(stats.files_deleted, 0);
            assert_eq!(stats.bytes_freed, 0);
        }

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[tokio::test]
    async fn test_clean_expired_cache_rejects_paths_outside_workspace() {
        let _env_guard = ENV_LOCK.lock().await;
        let cache_root =
            std::env::temp_dir().join(format!("neuroclip_test_clean_root_{}", std::process::id()));
        let outside_dir = std::env::temp_dir().join(format!(
            "neuroclip_test_clean_outside_{}",
            std::process::id()
        ));
        let _ = std::fs::create_dir_all(&cache_root);
        let _ = std::fs::create_dir_all(&outside_dir);
        let _cache_env = ScopedEnvVar::set("NEUROCLIP_CACHE_DIR", &cache_root.to_string_lossy());

        for bad in [
            outside_dir.to_string_lossy().to_string(),
            format!("{}\\..\\escape", cache_root.display()),
            "../relative_escape".to_string(),
            r"C:\Windows\Temp".to_string(),
        ] {
            let res = clean_expired_cache(Some(bad.clone()), Some(0)).await;
            assert!(
                matches!(
                    res,
                    Err(AppError::Storage(
                        StorageError::DirectoryCreationFailed { .. }
                    ))
                ),
                "Expected containment rejection for {bad:?}, got {res:?}"
            );
        }

        let _ = std::fs::remove_dir_all(&cache_root);
        let _ = std::fs::remove_dir_all(&outside_dir);
    }
}
