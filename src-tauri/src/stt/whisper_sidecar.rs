//! Whisper.cpp sidecar process supervisor and lifecycle management.
//!
//! Enforces zero zombie processes on Windows via `WindowsJobObjectGuard` (`ADR-007`),
//! streams real-time progress events (`stt:progress`), and provides mock execution
//! capabilities for deterministic offline CI testing.

use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::Arc;
use serde::{Deserialize, Serialize};
use tokio::io::{AsyncBufReadExt, BufReader};

use crate::error::{AppError, SidecarError, SttError};
use crate::ingestion::streamer::assign_child_to_job_guard;
use crate::stt::transcript_parser::{parse_whisper_json_full, TranscriptSegmentsContainer};

/// Progress callback closure type.
pub type ProgressCallback = Arc<dyn Fn(SttProgressPayload) + Send + Sync>;

/// Progress payload emitted across the Tauri IPC bridge (`stt:progress`).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SttProgressPayload {
    pub task_id: String,
    pub stage: String,
    pub percent: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub eta_sec: Option<f64>,
    pub current_chunk: u32,
    pub total_chunks: u32,
}

/// Configuration options for launching the Whisper.cpp sidecar process.
#[derive(Debug, Clone)]
pub struct WhisperSidecarConfig {
    pub binary_path: Option<PathBuf>,
    pub model_path: PathBuf,
    pub threads: u32,
    pub language: String,
    pub prompt: Option<String>,
}

impl Default for WhisperSidecarConfig {
    fn default() -> Self {
        Self {
            binary_path: None,
            model_path: PathBuf::new(),
            threads: 4,
            language: "en".to_string(),
            prompt: Some("Vedal, Neuro-sama, Evil Neuro, Tutel, Gymbag".to_string()),
        }
    }
}

/// Resolve the path to the native `whisper-cli.exe` (or `main.exe`) sidecar binary.
pub fn resolve_whisper_binary() -> Result<PathBuf, AppError> {
    if let Ok(override_path) = std::env::var("NEUROCLIP_WHISPER_PATH") {
        let p = PathBuf::from(override_path.trim());
        if p.exists() {
            return Ok(p);
        }
    }

    if let Ok(local_app_data) = std::env::var("LOCALAPPDATA") {
        let p = PathBuf::from(local_app_data)
            .join("NeuroClip")
            .join("sidecars")
            .join("whisper")
            .join("whisper-cli.exe");
        if p.exists() {
            return Ok(p);
        }
    }

    if let Ok(app_data) = std::env::var("APPDATA") {
        let p = PathBuf::from(app_data)
            .join("NeuroClip")
            .join("bin")
            .join("whisper-cli.exe");
        if p.exists() {
            return Ok(p);
        }
    }

    let local_resource = PathBuf::from("resources")
        .join("sidecars")
        .join("whisper-cli.exe");
    if local_resource.exists() {
        return Ok(local_resource);
    }

    // Check system PATH
    let probe_names = if cfg!(windows) {
        vec!["whisper-cli.exe", "whisper.exe", "main.exe"]
    } else {
        vec!["whisper-cli", "whisper", "main"]
    };

    if let Ok(path_var) = std::env::var("PATH") {
        for dir in std::env::split_paths(&path_var) {
            for name in &probe_names {
                let candidate = dir.join(name);
                if candidate.exists() {
                    return Ok(candidate);
                }
            }
        }
    }

    Err(AppError::Sidecar(SidecarError::BinaryNotFound {
        name: "whisper-cli".to_string(),
        path: "whisper-cli.exe".to_string(),
    }))
}

/// Run Whisper.cpp transcription on a 16kHz mono WAV audio file.
///
/// Features:
/// - Windows Job Object supervision (`assign_child_to_job_guard`)
/// - `kill_on_drop(true)` on spawned child process
/// - Streaming progress event emission
/// - Deterministic mock runner mode for offline test environments
pub async fn run_whisper_transcription(
    clip_id: &str,
    audio_path: &Path,
    config: &WhisperSidecarConfig,
    on_progress: Option<ProgressCallback>,
) -> Result<TranscriptSegmentsContainer, AppError> {
    // Check for simulated mock error
    if let Ok(err_msg) = std::env::var("NEUROCLIP_MOCK_WHISPER_ERROR") {
        return Err(AppError::Stt(SttError::SidecarExecutionFailed {
            details: err_msg,
        }));
    }

    // Check for simulated mock output
    if let Ok(mock_json_path) = std::env::var("NEUROCLIP_MOCK_WHISPER_OUTPUT") {
        let mock_file = PathBuf::from(mock_json_path.trim());
        let mock_content = std::fs::read_to_string(&mock_file).map_err(|e| {
            AppError::Stt(SttError::ParserFailed(format!(
                "Failed reading mock whisper output {}: {e}",
                mock_file.display()
            )))
        })?;

        // Emit simulated progress
        if let Some(ref cb) = on_progress {
            cb(SttProgressPayload {
                task_id: clip_id.to_string(),
                stage: "transcribing".to_string(),
                percent: 50.0,
                eta_sec: Some(2.0),
                current_chunk: 1,
                total_chunks: 1,
            });
            cb(SttProgressPayload {
                task_id: clip_id.to_string(),
                stage: "completed".to_string(),
                percent: 100.0,
                eta_sec: Some(0.0),
                current_chunk: 1,
                total_chunks: 1,
            });
        }

        return parse_whisper_json_full(&mock_content, clip_id);
    }

    // Verify audio file exists
    if !audio_path.exists() {
        return Err(AppError::Stt(SttError::VadProcessingFailed(format!(
            "Audio file not found: {}",
            audio_path.display()
        ))));
    }

    // Resolve binary path
    let binary = match &config.binary_path {
        Some(p) if p.exists() => p.clone(),
        _ => resolve_whisper_binary()?,
    };

    let temp_dir = std::env::temp_dir();
    let output_prefix = temp_dir.join(format!("whisper_out_{clip_id}"));
    let output_json = temp_dir.join(format!("whisper_out_{clip_id}.json"));

    let mut args = vec![
        "-m".to_string(),
        config.model_path.to_string_lossy().to_string(),
        "-f".to_string(),
        audio_path.to_string_lossy().to_string(),
        "--output-json-full".to_string(),
        "-of".to_string(),
        output_prefix.to_string_lossy().to_string(),
        "--max-len".to_string(),
        "0".to_string(),
        "--word-thold".to_string(),
        "0.01".to_string(),
        "--threads".to_string(),
        config.threads.to_string(),
        "--language".to_string(),
        config.language.clone(),
        "--no-prints".to_string(),
    ];

    if let Some(prompt_str) = &config.prompt {
        args.push("--prompt".to_string());
        args.push(prompt_str.clone());
    }

    let mut cmd = tokio::process::Command::new(&binary);
    cmd.args(&args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);

    let mut child = cmd.spawn().map_err(|e| {
        AppError::Sidecar(SidecarError::ExecutionFailed {
            binary: binary.display().to_string(),
            exit_code: None,
            details: format!("Failed spawning whisper-cli: {e}"),
        })
    })?;

    // Attach to Windows Job Object immediately to guarantee zero zombies (`ADR-007`)
    assign_child_to_job_guard(&child);

    // Read stderr for progress reports
    if let Some(stderr) = child.stderr.take() {
        let callback = on_progress.clone();
        let clip_id_owned = clip_id.to_string();

        tokio::spawn(async move {
            let reader = BufReader::new(stderr);
            let mut lines = reader.lines();

            while let Ok(Some(line)) = lines.next_line().await {
                let trimmed = line.trim();
                // Whisper.cpp progress patterns (e.g. "whisper_print_progress_callback: ... 45%")
                if let Some(pct_idx) = trimmed.find('%') {
                    let prefix = &trimmed[..pct_idx];
                    if let Some(start_num) = prefix.rfind(|c: char| !c.is_ascii_digit()) {
                        let num_str = &prefix[start_num + 1..];
                        if let Ok(pct) = num_str.parse::<f64>() {
                            if let Some(ref cb) = callback {
                                cb(SttProgressPayload {
                                    task_id: clip_id_owned.clone(),
                                    stage: "transcribing".to_string(),
                                    percent: pct.clamp(0.0, 100.0),
                                    eta_sec: None,
                                    current_chunk: 1,
                                    total_chunks: 1,
                                });
                            }
                        }
                    }
                }
            }
        });
    }

    let status = child.wait().await.map_err(|e| {
        AppError::Sidecar(SidecarError::ExecutionFailed {
            binary: binary.display().to_string(),
            exit_code: None,
            details: format!("Whisper process wait error: {e}"),
        })
    })?;

    if !status.success() {
        return Err(AppError::Sidecar(SidecarError::ExecutionFailed {
            binary: binary.display().to_string(),
            exit_code: status.code(),
            details: format!("Whisper.cpp process exited with status: {status}"),
        }));
    }

    if !output_json.exists() {
        return Err(AppError::Stt(SttError::ParserFailed(format!(
            "Whisper JSON output file was not produced at: {}",
            output_json.display()
        ))));
    }

    let json_content = std::fs::read_to_string(&output_json).map_err(|e| {
        AppError::Stt(SttError::ParserFailed(format!(
            "Failed reading Whisper JSON output {}: {e}",
            output_json.display()
        )))
    })?;

    // Cleanup temp JSON file
    let _ = std::fs::remove_file(&output_json);

    let container = parse_whisper_json_full(&json_content, clip_id)?;

    if let Some(ref cb) = on_progress {
        cb(SttProgressPayload {
            task_id: clip_id.to_string(),
            stage: "completed".to_string(),
            percent: 100.0,
            eta_sec: Some(0.0),
            current_chunk: 1,
            total_chunks: 1,
        });
    }

    Ok(container)
}
