//! Lossless Precision Range Slicer module for NeuroClip.
//!
//! Extracts precision video segments directly from online streams (via HTTP Range / HLS)
//! or local files without re-encoding (`-c copy`), aligns PTS with `-avoid_negative_ts make_zero`,
//! and verifies keyframe alignment / stream drift (ED-13.1 <= 50ms).
//!
//! Complies with `docs/CONVENTIONS.md` (zero-panic invariant, Result<T, AppError>),
//! `docs/ARCHITECTURE.md` §4.2, `docs/adr/ADR-003`, and `docs/TASK_BOARD.md` TASK-M4-13.

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::Instant;
use tokio_util::sync::CancellationToken;

use crate::core::storage::query_disk_status;
use crate::error::{AppError, IngestError, SidecarError};
use crate::ingestion::streamer::{assign_child_to_job_guard, ChildProcessGuard};

/// Tolerance threshold in milliseconds for audio/video duration drift (ED-13.1).
pub const MAX_KEYFRAME_DRIFT_MS: f64 = 50.0;

/// Maximum allowable start presentation timestamp in seconds (AC-13.3).
pub const MAX_INITIAL_PTS_DELAY_SEC: f64 = 0.050;

/// Minimum free disk space in MB required for slicing (pre-flight check).
pub const MIN_SLICING_DISK_SPACE_MB: u64 = 500;

/// Request payload for range slicing operations.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RangeSliceRequest {
    /// Input video source: either online URL (m3u8/https) or local file path.
    #[serde(alias = "input_source")]
    pub input_source: String,

    /// Start timestamp in seconds.
    #[serde(alias = "start_sec")]
    pub start_sec: f64,

    /// End timestamp in seconds.
    #[serde(alias = "end_sec")]
    pub end_sec: f64,

    /// Destination path for the sliced clip.
    #[serde(alias = "output_path")]
    pub output_path: PathBuf,

    /// Whether the input source is an online remote stream.
    #[serde(default, alias = "is_online")]
    pub is_online: bool,

    /// Optional HTTP headers (e.g. User-Agent or Authorization cookies).
    #[serde(default, alias = "http_headers")]
    pub http_headers: Option<Vec<(String, String)>>,

    /// Whether to use `-t <duration>` instead of `-to <end>`.
    #[serde(default, alias = "use_duration_flag")]
    pub use_duration_flag: bool,

    /// Optional cancellation task identifier.
    #[serde(default, alias = "task_id")]
    pub task_id: Option<String>,
}

/// Result of a range slice operation.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct SliceResult {
    /// Absolute or relative path to the generated clip.
    #[serde(alias = "clipPath", alias = "outputPath", alias = "output_path")]
    pub clip_path: String,

    /// Duration of the sliced segment in seconds.
    #[serde(
        alias = "durationSeconds",
        alias = "durationSec",
        alias = "duration_sec"
    )]
    pub duration_seconds: f64,

    /// Sliced file size in bytes.
    #[serde(alias = "fileSizeBytes", alias = "file_size_bytes")]
    pub file_size_bytes: u64,

    /// Video stream duration in seconds (if probed).
    #[serde(
        default,
        alias = "videoDuration",
        alias = "videoDurationSec",
        alias = "video_duration_sec"
    )]
    pub video_duration: Option<f64>,

    /// Audio stream duration in seconds (if probed).
    #[serde(
        default,
        alias = "audioDuration",
        alias = "audioDurationSec",
        alias = "audio_duration_sec"
    )]
    pub audio_duration: Option<f64>,

    /// Duration drift between video and audio in milliseconds (|video - audio| * 1000).
    #[serde(
        alias = "streamDriftMs",
        alias = "durationDeltaMs",
        alias = "duration_delta_ms"
    )]
    pub stream_drift_ms: f64,

    /// Whether the clip passes the keyframe alignment check (drift <= 50ms & start <= 50ms).
    #[serde(alias = "keyframeAligned", alias = "keyframe_aligned")]
    pub keyframe_aligned: bool,
}

/// ffprobe stream metadata format.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct FfprobeOutput {
    #[serde(default)]
    pub streams: Vec<FfprobeStream>,
    #[serde(default)]
    pub format: Option<FfprobeFormat>,
}

/// Individual stream entry from ffprobe JSON.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct FfprobeStream {
    #[serde(default)]
    pub index: usize,
    pub codec_type: Option<String>,
    pub duration: Option<String>,
    pub start_time: Option<String>,
}

/// Container format metadata from ffprobe JSON.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct FfprobeFormat {
    pub duration: Option<String>,
    pub size: Option<String>,
}

/// Keyframe and audio/video stream alignment verification report (ED-13.1).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct KeyframeAlignmentReport {
    /// True if duration drift <= 50ms and video start time <= 50ms.
    pub is_aligned: bool,

    /// Absolute drift between video and audio duration in milliseconds.
    pub drift_ms: f64,

    /// Alias for drift_ms matching test suites.
    pub duration_delta_ms: f64,

    /// Probed duration of the video stream in seconds.
    pub video_duration: Option<f64>,

    /// Probed duration of the audio stream in seconds.
    pub audio_duration: Option<f64>,

    /// Probed initial PTS of the video stream in seconds.
    pub video_start_time: Option<f64>,
}

impl KeyframeAlignmentReport {
    /// Convert alignment report to tuple format `(drift_ms, is_aligned, video_duration, audio_duration)`.
    #[must_use]
    pub const fn as_tuple(&self) -> (f64, bool, Option<f64>, Option<f64>) {
        (
            self.drift_ms,
            self.is_aligned,
            self.video_duration,
            self.audio_duration,
        )
    }
}

/// Pure evaluation function to check stream durations and start presentation timestamp (ED-13.1).
///
/// Returns `(is_aligned, drift_ms)`:
/// - `drift_ms = |video_dur - audio_dur| * 1000.0`
/// - `is_aligned = drift_ms <= max_drift_ms && video_start <= MAX_INITIAL_PTS_DELAY_SEC`
#[must_use]
pub fn verify_stream_durations(video_dur: f64, audio_dur: f64, video_start: f64) -> (bool, f64) {
    let drift_ms = (video_dur - audio_dur).abs() * 1000.0;
    let is_aligned = drift_ms <= (MAX_KEYFRAME_DRIFT_MS + 1e-6)
        && video_start <= (MAX_INITIAL_PTS_DELAY_SEC + 1e-6);
    (is_aligned, drift_ms)
}

/// Pure evaluation function to parse and verify `FfprobeOutput` against alignment limits (ED-13.1).
#[must_use]
pub fn evaluate_keyframe_alignment(
    probe: &FfprobeOutput,
    max_drift_ms: f64,
) -> KeyframeAlignmentReport {
    let video_stream = probe
        .streams
        .iter()
        .find(|s| s.codec_type.as_deref() == Some("video"));

    let audio_stream = probe
        .streams
        .iter()
        .find(|s| s.codec_type.as_deref() == Some("audio"));

    let video_duration = video_stream
        .and_then(|s| s.duration.as_deref())
        .and_then(|d| d.parse::<f64>().ok());

    let audio_duration = audio_stream
        .and_then(|s| s.duration.as_deref())
        .and_then(|d| d.parse::<f64>().ok());

    let video_start_time = video_stream
        .and_then(|s| s.start_time.as_deref())
        .and_then(|t| t.parse::<f64>().ok());

    let video_start = video_start_time.unwrap_or(0.0);

    let (is_aligned, drift_ms) = match (video_duration, audio_duration) {
        (Some(v_dur), Some(a_dur)) => {
            let drift = (v_dur - a_dur).abs() * 1000.0;
            let aligned =
                drift <= (max_drift_ms + 1e-6) && video_start <= (MAX_INITIAL_PTS_DELAY_SEC + 1e-6);
            (aligned, drift)
        }
        (Some(_), None) => {
            // Video-only stream (no audio track): aligned if start PTS <= 50ms
            let aligned = video_start <= (MAX_INITIAL_PTS_DELAY_SEC + 1e-6);
            (aligned, 0.0)
        }
        (None, Some(_)) => {
            // Audio-only stream
            (true, 0.0)
        }
        (None, None) => {
            // If streams lack duration, fallback to format duration if available
            if let Some(_format_dur) = probe
                .format
                .as_ref()
                .and_then(|f| f.duration.as_deref())
                .and_then(|d| d.parse::<f64>().ok())
            {
                (video_start <= (MAX_INITIAL_PTS_DELAY_SEC + 1e-6), 0.0)
            } else {
                (false, 0.0)
            }
        }
    };

    KeyframeAlignmentReport {
        is_aligned,
        drift_ms,
        duration_delta_ms: drift_ms,
        video_duration,
        audio_duration,
        video_start_time,
    }
}

/// Parse ffprobe JSON output string into `FfprobeOutput`.
///
/// # Errors
/// Returns `AppError::Ingestion` if JSON deserialization fails.
pub fn parse_ffprobe_json(json_str: &str) -> Result<FfprobeOutput, AppError> {
    serde_json::from_str::<FfprobeOutput>(json_str).map_err(|e| {
        AppError::Ingestion(IngestError::SliceFailed(format!(
            "Lỗi phân tích ffprobe JSON: {e}"
        )))
    })
}

/// Resolve path to the `ffmpeg` executable.
#[must_use]
pub fn resolve_ffmpeg_binary() -> String {
    crate::sidecar::resolve_sidecar_binary("ffmpeg", "NEUROCLIP_FFMPEG_PATH")
}

/// Resolve path to the `ffprobe` executable.
#[must_use]
pub fn resolve_ffprobe_binary() -> String {
    crate::sidecar::resolve_sidecar_binary("ffprobe", "NEUROCLIP_FFPROBE_PATH")
}

/// Build FFmpeg command-line arguments for range slicing (UT-13.1).
///
/// Invariant:
/// - Strictly place `-ss <start>` before `-i <source>` (input-seeking demuxer).
/// - Place `-to <end>` (or `-t <duration>`) before `-i <source>`.
/// - Lossless stream copy `-c copy`.
/// - PTS alignment: `-avoid_negative_ts make_zero`.
/// - Automatic HTTP reconnect for remote online streams: `-reconnect 1 -reconnect_streamed 1 -reconnect_delay_max 5`.
///
/// # Errors
/// Returns `AppError::Ingestion(IngestError::InvalidTimeRange)` if `start_sec < 0.0` or `start_sec >= end_sec`.
pub fn build_slice_ffmpeg_args(req: &RangeSliceRequest) -> Result<Vec<String>, AppError> {
    if !req.start_sec.is_finite()
        || !req.end_sec.is_finite()
        || req.start_sec < 0.0
        || req.start_sec >= req.end_sec
    {
        return Err(AppError::Ingestion(IngestError::InvalidTimeRange {
            start_sec: req.start_sec,
            end_sec: req.end_sec,
        }));
    }

    let mut args = Vec::with_capacity(20);

    // General flags
    args.push("-hide_banner".to_string());
    args.push("-loglevel".to_string());
    args.push("error".to_string());
    args.push("-nostdin".to_string());
    args.push("-y".to_string());

    let is_online = req.is_online
        || req.input_source.starts_with("http://")
        || req.input_source.starts_with("https://");

    if is_online {
        args.push("-reconnect".to_string());
        args.push("1".to_string());
        args.push("-reconnect_streamed".to_string());
        args.push("1".to_string());
        args.push("-reconnect_delay_max".to_string());
        args.push("5".to_string());

        if let Some(ref headers) = req.http_headers {
            if !headers.is_empty() {
                use std::fmt::Write as _;
                let mut header_str = String::new();
                for (k, v) in headers {
                    let _ = write!(header_str, "{k}: {v}\r\n");
                }
                args.push("-headers".to_string());
                args.push(header_str);
            }
        }
    }

    // Input seeking: MUST be placed BEFORE -i
    args.push("-ss".to_string());
    args.push(format!("{:.3}", req.start_sec));

    if req.use_duration_flag {
        args.push("-t".to_string());
        args.push(format!("{:.3}", req.end_sec - req.start_sec));
    } else {
        args.push("-to".to_string());
        args.push(format!("{:.3}", req.end_sec));
    }

    // Input source
    args.push("-i".to_string());
    args.push(req.input_source.clone());

    // Stream copy & timestamp normalization
    args.push("-c".to_string());
    args.push("copy".to_string());
    args.push("-avoid_negative_ts".to_string());
    args.push("make_zero".to_string());

    // Destination output path
    args.push(req.output_path.to_string_lossy().to_string());

    Ok(args)
}

/// Convenience builder function matching UT-13.1 test signatures.
///
/// # Errors
/// Returns `AppError::Ingestion(IngestError::InvalidTimeRange)` if timestamps are invalid.
pub fn build_ffmpeg_range_slice_args(
    input_source: &str,
    start_sec: f64,
    end_sec: f64,
    output_path: &str,
    is_online: bool,
) -> Result<Vec<String>, AppError> {
    let req = RangeSliceRequest {
        input_source: input_source.to_string(),
        start_sec,
        end_sec,
        output_path: PathBuf::from(output_path),
        is_online,
        http_headers: None,
        use_duration_flag: false,
        task_id: None,
    };
    build_slice_ffmpeg_args(&req)
}

/// Run `ffprobe` to inspect keyframe alignment and stream duration drift of a generated clip.
///
/// # Errors
/// Returns `AppError` if probing fails or output cannot be parsed.
pub async fn probe_keyframe_alignment(
    file_path: &Path,
) -> Result<KeyframeAlignmentReport, AppError> {
    // 1. Mock hook for test environments without ffprobe binary
    if let Ok(mock_json) = std::env::var("NEUROCLIP_MOCK_FFPROBE_OUTPUT") {
        let probe = parse_ffprobe_json(&mock_json)?;
        return Ok(evaluate_keyframe_alignment(&probe, MAX_KEYFRAME_DRIFT_MS));
    }

    if std::env::var("NEUROCLIP_MOCK_RANGE_SLICER").is_ok() {
        // Deterministic synthetic mock probe passing ED-13.1
        let mock_probe = FfprobeOutput {
            streams: vec![
                FfprobeStream {
                    index: 0,
                    codec_type: Some("video".to_string()),
                    duration: Some("90.033000".to_string()),
                    start_time: Some("0.000000".to_string()),
                },
                FfprobeStream {
                    index: 1,
                    codec_type: Some("audio".to_string()),
                    duration: Some("90.025000".to_string()),
                    start_time: Some("0.000000".to_string()),
                },
            ],
            format: Some(FfprobeFormat {
                duration: Some("90.033000".to_string()),
                size: Some("45000000".to_string()),
            }),
        };
        return Ok(evaluate_keyframe_alignment(
            &mock_probe,
            MAX_KEYFRAME_DRIFT_MS,
        ));
    }

    // 2. Real ffprobe process execution
    let binary = resolve_ffprobe_binary();
    let mut cmd = tokio::process::Command::new(&binary);
    cmd.args([
        "-v",
        "error",
        "-show_entries",
        "stream=index,codec_type,duration,start_time",
        "-show_entries",
        "format=duration,size",
        "-of",
        "json",
    ]);
    cmd.arg(file_path.as_os_str());
    cmd.stdout(Stdio::piped());
    cmd.stderr(Stdio::piped());
    cmd.kill_on_drop(true);

    let child = cmd.spawn().map_err(|e| {
        AppError::Sidecar(SidecarError::BinaryNotFound {
            name: "ffprobe".to_string(),
            path: format!("{binary}: {e}"),
        })
    })?;

    assign_child_to_job_guard(&child);
    let mut guard = ChildProcessGuard::new(child);

    let output = match guard.take_child() {
        Some(child_proc) => child_proc.wait_with_output().await.map_err(|e| {
            AppError::Sidecar(SidecarError::ExecutionFailed {
                binary: "ffprobe".to_string(),
                exit_code: None,
                details: e.to_string(),
            })
        })?,
        None => {
            return Err(AppError::Internal(
                "ChildProcessGuard missing active process".to_string(),
            ));
        }
    };

    if !output.status.success() {
        let stderr_msg = String::from_utf8_lossy(&output.stderr).to_string();
        return Err(AppError::Sidecar(SidecarError::ExecutionFailed {
            binary: "ffprobe".to_string(),
            exit_code: output.status.code(),
            details: stderr_msg,
        }));
    }

    let stdout_str = String::from_utf8_lossy(&output.stdout);
    let probe = parse_ffprobe_json(&stdout_str)?;
    Ok(evaluate_keyframe_alignment(&probe, MAX_KEYFRAME_DRIFT_MS))
}

/// Execute range slice operation asynchronously with zero zombie processes and cancellation support.
///
/// Features:
/// - Pre-flight storage check (minimum 500MB free space).
/// - Non-blocking asynchronous process execution.
/// - RAII child process guard (`ChildProcessGuard`) & Windows Job Object (`ADR-007`).
/// - Real-time task cancellation token support.
/// - Deterministic offline mock testing hook via `NEUROCLIP_MOCK_RANGE_SLICER`.
///
/// # Errors
/// Returns `AppError` if time range is invalid, storage is full, FFmpeg fails, or process is cancelled.
#[allow(
    clippy::too_many_lines,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss
)]
pub async fn execute_range_slice(
    req: RangeSliceRequest,
    cancel_token: Option<CancellationToken>,
) -> Result<SliceResult, AppError> {
    // 1. Validate boundary invariants: 0.0 <= start_sec < end_sec
    if !req.start_sec.is_finite()
        || !req.end_sec.is_finite()
        || req.start_sec < 0.0
        || req.start_sec >= req.end_sec
    {
        return Err(AppError::Ingestion(IngestError::InvalidTimeRange {
            start_sec: req.start_sec,
            end_sec: req.end_sec,
        }));
    }

    // 2. Cancellation checkpoint
    if let Some(ref token) = cancel_token {
        if token.is_cancelled() {
            return Err(AppError::TaskCancelled);
        }
    }

    // 3. Pre-flight storage check
    let target_dir = req
        .output_path
        .parent()
        .map_or_else(|| PathBuf::from("."), PathBuf::from);

    std::fs::create_dir_all(&target_dir).map_err(|e| {
        AppError::Storage(crate::error::StorageError::DirectoryCreationFailed {
            path: target_dir.display().to_string(),
            details: e.to_string(),
        })
    })?;

    let disk_status = query_disk_status(Some(&target_dir))?;
    if disk_status.free_space_mb < MIN_SLICING_DISK_SPACE_MB {
        return Err(AppError::Ingestion(IngestError::DiskSpaceInsufficient {
            required_mb: MIN_SLICING_DISK_SPACE_MB,
            available_mb: disk_status.free_space_mb,
        }));
    }

    let target_duration = req.end_sec - req.start_sec;

    // 4. Deterministic offline CI / test environment mock
    if std::env::var("NEUROCLIP_MOCK_RANGE_SLICER").is_ok() {
        if let Some(ref token) = cancel_token {
            if token.is_cancelled() {
                return Err(AppError::TaskCancelled);
            }
        }

        // Generate synthetic clip file if not present (simulating 30MB-80MB for 90s clip)
        // 90s at standard 4.5Mbps ~ 50MB
        let simulated_size_bytes = ((target_duration / 90.0) * 50.0 * 1024.0 * 1024.0)
            .clamp(30.0 * 1024.0 * 1024.0, 80.0 * 1024.0 * 1024.0)
            as u64;

        if !req.output_path.exists() {
            let file = std::fs::File::create(&req.output_path).map_err(|e| {
                AppError::Storage(crate::error::StorageError::DirectoryCreationFailed {
                    path: req.output_path.display().to_string(),
                    details: e.to_string(),
                })
            })?;
            let _ = file.set_len(simulated_size_bytes);
        }

        let probe_report = probe_keyframe_alignment(&req.output_path).await?;

        return Ok(SliceResult {
            clip_path: req.output_path.to_string_lossy().to_string(),
            duration_seconds: target_duration,
            file_size_bytes: simulated_size_bytes,
            video_duration: probe_report
                .video_duration
                .or(Some(target_duration + 0.015)),
            audio_duration: probe_report
                .audio_duration
                .or(Some(target_duration + 0.005)),
            stream_drift_ms: probe_report.drift_ms,
            keyframe_aligned: probe_report.is_aligned,
        });
    }

    // 5. Production execution via FFmpeg
    let args = build_slice_ffmpeg_args(&req)?;
    let binary = resolve_ffmpeg_binary();

    let start_time = Instant::now();
    let mut cmd = tokio::process::Command::new(&binary);
    cmd.args(&args);
    cmd.stdout(Stdio::null());
    cmd.stderr(Stdio::piped());
    cmd.kill_on_drop(true);

    let mut child = cmd.spawn().map_err(|e| {
        AppError::Sidecar(SidecarError::BinaryNotFound {
            name: "ffmpeg".to_string(),
            path: format!("{binary}: {e}"),
        })
    })?;

    assign_child_to_job_guard(&child);
    let stderr_handle = child.stderr.take().map(|stderr| {
        tokio::spawn(async move {
            let mut lines = crate::sidecar::LossyLineReader::new(stderr);
            let mut tail = std::collections::VecDeque::with_capacity(64);
            while let Ok(Some(line)) = lines.next_line().await {
                if tail.len() >= 64 {
                    tail.pop_front();
                }
                tail.push_back(line);
            }
            tail.into_iter().collect::<Vec<_>>().join("\n")
        })
    });
    let mut guard = ChildProcessGuard::new(child);

    let output_status = if let Some(ref token) = cancel_token {
        let token_cancelled = token.clone();
        if let Some(child_ref) = guard.child_mut() {
            tokio::select! {
                biased;
                () = token_cancelled.cancelled() => {
                    if let Some(h) = stderr_handle {
                        h.abort();
                    }
                    return Err(AppError::TaskCancelled);
                }
                res = child_ref.wait() => {
                    res.map_err(|e| AppError::Sidecar(SidecarError::ExecutionFailed {
                        binary: "ffmpeg".to_string(),
                        exit_code: None,
                        details: e.to_string(),
                    }))?
                }
            }
        } else {
            if let Some(h) = stderr_handle {
                h.abort();
            }
            return Err(AppError::Internal(
                "ChildProcessGuard missing active process".to_string(),
            ));
        }
    } else if let Some(child_ref) = guard.child_mut() {
        child_ref.wait().await.map_err(|e| {
            AppError::Sidecar(SidecarError::ExecutionFailed {
                binary: "ffmpeg".to_string(),
                exit_code: None,
                details: e.to_string(),
            })
        })?
    } else {
        if let Some(h) = stderr_handle {
            h.abort();
        }
        return Err(AppError::Internal(
            "ChildProcessGuard missing active process".to_string(),
        ));
    };

    let stderr_tail = match stderr_handle {
        Some(h) => h.await.unwrap_or_default(),
        None => String::new(),
    };

    if !output_status.success() {
        let details = if stderr_tail.is_empty() {
            format!("FFmpeg thoát với mã lỗi: {:?}", output_status.code())
        } else {
            format!(
                "FFmpeg thoát với mã lỗi: {:?} ({stderr_tail})",
                output_status.code()
            )
        };
        return Err(AppError::Ingestion(IngestError::SliceFailed(details)));
    }

    let file_metadata = std::fs::metadata(&req.output_path).map_err(|e| {
        AppError::Ingestion(IngestError::SliceFailed(format!(
            "Không thể đọc thông tin tệp video cắt ra: {e}"
        )))
    })?;

    let file_size_bytes = file_metadata.len();
    let probe_report = probe_keyframe_alignment(&req.output_path).await?;
    let _ = start_time.elapsed();

    Ok(SliceResult {
        clip_path: req.output_path.to_string_lossy().to_string(),
        duration_seconds: target_duration,
        file_size_bytes,
        video_duration: probe_report.video_duration,
        audio_duration: probe_report.audio_duration,
        stream_drift_ms: probe_report.drift_ms,
        keyframe_aligned: probe_report.is_aligned,
    })
}
