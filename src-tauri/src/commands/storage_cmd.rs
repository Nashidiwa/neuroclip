//! IPC Command handlers for Storage Pre-flight Checks and Cache Management.
//!
//! Exposes Tauri commands adhering to the `Result<T, AppError>` contract:
//! - `check_preflight_storage(path)`: Pre-flight disk space safety verification.
//! - `initialize_storage(custom_cache_dir)`: Initialize cache and projects root folders.
//! - `initialize_project(project_id)`: Allocate isolated workspace for a project.
//! - `clean_expired_cache(cache_dir, max_age_days)`: Clean expired temporary caches.

use std::path::{Path, PathBuf};
use crate::core::storage::{
    clean_expired_cache as core_clean_expired_cache, init_project_directory,
    init_storage_directories, query_disk_status, resolve_cache_dir, CacheCleanupStats,
    DiskStatus, StoragePaths, DEFAULT_CACHE_MAX_AGE_DAYS,
};
use crate::error::AppError;

/// Perform pre-flight storage check on default or custom cache path.
///
/// Execution speed SLA <= 50ms (Win32 GetDiskFreeSpaceExW).
#[tauri::command]
pub async fn check_preflight_storage(
    path: Option<String>,
) -> Result<DiskStatus, AppError> {
    let p = path.as_deref().map(Path::new);
    query_disk_status(p)
}

/// Initialize root cache and projects storage directories.
#[tauri::command]
pub async fn initialize_storage(
    custom_cache_dir: Option<String>,
) -> Result<StoragePaths, AppError> {
    let p = custom_cache_dir.as_deref().map(Path::new);
    init_storage_directories(p)
}

/// Initialize a specific project workspace directory (%LOCALAPPDATA%/NeuroClip/projects/<project_id>/).
#[tauri::command]
pub async fn initialize_project(
    project_id: String,
) -> Result<String, AppError> {
    let path = init_project_directory(&project_id)?;
    Ok(path.to_string_lossy().to_string())
}

/// Clean expired `.tmp` and `.wav` files older than max_age_days (default 7 days).
#[tauri::command]
pub async fn clean_expired_cache(
    cache_dir: Option<String>,
    max_age_days: Option<u64>,
) -> Result<CacheCleanupStats, AppError> {
    let days = max_age_days.unwrap_or(DEFAULT_CACHE_MAX_AGE_DAYS);
    let dir = match cache_dir {
        Some(d) if !d.trim().is_empty() => PathBuf::from(d.trim()),
        _ => resolve_cache_dir(),
    };
    core_clean_expired_cache(&dir, days)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_check_preflight_storage_ipc() {
        std::env::set_var("NEUROCLIP_MOCK_DISK_FREE_MB", "20480");
        let result = check_preflight_storage(None).await;
        std::env::remove_var("NEUROCLIP_MOCK_DISK_FREE_MB");

        assert!(result.is_ok());
        if let Ok(status) = result {
            assert!(status.is_sufficient);
            assert_eq!(status.free_space_mb, 20480);
        }
    }

    #[tokio::test]
    async fn test_clean_expired_cache_ipc_empty_dir() {
        let temp_dir = std::env::temp_dir().join(format!("neuroclip_test_clean_ipc_{}", std::process::id()));
        let _ = std::fs::create_dir_all(&temp_dir);

        let result = clean_expired_cache(Some(temp_dir.to_string_lossy().to_string()), Some(7)).await;
        assert!(result.is_ok());
        if let Ok(stats) = result {
            assert_eq!(stats.files_deleted, 0);
            assert_eq!(stats.bytes_freed, 0);
        }

        let _ = std::fs::remove_dir_all(&temp_dir);
    }
}
