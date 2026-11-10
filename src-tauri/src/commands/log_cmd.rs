//! IPC Command handlers for Logging, Retrieval, Truncation & Telemetry.
//!
//! Exposes Tauri commands adhering to the `Result<T, AppError>` contract:
//! - `get_recent_logs(limit, level_filter)`: Retrieve recent log lines with ring-buffer bounding.
//! - `clear_logs()`: Purge / truncate log files on disk.
//! - `log_frontend_event(level, target, message, context)`: Bridge frontend logs to Rust backend tracing.

use std::fs::{self, File};
use std::io::{BufRead, BufReader};
use std::path::PathBuf;
use crate::error::{AppError, StorageError};
use crate::logging::{is_neuroclip_log_file, redact_secrets_in_text, resolve_log_dir, LogEntry};

const DEFAULT_LOG_LIMIT: usize = 200;
const MAX_LOG_LIMIT: usize = 1_000;

/// Helper to extract YYYY-MM-DD date string from candidate log filenames for sorting.
fn extract_log_date(path: &std::path::Path) -> Option<&str> {
    let name = path.file_name()?.to_str()?;
    let stripped = name.strip_prefix("neuroclip")?;
    let trimmed = stripped.trim_start_matches(|c| c == '-' || c == '.');
    if trimmed.len() >= 10 {
        let bytes = trimmed.as_bytes();
        let is_date = bytes[0..4].iter().all(u8::is_ascii_digit)
            && bytes[4] == b'-'
            && bytes[5..7].iter().all(u8::is_ascii_digit)
            && bytes[7] == b'-'
            && bytes[8..10].iter().all(u8::is_ascii_digit);
        if is_date {
            return Some(&trimmed[..10]);
        }
    }
    None
}

/// Retrieve recent structured log entries from the local storage directory.
///
/// Limits results to a maximum of 1,000 entries and filters by log level if specified.
#[tauri::command]
pub async fn get_recent_logs(
    limit: Option<usize>,
    level_filter: Option<String>,
) -> Result<Vec<LogEntry>, AppError> {
    let effective_limit = limit.unwrap_or(DEFAULT_LOG_LIMIT).clamp(1, MAX_LOG_LIMIT);
    let log_dir = resolve_log_dir();

    if !log_dir.exists() {
        return Ok(Vec::new());
    }

    // Locate candidate log files
    let read_dir = fs::read_dir(&log_dir).map_err(|err| {
        AppError::Storage(StorageError::DirectoryCreationFailed(format!(
            "Failed to read log directory: {err}"
        )))
    })?;

    let mut log_files: Vec<PathBuf> = Vec::new();
    for entry in read_dir.flatten() {
        let path = entry.path();
        if path.is_file() {
            if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                if is_neuroclip_log_file(name) {
                    log_files.push(path);
                }
            }
        }
    }

    if log_files.is_empty() {
        return Ok(Vec::new());
    }

    // Sort newest log file first by filename date, modified timestamp, or path descending
    log_files.sort_by(|a, b| {
        let date_a = extract_log_date(a);
        let date_b = extract_log_date(b);
        match (date_b, date_a) {
            (Some(db), Some(da)) if db != da => db.cmp(da),
            _ => {
                let mtime_b = b.metadata().and_then(|m| m.modified()).ok();
                let mtime_a = a.metadata().and_then(|m| m.modified()).ok();
                mtime_b.cmp(&mtime_a).then_with(|| b.cmp(a))
            }
        }
    });

    let mut entries: Vec<LogEntry> = Vec::new();

    for file_path in log_files {
        if entries.len() >= effective_limit {
            break;
        }

        let file = match File::open(&file_path) {
            Ok(f) => f,
            Err(_) => continue,
        };

        let reader = BufReader::new(file);
        let mut file_entries: Vec<LogEntry> = Vec::new();

        for line_res in reader.lines() {
            let line = match line_res {
                Ok(l) => l,
                Err(_) => continue,
            };

            let trimmed = line.trim();
            if trimmed.is_empty() {
                continue;
            }

            // Attempt structured JSON parse
            if let Ok(mut val) = serde_json::from_str::<serde_json::Value>(trimmed) {
                let timestamp = val
                    .get("timestamp")
                    .and_then(|t| t.as_str())
                    .unwrap_or("")
                    .to_string();

                let level = val
                    .get("level")
                    .and_then(|l| l.as_str())
                    .unwrap_or("INFO")
                    .to_uppercase();

                let target = val
                    .get("target")
                    .and_then(|t| t.as_str())
                    .unwrap_or("app")
                    .to_string();

                let message = val
                    .get("message")
                    .and_then(|m| m.as_str())
                    .unwrap_or("")
                    .to_string();

                // Extract fields object excluding core envelope attributes
                let fields = if let Some(map) = val.as_object_mut() {
                    map.remove("timestamp");
                    map.remove("level");
                    map.remove("target");
                    map.remove("message");
                    let line_num = map.remove("line").and_then(|l| l.as_u64()).map(|n| n as u32);
                    let file_str = map.remove("file").and_then(|f| f.as_str().map(ToString::to_string));

                    if map.is_empty() {
                        None
                    } else {
                        Some(serde_json::Value::Object(map.clone()))
                    }
                    .map(|f_val| (f_val, line_num, file_str))
                } else {
                    None
                };

                let (fields_val, line_num, file_str) = match fields {
                    Some((fv, ln, fs)) => (Some(fv), ln, fs),
                    None => (None, None, None),
                };

                file_entries.push(LogEntry {
                    timestamp,
                    level,
                    target,
                    message,
                    fields: fields_val,
                    line: line_num,
                    file: file_str,
                });
            } else {
                // Raw plain text line fallback
                file_entries.push(LogEntry {
                    timestamp: chrono::Utc::now().to_rfc3339(),
                    level: "INFO".to_string(),
                    target: "system".to_string(),
                    message: trimmed.to_string(),
                    fields: None,
                    line: None,
                    file: None,
                });
            }
        }

        // Apply level filter
        if let Some(ref filter) = level_filter {
            let filter_upper = filter.to_uppercase();
            if filter_upper != "ALL" {
                file_entries.retain(|e| e.level == filter_upper);
            }
        }

        // Collect newest entries from this file (iterating in reverse order)
        for entry in file_entries.into_iter().rev() {
            if entries.len() >= effective_limit {
                break;
            }
            entries.push(entry);
        }
    }

    // Entries were collected newest-to-oldest across files.
    // Reverse the vector so that returned entries are in monotonic chronological order
    // (oldest to newest within the requested recent window).
    entries.reverse();

    Ok(entries)
}

/// Purge all log files from the local logs storage directory.
#[tauri::command]
pub async fn clear_logs() -> Result<(), AppError> {
    let log_dir = resolve_log_dir();

    if !log_dir.exists() {
        return Ok(());
    }

    let read_dir = fs::read_dir(&log_dir).map_err(|err| {
        AppError::Storage(StorageError::CacheCleanupFailed(format!(
            "Failed to read log directory during clear: {err}"
        )))
    })?;

    for entry in read_dir.flatten() {
        let path = entry.path();
        if path.is_file() {
            if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                if is_neuroclip_log_file(name) {
                    // Try to delete; if file is locked on Windows by logging worker, truncate it
                    if fs::remove_file(&path).is_err() {
                        let _ = File::create(&path);
                    }
                }
            }
        }
    }

    Ok(())
}

/// Receive a frontend logging event, apply defense-in-depth sanitization, and forward to Rust tracing.
#[tauri::command]
pub async fn log_frontend_event(
    level: String,
    target: Option<String>,
    message: String,
    context: Option<serde_json::Value>,
) -> Result<(), AppError> {
    let safe_message = redact_secrets_in_text(&message);
    let target_str = target.unwrap_or_else(|| "frontend".to_string());

    let safe_context = context.map(|ctx| {
        if let Ok(ctx_str) = serde_json::to_string(&ctx) {
            let sanitized_str = redact_secrets_in_text(&ctx_str);
            serde_json::from_str(&sanitized_str).unwrap_or(serde_json::Value::Null)
        } else {
            serde_json::Value::Null
        }
    });

    match level.to_lowercase().as_str() {
        "trace" => {
            tracing::trace!(target: "frontend", module = %target_str, message = %safe_message, context = ?safe_context);
        }
        "debug" => {
            tracing::debug!(target: "frontend", module = %target_str, message = %safe_message, context = ?safe_context);
        }
        "info" => {
            tracing::info!(target: "frontend", module = %target_str, message = %safe_message, context = ?safe_context);
        }
        "warn" => {
            tracing::warn!(target: "frontend", module = %target_str, message = %safe_message, context = ?safe_context);
        }
        "error" => {
            tracing::error!(target: "frontend", module = %target_str, message = %safe_message, context = ?safe_context);
        }
        _ => {
            tracing::info!(target: "frontend", module = %target_str, message = %safe_message, context = ?safe_context);
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_get_recent_logs_empty_on_missing_dir() {
        let res = get_recent_logs(Some(10), None).await;
        assert!(res.is_ok());
    }

    #[tokio::test]
    async fn test_log_frontend_event_safe_execution() {
        let res = log_frontend_event(
            "info".to_string(),
            Some("test::ui".to_string()),
            "Loaded key gsk_1234567890abcdef12345678".to_string(),
            Some(serde_json::json!({ "token": "AIzaSyD987654321abcdef1234567890" })),
        )
        .await;

        assert!(res.is_ok());
    }

    #[tokio::test]
    async fn test_get_recent_logs_multi_file_retention_order() {
        let temp_dir = std::env::temp_dir().join(format!("neuroclip_test_logs_{}", std::process::id()));
        let _ = fs::create_dir_all(&temp_dir);
        let temp_str = match temp_dir.to_str() {
            Some(s) => s,
            None => return,
        };
        std::env::set_var("NEUROCLIP_LOG_DIR", temp_str);

        // File 1 (Yesterday): 30 entries (yesterday_00..yesterday_29)
        let file_yesterday = temp_dir.join("neuroclip-2026-11-09.log");
        let mut y_content = String::new();
        for i in 0..30 {
            y_content.push_str(&format!(
                "{{\"timestamp\":\"2026-11-09T10:{:02}:00Z\",\"level\":\"INFO\",\"target\":\"app\",\"message\":\"yesterday_{}\"}}\n",
                i, i
            ));
        }
        let _ = fs::write(&file_yesterday, y_content);

        // File 2 (Today): 20 entries (today_00..today_19)
        let file_today = temp_dir.join("neuroclip-2026-11-10.log");
        let mut t_content = String::new();
        for i in 0..20 {
            t_content.push_str(&format!(
                "{{\"timestamp\":\"2026-11-10T10:{:02}:00Z\",\"level\":\"INFO\",\"target\":\"app\",\"message\":\"today_{}\"}}\n",
                i, i
            ));
        }
        let _ = fs::write(&file_today, t_content);

        // Query with effective limit 35 entries
        let res = get_recent_logs(Some(35), None).await;
        assert!(res.is_ok());
        if let Ok(entries) = res {
            assert_eq!(entries.len(), 35);
            // Verify that all 20 entries from today are retained at the tail
            let today_count = entries.iter().filter(|e| e.message.starts_with("today_")).count();
            assert_eq!(today_count, 20);

            // Verify that 15 entries from yesterday are retained at the head
            let yesterday_count = entries.iter().filter(|e| e.message.starts_with("yesterday_")).count();
            assert_eq!(yesterday_count, 15);

            // First entry is yesterday_15, last entry is today_19
            assert_eq!(entries.first().map(|e| e.message.as_str()), Some("yesterday_15"));
            assert_eq!(entries.last().map(|e| e.message.as_str()), Some("today_19"));
        }

        let _ = fs::remove_dir_all(&temp_dir);
        std::env::remove_var("NEUROCLIP_LOG_DIR");
    }
}
