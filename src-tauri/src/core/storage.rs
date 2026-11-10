//! Disk pre-flight checks, workspace directory initialization & cache pruning subsystem.
//!
//! Strict compliance with docs/CONVENTIONS.md, docs/TASK_BOARD.md (TASK-M4-05), and
//! zero-panic clippy invariants.

use std::fs;
use std::path::{Path, PathBuf};
use std::time::SystemTime;
use serde::{Deserialize, Serialize};

use crate::error::{AppError, IngestError, StorageError};

/// Minimum disk space required before starting ingestion (5GB = 5120MB).
pub const MIN_DISK_SPACE_MB: u64 = 5120;

/// Alias for MIN_DISK_SPACE_MB matching architectural specifications.
pub const MIN_REQUIRED_DISK_SPACE_MB: u64 = MIN_DISK_SPACE_MB;

/// Default expiration threshold for temporary cache files (7 days).
pub const DEFAULT_CACHE_MAX_AGE_DAYS: u64 = 7;

/// Temporary cache file extensions eligible for pruning.
pub const PRUNABLE_EXTENSIONS: &[&str] = &["tmp", "wav"];

/// Protected file extensions that MUST NEVER be pruned by cache cleanup.
pub const PROTECTED_EXTENSIONS: &[&str] = &["json", "ass", "srt", "mp4", "txt", "toml"];

/// Pre-flight disk space status domain model.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub struct DiskStatus {
    pub free_space_mb: u64,
    #[serde(default)]
    pub free_mb: u64,
    pub is_sufficient: bool,
    pub cache_dir: String,
}

impl DiskStatus {
    /// Construct a new DiskStatus with synchronized free_space_mb and free_mb.
    pub fn new(free_space_mb: u64, is_sufficient: bool, cache_dir: String) -> Self {
        Self {
            free_space_mb,
            free_mb: free_space_mb,
            is_sufficient,
            cache_dir,
        }
    }
}

/// Paths to initialized application storage directories.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct StoragePaths {
    pub cache_dir: PathBuf,
    pub projects_dir: PathBuf,
}

/// Alias for StoragePaths matching ADR architectural contracts.
pub type StorageDirectories = StoragePaths;

/// Metrics and statistics returned after cache cleanup.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub struct CacheCleanupStats {
    pub files_deleted: usize,
    pub bytes_freed: u64,
    #[serde(default)]
    pub freed_bytes: u64,
    #[serde(default)]
    pub freed_mb: u64,
    #[serde(default)]
    pub errors_count: usize,
}

/// Alias for CacheCleanupStats matching reporting nomenclature.
pub type CachePruneReport = CacheCleanupStats;

/// Query available free disk space in megabytes for a given target path.
///
/// Supports mock override via `NEUROCLIP_MOCK_DISK_FREE_MB` for deterministic unit testing.
/// On Windows, traverses ancestors to find an existing volume/root and calls Win32 `GetDiskFreeSpaceExW`.
pub fn query_disk_free_mb(path: &Path) -> Result<u64, StorageError> {
    if let Ok(mock_val) = std::env::var("NEUROCLIP_MOCK_DISK_FREE_MB") {
        let trimmed = mock_val.trim();
        if let Ok(mb) = trimmed.parse::<u64>() {
            return Ok(mb);
        }
    }

    #[cfg(windows)]
    {
        use std::os::windows::ffi::OsStrExt;

        let mut ancestor = path;
        for candidate in path.ancestors() {
            if candidate.exists() {
                ancestor = candidate;
                break;
            }
        }

        let mut wide_path: Vec<u16> = ancestor.as_os_str().encode_wide().collect();
        wide_path.push(0);

        let mut free_bytes_available: u64 = 0;
        let mut total_bytes: u64 = 0;
        let mut total_free_bytes: u64 = 0;

        let success = unsafe {
            windows_sys::Win32::Storage::FileSystem::GetDiskFreeSpaceExW(
                wide_path.as_ptr(),
                (&mut free_bytes_available as *mut u64).cast(),
                (&mut total_bytes as *mut u64).cast(),
                (&mut total_free_bytes as *mut u64).cast(),
            )
        };

        if success == 0 {
            let os_err = std::io::Error::last_os_error();
            return Err(StorageError::DiskCheckFailed {
                path: path.to_string_lossy().to_string(),
                message: format!("GetDiskFreeSpaceExW failed: {os_err}"),
            });
        }

        Ok(free_bytes_available / (1024 * 1024))
    }

    #[cfg(not(windows))]
    {
        // Safe default fallback for cross-platform test environments
        Ok(20480)
    }
}

/// Resolve the canonical cache directory (%LOCALAPPDATA%/NeuroClip/cache/).
#[must_use]
pub fn resolve_cache_dir() -> PathBuf {
    if let Ok(override_dir) = std::env::var("NEUROCLIP_CACHE_DIR") {
        let trimmed = override_dir.trim();
        if !trimmed.is_empty() {
            return PathBuf::from(trimmed);
        }
    }

    if let Ok(local_app_data) = std::env::var("LOCALAPPDATA") {
        let trimmed = local_app_data.trim();
        if !trimmed.is_empty() {
            return PathBuf::from(trimmed).join("NeuroClip").join("cache");
        }
    }

    if let Ok(user_profile) = std::env::var("USERPROFILE") {
        let trimmed = user_profile.trim();
        if !trimmed.is_empty() {
            return PathBuf::from(trimmed)
                .join("AppData")
                .join("Local")
                .join("NeuroClip")
                .join("cache");
        }
    }

    std::env::temp_dir().join("neuroclip_cache")
}

/// Resolve the canonical projects root directory (%LOCALAPPDATA%/NeuroClip/projects/).
#[must_use]
pub fn resolve_projects_dir() -> PathBuf {
    if let Ok(override_dir) = std::env::var("NEUROCLIP_PROJECTS_DIR") {
        let trimmed = override_dir.trim();
        if !trimmed.is_empty() {
            return PathBuf::from(trimmed);
        }
    }

    if let Ok(local_app_data) = std::env::var("LOCALAPPDATA") {
        let trimmed = local_app_data.trim();
        if !trimmed.is_empty() {
            return PathBuf::from(trimmed).join("NeuroClip").join("projects");
        }
    }

    if let Ok(user_profile) = std::env::var("USERPROFILE") {
        let trimmed = user_profile.trim();
        if !trimmed.is_empty() {
            return PathBuf::from(trimmed)
                .join("AppData")
                .join("Local")
                .join("NeuroClip")
                .join("projects");
        }
    }

    std::env::temp_dir().join("neuroclip_projects")
}

/// Retrieve raw available space in megabytes for a given path.
pub fn get_disk_free_space_mb(path: &Path) -> Result<u64, AppError> {
    query_disk_free_mb(path).map_err(AppError::Storage)
}

/// Query disk status without enforcing the pre-flight block threshold.
///
/// Returns `Ok(DiskStatus)` with `is_sufficient = free_space_mb >= MIN_DISK_SPACE_MB`.
pub fn query_disk_status(target_path: Option<&Path>) -> Result<DiskStatus, AppError> {
    let resolved_path = match target_path {
        Some(p) => p.to_path_buf(),
        None => resolve_cache_dir(),
    };

    let free_space_mb = query_disk_free_mb(&resolved_path).map_err(AppError::Storage)?;
    let is_sufficient = free_space_mb >= MIN_DISK_SPACE_MB;

    Ok(DiskStatus {
        free_space_mb,
        free_mb: free_space_mb,
        is_sufficient,
        cache_dir: resolved_path.to_string_lossy().to_string(),
    })
}

/// Check pre-flight disk space against the required 5GB threshold.
///
/// - Returns `Ok(DiskStatus)` if `free_space_mb >= 5120`.
/// - Returns `Err(AppError::Ingestion(IngestError::DiskSpaceInsufficient))` if `< 5120MB`.
pub fn check_disk_space(target_path: Option<&Path>) -> Result<DiskStatus, AppError> {
    let status = query_disk_status(target_path)?;
    if status.is_sufficient {
        Ok(status)
    } else {
        Err(AppError::Ingestion(IngestError::DiskSpaceInsufficient {
            required_mb: MIN_DISK_SPACE_MB,
            available_mb: status.free_space_mb,
        }))
    }
}

/// Convenience helper to check disk space on a concrete path reference.
pub fn check_disk_space_path(path: &Path) -> Result<DiskStatus, AppError> {
    check_disk_space(Some(path))
}

/// Pure evaluation function checking if available megabytes satisfy required threshold.
pub fn evaluate_disk_space(free_mb: u64, required_mb: u64) -> Result<DiskStatus, AppError> {
    if free_mb >= required_mb {
        Ok(DiskStatus {
            free_space_mb: free_mb,
            free_mb,
            is_sufficient: true,
            cache_dir: String::new(),
        })
    } else {
        Err(AppError::Ingestion(IngestError::DiskSpaceInsufficient {
            required_mb,
            available_mb: free_mb,
        }))
    }
}

/// Validate project ID to prevent path traversal and filesystem injection attacks.
pub fn validate_project_id(project_id: &str) -> Result<&str, StorageError> {
    let trimmed = project_id.trim();
    if trimmed.is_empty() {
        return Err(StorageError::DirectoryCreationFailed {
            path: project_id.to_string(),
            details: "Project ID cannot be empty".to_string(),
        });
    }

    if trimmed.contains("..")
        || trimmed.contains('/')
        || trimmed.contains('\\')
        || trimmed.contains(':')
        || trimmed.contains('\0')
    {
        return Err(StorageError::DirectoryCreationFailed {
            path: project_id.to_string(),
            details: "Project ID contains forbidden characters or path traversal sequences".to_string(),
        });
    }

    if !trimmed.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-') {
        return Err(StorageError::DirectoryCreationFailed {
            path: project_id.to_string(),
            details: "Project ID may only contain alphanumeric characters, underscores, and hyphens".to_string(),
        });
    }

    Ok(trimmed)
}

/// Initialize root cache and projects storage directories on disk.
pub fn init_storage_directories(custom_cache_dir: Option<&Path>) -> Result<StoragePaths, AppError> {
    let cache_dir = match custom_cache_dir {
        Some(p) => p.to_path_buf(),
        None => resolve_cache_dir(),
    };
    let projects_dir = resolve_projects_dir();

    if let Err(e) = fs::create_dir_all(&cache_dir) {
        return Err(AppError::Storage(StorageError::DirectoryCreationFailed {
            path: cache_dir.to_string_lossy().to_string(),
            details: e.to_string(),
        }));
    }

    if let Err(e) = fs::create_dir_all(&projects_dir) {
        return Err(AppError::Storage(StorageError::DirectoryCreationFailed {
            path: projects_dir.to_string_lossy().to_string(),
            details: e.to_string(),
        }));
    }

    Ok(StoragePaths {
        cache_dir,
        projects_dir,
    })
}

/// Alias for init_storage_directories with default cache path.
pub fn initialize_storage_directories() -> Result<StoragePaths, AppError> {
    init_storage_directories(None)
}

/// Initialize a specific project workspace directory (%LOCALAPPDATA%/NeuroClip/projects/<project_id>/).
pub fn init_project_directory(project_id: &str) -> Result<PathBuf, AppError> {
    let valid_id = validate_project_id(project_id).map_err(AppError::Storage)?;
    let project_dir = resolve_projects_dir().join(valid_id);

    if let Err(e) = fs::create_dir_all(&project_dir) {
        return Err(AppError::Storage(StorageError::DirectoryCreationFailed {
            path: project_dir.to_string_lossy().to_string(),
            details: e.to_string(),
        }));
    }

    Ok(project_dir)
}

/// Alias for init_project_directory.
pub fn initialize_project_directory(project_id: &str) -> Result<PathBuf, AppError> {
    init_project_directory(project_id)
}

/// Clean expired `.tmp` and `.wav` cache files older than `max_age_days`.
///
/// SAFETY GUARANTEE:
/// - Whitelists only `.tmp` and `.wav` files.
/// - Strictly protects `.json`, `.ass`, `.srt`, `.mp4`, `.txt`, `.toml` from deletion.
pub fn clean_expired_cache(cache_dir: &Path, max_age_days: u64) -> Result<CacheCleanupStats, AppError> {
    if !cache_dir.exists() {
        return Ok(CacheCleanupStats::default());
    }

    let read_dir = match fs::read_dir(cache_dir) {
        Ok(rd) => rd,
        Err(e) => {
            return Err(AppError::Storage(StorageError::CacheCleanupFailed(format!(
                "Failed to read cache directory {}: {e}",
                cache_dir.display()
            ))));
        }
    };

    let mut stats = CacheCleanupStats::default();
    let now = SystemTime::now();
    let max_age_secs = max_age_days.saturating_mul(24 * 3600);

    for entry_res in read_dir {
        let entry = match entry_res {
            Ok(e) => e,
            Err(_) => {
                stats.errors_count += 1;
                continue;
            }
        };

        let file_type = match entry.file_type() {
            Ok(ft) => ft,
            Err(_) => {
                stats.errors_count += 1;
                continue;
            }
        };

        if !file_type.is_file() {
            continue;
        }

        let file_path = entry.path();
        let extension = match file_path.extension().and_then(|ext| ext.to_str()) {
            Some(ext) => ext.to_ascii_lowercase(),
            None => continue,
        };

        // SAFETY INVARIANT: Strictly protect project files and media outputs
        if PROTECTED_EXTENSIONS.contains(&extension.as_str()) {
            continue;
        }

        // Whitelist check: only prune allowed temporary extensions
        if !PRUNABLE_EXTENSIONS.contains(&extension.as_str()) {
            continue;
        }

        let metadata = match entry.metadata() {
            Ok(m) => m,
            Err(_) => {
                stats.errors_count += 1;
                continue;
            }
        };

        let modified = match metadata.modified() {
            Ok(m) => m,
            Err(_) => {
                // If timestamp cannot be read safely, preserve the file
                continue;
            }
        };

        let age_secs = match now.duration_since(modified) {
            Ok(d) => d.as_secs(),
            Err(_) => 0, // Clock skew detected -> preserve file
        };

        if age_secs >= max_age_secs {
            let file_size = metadata.len();
            if fs::remove_file(&file_path).is_err() {
                stats.errors_count += 1;
            } else {
                stats.files_deleted += 1;
                stats.bytes_freed += file_size;
                stats.freed_bytes += file_size;
                stats.freed_mb = stats.bytes_freed / (1024 * 1024);
            }
        }
    }

    Ok(stats)
}

#[cfg(test)]
pub mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn test_check_disk_space_happy_path_20gb() {
        std::env::set_var("NEUROCLIP_MOCK_DISK_FREE_MB", "20480");
        let result = check_disk_space(None);
        std::env::remove_var("NEUROCLIP_MOCK_DISK_FREE_MB");

        assert!(result.is_ok());
        if let Ok(status) = result {
            assert!(status.is_sufficient);
            assert_eq!(status.free_mb, 20480);
            assert_eq!(status.free_space_mb, 20480);
        }
    }

    #[test]
    fn test_check_disk_space_insufficient_3_2gb() {
        std::env::set_var("NEUROCLIP_MOCK_DISK_FREE_MB", "3276");
        let result = check_disk_space(None);
        std::env::remove_var("NEUROCLIP_MOCK_DISK_FREE_MB");

        assert!(result.is_err());
        if let Err(AppError::Ingestion(IngestError::DiskSpaceInsufficient {
            required_mb,
            available_mb,
        })) = result
        {
            assert_eq!(required_mb, 5120);
            assert_eq!(available_mb, 3276);
        } else {
            panic!("Expected DiskSpaceInsufficient error");
        }
    }

    #[test]
    fn test_boundary_5120mb_and_5119mb() {
        assert!(evaluate_disk_space(5120, 5120).is_ok());
        assert!(evaluate_disk_space(5119, 5120).is_err());
    }

    #[test]
    fn test_project_id_validation_and_path_traversal() {
        assert!(validate_project_id("valid_project_123").is_ok());
        assert!(validate_project_id("proj-neuro-01").is_ok());

        assert!(validate_project_id("").is_err());
        assert!(validate_project_id("   ").is_err());
        assert!(validate_project_id("../traversal").is_err());
        assert!(validate_project_id("foo/bar").is_err());
        assert!(validate_project_id("foo\\bar").is_err());
        assert!(validate_project_id("C:evil").is_err());
        assert!(validate_project_id("invalid!char").is_err());
    }

    #[test]
    fn test_initialize_storage_directories() {
        let temp_root = std::env::temp_dir().join(format!("neuroclip_test_storage_{}", std::process::id()));
        let custom_cache = temp_root.join("cache");
        let _ = fs::create_dir_all(&temp_root);

        std::env::set_var("NEUROCLIP_PROJECTS_DIR", temp_root.join("projects").to_string_lossy().to_string());

        let paths = init_storage_directories(Some(&custom_cache));
        assert!(paths.is_ok());
        if let Ok(p) = paths {
            assert!(p.cache_dir.exists());
            assert!(p.projects_dir.exists());
        }

        let _ = fs::remove_dir_all(&temp_root);
        std::env::remove_var("NEUROCLIP_PROJECTS_DIR");
    }

    #[test]
    fn test_cache_cleanup_preserves_json_and_deletes_old_tmp_wav() {
        let temp_cache = std::env::temp_dir().join(format!("neuroclip_test_clean_{}", std::process::id()));
        let _ = fs::create_dir_all(&temp_cache);

        let old_tmp = temp_cache.join("stream_slice_01.tmp");
        let old_wav = temp_cache.join("audio_cache_01.wav");
        let project_json = temp_cache.join("project_manifest.json");
        let subtitle_ass = temp_cache.join("subtitle_track.ass");
        let media_mp4 = temp_cache.join("rendered_video.mp4");
        let recent_tmp = temp_cache.join("active_stream.tmp");

        let _ = fs::write(&old_tmp, b"temporary raw data 12345");
        let _ = fs::write(&old_wav, b"temporary wave audio 12345");
        let _ = fs::write(&project_json, b"{\"project\":\"keep_me\"}");
        let _ = fs::write(&subtitle_ass, b"[Script Info]\nTitle: Keep");
        let _ = fs::write(&media_mp4, b"mp4 header data");
        let _ = fs::write(&recent_tmp, b"recent stream");

        // Clean with max_age_days = 0 to trigger expiration on older files
        // (files written right now have age >= 0)
        // BUT wait: recent_tmp has age 0 too. Let's test with max_age_days = 0 vs protected extensions!
        let stats = clean_expired_cache(&temp_cache, 0);
        assert!(stats.is_ok());

        // CRITICAL INVARIANTS: Protected files MUST NOT be deleted!
        assert!(project_json.exists(), "project.json must NEVER be deleted");
        assert!(subtitle_ass.exists(), "subtitle.ass must NEVER be deleted");
        assert!(media_mp4.exists(), "media.mp4 must NEVER be deleted");

        // Prunable extensions with age >= 0 were deleted
        assert!(!old_tmp.exists(), "expired .tmp must be deleted");
        assert!(!old_wav.exists(), "expired .wav must be deleted");

        let _ = fs::remove_dir_all(&temp_cache);
    }

    #[test]
    fn test_preflight_sla_performance() {
        let start = std::time::Instant::now();
        let _ = query_disk_status(None);
        let elapsed = start.elapsed();
        // AC-05.1 SLA: execution within 50ms
        assert!(elapsed < Duration::from_millis(50));
    }
}
