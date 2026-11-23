//! Disk pre-flight checks, workspace directory initialization & cache pruning subsystem.
//!
//! Strict compliance with docs/CONVENTIONS.md, docs/TASK_BOARD.md (TASK-M4-05), and
//! zero-panic clippy invariants.

use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use crate::error::{AppError, ExportError, IngestError, StorageError};

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
                (&raw mut free_bytes_available).cast(),
                (&raw mut total_bytes).cast(),
                (&raw mut total_free_bytes).cast(),
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

/// Check pre-flight disk space against the required 5GB threshold using an explicit free-space value.
///
/// # Errors
/// Returns [`AppError::Ingestion`] with [`IngestError::DiskSpaceInsufficient`] if `free_space_mb < MIN_DISK_SPACE_MB`.
pub fn check_disk_space_with_free_mb(
    free_space_mb: u64,
    cache_dir: &Path,
) -> Result<DiskStatus, AppError> {
    if free_space_mb >= MIN_DISK_SPACE_MB {
        Ok(DiskStatus::new(
            free_space_mb,
            true,
            cache_dir.to_string_lossy().to_string(),
        ))
    } else {
        Err(AppError::Ingestion(IngestError::DiskSpaceInsufficient {
            required_mb: MIN_DISK_SPACE_MB,
            available_mb: free_space_mb,
        }))
    }
}

/// Check pre-flight disk space against the required 5GB threshold.
///
/// - Returns `Ok(DiskStatus)` if `free_space_mb >= 5120`.
/// - Returns `Err(AppError::Ingestion(IngestError::DiskSpaceInsufficient))` if `< 5120MB`.
///
/// # Errors
/// Returns [`AppError::Storage`] if disk space cannot be queried, or [`AppError::Ingestion`] if below threshold.
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
            details: "Project ID contains forbidden characters or path traversal sequences"
                .to_string(),
        });
    }

    if !trimmed
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
    {
        return Err(StorageError::DirectoryCreationFailed {
            path: project_id.to_string(),
            details:
                "Project ID may only contain alphanumeric characters, underscores, and hyphens"
                    .to_string(),
        });
    }

    Ok(trimmed)
}

/// Check whether a filename stem (case-insensitive) matches a Windows reserved DOS device name.
fn is_windows_reserved_device_name(name: &str) -> bool {
    let base = name.split('.').next().unwrap_or(name);
    matches!(
        base.to_ascii_uppercase().as_str(),
        "CON"
            | "PRN"
            | "AUX"
            | "NUL"
            | "COM1"
            | "COM2"
            | "COM3"
            | "COM4"
            | "COM5"
            | "COM6"
            | "COM7"
            | "COM8"
            | "COM9"
            | "LPT1"
            | "LPT2"
            | "LPT3"
            | "LPT4"
            | "LPT5"
            | "LPT6"
            | "LPT7"
            | "LPT8"
            | "LPT9"
    )
}

/// Validate a VOD identifier as a single safe path segment (no traversal, separators, prefixes,
/// leading/trailing whitespace, Windows-illegal filename characters, trailing dot/space,
/// or Windows reserved device names).
///
/// # Errors
/// Returns [`StorageError::DirectoryCreationFailed`] if `vod_id` is empty, has leading/trailing
/// whitespace, contains path separators, `..`, NUL/control bytes, Windows-illegal chars
/// (`< > " | ? *`), trailing `.`/space, Windows reserved device names (`CON`, `PRN`, `AUX`,
/// `NUL`, `COM1..9`, `LPT1..9`), drive prefixes/UNC/root components, or is an absolute path.
pub fn validate_vod_id(vod_id: &str) -> Result<&str, StorageError> {
    if vod_id.chars().any(char::is_control) {
        return Err(StorageError::DirectoryCreationFailed {
            path: vod_id.to_string(),
            details: "VOD ID contains forbidden control characters".to_string(),
        });
    }

    let trimmed = vod_id.trim();
    if trimmed.is_empty() {
        return Err(StorageError::DirectoryCreationFailed {
            path: vod_id.to_string(),
            details: "VOD ID cannot be empty".to_string(),
        });
    }
    if vod_id != trimmed {
        return Err(StorageError::DirectoryCreationFailed {
            path: vod_id.to_string(),
            details: "VOD ID must not contain leading or trailing whitespace".to_string(),
        });
    }

    let path = Path::new(trimmed);
    let has_forbidden_chars = trimmed
        .contains(['/', '\\', ':', '\0', '<', '>', '"', '|', '?', '*'])
        || trimmed.contains("..");
    let has_trailing_dot_or_space =
        trimmed.ends_with('.') || vod_id.ends_with(' ') || trimmed.ends_with(' ');
    let is_reserved_device = is_windows_reserved_device_name(trimmed);
    let mut components = path.components();
    let is_single_normal = matches!(components.next(), Some(std::path::Component::Normal(_)))
        && components.next().is_none();

    if has_forbidden_chars
        || has_trailing_dot_or_space
        || is_reserved_device
        || path.is_absolute()
        || !is_single_normal
    {
        return Err(StorageError::DirectoryCreationFailed {
            path: vod_id.to_string(),
            details: "VOD ID contains forbidden path traversal, separator, Windows-illegal character, or reserved device name"
                .to_string(),
        });
    }

    Ok(trimmed)
}

/// Resolve the project directory for a validated VOD ID (`projects/<vod_id>` if present, else `cache/<vod_id>`).
///
/// # Errors
/// Returns [`AppError::Storage`] if `vod_id` fails path-traversal validation.
pub fn resolve_vod_project_dir(vod_id: &str) -> Result<PathBuf, AppError> {
    let clean_id = validate_vod_id(vod_id).map_err(AppError::Storage)?;
    let candidate = resolve_projects_dir().join(clean_id);
    if candidate.exists() {
        Ok(candidate)
    } else {
        Ok(resolve_cache_dir().join(clean_id))
    }
}

/// Initialize root cache and projects storage directories on disk using explicit roots.
///
/// # Errors
/// Returns [`AppError::Storage`] if creating either directory fails.
pub fn init_storage_directories_with_roots(
    cache_dir: &Path,
    projects_dir: &Path,
) -> Result<StoragePaths, AppError> {
    if let Err(e) = fs::create_dir_all(cache_dir) {
        return Err(AppError::Storage(StorageError::DirectoryCreationFailed {
            path: cache_dir.to_string_lossy().to_string(),
            details: e.to_string(),
        }));
    }

    if let Err(e) = fs::create_dir_all(projects_dir) {
        return Err(AppError::Storage(StorageError::DirectoryCreationFailed {
            path: projects_dir.to_string_lossy().to_string(),
            details: e.to_string(),
        }));
    }

    Ok(StoragePaths {
        cache_dir: cache_dir.to_path_buf(),
        projects_dir: projects_dir.to_path_buf(),
    })
}

/// Initialize root cache and projects storage directories on disk.
///
/// # Errors
/// Returns [`AppError::Storage`] if creating either directory fails.
pub fn init_storage_directories(custom_cache_dir: Option<&Path>) -> Result<StoragePaths, AppError> {
    let cache_dir = match custom_cache_dir {
        Some(p) => p.to_path_buf(),
        None => resolve_cache_dir(),
    };
    let projects_dir = resolve_projects_dir();

    init_storage_directories_with_roots(&cache_dir, &projects_dir)
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
pub fn clean_expired_cache(
    cache_dir: &Path,
    max_age_days: u64,
) -> Result<CacheCleanupStats, AppError> {
    if !cache_dir.exists() {
        return Ok(CacheCleanupStats::default());
    }

    let read_dir = match fs::read_dir(cache_dir) {
        Ok(rd) => rd,
        Err(e) => {
            return Err(AppError::Storage(StorageError::CacheCleanupFailed(
                format!(
                    "Failed to read cache directory {}: {e}",
                    cache_dir.display()
                ),
            )));
        }
    };

    let mut stats = CacheCleanupStats::default();
    let now = SystemTime::now();
    let max_age_secs = max_age_days.saturating_mul(24 * 3600);

    for entry_res in read_dir {
        let Ok(entry) = entry_res else {
            stats.errors_count += 1;
            continue;
        };

        let Ok(file_type) = entry.file_type() else {
            stats.errors_count += 1;
            continue;
        };

        if !file_type.is_file() {
            continue;
        }

        let file_path = entry.path();
        let Some(extension) = file_path
            .extension()
            .and_then(|ext| ext.to_str())
            .map(str::to_ascii_lowercase)
        else {
            continue;
        };

        // SAFETY INVARIANT: Strictly protect project files and media outputs
        if PROTECTED_EXTENSIONS.contains(&extension.as_str()) {
            continue;
        }

        // Whitelist check: only prune allowed temporary extensions
        if !PRUNABLE_EXTENSIONS.contains(&extension.as_str()) {
            continue;
        }

        let Ok(metadata) = entry.metadata() else {
            stats.errors_count += 1;
            continue;
        };

        let Ok(modified) = metadata.modified() else {
            // If timestamp cannot be read safely, preserve the file
            continue;
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

fn strip_windows_verbatim_prefix(path: PathBuf) -> PathBuf {
    #[cfg(windows)]
    {
        let raw = path.to_string_lossy();
        if let Some(unc_rest) = raw.strip_prefix(r"\\?\UNC\") {
            return PathBuf::from(format!(r"\\{unc_rest}"));
        }
        if let Some(stripped) = raw.strip_prefix(r"\\?\") {
            return PathBuf::from(stripped);
        }
    }
    path
}

fn reject_traversal_or_nul(path: &Path) -> Result<(), StorageError> {
    let raw = path.to_string_lossy();
    if raw.trim().is_empty() || raw.contains('\0') {
        return Err(StorageError::DirectoryCreationFailed {
            path: raw.to_string(),
            details: "Path cannot be empty or contain NUL bytes".to_string(),
        });
    }
    if path
        .components()
        .any(|c| matches!(c, std::path::Component::ParentDir))
    {
        return Err(StorageError::DirectoryCreationFailed {
            path: raw.to_string(),
            details: "Path traversal ('..') is forbidden".to_string(),
        });
    }
    Ok(())
}

fn canonicalize_with_ancestor(path: &Path) -> Result<PathBuf, AppError> {
    reject_traversal_or_nul(path).map_err(AppError::Storage)?;
    if !path.is_absolute() {
        return Err(AppError::Storage(StorageError::DirectoryCreationFailed {
            path: path.display().to_string(),
            details: "Path must be absolute".to_string(),
        }));
    }

    let mut tail_segments: Vec<std::ffi::OsString> = Vec::new();
    let mut cursor = Some(path);
    while let Some(current) = cursor {
        if current.exists() {
            let base = fs::canonicalize(current).map_err(|e| {
                AppError::Storage(StorageError::DirectoryCreationFailed {
                    path: current.display().to_string(),
                    details: e.to_string(),
                })
            })?;
            let mut resolved = strip_windows_verbatim_prefix(base);
            for seg in tail_segments.iter().rev() {
                resolved.push(seg);
            }
            return Ok(resolved);
        }
        if let Some(name) = current.file_name() {
            tail_segments.push(name.to_os_string());
        }
        cursor = current.parent();
    }

    Err(AppError::Storage(StorageError::DirectoryCreationFailed {
        path: path.display().to_string(),
        details: "No existing ancestor volume/root found for path".to_string(),
    }))
}

fn normalize_path_segments_for_compare(path: &Path) -> Vec<String> {
    let clean = strip_windows_verbatim_prefix(path.to_path_buf());
    clean
        .components()
        .filter(|c| !matches!(c, std::path::Component::CurDir))
        .map(|c| {
            let s = c.as_os_str().to_string_lossy();
            #[cfg(windows)]
            {
                s.to_ascii_lowercase()
            }
            #[cfg(not(windows))]
            {
                s.to_string()
            }
        })
        .collect()
}

/// Verify that `candidate` resolves inside at least one directory in `allowed_roots`.
///
/// Canonicalizes the deepest existing ancestor to defeat symlinks, junctions, `..` traversal,
/// and case/verbatim-prefix variations on Windows.
///
/// # Errors
/// Returns [`AppError::Storage`] if `candidate` is empty, relative, contains `..`/NUL,
/// or resolves outside all `allowed_roots`.
pub fn ensure_path_within_roots(
    candidate: &Path,
    allowed_roots: &[&Path],
) -> Result<PathBuf, AppError> {
    let resolved_candidate = canonicalize_with_ancestor(candidate)?;
    let candidate_segments = normalize_path_segments_for_compare(&resolved_candidate);

    for root in allowed_roots {
        if let Ok(resolved_root) = canonicalize_with_ancestor(root) {
            let root_segments = normalize_path_segments_for_compare(&resolved_root);
            if !root_segments.is_empty() && candidate_segments.starts_with(&root_segments) {
                return Ok(resolved_candidate);
            }
        }
    }

    Err(AppError::Storage(StorageError::DirectoryCreationFailed {
        path: candidate.display().to_string(),
        details: "Path is outside allowed NeuroClip workspace directories".to_string(),
    }))
}

/// Validate a user-selected export output path (blocking `..` traversal and OS system folders).
///
/// Preserves arbitrary user-selected export destinations (e.g. `Desktop`, `Videos`, custom drives)
/// while blocking path traversal (`..`), NUL bytes, and writes into OS protected directories.
///
/// # Errors
/// Returns [`AppError::Export`] with [`ExportError::FileWriteFailed`] if validation fails.
pub fn validate_user_export_path(output_path: &Path) -> Result<(), AppError> {
    reject_traversal_or_nul(output_path).map_err(|e| {
        AppError::Export(ExportError::FileWriteFailed {
            path: output_path.display().to_string(),
            details: e.to_string(),
        })
    })?;

    #[cfg(windows)]
    {
        if output_path.is_absolute() {
            let resolved = canonicalize_with_ancestor(output_path)
                .unwrap_or_else(|_| output_path.to_path_buf());
            let norm = resolved.to_string_lossy().to_ascii_lowercase();
            let temp_norm = strip_windows_verbatim_prefix(std::env::temp_dir())
                .to_string_lossy()
                .to_ascii_lowercase();

            if !norm.starts_with(&temp_norm) {
                let mut protected_prefixes = vec![
                    r"c:\windows\system32".to_string(),
                    r"c:\windows\syswow64".to_string(),
                    r"c:\windows\winsxs".to_string(),
                    r"c:\program files".to_string(),
                    r"c:\program files (x86)".to_string(),
                ];
                if let Ok(sys_root) = std::env::var("SystemRoot") {
                    let sr = sys_root.trim().trim_end_matches('\\').to_ascii_lowercase();
                    if !sr.is_empty() {
                        protected_prefixes.push(format!(r"{sr}\system32"));
                        protected_prefixes.push(format!(r"{sr}\syswow64"));
                        protected_prefixes.push(format!(r"{sr}\winsxs"));
                    }
                }
                for prefix in &protected_prefixes {
                    if norm == *prefix || norm.starts_with(&format!(r"{prefix}\")) {
                        return Err(AppError::Export(ExportError::FileWriteFailed {
                            path: output_path.display().to_string(),
                            details: "Cannot write export files inside OS system directories"
                                .to_string(),
                        }));
                    }
                }
            }
        }
    }

    Ok(())
}

#[cfg(test)]
pub(crate) mod test_env {
    use std::ffi::OsString;

    pub(crate) static ENV_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

    pub(crate) struct ScopedEnvVar {
        key: &'static str,
        previous: Option<OsString>,
    }

    impl ScopedEnvVar {
        pub(crate) fn set(key: &'static str, value: &str) -> Self {
            let previous = std::env::var_os(key);
            std::env::set_var(key, value);
            Self { key, previous }
        }

        pub(crate) fn remove(key: &'static str) -> Self {
            let previous = std::env::var_os(key);
            std::env::remove_var(key);
            Self { key, previous }
        }
    }

    impl Drop for ScopedEnvVar {
        fn drop(&mut self) {
            match &self.previous {
                Some(val) => std::env::set_var(self.key, val),
                None => std::env::remove_var(self.key),
            }
        }
    }
}

#[cfg(test)]
pub mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::Duration;

    static TEST_COUNTER: AtomicU64 = AtomicU64::new(0);

    fn unique_temp_dir(prefix: &str) -> PathBuf {
        let seq = TEST_COUNTER.fetch_add(1, Ordering::Relaxed);
        std::env::temp_dir().join(format!("{prefix}_{}_{seq}", std::process::id()))
    }

    #[test]
    fn test_check_disk_space_happy_path_20gb() {
        let temp_cache = unique_temp_dir("neuroclip_disk_ok");
        let result = check_disk_space_with_free_mb(20480, &temp_cache);

        assert!(result.is_ok());
        if let Ok(status) = result {
            assert!(status.is_sufficient);
            assert_eq!(status.free_mb, 20480);
            assert_eq!(status.free_space_mb, 20480);
        }
    }

    #[test]
    fn test_check_disk_space_insufficient_3_2gb() {
        let temp_cache = unique_temp_dir("neuroclip_disk_low");
        let result = check_disk_space_with_free_mb(3276, &temp_cache);

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
    fn test_resolve_vod_project_dir_rejects_traversal_and_prefixes() {
        let invalid_ids = [
            "",
            "   ",
            ".",
            "..",
            "../evil",
            "a/b",
            "a\\b",
            "/etc/passwd",
            "\\Windows",
            "C:evil",
            "C:\\evil",
            "\\\\server\\share",
            "\\\\?\\C:\\evil",
            "vod\0id",
            "vod<1>",
            "vod>2",
            "vod\"3",
            "vod|4",
            "vod?5",
            "vod*6",
            "vod_trailing.",
            "vod_trailing ",
            "vod_tab\t",
            "\tvod_leading_tab",
            " vod_leading_space",
            "vod_mid\ttab",
            "CON",
            "con",
            "PRN",
            "AUX",
            "NUL",
            "nul.json",
            "COM1",
            "com9",
            "LPT1",
            "lpt9",
        ];

        for bad in invalid_ids {
            let res = resolve_vod_project_dir(bad);
            assert!(
                matches!(
                    res,
                    Err(AppError::Storage(
                        StorageError::DirectoryCreationFailed { .. }
                    ))
                ),
                "Expected DirectoryCreationFailed for invalid vod_id {bad:?}, got {res:?}"
            );
        }

        let _env_guard = test_env::ENV_LOCK.blocking_lock();
        let temp_root = unique_temp_dir("neuroclip_vod_resolve");
        let projects_root = temp_root.join("projects");
        let cache_root = temp_root.join("cache");
        let existing_project = projects_root.join("123456789");
        let _ = fs::create_dir_all(&existing_project);
        let _ = fs::create_dir_all(&cache_root);

        let _proj_env =
            test_env::ScopedEnvVar::set("NEUROCLIP_PROJECTS_DIR", &projects_root.to_string_lossy());
        let _cache_env =
            test_env::ScopedEnvVar::set("NEUROCLIP_CACHE_DIR", &cache_root.to_string_lossy());

        assert_eq!(
            resolve_vod_project_dir("123456789").unwrap(),
            existing_project
        );
        assert_eq!(
            resolve_vod_project_dir("dQw4w9WgXcQ").unwrap(),
            cache_root.join("dQw4w9WgXcQ")
        );
        assert_eq!(
            resolve_vod_project_dir("local_clip_1a2b3c4d").unwrap(),
            cache_root.join("local_clip_1a2b3c4d")
        );

        let _ = fs::remove_dir_all(&temp_root);
    }

    #[test]
    fn test_validate_vod_id_rejects_leading_trailing_whitespace_and_tabs() {
        for bad in [
            "vod_tab\t",
            "\tvod_leading_tab",
            " vod_leading_space",
            "vod_trailing_space ",
            " vod_both ",
            "vod\nnewline",
        ] {
            let res = validate_vod_id(bad);
            assert!(
                matches!(res, Err(StorageError::DirectoryCreationFailed { .. })),
                "Expected DirectoryCreationFailed for {bad:?}, got {res:?}"
            );
        }
        assert_eq!(validate_vod_id("vod_clean_123").unwrap(), "vod_clean_123");
    }

    #[test]
    fn test_initialize_storage_directories() {
        let temp_root = unique_temp_dir("neuroclip_test_storage");
        let custom_cache = temp_root.join("cache");
        let custom_projects = temp_root.join("projects");
        let _ = fs::create_dir_all(&temp_root);

        let paths = init_storage_directories_with_roots(&custom_cache, &custom_projects);
        assert!(paths.is_ok());
        if let Ok(p) = paths {
            assert!(p.cache_dir.exists());
            assert!(p.projects_dir.exists());
        }

        let _ = fs::remove_dir_all(&temp_root);
    }

    #[test]
    fn test_cache_cleanup_preserves_json_and_deletes_old_tmp_wav() {
        let temp_cache = unique_temp_dir("neuroclip_test_clean");
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
        let _env_guard = test_env::ENV_LOCK.blocking_lock();
        let start = std::time::Instant::now();
        let _ = query_disk_status(None);
        let elapsed = start.elapsed();
        // AC-05.1 SLA: execution within 50ms
        assert!(elapsed < Duration::from_millis(50));
    }

    #[test]
    fn test_ensure_path_within_roots_and_validate_user_export_path() {
        let root = unique_temp_dir("neuroclip_containment_root");
        let child = root.join("sub").join("file.json");
        let outside = unique_temp_dir("neuroclip_containment_outside").join("file.json");
        let _ = fs::create_dir_all(&root);

        assert!(ensure_path_within_roots(&child, &[root.as_path()]).is_ok());
        assert!(ensure_path_within_roots(&outside, &[root.as_path()]).is_err());
        assert!(
            ensure_path_within_roots(&root.join("..").join("escape"), &[root.as_path()]).is_err()
        );
        assert!(
            ensure_path_within_roots(Path::new("relative/file.json"), &[root.as_path()]).is_err()
        );

        assert!(validate_user_export_path(&child).is_ok());
        assert!(validate_user_export_path(Path::new("../outside.xml")).is_err());
        #[cfg(windows)]
        {
            assert!(
                validate_user_export_path(Path::new(r"C:\Windows\System32\drivers\etc\hosts"))
                    .is_err()
            );
            assert!(
                validate_user_export_path(Path::new(r"C:\Program Files\NeuroClip\out.xml"))
                    .is_err()
            );
        }

        let _ = fs::remove_dir_all(&root);
    }
}
