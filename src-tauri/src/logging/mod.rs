//! Structured Logging, Secret Redaction & Rotation Infrastructure.
//!
//! Subsystem implementing `docs/CONVENTIONS.md` §6 and `docs/TASK_BOARD.md` §3.1 (TASK-M4-04).
//!
//! Features:
//! - Layered Tracing Subscriber (`tracing-subscriber`) with `EnvFilter`.
//! - Streaming Secret Redaction (`RedactingWriter`) for zero-leak protection (AC-04.1).
//! - Daily Rolling NDJSON file logging with non-blocking appender (`tracing_appender::non_blocking`).
//! - Automatic Log Rotation & Pruning (14 days retention, 50MB ceiling, 30MB low-water mark) (AC-04.2).
//! - Multi-tier path resolver targeting `%LOCALAPPDATA%/NeuroClip/logs/` with `NEUROCLIP_LOG_DIR` test override.
//! - Safe subscriber registration with `.try_init()` to eliminate multi-init test collisions.

pub mod rotation;
pub mod secrets;

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use serde::{Deserialize, Serialize};
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::EnvFilter;

pub use rotation::{is_neuroclip_log_file, prune_logs, PruneReport, RetentionPolicy};
pub use secrets::{redact_secret, redact_secrets_in_text, RedactingWriter};

/// Static worker guard preventing premature drop of non-blocking tracing worker thread.
static LOG_GUARD: OnceLock<tracing_appender::non_blocking::WorkerGuard> = OnceLock::new();

/// Structured log entry transfer contract between Rust backend and Tauri IPC/Frontend.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct LogEntry {
    /// ISO8601 UTC timestamp string.
    pub timestamp: String,
    /// Uppercase severity level: TRACE, DEBUG, INFO, WARN, ERROR.
    pub level: String,
    /// Tracing target / module name (e.g. `neuroclip::ingestion`, `frontend`).
    pub target: String,
    /// Primary event message with secrets safely redacted.
    pub message: String,
    /// Optional structured metadata fields in JSON format.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fields: Option<serde_json::Value>,
    /// Optional source file line number.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub line: Option<u32>,
    /// Optional source file path.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub file: Option<String>,
}

/// Resolve the canonical log storage directory.
///
/// Multi-tier resolution strategy:
/// 1. `NEUROCLIP_LOG_DIR` environment variable (for isolated unit/integration tests).
/// 2. Windows `%LOCALAPPDATA%\NeuroClip\logs\` per SSOT `CONVENTIONS.md` §6.4.
/// 3. `%USERPROFILE%\AppData\Local\NeuroClip\logs\`.
/// 4. Fallback to system temp directory `temp_dir()/neuroclip_logs`.
#[must_use]
pub fn resolve_log_dir() -> PathBuf {
    if let Ok(dir) = std::env::var("NEUROCLIP_LOG_DIR") {
        let trimmed = dir.trim();
        if !trimmed.is_empty() {
            return PathBuf::from(trimmed);
        }
    }

    if let Ok(local_app_data) = std::env::var("LOCALAPPDATA") {
        let trimmed = local_app_data.trim();
        if !trimmed.is_empty() {
            return PathBuf::from(trimmed).join("NeuroClip").join("logs");
        }
    }

    if let Ok(user_profile) = std::env::var("USERPROFILE") {
        let trimmed = user_profile.trim();
        if !trimmed.is_empty() {
            return PathBuf::from(trimmed)
                .join("AppData")
                .join("Local")
                .join("NeuroClip")
                .join("logs");
        }
    }

    std::env::temp_dir().join("neuroclip_logs")
}

/// Initialize the global logging subsystem with canonical settings and directory pruning.
///
/// # Errors
/// Returns error description if directory creation or tracing subscriber setup fails.
pub fn init_logging() -> Result<(), String> {
    let log_dir = resolve_log_dir();
    init_logging_with_dir(&log_dir)
}

/// Initialize the global logging subsystem targeting a specific directory.
///
/// # Errors
/// Returns error description if directory creation or subscriber initialization fails.
pub fn init_logging_with_dir(log_dir: &Path) -> Result<(), String> {
    // 1. Ensure target directory exists
    if !log_dir.exists() {
        fs::create_dir_all(log_dir).map_err(|err| {
            format!(
                "Failed to create log directory at '{}': {err}",
                log_dir.display()
            )
        })?;
    }

    // 2. Perform startup retention pruning (AC-04.2)
    let _ = prune_logs(log_dir, &RetentionPolicy::default());

    // 3. Configure daily rolling file appender with streaming secret redaction
    let rolling_appender = tracing_appender::rolling::daily(log_dir, "neuroclip-");
    let redacting_appender = RedactingWriter::new(rolling_appender);
    let (non_blocking_appender, guard) = tracing_appender::non_blocking(redacting_appender);

    // Keep guard alive across application lifecycle
    let _ = LOG_GUARD.set(guard);

    // 4. Configure EnvFilter with sensible defaults
    let env_filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("info,neuroclip=debug"));

    // 5. Build structured file logging layer (Release & Debug)
    let file_layer = tracing_subscriber::fmt::layer()
        .json()
        .with_current_span(false)
        .with_span_list(false)
        .flatten_event(true)
        .with_writer(non_blocking_appender);

    // 6. Build console logging layer (Pretty in debug mode, compact in release)
    #[cfg(debug_assertions)]
    let console_layer = tracing_subscriber::fmt::layer()
        .pretty()
        .with_target(true)
        .with_line_number(true)
        .with_file(true);

    #[cfg(not(debug_assertions))]
    let console_layer = tracing_subscriber::fmt::layer().compact();

    // 7. Compose subscriber registry and initialize safely with try_init()
    let subscriber = tracing_subscriber::registry()
        .with(env_filter)
        .with(console_layer)
        .with(file_layer);

    // Use try_init() to gracefully ignore repeated initialization during test runs
    let _ = subscriber.try_init();

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_resolve_log_dir_override() {
        std::env::set_var("NEUROCLIP_LOG_DIR", "C:\\test\\neuroclip_isolated_logs");
        let dir = resolve_log_dir();
        assert_eq!(dir, PathBuf::from("C:\\test\\neuroclip_isolated_logs"));
        std::env::remove_var("NEUROCLIP_LOG_DIR");
    }

    #[test]
    fn test_log_entry_serialization_roundtrip() {
        let entry = LogEntry {
            timestamp: "2026-11-10T04:15:30.123Z".to_string(),
            level: "INFO".to_string(),
            target: "neuroclip::ingestion".to_string(),
            message: "Test message".to_string(),
            fields: Some(serde_json::json!({ "vod_id": "12345" })),
            line: Some(42),
            file: Some("src/ingestion.rs".to_string()),
        };

        let json_str = serde_json::to_string(&entry);
        assert!(json_str.is_ok());
        if let Ok(serialized) = json_str {
            let deserialized: Result<LogEntry, _> = serde_json::from_str(&serialized);
            assert!(deserialized.is_ok());
            if let Ok(deser) = deserialized {
                assert_eq!(deser, entry);
            }
        }
    }
}
