//! Tauri IPC commands for managed sidecar auto-updates and anti-bot defense (`TASK-M4-29`).
//!
//! Complies with ADR-007, ERR-04, AC-29.1 to AC-29.4, and `docs/CONVENTIONS.md`.

use serde::{Deserialize, Serialize};
use tauri::{command, AppHandle, Emitter};
use tracing::{info, warn};

use crate::error::AppError;
use crate::sidecar::manifest::SidecarTool;
use crate::sidecar::updater::{
    AntiBotGuidance, ReleaseCheckInfo, SidecarStatusReport, SidecarUpdateReport, SidecarUpdater,
    SidecarUpdaterConfig,
};

/// Canonical Tauri event name for sidecar status changes (`CONVENTIONS.md` §4.2).
pub const SIDECAR_STATUS_EVENT: &str = "sidecar:status";

/// Real-time status payload emitted on `sidecar:status`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SidecarStatusEventPayload {
    /// Binary name (`yt-dlp` or `ffmpeg`).
    pub binary: String,
    /// Current status (`ready`, `updating`, or `failed`).
    pub status: String,
    /// Optional user-facing message or error reason.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}

impl SidecarStatusEventPayload {
    /// Helper to construct a "ready" status event.
    #[must_use]
    pub fn ready(binary: impl Into<String>, message: Option<String>) -> Self {
        Self {
            binary: binary.into(),
            status: "ready".to_string(),
            message,
        }
    }

    /// Helper to construct an "updating" status event.
    #[must_use]
    pub fn updating(binary: impl Into<String>, message: Option<String>) -> Self {
        Self {
            binary: binary.into(),
            status: "updating".to_string(),
            message,
        }
    }

    /// Helper to construct a "failed" status event.
    #[must_use]
    pub fn failed(binary: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            binary: binary.into(),
            status: "failed".to_string(),
            message: Some(message.into()),
        }
    }
}

/// Query for available sidecar updates via GitHub Releases API (`AC-29.2`).
///
/// If `tool_name` is omitted, queries all managed tools (defaulting to `yt-dlp`).
///
/// # Errors
/// Returns [`AppError::Sidecar`] if network requests or JSON parsing fails.
#[command]
pub async fn check_sidecar_updates(
    tool_name: Option<String>,
    _force: Option<bool>,
) -> Result<Vec<ReleaseCheckInfo>, AppError> {
    let updater = SidecarUpdater::new(SidecarUpdaterConfig::default());
    let tools = match tool_name.as_deref() {
        Some("all") | None => vec![SidecarTool::YtDlp],
        Some(name) => match SidecarTool::from_str_loose(name) {
            Some(t) => vec![t],
            None => vec![SidecarTool::YtDlp],
        },
    };

    let mut results = Vec::with_capacity(tools.len());
    for tool in tools {
        let info = updater.check_latest_release(tool).await?;
        results.push(info);
    }

    Ok(results)
}

/// Download, verify SHA-256 hash, and atomically apply update in place (`AC-29.1`, `AC-29.2`, `AC-29.3`).
///
/// Emits `sidecar:status` events (`updating` -> `ready` / `failed`) during execution.
///
/// # Errors
/// Returns [`AppError::Sidecar`] if downloading, checksum verification, or atomic swap fails.
#[command]
pub async fn download_and_apply_sidecar_update(
    tool_name: Option<String>,
    app: AppHandle,
) -> Result<SidecarUpdateReport, AppError> {
    let tool = tool_name
        .as_deref()
        .and_then(SidecarTool::from_str_loose)
        .unwrap_or(SidecarTool::YtDlp);

    let updater = SidecarUpdater::new(SidecarUpdaterConfig::default());

    // Emit updating event
    let _ = app.emit(
        SIDECAR_STATUS_EVENT,
        SidecarStatusEventPayload::updating(
            tool.as_str(),
            Some(format!(
                "Đang tải và kiểm tra bản cập nhật cho '{}'...",
                tool.as_str()
            )),
        ),
    );

    match updater.apply_update(tool).await {
        Ok(report) => {
            let _ = app.emit(
                SIDECAR_STATUS_EVENT,
                SidecarStatusEventPayload::ready(tool.as_str(), Some(report.message.clone())),
            );
            Ok(report)
        }
        Err(e) => {
            let _ = app.emit(
                SIDECAR_STATUS_EVENT,
                SidecarStatusEventPayload::failed(tool.as_str(), e.to_string()),
            );
            Err(e)
        }
    }
}

/// Get inventory and current execution status of managed sidecar binaries.
///
/// # Errors
/// Returns [`AppError`] if querying status fails.
#[command]
pub async fn get_sidecar_status(
    tool_name: Option<String>,
) -> Result<Vec<SidecarStatusReport>, AppError> {
    let updater = SidecarUpdater::new(SidecarUpdaterConfig::default());
    let tools = match tool_name.as_deref() {
        Some("all") | None => vec![SidecarTool::YtDlp, SidecarTool::Ffmpeg],
        Some(name) => match SidecarTool::from_str_loose(name) {
            Some(t) => vec![t],
            None => vec![SidecarTool::YtDlp, SidecarTool::Ffmpeg],
        },
    };

    let mut reports = Vec::with_capacity(tools.len());
    for tool in tools {
        reports.push(updater.get_status(tool));
    }

    Ok(reports)
}

/// Retrieve comprehensive multi-layer anti-bot defense guidance (`ERR-04`, `AC-29.4`).
///
/// # Errors
/// Returns [`AppError`] if generating guidance fails.
#[command]
pub async fn get_anti_bot_guidance(
    error_details: Option<String>,
) -> Result<AntiBotGuidance, AppError> {
    let updater = SidecarUpdater::new(SidecarUpdaterConfig::default());
    Ok(updater.create_anti_bot_guidance(SidecarTool::YtDlp, error_details))
}

/// Trigger defensive sidecar update upon encountering `ERR_INGEST_BOT_BLOCKED` (`ED-29.1`).
///
/// Asynchronously schedules a background update check/download while immediately returning
/// structured guidance to the UI.
///
/// # Errors
/// Returns [`AppError`] if initial guidance creation fails.
#[command]
pub async fn trigger_bot_defense_update(
    app: AppHandle,
    error_details: Option<String>,
) -> Result<AntiBotGuidance, AppError> {
    let updater = SidecarUpdater::new(SidecarUpdaterConfig::default());
    let guidance = updater.create_anti_bot_guidance(SidecarTool::YtDlp, error_details);

    // Spawn non-blocking background task to attempt update (`AC-29.2`)
    let bg_updater = updater.clone();
    let bg_app = app.clone();
    tokio::spawn(async move {
        info!("Executing background yt-dlp update triggered by bot block challenge");
        let _ = bg_app.emit(
            SIDECAR_STATUS_EVENT,
            SidecarStatusEventPayload::updating(
                "yt-dlp",
                Some(
                    "Hệ thống đang tự động nâng cấp yt-dlp để vượt qua kiểm tra bot...".to_string(),
                ),
            ),
        );

        match bg_updater.apply_update(SidecarTool::YtDlp).await {
            Ok(report) => {
                info!(new_version = %report.updated_version, "Background bot defense update succeeded");
                let _ = bg_app.emit(
                    SIDECAR_STATUS_EVENT,
                    SidecarStatusEventPayload::ready("yt-dlp", Some(report.message)),
                );
            }
            Err(e) => {
                warn!(error = %e, "Background bot defense update failed or was already latest");
                let _ = bg_app.emit(
                    SIDECAR_STATUS_EVENT,
                    SidecarStatusEventPayload::failed("yt-dlp", e.to_string()),
                );
            }
        }
    });

    Ok(guidance)
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;

    #[test]
    fn test_sidecar_status_event_payload() {
        let p1 = SidecarStatusEventPayload::ready("yt-dlp", Some("Ready".to_string()));
        assert_eq!(p1.binary, "yt-dlp");
        assert_eq!(p1.status, "ready");

        let p2 = SidecarStatusEventPayload::updating("yt-dlp", None);
        assert_eq!(p2.status, "updating");

        let p3 = SidecarStatusEventPayload::failed("yt-dlp", "Network timeout");
        assert_eq!(p3.status, "failed");
        assert_eq!(p3.message, Some("Network timeout".to_string()));
    }
}
