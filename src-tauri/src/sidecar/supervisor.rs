//! Subprocess supervisor for `FFmpeg`, `yt-dlp`, and Python workers (`TASK-M4-03`).
//!
//! Enforces:
//! - Bounded 1,000-line stdout/stderr async streaming (`AC-03.2`)
//! - `FFmpeg` progress parsing (`UT-03.1`) and `yt-dlp` error parsing (`UT-03.2`)
//! - `sidecar:progress` Tauri event emission (`IT-03.1`)
//! - Kill-on-Drop & `CancellationToken` termination <= 200ms (`AC-03.3`)
//! - Windows Job Object `JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE` assignment (`AC-03.1`)

use std::collections::VecDeque;
use std::process::Stdio;
use std::sync::OnceLock;
use std::time::Duration;

use regex::Regex;
use serde::{Deserialize, Serialize};
use tauri::Emitter;
use tokio::io::{AsyncRead, AsyncReadExt};
use tokio::process::Command;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

use crate::error::{AppError, IngestError, SidecarError};
use crate::sidecar::kill_switch::{assign_child_to_job_guard, ChildProcessGuard};

/// Maximum number of log lines buffered in bounded stdout/stderr channels (`AC-03.2`).
pub const BOUNDED_LOG_CAPACITY: usize = 1_000;

/// Maximum byte length buffered for a single stdout/stderr line before flushing (64 KiB).
pub const MAX_LOG_LINE_BYTES: usize = 64 * 1024;

/// Event name emitted to frontend listeners for sidecar subprocess progress updates.
pub const SIDECAR_PROGRESS_EVENT: &str = "sidecar:progress";

/// Standardized payload emitted via the `sidecar:progress` Tauri event channel.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub struct SidecarProgressPayload {
    pub task_id: String,
    pub percentage: f64,
    pub current_fps: Option<f64>,
    pub eta_sec: Option<u64>,
}

/// Parsed `FFmpeg` progress telemetry from a single stdout/stderr status line (`UT-03.1`).
#[derive(Debug, Clone, PartialEq)]
pub struct FfmpegProgress {
    pub time_sec: f64,
    pub fps: Option<f64>,
    pub bitrate_kbps: Option<f64>,
    pub speed: Option<f64>,
}

/// Configuration for running an external binary under [`SubprocessSupervisor`].
#[derive(Debug, Clone, PartialEq)]
pub struct SubprocessConfig {
    pub task_id: String,
    pub binary: String,
    pub args: Vec<String>,
    pub total_duration_sec: Option<f64>,
}

/// Bounded summary returned after a supervised subprocess completes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SubprocessOutput {
    pub exit_code: Option<i32>,
    pub stdout_lines: Vec<String>,
    pub stderr_lines: Vec<String>,
    pub total_stdout_lines: usize,
    pub total_stderr_lines: usize,
}

/// Create a bounded string channel with capacity [`BOUNDED_LOG_CAPACITY`] (1,000 lines).
#[must_use]
pub fn create_bounded_log_channel() -> (mpsc::Sender<String>, mpsc::Receiver<String>) {
    mpsc::channel::<String>(BOUNDED_LOG_CAPACITY)
}

/// Parse an `FFmpeg` progress status line (`time=HH:MM:SS.xx ...`) into [`FfmpegProgress`].
#[must_use]
pub fn parse_ffmpeg_progress_line(line: &str) -> Option<FfmpegProgress> {
    static TIME_RE: OnceLock<Option<Regex>> = OnceLock::new();
    static FPS_RE: OnceLock<Option<Regex>> = OnceLock::new();
    static BITRATE_RE: OnceLock<Option<Regex>> = OnceLock::new();
    static SPEED_RE: OnceLock<Option<Regex>> = OnceLock::new();

    let time_re = TIME_RE
        .get_or_init(|| Regex::new(r"time=\s*(\d+):(\d{2}):(\d{2}(?:\.\d+)?)").ok())
        .as_ref()?;
    let caps = time_re.captures(line)?;

    let hours = caps.get(1)?.as_str().parse::<f64>().ok()?;
    let mins = caps.get(2)?.as_str().parse::<f64>().ok()?;
    let secs = caps.get(3)?.as_str().parse::<f64>().ok()?;
    let time_sec = hours * 3600.0 + mins * 60.0 + secs;

    let fps_re = FPS_RE
        .get_or_init(|| Regex::new(r"fps=\s*(\d+(?:\.\d+)?)").ok())
        .as_ref();
    let fps = fps_re.and_then(|re| {
        re.captures(line)
            .and_then(|c| c.get(1)?.as_str().parse::<f64>().ok())
    });

    let bitrate_re = BITRATE_RE
        .get_or_init(|| Regex::new(r"bitrate=\s*(\d+(?:\.\d+)?)\s*([kKmMgG]?bits/s)").ok())
        .as_ref();
    let bitrate_kbps = bitrate_re.and_then(|re| {
        let c = re.captures(line)?;
        let val = c.get(1)?.as_str().parse::<f64>().ok()?;
        let unit = c.get(2)?.as_str();
        let mult = if unit.starts_with('M') || unit.starts_with('m') {
            1000.0
        } else if unit.starts_with('G') || unit.starts_with('g') {
            1_000_000.0
        } else if unit.starts_with('k') || unit.starts_with('K') {
            1.0
        } else {
            0.001
        };
        Some(val * mult)
    });

    let speed_re = SPEED_RE
        .get_or_init(|| Regex::new(r"speed=\s*(\d+(?:\.\d+)?)x").ok())
        .as_ref();
    let speed = speed_re.and_then(|re| {
        re.captures(line)
            .and_then(|c| c.get(1)?.as_str().parse::<f64>().ok())
    });

    Some(FfmpegProgress {
        time_sec,
        fps,
        bitrate_kbps,
        speed,
    })
}

#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
fn safe_ceil_f64_to_u64(val: f64) -> Option<u64> {
    if !val.is_finite() || val < 0.0 {
        return None;
    }
    let ceiled = val.ceil();
    if ceiled >= 18_446_744_073_709_551_615.0 {
        Some(u64::MAX)
    } else {
        Some(ceiled as u64)
    }
}

/// Convert parsed [`FfmpegProgress`] into a [`SidecarProgressPayload`] given optional total duration.
#[must_use]
pub fn compute_sidecar_progress(
    task_id: &str,
    progress: &FfmpegProgress,
    total_duration_sec: Option<f64>,
) -> SidecarProgressPayload {
    let (percentage, eta_sec) = match total_duration_sec {
        Some(total) if total.is_finite() && total > 0.0 => {
            let pct = ((progress.time_sec / total) * 100.0).clamp(0.0, 100.0);
            let remaining = (total - progress.time_sec).max(0.0);
            let eta = match progress.speed {
                Some(spd) if spd.is_finite() && spd > 0.0 => safe_ceil_f64_to_u64(remaining / spd),
                _ => safe_ceil_f64_to_u64(remaining),
            };
            (pct, eta)
        }
        _ => (0.0, None),
    };

    SidecarProgressPayload {
        task_id: task_id.to_string(),
        percentage,
        current_fps: progress.fps,
        eta_sec,
    }
}

/// Parse a `yt-dlp` error line into a structured [`AppError`] (`UT-03.2`).
#[must_use]
pub fn parse_ytdlp_error_line(line: &str) -> Option<AppError> {
    let trimmed = line.trim();
    if trimmed.is_empty() {
        return None;
    }
    let lower = trimmed.to_ascii_lowercase();

    if lower.contains("http error 403")
        || lower.contains("403 forbidden")
        || lower.contains("http error 429")
        || lower.contains("too many requests")
        || lower.contains("not a bot")
        || lower.contains("confirm you're not a bot")
        || lower.contains("confirm you\u{2019}re not a bot")
    {
        return Some(AppError::Ingestion(IngestError::BotBlocked(
            trimmed.to_string(),
        )));
    }

    if lower.contains("private video")
        || lower.contains("this video is private")
        || lower.contains("video unavailable")
        || lower.contains("http error 404")
        || lower.contains("404 not found")
        || lower.contains("sign in to confirm your age")
        || trimmed.starts_with("ERROR:")
    {
        return Some(AppError::Ingestion(IngestError::ExtractionFailed(
            trimmed.to_string(),
        )));
    }

    None
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum YtdlpErrorPriority {
    ExtractionFailed = 1,
    LiveStreamUnsupported = 2,
    BotBlocked = 3,
}

fn ytdlp_error_priority(err: &AppError) -> YtdlpErrorPriority {
    match err {
        AppError::Ingestion(IngestError::BotBlocked(_)) => YtdlpErrorPriority::BotBlocked,
        AppError::Ingestion(IngestError::LiveStreamUnsupported) => {
            YtdlpErrorPriority::LiveStreamUnsupported
        }
        _ => YtdlpErrorPriority::ExtractionFailed,
    }
}

fn update_best_ytdlp_error(slot: &mut Option<(YtdlpErrorPriority, AppError)>, line: &str) {
    if let Some(err) = parse_ytdlp_error_line(line) {
        let priority = ytdlp_error_priority(&err);
        let should_replace = match slot {
            Some((existing_priority, _)) => priority > *existing_priority,
            None => true,
        };
        if should_replace {
            *slot = Some((priority, err));
        }
    }
}

async fn pump_lines<R: AsyncRead + Unpin>(mut reader: R, tx: mpsc::Sender<String>) {
    let mut read_buf = [0u8; 8192];
    let mut line_buf: Vec<u8> = Vec::with_capacity(512);
    let mut skip_next_lf = false;

    loop {
        match reader.read(&mut read_buf).await {
            Ok(0) => {
                if !line_buf.is_empty() {
                    let line = String::from_utf8_lossy(&line_buf).into_owned();
                    let _ = tx.send(line).await;
                }
                break;
            }
            Ok(n) => {
                for &byte in &read_buf[..n] {
                    if skip_next_lf {
                        skip_next_lf = false;
                        if byte == b'\n' {
                            continue;
                        }
                    }
                    if byte == b'\r' {
                        let line = String::from_utf8_lossy(&line_buf).into_owned();
                        line_buf.clear();
                        if tx.send(line).await.is_err() {
                            return;
                        }
                        skip_next_lf = true;
                    } else if byte == b'\n' {
                        let line = String::from_utf8_lossy(&line_buf).into_owned();
                        line_buf.clear();
                        if tx.send(line).await.is_err() {
                            return;
                        }
                    } else {
                        line_buf.push(byte);
                        if line_buf.len() >= MAX_LOG_LINE_BYTES {
                            let line = String::from_utf8_lossy(&line_buf).into_owned();
                            line_buf.clear();
                            if tx.send(line).await.is_err() {
                                return;
                            }
                        }
                    }
                }
            }
            Err(_) => break,
        }
    }
}

fn push_bounded(ring: &mut VecDeque<String>, line: String) {
    if ring.len() >= BOUNDED_LOG_CAPACITY {
        ring.pop_front();
    }
    ring.push_back(line);
}

fn emit_line_progress(
    line: &str,
    task_id: &str,
    total_duration_sec: Option<f64>,
    app: Option<&tauri::AppHandle>,
    progress_cb: Option<SidecarProgressCallback<'_>>,
) {
    if let Some(progress) = parse_ffmpeg_progress_line(line) {
        let payload = compute_sidecar_progress(task_id, &progress, total_duration_sec);
        if let Some(app_handle) = app {
            let _ = app_handle.emit(SIDECAR_PROGRESS_EVENT, &payload);
        }
        if let Some(cb) = progress_cb {
            cb(payload);
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn finalize_exit_status(
    binary: &str,
    status: std::process::ExitStatus,
    stdout_lines: Vec<String>,
    stderr_lines: Vec<String>,
    total_stdout_lines: usize,
    total_stderr_lines: usize,
    best_stderr_error: Option<(YtdlpErrorPriority, AppError)>,
    best_stdout_error: Option<(YtdlpErrorPriority, AppError)>,
) -> Result<SubprocessOutput, AppError> {
    if !status.success() {
        let best_mapped_err = match (best_stderr_error, best_stdout_error) {
            (Some((p_err, e_err)), Some((p_out, e_out))) => {
                if p_out > p_err {
                    Some(e_out)
                } else {
                    Some(e_err)
                }
            }
            (Some((_, e_err)), None) => Some(e_err),
            (None, Some((_, e_out))) => Some(e_out),
            (None, None) => None,
        };
        if let Some(mapped_err) = best_mapped_err {
            return Err(mapped_err);
        }
        let tail = stderr_lines
            .iter()
            .rev()
            .take(10)
            .cloned()
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .collect::<Vec<_>>()
            .join("\n");
        return Err(AppError::Sidecar(SidecarError::ExecutionFailed {
            binary: binary.to_string(),
            exit_code: status.code(),
            details: if tail.is_empty() {
                format!("Process exited with status {:?}", status.code())
            } else {
                tail
            },
        }));
    }

    Ok(SubprocessOutput {
        exit_code: status.code(),
        stdout_lines,
        stderr_lines,
        total_stdout_lines,
        total_stderr_lines,
    })
}

type SpawnedPipes = (
    ChildProcessGuard,
    tokio::process::ChildStdout,
    tokio::process::ChildStderr,
);

fn spawn_supervised_child(config: &SubprocessConfig) -> Result<SpawnedPipes, AppError> {
    let mut cmd = Command::new(&config.binary);
    cmd.args(&config.args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);

    let mut child = cmd.spawn().map_err(|e| {
        AppError::Sidecar(SidecarError::BinaryNotFound {
            name: config.binary.clone(),
            path: format!("{}: {e}", config.binary),
        })
    })?;

    assign_child_to_job_guard(&child);

    let stdout = child.stdout.take().ok_or_else(|| {
        AppError::Sidecar(SidecarError::ExecutionFailed {
            binary: config.binary.clone(),
            exit_code: None,
            details: "Failed to capture subprocess stdout pipe".to_string(),
        })
    })?;

    let stderr = child.stderr.take().ok_or_else(|| {
        AppError::Sidecar(SidecarError::ExecutionFailed {
            binary: config.binary.clone(),
            exit_code: None,
            details: "Failed to capture subprocess stderr pipe".to_string(),
        })
    })?;

    Ok((ChildProcessGuard::new(child), stdout, stderr))
}

/// Callback type alias for receiving real-time [`SidecarProgressPayload`] telemetry.
pub type SidecarProgressCallback<'a> = &'a (dyn Fn(SidecarProgressPayload) + Send + Sync);

/// Managed async subprocess supervisor enforcing bounded channels, Job Objects, and fast cancellation.
pub struct SubprocessSupervisor;

impl SubprocessSupervisor {
    /// Spawn and monitor a subprocess until completion or cancellation.
    ///
    /// # Errors
    /// Returns:
    /// - `AppError::Sidecar(SidecarError::BinaryNotFound)` if spawning fails
    /// - `AppError::TaskCancelled` if `cancellation_token` triggers (child killed promptly <= 200ms)
    /// - Mapped `yt-dlp` [`AppError`] or `AppError::Sidecar(SidecarError::ExecutionFailed)` on non-zero exit
    pub async fn run(
        config: &SubprocessConfig,
        app: Option<&tauri::AppHandle>,
        progress_cb: Option<SidecarProgressCallback<'_>>,
        cancellation_token: &CancellationToken,
    ) -> Result<SubprocessOutput, AppError> {
        if cancellation_token.is_cancelled() {
            return Err(AppError::TaskCancelled);
        }

        let (mut guard, stdout, stderr) = spawn_supervised_child(config)?;
        let (tx_out, mut rx_out) = create_bounded_log_channel();
        let (tx_err, mut rx_err) = create_bounded_log_channel();

        let stdout_pump = tokio::spawn(pump_lines(stdout, tx_out));
        let stderr_pump = tokio::spawn(pump_lines(stderr, tx_err));

        let mut stdout_ring: VecDeque<String> = VecDeque::with_capacity(BOUNDED_LOG_CAPACITY);
        let mut stderr_ring: VecDeque<String> = VecDeque::with_capacity(BOUNDED_LOG_CAPACITY);
        let mut best_stdout_error: Option<(YtdlpErrorPriority, AppError)> = None;
        let mut best_stderr_error: Option<(YtdlpErrorPriority, AppError)> = None;
        let (mut total_stdout_lines, mut total_stderr_lines) = (0usize, 0usize);
        let (mut out_closed, mut err_closed) = (false, false);

        while !out_closed || !err_closed {
            tokio::select! {
                biased;
                () = cancellation_token.cancelled() => {
                    if let Some(child_proc) = guard.child_mut() {
                        let _ = child_proc.start_kill();
                        let _ = tokio::time::timeout(Duration::from_millis(150), child_proc.wait()).await;
                    }
                    stdout_pump.abort();
                    stderr_pump.abort();
                    return Err(AppError::TaskCancelled);
                }
                maybe_out = rx_out.recv(), if !out_closed => {
                    if let Some(line) = maybe_out {
                        total_stdout_lines += 1;
                        emit_line_progress(&line, &config.task_id, config.total_duration_sec, app, progress_cb);
                        update_best_ytdlp_error(&mut best_stdout_error, &line);
                        push_bounded(&mut stdout_ring, line);
                    } else {
                        out_closed = true;
                    }
                }
                maybe_err = rx_err.recv(), if !err_closed => {
                    if let Some(line) = maybe_err {
                        total_stderr_lines += 1;
                        emit_line_progress(&line, &config.task_id, config.total_duration_sec, app, progress_cb);
                        update_best_ytdlp_error(&mut best_stderr_error, &line);
                        push_bounded(&mut stderr_ring, line);
                    } else {
                        err_closed = true;
                    }
                }
            }
        }

        let _ = stdout_pump.await;
        let _ = stderr_pump.await;

        let Some(child_proc) = guard.child_mut() else {
            return Err(AppError::Sidecar(SidecarError::ExecutionFailed {
                binary: config.binary.clone(),
                exit_code: None,
                details: "Child process handle missing before wait".to_string(),
            }));
        };

        let status = tokio::select! {
            biased;
            () = cancellation_token.cancelled() => {
                let _ = child_proc.start_kill();
                let _ = tokio::time::timeout(Duration::from_millis(150), child_proc.wait()).await;
                return Err(AppError::TaskCancelled);
            }
            wait_res = child_proc.wait() => {
                wait_res.map_err(|e| AppError::Sidecar(SidecarError::ExecutionFailed {
                    binary: config.binary.clone(),
                    exit_code: None,
                    details: e.to_string(),
                }))?
            }
        };

        let _ = guard.take_child();
        finalize_exit_status(
            &config.binary,
            status,
            stdout_ring.into_iter().collect(),
            stderr_ring.into_iter().collect(),
            total_stdout_lines,
            total_stderr_lines,
            best_stderr_error,
            best_stdout_error,
        )
    }
}
