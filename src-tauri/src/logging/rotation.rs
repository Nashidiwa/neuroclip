//! Log file retention and rotation management.
//!
//! Enforces `CONVENTIONS.md` §6.4:
//! - Daily rolling log files: `neuroclip-YYYY-MM-DD.log`
//! - Retention: Delete files older than 14 days
//! - Storage ceiling: Max 50MB; if exceeded, prune oldest files until <= 30MB
//! - Safety: Only prune NeuroClip log files, never foreign files
//! - Zero-panic error propagation via `StorageError`

use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};
use crate::error::StorageError;

/// Retention policy parameters for the log storage directory.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RetentionPolicy {
    /// Maximum age in days for log files before deletion (default: 14 days).
    pub max_age_days: u32,
    /// Maximum allowable cumulative size in bytes for the log directory (default: 50MB).
    pub max_dir_size_bytes: u64,
    /// Target low-water mark in bytes to prune down to when max size is exceeded (default: 30MB).
    pub target_dir_size_bytes: u64,
}

impl Default for RetentionPolicy {
    fn default() -> Self {
        Self {
            max_age_days: 14,
            max_dir_size_bytes: 50 * 1024 * 1024,      // 50 MB
            target_dir_size_bytes: 30 * 1024 * 1024,   // 30 MB
        }
    }
}

/// Summary report returned after executing log directory pruning.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct PruneReport {
    /// Number of expired log files deleted due to age > `max_age_days`.
    pub deleted_expired_files: usize,
    /// Number of log files deleted to bring directory size down to `target_dir_size_bytes`.
    pub deleted_overflow_files: usize,
    /// Cumulative disk space freed in bytes.
    pub bytes_freed: u64,
    /// Total bytes remaining in active log files.
    pub remaining_bytes: u64,
}

/// Metadata record for an identified log file candidate.
#[derive(Debug, Clone)]
struct LogFileCandidate {
    path: PathBuf,
    size_bytes: u64,
    modified_time: SystemTime,
}

/// Helper checking if a string slice conforms to the daily date pattern `YYYY-MM-DD`.
fn is_daily_date_suffix(s: &str) -> bool {
    let bytes = s.as_bytes();
    if bytes.len() != 10 {
        return false;
    }
    // Expected format: YYYY-MM-DD (e.g., 2026-11-10)
    bytes[0..4].iter().all(u8::is_ascii_digit)
        && bytes[4] == b'-'
        && bytes[5..7].iter().all(u8::is_ascii_digit)
        && bytes[7] == b'-'
        && bytes[8..10].iter().all(u8::is_ascii_digit)
}

/// Determine whether a filename is a candidate NeuroClip log file.
///
/// Strictly matches valid NeuroClip log files:
/// - Filenames starting with "neuroclip" and ending with ".log" (e.g. `neuroclip-2026-11-10.log`, `neuroclip.log`, `neuroclip-trace.log`)
/// - Daily rolling files produced by `tracing-appender` without extension (e.g. `neuroclip.2026-11-10`, `neuroclip-2026-11-10`, `neuroclip-.2026-11-10`)
///
/// Strictly rejects non-log files such as `.json`, `.txt`, `.bak`, `.ass`, etc., even if prefixed with "neuroclip".
#[must_use]
pub fn is_neuroclip_log_file(filename: &str) -> bool {
    if !filename.starts_with("neuroclip") {
        return false;
    }

    // Standard .log extension
    if filename.ends_with(".log") {
        return true;
    }

    // Daily rolling files without .log produced by tracing-appender
    if let Some(date_part) = filename
        .strip_prefix("neuroclip.")
        .or_else(|| filename.strip_prefix("neuroclip-."))
        .or_else(|| filename.strip_prefix("neuroclip-"))
    {
        return is_daily_date_suffix(date_part);
    }

    false
}

/// Scan and prune the log directory according to the retention policy.
///
/// # Errors
/// Returns `StorageError` if directory traversal or file operations encounter I/O errors.
pub fn prune_logs(log_dir: &Path, policy: &RetentionPolicy) -> Result<PruneReport, StorageError> {
    if !log_dir.exists() {
        return Ok(PruneReport::default());
    }

    let read_dir = fs::read_dir(log_dir).map_err(|err| {
        StorageError::DirectoryCreationFailed(format!(
            "Failed to read log directory '{}': {err}",
            log_dir.display()
        ))
    })?;

    let now = SystemTime::now();
    let max_age_duration = Duration::from_secs(u64::from(policy.max_age_days) * 86_400);

    let mut report = PruneReport::default();
    let mut active_files: Vec<LogFileCandidate> = Vec::new();

    for entry_res in read_dir {
        let entry = match entry_res {
            Ok(e) => e,
            Err(_) => continue,
        };

        let path = entry.path();
        if !path.is_file() {
            continue;
        }

        let Some(file_name) = path.file_name().and_then(|n| n.to_str()) else {
            continue;
        };

        // Strict guard: only process NeuroClip log files
        if !is_neuroclip_log_file(file_name) {
            continue;
        }

        let metadata = match entry.metadata() {
            Ok(m) => m,
            Err(_) => continue,
        };

        let size_bytes = metadata.len();
        let modified_time = metadata.modified().unwrap_or(now);

        // Phase 1: Prune expired files older than max_age_days
        let is_expired = if policy.max_age_days == 0 {
            true
        } else {
            now.duration_since(modified_time)
                .map(|elapsed| elapsed > max_age_duration)
                .unwrap_or(false)
        };

        if is_expired {
            if fs::remove_file(&path).is_ok() {
                report.deleted_expired_files += 1;
                report.bytes_freed += size_bytes;
            }
        } else {
            active_files.push(LogFileCandidate {
                path,
                size_bytes,
                modified_time,
            });
        }
    }

    // Phase 2: Prune overflow files if total active size > max_dir_size_bytes
    let mut total_size: u64 = active_files.iter().map(|f| f.size_bytes).sum();

    if total_size > policy.max_dir_size_bytes {
        // Sort oldest first by modification time
        active_files.sort_by(|a, b| a.modified_time.cmp(&b.modified_time));

        for file in active_files {
            if total_size <= policy.target_dir_size_bytes {
                break;
            }

            if fs::remove_file(&file.path).is_ok() {
                report.deleted_overflow_files += 1;
                report.bytes_freed += file.size_bytes;
                total_size = total_size.saturating_sub(file.size_bytes);
            }
        }
    }

    report.remaining_bytes = total_size;
    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_neuroclip_log_file() {
        // Valid log files
        assert!(is_neuroclip_log_file("neuroclip-2026-11-10.log"));
        assert!(is_neuroclip_log_file("neuroclip.2026-11-10"));
        assert!(is_neuroclip_log_file("neuroclip-.2026-11-10"));
        assert!(is_neuroclip_log_file("neuroclip-2026-11-10"));
        assert!(is_neuroclip_log_file("neuroclip-test.log"));
        assert!(is_neuroclip_log_file("neuroclip.log"));
        assert!(is_neuroclip_log_file("neuroclip-trace.log"));

        // Foreign and non-log files (MUST BE REJECTED)
        assert!(!is_neuroclip_log_file("random_file.txt"));
        assert!(!is_neuroclip_log_file("hardware_profile.json"));
        assert!(!is_neuroclip_log_file("app_config.json"));
        assert!(!is_neuroclip_log_file("neuroclip-config.json"));
        assert!(!is_neuroclip_log_file("neuroclip.json"));
        assert!(!is_neuroclip_log_file("neuroclip.txt"));
        assert!(!is_neuroclip_log_file("neuroclip-backup.bak"));
        assert!(!is_neuroclip_log_file("neuroclip.bak"));
        assert!(!is_neuroclip_log_file("neuroclip.old"));
        assert!(!is_neuroclip_log_file("subtitles.ass"));
        assert!(!is_neuroclip_log_file("vod_audio.wav"));
        assert!(!is_neuroclip_log_file("random_app.log"));
    }

    #[test]
    fn test_prune_logs_non_existent_dir() {
        let dummy = Path::new("non_existent_neuroclip_dir_xyz123");
        let res = prune_logs(dummy, &RetentionPolicy::default());
        assert!(res.is_ok());
        if let Ok(report) = res {
            assert_eq!(report, PruneReport::default());
        }
    }
}
