//! Audio-first ingestion engine & FFmpeg streaming transcode pipeline.
//!
//! Complies with docs/CONVENTIONS.md, docs/ARCHITECTURE.md §4.2, §5.2, and TASK-M4-06.
//! Strict zero-panic invariant: all fallible operations return Result<T, AppError>.

use chrono::Utc;
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::{Duration, Instant};
use tokio::io::AsyncReadExt;
use tokio::process::Command;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

use crate::core::storage::check_disk_space;
use crate::error::{AppError, IngestError, SidecarError};
use crate::ingestion::buffer::{
    create_bounded_audio_channel, AudioChunk, AudioRingBuffer, DEFAULT_CHUNK_SIZE_BYTES,
    DEFAULT_QUEUE_CAPACITY,
};
use crate::ingestion::cookies::{try_build_bot_defense_args, CookieSource};
use crate::ingestion::validator::AudioStreamValidator;
use crate::sidecar::{drain_pipe_lossy, resolve_sidecar_binary, LossyLineReader};

/// Standard payload emitted periodically through the `ingest:progress` Tauri event stream.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IngestProgressPayload {
    pub task_id: String,
    pub stage: String,
    pub percent: f64,
    pub speed_kbps: f64,
    pub eta_sec: u64,
}

/// VOD Metadata parsed from `yt-dlp --dump-single-json`.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct VodMetadata {
    pub id: String,
    pub title: String,
    #[serde(default)]
    pub uploader: Option<String>,
    #[serde(default)]
    pub channel: Option<String>,
    #[serde(default)]
    pub duration: Option<f64>,
    #[serde(default)]
    pub is_live: Option<bool>,
    #[serde(default)]
    pub live_status: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub video_stream_url: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Default)]
struct YtDlpFormat {
    #[serde(default)]
    url: Option<String>,
    #[serde(default)]
    manifest_url: Option<String>,
    #[serde(default)]
    vcodec: Option<String>,
    #[serde(default)]
    acodec: Option<String>,
    #[serde(default)]
    height: Option<u32>,
    #[serde(default)]
    duration: Option<f64>,
}

fn deserialize_null_default<'de, D, T>(deserializer: D) -> Result<T, D::Error>
where
    T: Default + serde::Deserialize<'de>,
    D: serde::Deserializer<'de>,
{
    let opt = Option::deserialize(deserializer)?;
    Ok(opt.unwrap_or_default())
}

#[derive(Debug, Clone, Deserialize)]
struct YtDlpRawMetadata {
    id: String,
    title: String,
    #[serde(default)]
    uploader: Option<String>,
    #[serde(default)]
    channel: Option<String>,
    #[serde(default)]
    duration: Option<f64>,
    #[serde(default)]
    duration_string: Option<String>,
    #[serde(default)]
    is_live: Option<bool>,
    #[serde(default)]
    live_status: Option<String>,
    #[serde(default)]
    video_stream_url: Option<String>,
    #[serde(default)]
    manifest_url: Option<String>,
    #[serde(default)]
    url: Option<String>,
    #[serde(default, deserialize_with = "deserialize_null_default")]
    requested_formats: Vec<YtDlpFormat>,
    #[serde(default, deserialize_with = "deserialize_null_default")]
    formats: Vec<YtDlpFormat>,
}

fn is_valid_stream_url(candidate: &str, webpage_url: &str) -> bool {
    let trimmed = candidate.trim();
    if trimmed.is_empty() {
        return false;
    }
    if !trimmed.starts_with("http://") && !trimmed.starts_with("https://") {
        return false;
    }
    trimmed != webpage_url.trim()
}

fn format_valid_stream_url(fmt: &YtDlpFormat, webpage_url: &str) -> Option<String> {
    if let Some(ref manifest) = fmt.manifest_url {
        if is_valid_stream_url(manifest, webpage_url) {
            return Some(manifest.trim().to_string());
        }
    }
    if let Some(ref u) = fmt.url {
        if is_valid_stream_url(u, webpage_url) {
            return Some(u.trim().to_string());
        }
    }
    None
}

fn extract_video_stream_url(raw: &YtDlpRawMetadata, webpage_url: &str) -> Option<String> {
    if let Some(ref direct) = raw.video_stream_url {
        if is_valid_stream_url(direct, webpage_url) {
            return Some(direct.trim().to_string());
        }
    }

    if let Some(ref manifest) = raw.manifest_url {
        if is_valid_stream_url(manifest, webpage_url) {
            return Some(manifest.trim().to_string());
        }
    }

    for fmt in &raw.requested_formats {
        if fmt.vcodec.as_deref() == Some("none")
            || (fmt.vcodec.is_none() && fmt.acodec.as_deref().is_some_and(|a| a != "none"))
        {
            continue;
        }
        if let Some(url) = format_valid_stream_url(fmt, webpage_url) {
            return Some(url);
        }
    }

    if let Some(ref u) = raw.url {
        if is_valid_stream_url(u, webpage_url) {
            return Some(u.trim().to_string());
        }
    }

    // Best muxed format first (valid stream URL filtered before max height), then best video-only format
    if let Some(url) = raw
        .formats
        .iter()
        .filter(|f| {
            f.vcodec.as_deref().is_some_and(|v| v != "none")
                && f.acodec.as_deref().is_some_and(|a| a != "none")
        })
        .filter_map(|f| format_valid_stream_url(f, webpage_url).map(|u| (f.height.unwrap_or(0), u)))
        .max_by_key(|(height, _)| *height)
        .map(|(_, url)| url)
    {
        return Some(url);
    }

    if let Some(url) = raw
        .formats
        .iter()
        .filter(|f| f.vcodec.as_deref().is_some_and(|v| v != "none"))
        .filter_map(|f| format_valid_stream_url(f, webpage_url).map(|u| (f.height.unwrap_or(0), u)))
        .max_by_key(|(height, _)| *height)
        .map(|(_, url)| url)
    {
        return Some(url);
    }

    None
}

/// Parse a `yt-dlp` duration string (`HH:MM:SS`, `MM:SS`, or `SS`, with optional fractional seconds)
/// into total seconds.
#[must_use]
pub fn parse_duration_string(raw: &str) -> Option<f64> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return None;
    }

    let parts: Vec<&str> = trimmed.split(':').collect();
    if parts.is_empty() || parts.len() > 3 {
        return None;
    }

    let mut total = 0.0_f64;
    for part in parts {
        let val = part.trim().parse::<f64>().ok()?;
        if !val.is_finite() || val < 0.0 {
            return None;
        }
        total = total * 60.0 + val;
    }

    if total.is_finite() && total > 0.0 {
        Some(total)
    } else {
        None
    }
}

fn raw_to_vod_metadata(raw: YtDlpRawMetadata, webpage_url: &str) -> VodMetadata {
    let video_stream_url = extract_video_stream_url(&raw, webpage_url);
    let duration = raw
        .duration
        .filter(|d| d.is_finite() && *d > 0.0)
        .or_else(|| {
            raw.duration_string
                .as_deref()
                .and_then(parse_duration_string)
        })
        .or_else(|| {
            raw.requested_formats
                .iter()
                .chain(raw.formats.iter())
                .filter_map(|f| f.duration)
                .filter(|d| d.is_finite() && *d > 0.0)
                .max_by(f64::total_cmp)
        });
    VodMetadata {
        id: raw.id,
        title: raw.title,
        uploader: raw.uploader,
        channel: raw.channel,
        duration,
        is_live: raw.is_live,
        live_status: raw.live_status,
        video_stream_url,
    }
}

/// Schema 5.2 compliant VOD manifest document stored on disk.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct VodManifest {
    pub vod_id: String,
    pub source_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub original_url: Option<String>,
    pub title: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub streamer: Option<String>,
    pub duration_seconds: f64,
    pub audio_path: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub chat_file_path: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub video_stream_url: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub local_video_path: Option<String>,
    pub created_at: String,
}

/// Ingestion configuration options passed from IPC caller.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct IngestOptions {
    #[serde(default = "default_audio_only")]
    pub audio_only: bool,
    pub cookies_path: Option<String>,
    pub browser_cookies: Option<String>,
    pub custom_user_agent: Option<String>,
    pub custom_cache_dir: Option<String>,
    pub task_id: Option<String>,
}

const fn default_audio_only() -> bool {
    true
}

#[cfg(windows)]
pub use crate::sidecar::get_global_job_guard;
pub use crate::sidecar::{assign_child_to_job_guard, ChildProcessGuard, WindowsJobObjectGuard};

/// Build standard FFmpeg arguments for real-time 16kHz Mono PCM streaming over stdout pipe.
#[must_use]
pub fn build_ffmpeg_pipe_args(input_source: &str) -> Vec<String> {
    vec![
        "-hide_banner".to_string(),
        "-loglevel".to_string(),
        "error".to_string(),
        "-nostdin".to_string(),
        "-reconnect".to_string(),
        "1".to_string(),
        "-reconnect_streamed".to_string(),
        "1".to_string(),
        "-reconnect_delay_max".to_string(),
        "5".to_string(),
        "-i".to_string(),
        input_source.to_string(),
        "-vn".to_string(),
        "-sn".to_string(),
        "-dn".to_string(),
        "-f".to_string(),
        "s16le".to_string(),
        "-ac".to_string(),
        "1".to_string(),
        "-ar".to_string(),
        "16000".to_string(),
        "pipe:1".to_string(),
    ]
}

/// Build standard FFmpeg arguments for direct transcode to a 16kHz mono PCM WAV file.
#[must_use]
pub fn build_ffmpeg_file_args(input_source: &str, output_path: &str) -> Vec<String> {
    vec![
        "-hide_banner".to_string(),
        "-loglevel".to_string(),
        "error".to_string(),
        "-nostdin".to_string(),
        "-y".to_string(),
        "-i".to_string(),
        input_source.to_string(),
        "-vn".to_string(),
        "-sn".to_string(),
        "-dn".to_string(),
        "-acodec".to_string(),
        "pcm_s16le".to_string(),
        "-ac".to_string(),
        "1".to_string(),
        "-ar".to_string(),
        "16000".to_string(),
        output_path.to_string(),
    ]
}

/// Determine source platform type from URL.
#[must_use]
pub fn detect_source_type(url: &str) -> &'static str {
    let lower = url.to_ascii_lowercase();
    if lower.contains("twitch.tv") {
        "twitch_vod"
    } else if lower.contains("youtube.com") || lower.contains("youtu.be") {
        "youtube_vod"
    } else {
        "local_file"
    }
}

/// Parse progress percentage, download speed, and ETA from yt-dlp stdout line.
#[must_use]
pub fn parse_ytdlp_progress_line(line: &str) -> Option<(f64, f64, u64)> {
    // Expected pattern: [download]  45.2% of 400.00MiB at 5.20MiB/s ETA 00:42
    if !line.contains("[download]") {
        return None;
    }

    let percent_re = Regex::new(r"(\d+(?:\.\d+)?)%").ok()?;
    let speed_re = Regex::new(r"(\d+(?:\.\d+)?)\s*([kKM]i?B/s)").ok()?;
    let eta_re = Regex::new(r"ETA\s+(\d+):(\d+)").ok()?;

    let percent = percent_re
        .captures(line)
        .and_then(|caps| caps.get(1)?.as_str().parse::<f64>().ok())
        .unwrap_or(0.0);

    let speed_kbps = speed_re
        .captures(line)
        .and_then(|caps| {
            let num = caps.get(1)?.as_str().parse::<f64>().ok()?;
            let unit = caps.get(2)?.as_str();
            let multiplier = if unit.starts_with('M') || unit.starts_with('m') {
                1024.0
            } else if unit.starts_with('K') || unit.starts_with('k') {
                1.0
            } else {
                0.001
            };
            Some(num * multiplier)
        })
        .unwrap_or(0.0);

    let eta_sec = eta_re
        .captures(line)
        .and_then(|caps| {
            let min = caps.get(1)?.as_str().parse::<u64>().ok()?;
            let sec = caps.get(2)?.as_str().parse::<u64>().ok()?;
            Some(min * 60 + sec)
        })
        .unwrap_or(0);

    Some((percent, speed_kbps, eta_sec))
}

/// Validate that an external VOD/stream URL uses `http://` or `https://` and contains no
/// leading whitespace, leading `-` (CLI flag injection), or control/`\0` characters.
///
/// # Errors
/// Returns [`AppError::Ingestion`] with [`IngestError::InvalidUrl`] if validation fails.
pub fn validate_http_url(url: &str) -> Result<&str, AppError> {
    if url.is_empty() || url.trim().is_empty() {
        return Err(AppError::Ingestion(IngestError::InvalidUrl(
            "URL cannot be empty".to_string(),
        )));
    }
    if url.chars().any(|c| c.is_control() || c == '\0') {
        return Err(AppError::Ingestion(IngestError::InvalidUrl(
            "URL contains control or NUL characters".to_string(),
        )));
    }
    if url.chars().next().is_some_and(char::is_whitespace) || url.starts_with('-') {
        return Err(AppError::Ingestion(IngestError::InvalidUrl(
            "URL must not start with whitespace or '-'".to_string(),
        )));
    }
    let trimmed = url.trim();
    if !trimmed.starts_with("http://") && !trimmed.starts_with("https://") {
        return Err(AppError::Ingestion(IngestError::InvalidUrl(
            trimmed.to_string(),
        )));
    }
    Ok(trimmed)
}

/// Probe online VOD metadata using `yt-dlp --dump-single-json`.
///
/// Fast rejection guardrail (AC-06.5): Rejects live streams (`is_live == true`) in <= 500ms
/// with `ERR_INGEST_LIVE_STREAM_UNSUPPORTED`.
pub async fn probe_vod_metadata(
    url: &str,
    cookie_source: &CookieSource,
    custom_ua: Option<&str>,
) -> Result<VodMetadata, AppError> {
    let trimmed_url = validate_http_url(url)?;
    let defense_args = try_build_bot_defense_args(cookie_source, custom_ua)?;

    // Check mock override environment variable for automated testing
    if let Ok(mock_json) = std::env::var("NEUROCLIP_MOCK_YTDLP_METADATA") {
        let raw: YtDlpRawMetadata = serde_json::from_str(&mock_json)
            .map_err(|e| AppError::Ingestion(IngestError::ExtractionFailed(e.to_string())))?;
        let meta = raw_to_vod_metadata(raw, trimmed_url);
        if meta.is_live == Some(true) || meta.live_status.as_deref() == Some("is_live") {
            return Err(AppError::Ingestion(IngestError::LiveStreamUnsupported));
        }
        return Ok(meta);
    }

    let mut args = vec![
        "--dump-single-json".to_string(),
        "--no-warnings".to_string(),
        "--no-playlist".to_string(),
    ];
    args.extend(defense_args);
    args.push("--".to_string());
    args.push(trimmed_url.to_string());

    let ytdlp_bin = resolve_ytdlp_binary();

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

    let output = child.wait_with_output().await.map_err(|e| {
        AppError::Sidecar(SidecarError::ExecutionFailed {
            binary: "yt-dlp".to_string(),
            exit_code: None,
            details: e.to_string(),
        })
    })?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        if stderr.contains("403") || stderr.contains("429") || stderr.contains("Sign in to confirm")
        {
            return Err(AppError::Ingestion(IngestError::BotBlocked(
                stderr.to_string(),
            )));
        }
        return Err(AppError::Ingestion(IngestError::ExtractionFailed(
            stderr.to_string(),
        )));
    }

    let raw: YtDlpRawMetadata = serde_json::from_slice(&output.stdout)
        .map_err(|e| AppError::Ingestion(IngestError::ExtractionFailed(e.to_string())))?;
    let meta = raw_to_vod_metadata(raw, trimmed_url);

    if meta.is_live == Some(true) || meta.live_status.as_deref() == Some("is_live") {
        return Err(AppError::Ingestion(IngestError::LiveStreamUnsupported));
    }

    Ok(meta)
}

/// Resolve a direct playable/slicable stream URL from an online VOD webpage URL via `yt-dlp -g`.
///
/// Validates that the resolved URL starts with `http://` or `https://` and is NOT
/// identical to the webpage `original_url`.
///
/// # Errors
/// Returns `AppError::Ingestion` or `AppError::Sidecar` if URL validation or `yt-dlp` fails.
pub async fn resolve_video_stream_url(
    original_url: &str,
    cookie_source: &CookieSource,
    custom_ua: Option<&str>,
) -> Result<String, AppError> {
    let trimmed_url = validate_http_url(original_url)?;
    let defense_args = try_build_bot_defense_args(cookie_source, custom_ua)?;

    if let Ok(mock_stream_url) = std::env::var("NEUROCLIP_MOCK_YTDLP_STREAM_URL") {
        let candidate = mock_stream_url
            .lines()
            .map(str::trim)
            .find(|line| is_valid_stream_url(line, trimmed_url));
        return match candidate {
            Some(url) => Ok(url.to_string()),
            None => Err(AppError::Ingestion(IngestError::ExtractionFailed(
                "Mock yt-dlp stream URL was empty, non-HTTP, or equal to webpage URL".to_string(),
            ))),
        };
    }

    let mut args = vec![
        "-g".to_string(),
        "-f".to_string(),
        "b[ext=mp4]/b/best".to_string(),
        "--no-warnings".to_string(),
        "--no-playlist".to_string(),
    ];
    args.extend(defense_args);
    args.push("--".to_string());
    args.push(trimmed_url.to_string());

    let ytdlp_bin = resolve_ytdlp_binary();
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

    let output = child.wait_with_output().await.map_err(|e| {
        AppError::Sidecar(SidecarError::ExecutionFailed {
            binary: "yt-dlp".to_string(),
            exit_code: None,
            details: e.to_string(),
        })
    })?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        if stderr.contains("403") || stderr.contains("429") || stderr.contains("Sign in to confirm")
        {
            return Err(AppError::Ingestion(IngestError::BotBlocked(
                stderr.to_string(),
            )));
        }
        return Err(AppError::Ingestion(IngestError::ExtractionFailed(
            stderr.to_string(),
        )));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let resolved = stdout
        .lines()
        .map(str::trim)
        .find(|line| is_valid_stream_url(line, trimmed_url));

    match resolved {
        Some(url) => Ok(url.to_string()),
        None => Err(AppError::Ingestion(IngestError::ExtractionFailed(
            "yt-dlp -g did not return a valid direct stream URL".to_string(),
        ))),
    }
}

/// Download audio-only stream from Twitch or YouTube using yt-dlp.
///
/// Guaranteed to download ONLY audio (<= 450MB for 8h VOD, saving >= 98.5% disk/network).
#[allow(clippy::too_many_lines)]
pub async fn download_audio_stream<F>(
    url: &str,
    output_dir: &Path,
    vod_id: &str,
    cookie_source: &CookieSource,
    custom_ua: Option<&str>,
    progress_callback: F,
    cancellation_token: CancellationToken,
) -> Result<PathBuf, AppError>
where
    F: Fn(IngestProgressPayload) + Send + Sync + 'static,
{
    let validated_url = validate_http_url(url)?;
    let defense_args = try_build_bot_defense_args(cookie_source, custom_ua)?;

    // Pre-flight disk space check: enforce 5GB minimum threshold
    check_disk_space(Some(output_dir))?;

    let ytdlp_bin = resolve_ytdlp_binary();
    let audio_file_template = output_dir.join(format!("{vod_id}_audio.%(ext)s"));

    // Check mock override for testing
    if std::env::var("NEUROCLIP_MOCK_INGEST_RUN").is_ok() {
        let mock_file = output_dir.join(format!("{vod_id}_audio.m4a"));
        std::fs::write(&mock_file, b"MOCK_AUDIO_DATA")
            .map_err(|e| AppError::Internal(e.to_string()))?;
        progress_callback(IngestProgressPayload {
            task_id: vod_id.to_string(),
            stage: "downloading_audio".to_string(),
            percent: 100.0,
            speed_kbps: 5120.0,
            eta_sec: 0,
        });
        return Ok(mock_file);
    }

    let mut args = vec![
        "-f".to_string(),
        "ba[ext=m4a]/ba/bestaudio".to_string(),
        "-x".to_string(),
        "--audio-format".to_string(),
        "aac".to_string(),
        "--retries".to_string(),
        "5".to_string(),
        "--fragment-retries".to_string(),
        "10".to_string(),
        "--socket-timeout".to_string(),
        "30".to_string(),
        "-o".to_string(),
        audio_file_template.to_string_lossy().to_string(),
        "--no-playlist".to_string(),
    ];
    let ffmpeg_bin = resolve_ffmpeg_binary();
    if let Some(parent) = Path::new(&ffmpeg_bin).parent() {
        if !parent.as_os_str().is_empty() && parent.is_dir() {
            args.push("--ffmpeg-location".to_string());
            args.push(parent.to_string_lossy().to_string());
        }
    }
    args.extend(defense_args);
    args.push("--".to_string());
    args.push(validated_url.to_string());

    let mut cmd = Command::new(&ytdlp_bin);
    cmd.args(&args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);

    let mut child = cmd.spawn().map_err(|e| {
        AppError::Sidecar(SidecarError::BinaryNotFound {
            name: "yt-dlp".to_string(),
            path: format!("{ytdlp_bin}: {e}"),
        })
    })?;

    assign_child_to_job_guard(&child);

    let stdout = child.stdout.take().ok_or_else(|| {
        AppError::Sidecar(SidecarError::ExecutionFailed {
            binary: "yt-dlp".to_string(),
            exit_code: None,
            details: "Failed to capture yt-dlp stdout".to_string(),
        })
    })?;

    let stderr = child.stderr.take().ok_or_else(|| {
        AppError::Sidecar(SidecarError::ExecutionFailed {
            binary: "yt-dlp".to_string(),
            exit_code: None,
            details: "Failed to capture yt-dlp stderr".to_string(),
        })
    })?;

    let mut guard = ChildProcessGuard::new(child);

    // Concurrent stderr draining with bounded tail ring buffer + streaming bot-block detection
    let stderr_task = tokio::spawn(async move {
        const STDERR_TAIL_LIMIT: usize = 64;
        let mut reader = LossyLineReader::new(stderr);
        let mut tail = std::collections::VecDeque::with_capacity(STDERR_TAIL_LIMIT);
        let mut bot_blocked_hint: Option<String> = None;
        while let Ok(Some(line)) = reader.next_line().await {
            let lower = line.to_ascii_lowercase();
            if bot_blocked_hint.is_none()
                && (lower.contains("403")
                    || lower.contains("429")
                    || lower.contains("sign in to confirm"))
            {
                bot_blocked_hint = Some(line.clone());
            }
            if tail.len() >= STDERR_TAIL_LIMIT {
                tail.pop_front();
            }
            tail.push_back(line);
        }
        (
            tail.into_iter().collect::<Vec<_>>().join("\n"),
            bot_blocked_hint,
        )
    });

    let mut stdout_reader = LossyLineReader::new(stdout);
    let mut last_emit_instant = Instant::now();

    loop {
        tokio::select! {
            biased;
            () = cancellation_token.cancelled() => {
                if let Some(child) = guard.child_mut() {
                    let _ = child.start_kill();
                }
                stderr_task.abort();
                return Err(AppError::TaskCancelled);
            }
            line_res = stdout_reader.next_line() => {
                match line_res {
                    Ok(Some(line)) => {
                        if let Some((percent, speed_kbps, eta_sec)) = parse_ytdlp_progress_line(&line) {
                            if last_emit_instant.elapsed() >= Duration::from_millis(900) || percent >= 100.0 {
                                last_emit_instant = Instant::now();
                                progress_callback(IngestProgressPayload {
                                    task_id: vod_id.to_string(),
                                    stage: "downloading_audio".to_string(),
                                    percent,
                                    speed_kbps,
                                    eta_sec,
                                });
                            }
                        }
                    }
                    Ok(None) => break,
                    Err(_) => {
                        stderr_task.abort();
                        return Err(AppError::Ingestion(IngestError::NetworkStreamDropped));
                    }
                }
            }
        }
    }

    let status = if let Some(child) = guard.child_mut() {
        child.wait().await.map_err(|e| {
            stderr_task.abort();
            AppError::Sidecar(SidecarError::ExecutionFailed {
                binary: "yt-dlp".to_string(),
                exit_code: None,
                details: e.to_string(),
            })
        })?
    } else {
        stderr_task.abort();
        return Err(AppError::Sidecar(SidecarError::ExecutionFailed {
            binary: "yt-dlp".to_string(),
            exit_code: None,
            details: "Child process already terminated".to_string(),
        }));
    };

    let (stderr_output, bot_blocked_hint) = stderr_task.await.unwrap_or_default();

    if !status.success() {
        let lower_tail = stderr_output.to_ascii_lowercase();
        if bot_blocked_hint.is_some()
            || lower_tail.contains("403")
            || lower_tail.contains("429")
            || lower_tail.contains("sign in to confirm")
        {
            let details = if stderr_output.is_empty() {
                bot_blocked_hint.unwrap_or_default()
            } else {
                stderr_output
            };
            return Err(AppError::Ingestion(IngestError::BotBlocked(details)));
        }
        return Err(AppError::Ingestion(IngestError::ExtractionFailed(
            stderr_output,
        )));
    }

    // Locate the finalized audio file
    let candidate_extensions = &["aac", "m4a", "opus", "webm", "mp3"];
    for ext in candidate_extensions {
        let candidate = output_dir.join(format!("{vod_id}_audio.{ext}"));
        if candidate.exists() {
            return Ok(candidate);
        }
    }

    Err(AppError::Ingestion(IngestError::ExtractionFailed(
        "Downloaded audio file not found on disk".to_string(),
    )))
}

/// Transcode or pipe any audio source to 16kHz mono PCM using FFmpeg.
///
/// Yields `AudioChunk` items through a bounded Tokio channel (backpressure enforced).
pub async fn start_ffmpeg_pcm_stream(
    input_source: &str,
    channel_capacity: usize,
    cancellation_token: CancellationToken,
) -> Result<mpsc::Receiver<AudioChunk>, AppError> {
    let ffmpeg_bin = resolve_ffmpeg_binary();
    let pipe_args = build_ffmpeg_pipe_args(input_source);

    let (tx, rx) = create_bounded_audio_channel(channel_capacity);

    // Check mock override for testing
    if std::env::var("NEUROCLIP_MOCK_FFMPEG_STREAM").is_ok() {
        let tx_mock = tx;
        tokio::spawn(async move {
            let mut validator = AudioStreamValidator::new();
            // Emit 4 mock 16kHz samples (8 bytes)
            let bytes = [0x00, 0x10, 0x00, 0x20, 0x00, 0x30, 0x00, 0x40];
            if let Ok(chunk) = validator.process_chunk(&bytes, true) {
                let _ = tx_mock.send(chunk).await;
            }
        });
        return Ok(rx);
    }

    let mut cmd = Command::new(&ffmpeg_bin);
    cmd.args(&pipe_args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);

    let mut child = cmd.spawn().map_err(|e| {
        AppError::Sidecar(SidecarError::BinaryNotFound {
            name: "ffmpeg".to_string(),
            path: format!("{ffmpeg_bin}: {e}"),
        })
    })?;

    assign_child_to_job_guard(&child);

    let stdout = child.stdout.take().ok_or_else(|| {
        AppError::Sidecar(SidecarError::ExecutionFailed {
            binary: "ffmpeg".to_string(),
            exit_code: None,
            details: "Failed to open FFmpeg stdout pipe".to_string(),
        })
    })?;

    let stderr = child.stderr.take().ok_or_else(|| {
        AppError::Sidecar(SidecarError::ExecutionFailed {
            binary: "ffmpeg".to_string(),
            exit_code: None,
            details: "Failed to open FFmpeg stderr pipe".to_string(),
        })
    })?;

    // Drain stderr concurrently to prevent Windows anonymous pipe deadlocks
    tokio::spawn(async move {
        drain_pipe_lossy(stderr).await;
    });

    let guard = ChildProcessGuard::new(child);

    // Stream decoding task with backpressure bounded channel
    tokio::spawn(async move {
        let mut guard = guard;
        let mut stdout_reader = stdout;
        let mut buffer = vec![0u8; DEFAULT_CHUNK_SIZE_BYTES];
        let mut validator = AudioStreamValidator::new();

        loop {
            tokio::select! {
                biased;
                () = cancellation_token.cancelled() => {
                    if let Some(child) = guard.child_mut() {
                        let _ = child.start_kill();
                    }
                    break;
                }
                read_res = stdout_reader.read(&mut buffer) => {
                    match read_res {
                        Ok(0) => {
                            // EOF
                            if let Ok(last_chunk) = validator.process_chunk(&[], true) {
                                let _ = tx.send(last_chunk).await;
                            }
                            break;
                        }
                        Ok(n) => {
                            match validator.process_chunk(&buffer[..n], false) {
                                Ok(chunk) => {
                                    if tx.send(chunk).await.is_err() {
                                        // Receiver dropped (backpressure / cancelled)
                                        break;
                                    }
                                }
                                Err(_) => {
                                    break;
                                }
                            }
                        }
                        Err(_) => {
                            break;
                        }
                    }
                }
            }
        }
    });

    Ok(rx)
}

/// Telemetry window produced by `compute_audio_energy_stream` via `AudioRingBuffer`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AudioEnergyWindow {
    pub window_index: u64,
    pub timestamp_sec: f64,
    pub rms_energy: f64,
    pub peak_amplitude: i16,
}

/// Stream audio from input source through FFmpeg 16kHz mono pipe and process
/// samples via `AudioRingBuffer` sliding window to compute continuous RMS energy telemetry.
///
/// Complies with docs/ARCHITECTURE.md §4.3 (Audio RMS blocks) and CONVENTIONS.md zero-panic policy.
#[allow(clippy::cast_precision_loss)]
pub async fn compute_audio_energy_stream(
    input_source: &str,
    window_samples: usize,
    max_windows: Option<usize>,
    cancellation_token: CancellationToken,
) -> Result<Vec<AudioEnergyWindow>, AppError> {
    let mut rx = start_ffmpeg_pcm_stream(
        input_source,
        DEFAULT_QUEUE_CAPACITY,
        cancellation_token.clone(),
    )
    .await?;

    let win_size = if window_samples == 0 {
        8000
    } else {
        window_samples
    }; // default 500ms at 16kHz
    let mut ring_buf = AudioRingBuffer::with_capacity(win_size * 2);
    let mut windows = Vec::new();
    let mut window_index = 0u64;
    let mut scratch = vec![0i16; win_size];

    while let Some(chunk) = rx.recv().await {
        if cancellation_token.is_cancelled() {
            return Err(AppError::TaskCancelled);
        }

        let mut offset = 0;
        while offset < chunk.samples.len() {
            let written = ring_buf.push_samples(&chunk.samples[offset..]);
            offset += written;

            while ring_buf.available_read() >= win_size {
                let popped = ring_buf.pop_samples(&mut scratch);
                if popped == win_size {
                    let sum_sq: f64 = scratch
                        .iter()
                        .map(|&s| {
                            let v = f64::from(s);
                            v * v
                        })
                        .sum();
                    let rms = (sum_sq / win_size as f64).sqrt();
                    let peak = scratch
                        .iter()
                        .copied()
                        .map(i16::saturating_abs)
                        .max()
                        .unwrap_or_default();
                    let ts = window_index as f64 * (win_size as f64 / 16000.0);

                    windows.push(AudioEnergyWindow {
                        window_index,
                        timestamp_sec: ts,
                        rms_energy: rms,
                        peak_amplitude: peak,
                    });
                    window_index += 1;

                    if let Some(max) = max_windows {
                        if window_index >= max as u64 {
                            return Ok(windows);
                        }
                    }
                }
            }
        }
    }

    // Drain remaining trailing samples in ring buffer on EOF
    if ring_buf.available_read() > 0 && !cancellation_token.is_cancelled() {
        let rem_len = ring_buf.available_read();
        let mut final_scratch = vec![0i16; rem_len];
        let popped = ring_buf.pop_samples(&mut final_scratch);
        if popped > 0 {
            let sum_sq: f64 = final_scratch
                .iter()
                .map(|&s| {
                    let v = f64::from(s);
                    v * v
                })
                .sum();
            let rms = (sum_sq / popped as f64).sqrt();
            let peak = final_scratch
                .iter()
                .copied()
                .map(i16::saturating_abs)
                .max()
                .unwrap_or_default();
            let ts = window_index as f64 * (win_size as f64 / 16000.0);

            windows.push(AudioEnergyWindow {
                window_index,
                timestamp_sec: ts,
                rms_energy: rms,
                peak_amplitude: peak,
            });
        }
    }

    Ok(windows)
}

/// Create and persist `vod_manifest.json` compliant with Schema 5.2 into the project directory,
/// optionally setting `video_stream_url` and `local_video_path`.
///
/// # Errors
/// Returns `AppError::Internal` if JSON serialization or writing `vod_manifest.json` fails.
#[allow(clippy::too_many_arguments)]
pub fn generate_vod_manifest_with_streams(
    vod_id: &str,
    source_type: &str,
    original_url: Option<&str>,
    title: &str,
    streamer: Option<&str>,
    duration_seconds: f64,
    audio_path: &Path,
    project_dir: &Path,
    video_stream_url: Option<&str>,
    local_video_path: Option<&str>,
) -> Result<VodManifest, AppError> {
    let manifest = VodManifest {
        vod_id: vod_id.to_string(),
        source_type: source_type.to_string(),
        original_url: original_url.map(ToString::to_string),
        title: title.to_string(),
        streamer: streamer.map(ToString::to_string),
        duration_seconds,
        audio_path: audio_path.to_string_lossy().to_string(),
        chat_file_path: None,
        video_stream_url: video_stream_url.map(ToString::to_string),
        local_video_path: local_video_path.map(ToString::to_string),
        created_at: Utc::now().to_rfc3339(),
    };

    let manifest_path = project_dir.join("vod_manifest.json");
    let json_bytes = serde_json::to_vec_pretty(&manifest)
        .map_err(|e| AppError::Internal(format!("Failed to serialize vod_manifest: {e}")))?;

    std::fs::write(&manifest_path, json_bytes)
        .map_err(|e| AppError::Internal(format!("Failed to write vod_manifest.json: {e}")))?;

    Ok(manifest)
}

/// Create and persist `vod_manifest.json` compliant with Schema 5.2 into the project directory.
#[allow(clippy::too_many_arguments)]
pub fn generate_vod_manifest(
    vod_id: &str,
    source_type: &str,
    original_url: Option<&str>,
    title: &str,
    streamer: Option<&str>,
    duration_seconds: f64,
    audio_path: &Path,
    project_dir: &Path,
) -> Result<VodManifest, AppError> {
    generate_vod_manifest_with_streams(
        vod_id,
        source_type,
        original_url,
        title,
        streamer,
        duration_seconds,
        audio_path,
        project_dir,
        None,
        None,
    )
}

fn resolve_ytdlp_binary() -> String {
    resolve_sidecar_binary("yt-dlp", "NEUROCLIP_YTDLP_PATH")
}

fn resolve_ffmpeg_binary() -> String {
    resolve_sidecar_binary("ffmpeg", "NEUROCLIP_FFMPEG_PATH")
}

fn resolve_ffprobe_binary() -> String {
    resolve_sidecar_binary("ffprobe", "NEUROCLIP_FFPROBE_PATH")
}

/// Probe the duration (in seconds) of a RIFF/WAVE file directly from its header chunks
/// without loading the audio payload into RAM or spawning an external process.
#[allow(clippy::cast_precision_loss)]
#[must_use]
pub fn probe_wav_duration_from_header(path: &Path) -> Option<f64> {
    use std::io::{Read, Seek, SeekFrom};

    let mut file = std::fs::File::open(path).ok()?;
    let file_len = file.metadata().ok()?.len();
    if file_len < 44 {
        return None;
    }

    let mut riff_hdr = [0u8; 12];
    file.read_exact(&mut riff_hdr).ok()?;
    if &riff_hdr[0..4] != b"RIFF" || &riff_hdr[8..12] != b"WAVE" {
        return None;
    }

    let mut byte_rate: u32 = 0;
    let mut data_bytes: Option<u64> = None;
    let mut offset: u64 = 12;

    while offset.saturating_add(8) <= file_len {
        let mut chunk_hdr = [0u8; 8];
        if file.read_exact(&mut chunk_hdr).is_err() {
            break;
        }
        let chunk_id = &chunk_hdr[0..4];
        let chunk_size =
            u32::from_le_bytes([chunk_hdr[4], chunk_hdr[5], chunk_hdr[6], chunk_hdr[7]]);
        let payload_offset = offset.saturating_add(8);

        if chunk_id == b"fmt " && chunk_size >= 16 {
            let mut fmt_buf = [0u8; 16];
            file.read_exact(&mut fmt_buf).ok()?;
            byte_rate = u32::from_le_bytes([fmt_buf[8], fmt_buf[9], fmt_buf[10], fmt_buf[11]]);
            let remaining = i64::from(chunk_size) - 16;
            if remaining > 0 {
                let _ = file.seek(SeekFrom::Current(remaining));
            }
        } else if chunk_id == b"data" {
            let effective_bytes = if chunk_size == 0 || chunk_size == u32::MAX {
                file_len.saturating_sub(payload_offset)
            } else {
                u64::from(chunk_size).min(file_len.saturating_sub(payload_offset))
            };
            data_bytes = Some(effective_bytes);
            break;
        } else {
            let step = i64::from(chunk_size);
            if file.seek(SeekFrom::Current(step)).is_err() {
                break;
            }
        }

        let padded_size = u64::from(chunk_size) + u64::from(chunk_size % 2);
        offset = payload_offset.saturating_add(padded_size);
        if chunk_id == b"fmt " && chunk_size % 2 == 1 {
            let _ = file.seek(SeekFrom::Current(1));
        }
    }

    let bytes = data_bytes?;
    if byte_rate == 0 || bytes == 0 {
        return None;
    }

    let duration = bytes as f64 / f64::from(byte_rate);
    if duration.is_finite() && duration > 0.0 {
        Some(duration)
    } else {
        None
    }
}

async fn probe_ffprobe_duration_fallback(
    path: &Path,
    cancellation_token: &CancellationToken,
) -> Option<f64> {
    let ffprobe_bin = resolve_ffprobe_binary();
    let mut probe_cmd = Command::new(&ffprobe_bin);
    probe_cmd
        .args([
            "-v",
            "error",
            "-show_entries",
            "format=duration:stream=duration",
            "-of",
            "json",
        ])
        .arg(path.as_os_str())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .kill_on_drop(true);

    let child = probe_cmd.spawn().ok()?;
    assign_child_to_job_guard(&child);
    let mut guard = ChildProcessGuard::new(child);
    let child_proc = guard.take_child()?;

    let output = tokio::select! {
        biased;
        () = cancellation_token.cancelled() => return None,
        res = child_proc.wait_with_output() => res.ok()?,
    };
    if !output.status.success() {
        return None;
    }

    let stdout_str = String::from_utf8_lossy(&output.stdout);
    let parsed = crate::ingestion::range_slicer::parse_ffprobe_json(&stdout_str).ok()?;
    if let Some(d) = parsed
        .format
        .as_ref()
        .and_then(|f| f.duration.as_deref())
        .and_then(|s| s.trim().parse::<f64>().ok())
        .filter(|d| d.is_finite() && *d > 0.0)
    {
        return Some(d);
    }

    parsed
        .streams
        .iter()
        .filter_map(|s| s.duration.as_deref())
        .filter_map(|s| s.trim().parse::<f64>().ok())
        .filter(|d| d.is_finite() && *d > 0.0)
        .max_by(f64::total_cmp)
}

#[allow(clippy::cast_precision_loss)]
async fn probe_ffmpeg_pcm_duration_fallback(
    path: &Path,
    cancellation_token: &CancellationToken,
) -> f64 {
    let ffmpeg_bin = resolve_ffmpeg_binary();
    let mut dec_cmd = Command::new(&ffmpeg_bin);
    dec_cmd
        .args(["-hide_banner", "-loglevel", "error", "-nostdin", "-i"])
        .arg(path.as_os_str())
        .args([
            "-vn", "-sn", "-dn", "-f", "s16le", "-ac", "1", "-ar", "16000", "pipe:1",
        ])
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .kill_on_drop(true);

    let Ok(mut child) = dec_cmd.spawn() else {
        return 0.0;
    };
    assign_child_to_job_guard(&child);
    let Some(mut stdout) = child.stdout.take() else {
        return 0.0;
    };
    let mut guard = ChildProcessGuard::new(child);

    let mut total_bytes: u64 = 0;
    let mut buf = vec![0u8; DEFAULT_CHUNK_SIZE_BYTES];
    loop {
        tokio::select! {
            biased;
            () = cancellation_token.cancelled() => {
                if let Some(c) = guard.child_mut() {
                    let _ = c.start_kill();
                }
                return 0.0;
            }
            read_res = stdout.read(&mut buf) => {
                match read_res {
                    Ok(0) | Err(_) => break,
                    Ok(n) => {
                        total_bytes = total_bytes.saturating_add(n as u64);
                    }
                }
            }
        }
    }

    if let Some(c) = guard.child_mut() {
        let _ = c.wait().await;
    }

    if total_bytes == 0 {
        return 0.0;
    }

    let duration = total_bytes as f64 / 32_000.0;
    if duration.is_finite() && duration > 0.0 {
        duration
    } else {
        0.0
    }
}

/// Probe media duration as a fallback when `yt-dlp` / initial metadata reports `0.0` or `None`.
///
/// Order:
/// 1. Direct RIFF/WAVE header inspection (instant, zero subprocess).
/// 2. `ffprobe -show_entries format=duration:stream=duration -of json`.
/// 3. Streaming `ffmpeg` decode to `s16le` `16kHz` mono `pipe:1` counting total PCM bytes in 64KB chunks.
pub async fn probe_media_duration_fallback(
    path: &Path,
    cancellation_token: &CancellationToken,
) -> f64 {
    if cancellation_token.is_cancelled() || !path.is_file() {
        return 0.0;
    }

    let path_for_wav = path.to_path_buf();
    if let Ok(Some(wav_dur)) =
        tokio::task::spawn_blocking(move || probe_wav_duration_from_header(&path_for_wav)).await
    {
        if wav_dur.is_finite() && wav_dur > 0.0 {
            return wav_dur;
        }
    }

    if let Some(ffprobe_dur) = probe_ffprobe_duration_fallback(path, cancellation_token).await {
        return ffprobe_dur;
    }

    if cancellation_token.is_cancelled() {
        return 0.0;
    }

    probe_ffmpeg_pcm_duration_fallback(path, cancellation_token).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ffmpeg_pipe_args() {
        let args = build_ffmpeg_pipe_args("test.mp4");
        assert!(args.contains(&"-vn".to_string()));
        assert!(args.contains(&"-sn".to_string()));
        assert!(args.contains(&"-dn".to_string()));
        assert!(args.contains(&"-f".to_string()));
        assert!(args.contains(&"s16le".to_string()));
        assert!(args.contains(&"-ac".to_string()));
        assert!(args.contains(&"1".to_string()));
        assert!(args.contains(&"-ar".to_string()));
        assert!(args.contains(&"16000".to_string()));
        assert!(args.contains(&"pipe:1".to_string()));
    }

    #[test]
    fn test_ffmpeg_file_args() {
        let args = build_ffmpeg_file_args("test.mp4", "out.wav");
        assert!(args.contains(&"-acodec".to_string()));
        assert!(args.contains(&"pcm_s16le".to_string()));
        assert!(args.contains(&"out.wav".to_string()));
    }

    #[test]
    fn test_detect_source_type() {
        assert_eq!(
            detect_source_type("https://www.twitch.tv/videos/12345"),
            "twitch_vod"
        );
        assert_eq!(
            detect_source_type("https://www.youtube.com/watch?v=abc"),
            "youtube_vod"
        );
        assert_eq!(detect_source_type("https://youtu.be/abc"), "youtube_vod");
        assert_eq!(
            detect_source_type("C:\\Videos\\recording.mp4"),
            "local_file"
        );
    }

    #[test]
    fn test_parse_ytdlp_progress_line() {
        let line = "[download]  45.2% of 400.00MiB at 5.20MiB/s ETA 00:42";
        let parsed = parse_ytdlp_progress_line(line);
        assert!(parsed.is_some());
        let (percent, speed, eta) = parsed.unwrap();
        assert!((percent - 45.2).abs() < 1e-4);
        assert!((speed - 5.20 * 1024.0).abs() < 1e-4);
        assert_eq!(eta, 42);
    }

    #[test]
    fn test_generate_vod_manifest_schema_5_2() {
        let temp_dir = std::env::temp_dir().join(format!(
            "neuroclip_test_{}",
            Utc::now().timestamp_nanos_opt().unwrap_or(0)
        ));
        let _ = std::fs::create_dir_all(&temp_dir);

        let audio_path = temp_dir.join("audio.m4a");
        let manifest = generate_vod_manifest(
            "vod_test_1",
            "youtube_vod",
            Some("https://www.youtube.com/watch?v=123"),
            "Neuro-sama Sings",
            Some("Vedal987"),
            3600.0,
            &audio_path,
            &temp_dir,
        )
        .unwrap();

        assert_eq!(manifest.vod_id, "vod_test_1");
        assert_eq!(manifest.source_type, "youtube_vod");
        assert_eq!(manifest.title, "Neuro-sama Sings");
        assert_eq!(manifest.duration_seconds, 3600.0);
        assert_eq!(manifest.streamer, Some("Vedal987".to_string()));
        assert_ne!(manifest.created_at, "");

        let written_path = temp_dir.join("vod_manifest.json");
        assert!(written_path.exists());
        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_windows_job_object_guard_creation() {
        let guard = WindowsJobObjectGuard::new();
        assert!(guard.is_ok());
    }

    #[tokio::test]
    async fn test_compute_audio_energy_stream_with_ring_buffer() {
        let _env_guard = crate::core::storage::test_env::ENV_LOCK.lock().await;
        let _mock_env =
            crate::core::storage::test_env::ScopedEnvVar::set("NEUROCLIP_MOCK_FFMPEG_STREAM", "1");
        let cancel = CancellationToken::new();
        let res = compute_audio_energy_stream("mock_input", 2, None, cancel).await;

        assert!(res.is_ok());
        if let Ok(windows) = res {
            assert_eq!(windows.len(), 2);
            assert_eq!(windows[0].window_index, 0);
            assert!(windows[0].rms_energy > 0.0);
            assert_eq!(windows[1].window_index, 1);
        }
    }

    #[test]
    fn test_validate_http_url_rejects_flag_injection_and_control_chars() {
        assert!(validate_http_url("https://www.twitch.tv/videos/12345").is_ok());
        assert!(validate_http_url("http://example.com/stream.mp4").is_ok());

        for bad in [
            "",
            "   ",
            " https://www.youtube.com/watch?v=1",
            "--exec=calc",
            "-o",
            "ftp://example.com/video.mp4",
            "https://www.youtube.com/watch?v=1\0",
            "https://www.youtube.com/watch?v=1\n",
        ] {
            let res = validate_http_url(bad);
            assert!(
                matches!(res, Err(AppError::Ingestion(IngestError::InvalidUrl(_)))),
                "Expected InvalidUrl for {bad:?}, got {res:?}"
            );
        }
    }

    #[tokio::test]
    async fn test_ytdlp_entrypoints_reject_flag_urls_and_bad_cookies() {
        let temp_dir = std::env::temp_dir();
        let err_dl = download_audio_stream(
            "--exec=calc",
            &temp_dir,
            "vod_bad_url",
            &CookieSource::None,
            None,
            |_| {},
            CancellationToken::new(),
        )
        .await;
        assert!(matches!(
            err_dl,
            Err(AppError::Ingestion(IngestError::InvalidUrl(_)))
        ));

        let err_cookie = probe_vod_metadata(
            "https://www.youtube.com/watch?v=123",
            &CookieSource::File("-evil.txt".to_string()),
            None,
        )
        .await;
        assert!(matches!(
            err_cookie,
            Err(AppError::Ingestion(IngestError::InvalidUrl(_)))
        ));

        let err_ua = resolve_video_stream_url(
            "https://www.youtube.com/watch?v=123",
            &CookieSource::None,
            Some("Mozilla/5.0\r\nInjected"),
        )
        .await;
        assert!(matches!(
            err_ua,
            Err(AppError::Ingestion(IngestError::InvalidUrl(_)))
        ));
    }

    #[test]
    fn test_parse_duration_string_formats() {
        assert_eq!(parse_duration_string("45"), Some(45.0));
        assert_eq!(parse_duration_string("05:12"), Some(312.0));
        assert_eq!(parse_duration_string("1:02:03.5"), Some(3723.5));
        assert_eq!(parse_duration_string("00:00:00"), None);
        assert_eq!(parse_duration_string(""), None);
        assert_eq!(parse_duration_string("invalid"), None);
    }

    #[test]
    fn test_raw_to_vod_metadata_duration_fallbacks() {
        let json_duration_string = r#"{
            "id": "v1",
            "title": "Test VOD",
            "duration": null,
            "duration_string": "1:30:00",
            "formats": []
        }"#;
        let raw_str: YtDlpRawMetadata = serde_json::from_str(json_duration_string).unwrap();
        let meta_str = raw_to_vod_metadata(raw_str, "https://www.twitch.tv/videos/1");
        assert_eq!(meta_str.duration, Some(5400.0));

        let json_format_duration = r#"{
            "id": "v2",
            "title": "Test VOD 2",
            "duration": 0.0,
            "formats": [
                { "vcodec": "none", "acodec": "aac", "duration": 185.5 }
            ]
        }"#;
        let raw_fmt: YtDlpRawMetadata = serde_json::from_str(json_format_duration).unwrap();
        let meta_fmt = raw_to_vod_metadata(raw_fmt, "https://www.youtube.com/watch?v=2");
        assert_eq!(meta_fmt.duration, Some(185.5));
    }

    #[test]
    fn test_probe_wav_duration_from_header() {
        let temp_dir = std::env::temp_dir().join(format!(
            "neuroclip_wav_probe_{}_{}",
            std::process::id(),
            Utc::now().timestamp_nanos_opt().unwrap_or(0)
        ));
        let _ = std::fs::create_dir_all(&temp_dir);

        // 2.0 seconds of 16kHz 16-bit mono PCM = 64,000 bytes (byte_rate = 32,000)
        let wav_path = temp_dir.join("valid.wav");
        let data_len: u32 = 64_000;
        let mut bytes = Vec::with_capacity(44 + data_len as usize);
        bytes.extend_from_slice(b"RIFF");
        bytes.extend_from_slice(&(36 + data_len).to_le_bytes());
        bytes.extend_from_slice(b"WAVEfmt ");
        bytes.extend_from_slice(&16u32.to_le_bytes());
        bytes.extend_from_slice(&1u16.to_le_bytes());
        bytes.extend_from_slice(&1u16.to_le_bytes());
        bytes.extend_from_slice(&16_000u32.to_le_bytes());
        bytes.extend_from_slice(&32_000u32.to_le_bytes());
        bytes.extend_from_slice(&2u16.to_le_bytes());
        bytes.extend_from_slice(&16u16.to_le_bytes());
        bytes.extend_from_slice(b"data");
        bytes.extend_from_slice(&data_len.to_le_bytes());
        bytes.resize(44 + data_len as usize, 0);
        std::fs::write(&wav_path, &bytes).unwrap();

        let dur = probe_wav_duration_from_header(&wav_path).unwrap();
        assert!((dur - 2.0).abs() < 1e-6);

        // Pipe-written WAV with 0 data chunk size falls back to file length
        let zero_hdr_path = temp_dir.join("zero_data_size.wav");
        bytes[40..44].copy_from_slice(&0u32.to_le_bytes());
        std::fs::write(&zero_hdr_path, &bytes).unwrap();
        let dur_zero = probe_wav_duration_from_header(&zero_hdr_path).unwrap();
        assert!((dur_zero - 2.0).abs() < 1e-6);

        let _ = std::fs::remove_dir_all(&temp_dir);
    }
}
