//! YouTube Live Chat Replay extraction via managed yt-dlp sidecar supervisor.
//!
//! Complies with docs/CONVENTIONS.md (zero-panic invariant, Result<T, AppError>)
//! and docs/ARCHITECTURE.md §3.3, §4.2, §6 (ERR-05).

use std::path::{Path, PathBuf};
use std::process::Stdio;
use tokio::process::Command;
use tokio_util::sync::CancellationToken;

use crate::error::{AppError, IngestError, SidecarError, StorageError};
use crate::ingestion::streamer::assign_child_to_job_guard;
use crate::ingestion::twitch_gql::ChatProgressPayload;

fn resolve_ytdlp_binary() -> String {
    if let Ok(p) = std::env::var("NEUROCLIP_YTDLP_PATH") {
        let trimmed = p.trim();
        if !trimmed.is_empty() {
            return trimmed.to_string();
        }
    }
    "yt-dlp".to_string()
}

/// Download YouTube Live Chat replay NDJSON stream using managed yt-dlp.
///
/// If video has no live chat replay (e.g. standard pre-recorded upload or disabled replay),
/// returns `ERR_INGEST_CHAT_UNAVAILABLE` within <= 200ms to allow soft fallback (ERR-05).
pub async fn fetch_youtube_live_chat_replay<F>(
    url: &str,
    vod_id: &str,
    output_dir: &Path,
    cancel_token: CancellationToken,
    progress_callback: F,
) -> Result<PathBuf, AppError>
where
    F: Fn(ChatProgressPayload) + Send + Sync + 'static,
{
    // Fast mock check for unit / integration tests
    if std::env::var("NEUROCLIP_MOCK_CHAT_UNAVAILABLE").is_ok() {
        return Err(AppError::Ingestion(IngestError::ChatReplayUnavailable));
    }

    let expected_output = output_dir.join(format!("{vod_id}.live_chat.json"));

    if let Ok(mock_file_path) = std::env::var("NEUROCLIP_MOCK_YTDLP_CHAT_FILE") {
        if let Ok(content) = std::fs::read_to_string(&mock_file_path) {
            std::fs::write(&expected_output, content.as_bytes()).map_err(|e| {
                AppError::Storage(StorageError::DirectoryCreationFailed {
                    path: expected_output.display().to_string(),
                    details: e.to_string(),
                })
            })?;
            let line_count = content.lines().filter(|l| !l.trim().is_empty()).count() as u64;
            progress_callback(ChatProgressPayload {
                task_id: vod_id.to_string(),
                stage: "completed".to_string(),
                percent: 100.0,
                messages_extracted: line_count,
                current_offset_sec: 0.0,
            });
            return Ok(expected_output);
        }
    }

    let ytdlp_bin = resolve_ytdlp_binary();
    let template_path = output_dir.join(format!("{vod_id}.%(ext)s"));

    progress_callback(ChatProgressPayload {
        task_id: vod_id.to_string(),
        stage: "extracting_youtube_chat".to_string(),
        percent: 10.0,
        messages_extracted: 0,
        current_offset_sec: 0.0,
    });

    let args = [
        "--skip-download".to_string(),
        "--write-subs".to_string(),
        "--sub-langs".to_string(),
        "live_chat".to_string(),
        "--no-warnings".to_string(),
        "--no-playlist".to_string(),
        "--retries".to_string(),
        "3".to_string(),
        "--socket-timeout".to_string(),
        "20".to_string(),
        "-o".to_string(),
        template_path.to_string_lossy().to_string(),
        url.to_string(),
    ];

    let mut cmd = Command::new(&ytdlp_bin);
    cmd.args(&args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);

    let child = cmd.spawn().map_err(|e| {
        AppError::Sidecar(SidecarError::BinaryNotFound {
            name: "yt-dlp".to_string(),
            path: format!("{ytdlp_bin}: {e}"),
        })
    })?;
    assign_child_to_job_guard(&child);

    // Watch cancellation while waiting for process
    let output = tokio::select! {
        _ = cancel_token.cancelled() => {
            return Err(AppError::TaskCancelled);
        }
        res = child.wait_with_output() => {
            res.map_err(|e| AppError::Sidecar(SidecarError::ExecutionFailed {
                binary: "yt-dlp".to_string(),
                exit_code: None,
                details: e.to_string(),
            }))?
        }
    };

    let stderr = String::from_utf8_lossy(&output.stderr);
    if stderr.contains("Sign in to confirm you’re not a bot") || stderr.contains("429") {
        return Err(AppError::Ingestion(IngestError::BotBlocked(stderr.to_string())));
    }

    if stderr.contains("No live chat found")
        || stderr.contains("no subtitles")
        || stderr.contains("There's no subtitles")
    {
        return Err(AppError::Ingestion(IngestError::ChatReplayUnavailable));
    }

    // Locate the downloaded live chat file
    // yt-dlp produces {vod_id}.live_chat.json
    if expected_output.exists() {
        progress_callback(ChatProgressPayload {
            task_id: vod_id.to_string(),
            stage: "completed".to_string(),
            percent: 100.0,
            messages_extracted: 0,
            current_offset_sec: 0.0,
        });
        return Ok(expected_output);
    }

    // Check directory for alternate pattern e.g. {vod_id}.live_chat.json or .dump
    if let Ok(entries) = std::fs::read_dir(output_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                if name.starts_with(vod_id) && name.contains("live_chat") {
                    return Ok(path);
                }
            }
        }
    }

    // If file was not produced, chat replay is not available
    Err(AppError::Ingestion(IngestError::ChatReplayUnavailable))
}
