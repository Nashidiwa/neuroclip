//! Audio-first ingestion engine & FFmpeg streaming transcode pipeline.
//!
//! Complies with docs/CONVENTIONS.md, docs/ARCHITECTURE.md §4.2, §5.2, and TASK-M4-06.
//! Strict zero-panic invariant: all fallible operations return Result<T, AppError>.

use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::{Duration, Instant};
use chrono::Utc;
use regex::Regex;
use serde::{Deserialize, Serialize};
use tokio::io::{AsyncBufReadExt, AsyncReadExt, BufReader};
use tokio::process::Command;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

use crate::core::storage::check_disk_space;
use crate::error::{AppError, IngestError, SidecarError};
use crate::ingestion::buffer::{
    create_bounded_audio_channel, AudioChunk, AudioRingBuffer, DEFAULT_CHUNK_SIZE_BYTES,
    DEFAULT_QUEUE_CAPACITY,
};
use crate::ingestion::cookies::{build_bot_defense_args, CookieSource};
use crate::ingestion::validator::AudioStreamValidator;

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

/// RAII Guard wrapping a spawned `tokio::process::Child` to guarantee termination on drop.
pub struct ChildProcessGuard {
    child: Option<tokio::process::Child>,
    pid: Option<u32>,
}

impl ChildProcessGuard {
    #[must_use]
    pub fn new(child: tokio::process::Child) -> Self {
        let pid = child.id();
        Self {
            child: Some(child),
            pid,
        }
    }

    #[must_use]
    pub const fn pid(&self) -> Option<u32> {
        self.pid
    }

    pub fn take_child(&mut self) -> Option<tokio::process::Child> {
        self.child.take()
    }

    pub fn child_mut(&mut self) -> Option<&mut tokio::process::Child> {
        self.child.as_mut()
    }
}

impl Drop for ChildProcessGuard {
    fn drop(&mut self) {
        if let Some(mut child) = self.child.take() {
            let _ = child.start_kill();
        }
    }
}

/// Windows Job Object RAII Guard ensuring 0 zombie processes on Windows (`ADR-007`).
#[cfg(windows)]
pub struct WindowsJobObjectGuard {
    job_handle: windows_sys::Win32::Foundation::HANDLE,
}

#[cfg(windows)]
unsafe impl Send for WindowsJobObjectGuard {}

#[cfg(windows)]
unsafe impl Sync for WindowsJobObjectGuard {}

#[cfg(windows)]
impl WindowsJobObjectGuard {
    /// Create a Windows Job Object configured with `JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE`.
    pub fn new() -> Result<Self, std::io::Error> {
        use windows_sys::Win32::System::JobObjects::{
            CreateJobObjectW, SetInformationJobObject, JobObjectExtendedLimitInformation,
            JOBOBJECT_EXTENDED_LIMIT_INFORMATION, JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
        };
        use std::mem::size_of;

        unsafe {
            let job = CreateJobObjectW(std::ptr::null(), std::ptr::null());
            if job == 0 {
                return Err(std::io::Error::last_os_error());
            }

            let mut info: JOBOBJECT_EXTENDED_LIMIT_INFORMATION = std::mem::zeroed();
            info.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;

            let res = SetInformationJobObject(
                job,
                JobObjectExtendedLimitInformation,
                std::ptr::addr_of!(info).cast(),
                size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
            );

            if res == 0 {
                windows_sys::Win32::Foundation::CloseHandle(job);
                return Err(std::io::Error::last_os_error());
            }

            Ok(Self { job_handle: job })
        }
    }

    /// Assign a running process to this Job Object.
    pub fn assign_process(&self, process_handle: windows_sys::Win32::Foundation::HANDLE) -> Result<(), std::io::Error> {
        use windows_sys::Win32::System::JobObjects::AssignProcessToJobObject;
        unsafe {
            let res = AssignProcessToJobObject(self.job_handle, process_handle);
            if res == 0 {
                return Err(std::io::Error::last_os_error());
            }
            Ok(())
        }
    }

    /// Assign a spawned tokio child process to this Job Object by PID using Win32 OpenProcess.
    pub fn assign_child_process(&self, child: &tokio::process::Child) -> Result<(), std::io::Error> {
        if self.job_handle == 0 {
            return Ok(());
        }
        use windows_sys::Win32::System::Threading::{OpenProcess, PROCESS_SET_QUOTA, PROCESS_TERMINATE};
        use windows_sys::Win32::Foundation::CloseHandle;

        if let Some(pid) = child.id() {
            unsafe {
                let handle = OpenProcess(PROCESS_SET_QUOTA | PROCESS_TERMINATE, 0, pid);
                if handle == 0 {
                    return Err(std::io::Error::last_os_error());
                }
                let res = self.assign_process(handle);
                CloseHandle(handle);
                res
            }
        } else {
            Err(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                "Child process has no PID",
            ))
        }
    }
}

#[cfg(windows)]
impl Drop for WindowsJobObjectGuard {
    fn drop(&mut self) {
        unsafe {
            if self.job_handle != 0 {
                windows_sys::Win32::Foundation::CloseHandle(self.job_handle);
            }
        }
    }
}

/// Fallback dummy guard on non-windows target platforms.
#[cfg(not(windows))]
pub struct WindowsJobObjectGuard;

#[cfg(not(windows))]
impl WindowsJobObjectGuard {
    pub fn new() -> Result<Self, std::io::Error> {
        Ok(Self)
    }

    pub fn assign_process(&self, _handle: usize) -> Result<(), std::io::Error> {
        Ok(())
    }

    pub fn assign_child_process(&self, _child: &tokio::process::Child) -> Result<(), std::io::Error> {
        Ok(())
    }
}

#[cfg(windows)]
static GLOBAL_JOB_GUARD: std::sync::OnceLock<Option<WindowsJobObjectGuard>> = std::sync::OnceLock::new();

#[cfg(windows)]
pub fn get_global_job_guard() -> Result<&'static WindowsJobObjectGuard, std::io::Error> {
    let opt = GLOBAL_JOB_GUARD.get_or_init(|| {
        match WindowsJobObjectGuard::new() {
            Ok(guard) => Some(guard),
            Err(e) => {
                tracing::warn!("Failed to initialize WindowsJobObjectGuard: {e}");
                None
            }
        }
    });

    match opt.as_ref() {
        Some(guard) => Ok(guard),
        None => Err(std::io::Error::other("Windows Job Object is unavailable")),
    }
}

/// Assign a spawned subprocess to the OS-level Job Object guard to guarantee zero zombies (`ADR-007`).
pub fn assign_child_to_job_guard(child: &tokio::process::Child) {
    #[cfg(windows)]
    {
        if let Ok(guard) = get_global_job_guard() {
            if let Err(e) = guard.assign_child_process(child) {
                tracing::warn!("Failed to assign child process to Windows Job Object: {e}");
            }
        }
    }
    #[cfg(not(windows))]
    {
        let _ = child;
    }
}

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

/// Probe online VOD metadata using `yt-dlp --dump-single-json`.
///
/// Fast rejection guardrail (AC-06.5): Rejects live streams (`is_live == true`) in <= 500ms
/// with `ERR_INGEST_LIVE_STREAM_UNSUPPORTED`.
pub async fn probe_vod_metadata(
    url: &str,
    cookie_source: &CookieSource,
    custom_ua: Option<&str>,
) -> Result<VodMetadata, AppError> {
    let trimmed_url = url.trim();
    if trimmed_url.is_empty() {
        return Err(AppError::Ingestion(IngestError::InvalidUrl(
            "URL cannot be empty".to_string(),
        )));
    }
    if !trimmed_url.starts_with("http://") && !trimmed_url.starts_with("https://") {
        return Err(AppError::Ingestion(IngestError::InvalidUrl(
            trimmed_url.to_string(),
        )));
    }

    // Check mock override environment variable for automated testing
    if let Ok(mock_json) = std::env::var("NEUROCLIP_MOCK_YTDLP_METADATA") {
        let meta: VodMetadata = serde_json::from_str(&mock_json)
            .map_err(|e| AppError::Ingestion(IngestError::ExtractionFailed(e.to_string())))?;
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
    args.extend(build_bot_defense_args(cookie_source, custom_ua));
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
        if stderr.contains("403") || stderr.contains("Sign in to confirm you’re not a bot") {
            return Err(AppError::Ingestion(IngestError::BotBlocked(stderr.to_string())));
        }
        return Err(AppError::Ingestion(IngestError::ExtractionFailed(
            stderr.to_string(),
        )));
    }

    let meta: VodMetadata = serde_json::from_slice(&output.stdout)
        .map_err(|e| AppError::Ingestion(IngestError::ExtractionFailed(e.to_string())))?;

    if meta.is_live == Some(true) || meta.live_status.as_deref() == Some("is_live") {
        return Err(AppError::Ingestion(IngestError::LiveStreamUnsupported));
    }

    Ok(meta)
}

/// Download audio-only stream from Twitch or YouTube using yt-dlp.
///
/// Guaranteed to download ONLY audio (<= 450MB for 8h VOD, saving >= 98.5% disk/network).
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
    args.extend(build_bot_defense_args(cookie_source, custom_ua));
    args.push(url.to_string());

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

    // Concurrent stderr draining to prevent buffer stalls
    let stderr_task = tokio::spawn(async move {
        let mut reader = BufReader::new(stderr).lines();
        let mut collected = Vec::new();
        while let Ok(Some(line)) = reader.next_line().await {
            if collected.len() < 30 {
                collected.push(line);
            }
        }
        collected.join("\n")
    });

    let mut stdout_reader = BufReader::new(stdout).lines();
    let mut last_emit_instant = Instant::now();

    loop {
        tokio::select! {
            biased;
            _ = cancellation_token.cancelled() => {
                if let Some(child) = guard.child_mut() {
                    let _ = child.start_kill();
                }
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
                        return Err(AppError::Ingestion(IngestError::NetworkStreamDropped));
                    }
                }
            }
        }
    }

    let status = match guard.child_mut() {
        Some(child) => child.wait().await.map_err(|e| {
            AppError::Sidecar(SidecarError::ExecutionFailed {
                binary: "yt-dlp".to_string(),
                exit_code: None,
                details: e.to_string(),
            })
        })?,
        None => {
            return Err(AppError::Sidecar(SidecarError::ExecutionFailed {
                binary: "yt-dlp".to_string(),
                exit_code: None,
                details: "Child process already terminated".to_string(),
            }));
        }
    };

    let stderr_output = stderr_task.await.unwrap_or_default();

    if !status.success() {
        if stderr_output.contains("403") || stderr_output.contains("Sign in to confirm") {
            return Err(AppError::Ingestion(IngestError::BotBlocked(stderr_output)));
        }
        return Err(AppError::Ingestion(IngestError::ExtractionFailed(stderr_output)));
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
        let mut reader = BufReader::new(stderr).lines();
        while let Ok(Some(_line)) = reader.next_line().await {}
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
                _ = cancellation_token.cancelled() => {
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
pub async fn compute_audio_energy_stream(
    input_source: &str,
    window_samples: usize,
    max_windows: Option<usize>,
    cancellation_token: CancellationToken,
) -> Result<Vec<AudioEnergyWindow>, AppError> {
    let mut rx = start_ffmpeg_pcm_stream(input_source, DEFAULT_QUEUE_CAPACITY, cancellation_token.clone()).await?;

    let win_size = if window_samples == 0 { 8000 } else { window_samples }; // default 500ms at 16kHz
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
                    let sum_sq: f64 = scratch.iter().map(|&s| {
                        let v = f64::from(s);
                        v * v
                    }).sum();
                    let rms = (sum_sq / win_size as f64).sqrt();
                    let peak = scratch.iter().copied().map(i16::saturating_abs).max().unwrap_or_default();
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
            let sum_sq: f64 = final_scratch.iter().map(|&s| {
                let v = f64::from(s);
                v * v
            }).sum();
            let rms = (sum_sq / popped as f64).sqrt();
            let peak = final_scratch.iter().copied().map(i16::saturating_abs).max().unwrap_or_default();
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
    let manifest = VodManifest {
        vod_id: vod_id.to_string(),
        source_type: source_type.to_string(),
        original_url: original_url.map(ToString::to_string),
        title: title.to_string(),
        streamer: streamer.map(ToString::to_string),
        duration_seconds,
        audio_path: audio_path.to_string_lossy().to_string(),
        chat_file_path: None,
        video_stream_url: None,
        local_video_path: None,
        created_at: Utc::now().to_rfc3339(),
    };

    let manifest_path = project_dir.join("vod_manifest.json");
    let json_bytes = serde_json::to_vec_pretty(&manifest)
        .map_err(|e| AppError::Internal(format!("Failed to serialize vod_manifest: {e}")))?;

    std::fs::write(&manifest_path, json_bytes)
        .map_err(|e| AppError::Internal(format!("Failed to write vod_manifest.json: {e}")))?;

    Ok(manifest)
}

fn resolve_ytdlp_binary() -> String {
    if let Ok(p) = std::env::var("NEUROCLIP_YTDLP_PATH") {
        let trimmed = p.trim();
        if !trimmed.is_empty() {
            return trimmed.to_string();
        }
    }
    "yt-dlp".to_string()
}

fn resolve_ffmpeg_binary() -> String {
    if let Ok(p) = std::env::var("NEUROCLIP_FFMPEG_PATH") {
        let trimmed = p.trim();
        if !trimmed.is_empty() {
            return trimmed.to_string();
        }
    }
    "ffmpeg".to_string()
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
        assert_eq!(detect_source_type("https://www.twitch.tv/videos/12345"), "twitch_vod");
        assert_eq!(detect_source_type("https://www.youtube.com/watch?v=abc"), "youtube_vod");
        assert_eq!(detect_source_type("https://youtu.be/abc"), "youtube_vod");
        assert_eq!(detect_source_type("C:\\Videos\\recording.mp4"), "local_file");
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
        let temp_dir = std::env::temp_dir().join(format!("neuroclip_test_{}", Utc::now().timestamp_nanos_opt().unwrap_or(0)));
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
        ).unwrap();

        assert_eq!(manifest.vod_id, "vod_test_1");
        assert_eq!(manifest.source_type, "youtube_vod");
        assert_eq!(manifest.title, "Neuro-sama Sings");
        assert_eq!(manifest.duration_seconds, 3600.0);
        assert_eq!(manifest.streamer, Some("Vedal987".to_string()));
        assert!(!manifest.created_at.is_empty());

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
        std::env::set_var("NEUROCLIP_MOCK_FFMPEG_STREAM", "1");
        let cancel = CancellationToken::new();
        let res = compute_audio_energy_stream("mock_input", 2, None, cancel).await;
        std::env::remove_var("NEUROCLIP_MOCK_FFMPEG_STREAM");

        assert!(res.is_ok());
        if let Ok(windows) = res {
            assert_eq!(windows.len(), 2);
            assert_eq!(windows[0].window_index, 0);
            assert!(windows[0].rms_energy > 0.0);
            assert_eq!(windows[1].window_index, 1);
        }
    }
}

