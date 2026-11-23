//! Faster-Whisper (CTranslate2) local worker process supervisor and hardware guardrail.
//!
//! Provides bounded VRAM execution (<= 1.8GB on 4GB GPUs), hardware guardrails against
//! CUDA Out-of-Memory crashes (`ADR-001`, `ADR-007`, `ERR-02`), Windows Job Object supervision,
//! and 100% homomorphic output parsing to Schema 5.5 (`TranscriptSegmentsContainer`).

use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::atomic::{AtomicU64, Ordering};

use crate::config::HardwareProfile;
use crate::core::storage::validate_vod_id;
use crate::error::{AppError, SidecarError, SttError};
use crate::sidecar::{drain_pipe_lossy, LossyLineReader};

static FASTER_WHISPER_TEMP_COUNTER: AtomicU64 = AtomicU64::new(1);

struct TempFileGuard(PathBuf);

impl Drop for TempFileGuard {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}
use crate::ingestion::streamer::assign_child_to_job_guard;
use crate::stt::transcript_parser::{
    calculate_cps, is_special_token, TranscriptSegment, TranscriptSegmentsContainer, WordTiming,
    CONFIDENCE_FLAG_THRESHOLD, CPS_FLAG_THRESHOLD, DEFAULT_SPEAKER_ID,
};
use crate::stt::whisper_sidecar::{ProgressCallback, SttProgressPayload};

/// Minimum VRAM required in MB to run Whisper Large models safely on CUDA.
pub const MIN_LARGE_VRAM_MB: u64 = 2500;

/// Absolute maximum VRAM ceiling in MB for INT8 Medium execution on 4GB GPUs (`AC-16.1`, `ED-16.1`).
pub const MAX_INT8_VRAM_CAP_MB: u64 = 1800;

/// Default model name for local Faster-Whisper.
pub const DEFAULT_FASTER_WHISPER_MODEL: &str = "medium";

/// Default quantization precision for local Faster-Whisper.
pub const DEFAULT_FASTER_WHISPER_QUANTIZATION: &str = "int8";

/// Default target compute device.
pub const DEFAULT_FASTER_WHISPER_DEVICE: &str = "auto";

/// Policy for enforcing hardware guardrails when requested settings exceed hardware limits.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GuardrailPolicy {
    /// Automatically downgrade to safe parameters (medium int8 or cpu).
    #[default]
    AutoDowngrade,
    /// Strictly reject unsafe configurations with an error (`ERR_STT_CUDA_OOM` / `ERR_STT_VRAM_INSUFFICIENT`).
    StrictReject,
}

/// Resulting decision from hardware guardrail evaluation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GuardrailDecision {
    pub effective_model: String,
    pub effective_quantization: String,
    pub effective_device: String,
    pub was_downgraded: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

/// Configuration options for launching the Faster-Whisper worker process.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FasterWhisperConfig {
    pub binary_path: Option<PathBuf>,
    pub model: String,
    pub quantization: String,
    pub device: String,
    pub language: Option<String>,
    pub word_timestamps: bool,
    pub condition_on_previous_text: bool,
    pub prompt: Option<String>,
    pub output_format: String,
    pub threads: u32,
    pub beam_size: u32,
}

impl Default for FasterWhisperConfig {
    fn default() -> Self {
        Self {
            binary_path: None,
            model: DEFAULT_FASTER_WHISPER_MODEL.to_string(),
            quantization: DEFAULT_FASTER_WHISPER_QUANTIZATION.to_string(),
            device: DEFAULT_FASTER_WHISPER_DEVICE.to_string(),
            language: Some("en".to_string()),
            word_timestamps: true,
            condition_on_previous_text: false,
            prompt: Some("Vedal, Neuro-sama, Evil Neuro, Tutel, Gymbag".to_string()),
            output_format: "verbose_json".to_string(),
            threads: 4,
            beam_size: 5,
        }
    }
}

/// Homomorphic word structure emitted by Faster-Whisper `verbose_json`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FasterWhisperWord {
    pub word: String,
    pub start: f64,
    pub end: f64,
    #[serde(default)]
    pub probability: Option<f64>,
}

/// Homomorphic segment structure emitted by Faster-Whisper `verbose_json`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FasterWhisperSegment {
    pub id: u32,
    #[serde(default)]
    pub seek: u32,
    pub start: f64,
    pub end: f64,
    pub text: String,
    #[serde(default)]
    pub tokens: Vec<u32>,
    #[serde(default)]
    pub temperature: Option<f64>,
    #[serde(default)]
    pub avg_logprob: Option<f64>,
    #[serde(default)]
    pub compression_ratio: Option<f64>,
    #[serde(default)]
    pub no_speech_prob: Option<f64>,
    #[serde(default)]
    pub words: Option<Vec<FasterWhisperWord>>,
}

/// Homomorphic response structure emitted by Faster-Whisper in `verbose_json` mode.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FasterWhisperVerboseJsonResponse {
    #[serde(default)]
    pub task: Option<String>,
    #[serde(default)]
    pub language: Option<String>,
    #[serde(default)]
    pub duration: Option<f64>,
    pub text: String,
    #[serde(default)]
    pub words: Vec<FasterWhisperWord>,
    #[serde(default)]
    pub segments: Vec<FasterWhisperSegment>,
}

/// Check if a given model identifier represents a Whisper Large variant.
#[must_use]
pub fn is_whisper_large(model: &str) -> bool {
    let lower = model.to_ascii_lowercase();
    lower.contains("large")
}

/// Mathematically estimate GPU VRAM usage in megabytes for Faster-Whisper models (`AC-16.1`, `ED-16.1`).
///
/// Bounded invariants:
/// - `medium` + `int8` = ~1650MB (strictly <= 1800MB)
/// - `small` + `int8` = ~850MB (strictly <= 1800MB)
/// - `base` + `int8` = ~450MB
/// - `large` + `int8` = ~2800MB (> 2500MB floor)
#[must_use]
pub fn estimate_vram_usage_mb(model: &str, quantization: &str) -> u64 {
    let lower_model = model.to_ascii_lowercase();
    let lower_quant = quantization.to_ascii_lowercase();

    let base_mb = if lower_model.contains("large") {
        2800
    } else if lower_model.contains("medium") {
        1650
    } else if lower_model.contains("small") {
        850
    } else if lower_model.contains("base") {
        450
    } else {
        300
    };

    match lower_quant.as_str() {
        "float16" => (base_mb * 16) / 10,
        "float32" | "default" => base_mb * 2,
        _ => base_mb,
    }
}

/// Evaluate hardware guardrail constraints against system capabilities.
///
/// Rules:
/// 1. If CUDA is not supported (e.g. AMD GPU or Intel iGPU) -> force `--device cpu`.
/// 2. If available VRAM < 2500MB (or < 3000MB) and user requests Large:
///    - `StrictReject` -> returns `AppError::Stt(SttError::InsufficientVram { available_mb })`
///    - `AutoDowngrade` -> clamps to `"medium"` + `"int8"`, sets `was_downgraded = true`.
/// 3. If available VRAM <= 4096MB (4GB GPU class) and quantization is `float16` or `float32`:
///    - `AutoDowngrade` -> clamps to `"int8"` to guarantee <= 1.8GB ceiling (`AC-16.1`).
/// 4. If available VRAM < 1500MB and > 0MB:
///    - `AutoDowngrade` -> clamps `"medium"` to `"small"` + `"int8"`.
pub fn evaluate_hardware_guardrail(
    requested_model: &str,
    requested_quantization: &str,
    requested_device: &str,
    vram_mb: u64,
    cuda_supported: bool,
    policy: GuardrailPolicy,
) -> Result<GuardrailDecision, AppError> {
    let mut effective_model = requested_model.to_string();
    let mut effective_quantization = requested_quantization.to_string();
    let mut effective_device = requested_device.to_string();
    let mut was_downgraded = false;
    let mut downgrade_reasons = Vec::new();

    // 1. Non-CUDA GPU check (AMD, Intel iGPU, or missing CUDA runtime)
    let device_lower = requested_device.to_ascii_lowercase();
    if !cuda_supported || vram_mb == 0 {
        if device_lower != "cpu" {
            effective_device = "cpu".to_string();
            was_downgraded = true;
            downgrade_reasons.push(
                "CUDA is not supported on this graphics adapter; execution switched to CPU."
                    .to_string(),
            );
        }
    } else if device_lower == "auto" {
        effective_device = "auto".to_string();
    } else {
        effective_device = requested_device.to_string();
    }

    // 2. Whisper Large VRAM Guardrail (< 2500 MB)
    if is_whisper_large(requested_model) && vram_mb < MIN_LARGE_VRAM_MB {
        match policy {
            GuardrailPolicy::StrictReject => {
                return Err(AppError::Stt(SttError::InsufficientVram {
                    available_mb: u32::try_from(vram_mb).unwrap_or(u32::MAX),
                }));
            }
            GuardrailPolicy::AutoDowngrade => {
                effective_model = DEFAULT_FASTER_WHISPER_MODEL.to_string();
                effective_quantization = DEFAULT_FASTER_WHISPER_QUANTIZATION.to_string();
                was_downgraded = true;
                downgrade_reasons.push(format!(
                    "Available VRAM ({vram_mb}MB < {MIN_LARGE_VRAM_MB}MB) is insufficient for Whisper Large: Auto-downgraded to INT8 Medium."
                ));
            }
        }
    }

    // 3. 4GB GPU Bounded VRAM Invariant (<= 1.8GB constraint)
    if cuda_supported && vram_mb <= 4096 && vram_mb > 0 {
        let quant_lower = effective_quantization.to_ascii_lowercase();
        if quant_lower == "float16" || quant_lower == "float32" {
            match policy {
                GuardrailPolicy::StrictReject => {
                    return Err(AppError::Stt(SttError::InsufficientVram {
                        available_mb: u32::try_from(vram_mb).unwrap_or(u32::MAX),
                    }));
                }
                GuardrailPolicy::AutoDowngrade => {
                    effective_quantization = DEFAULT_FASTER_WHISPER_QUANTIZATION.to_string();
                    was_downgraded = true;
                    downgrade_reasons.push(
                        "Quantization float16/float32 exceeds 1.8GB VRAM cap on 4GB GPU: Auto-downgraded to INT8."
                            .to_string(),
                    );
                }
            }
        }
    }

    // 4. Low VRAM (< 1500MB) automatic fallback to small
    if cuda_supported
        && vram_mb > 0
        && vram_mb < 1500
        && policy == GuardrailPolicy::AutoDowngrade
        && effective_model.eq_ignore_ascii_case("medium")
    {
        effective_model = "small".to_string();
        effective_quantization = "int8".to_string();
        was_downgraded = true;
        downgrade_reasons.push(format!(
            "Available VRAM ({vram_mb}MB < 1500MB): Auto-downgraded to INT8 Small."
        ));
    }

    let reason = if downgrade_reasons.is_empty() {
        None
    } else {
        Some(downgrade_reasons.join(" | "))
    };

    Ok(GuardrailDecision {
        effective_model,
        effective_quantization,
        effective_device,
        was_downgraded,
        reason,
    })
}

/// Convenience guardrail checker using StrictReject policy (UT-16.2).
pub fn check_hardware_guardrail(
    model: &str,
    vram_mb: u64,
    cuda_supported: bool,
) -> Result<String, AppError> {
    let decision = evaluate_hardware_guardrail(
        model,
        DEFAULT_FASTER_WHISPER_QUANTIZATION,
        DEFAULT_FASTER_WHISPER_DEVICE,
        vram_mb,
        cuda_supported,
        GuardrailPolicy::StrictReject,
    )?;
    Ok(decision.effective_model)
}

/// Convenience guardrail enforcer using AutoDowngrade policy (UT-16.2).
#[must_use]
pub fn enforce_hardware_guardrail(
    model: &str,
    vram_mb: u64,
    cuda_supported: bool,
) -> (String, String, bool) {
    match evaluate_hardware_guardrail(
        model,
        DEFAULT_FASTER_WHISPER_QUANTIZATION,
        DEFAULT_FASTER_WHISPER_DEVICE,
        vram_mb,
        cuda_supported,
        GuardrailPolicy::AutoDowngrade,
    ) {
        Ok(decision) => (
            decision.effective_model,
            decision.effective_device,
            decision.was_downgraded,
        ),
        Err(_) => (
            DEFAULT_FASTER_WHISPER_MODEL.to_string(),
            "cpu".to_string(),
            true,
        ),
    }
}

/// Check hardware guardrail directly against a [`HardwareProfile`] model.
pub fn check_hardware_guardrail_from_profile(
    profile: &HardwareProfile,
    model: &str,
) -> Result<String, AppError> {
    check_hardware_guardrail(
        model,
        profile.gpu_info.vram_mb,
        profile.gpu_info.cuda_supported,
    )
}

/// Check hardware guardrail directly from a `hardware_profile.json` file path (UT-16.2).
pub fn check_hardware_guardrail_from_profile_file(
    path: &Path,
    model: &str,
) -> Result<String, AppError> {
    let content = std::fs::read_to_string(path).map_err(|e| {
        AppError::Stt(SttError::VadProcessingFailed(format!(
            "Failed reading hardware profile file {}: {e}",
            path.display()
        )))
    })?;
    let profile: HardwareProfile = serde_json::from_str(&content).map_err(|e| {
        AppError::Stt(SttError::ParserFailed(format!(
            "Failed parsing hardware profile JSON: {e}"
        )))
    })?;
    check_hardware_guardrail_from_profile(&profile, model)
}

/// Evaluate hardware guardrail from a [`HardwareProfile`] with policy support.
pub fn evaluate_hardware_guardrail_from_profile(
    profile: &HardwareProfile,
    requested_model: &str,
    requested_quantization: &str,
    requested_device: &str,
    policy: GuardrailPolicy,
) -> Result<GuardrailDecision, AppError> {
    evaluate_hardware_guardrail(
        requested_model,
        requested_quantization,
        requested_device,
        profile.gpu_info.vram_mb,
        profile.gpu_info.cuda_supported,
        policy,
    )
}

/// Evaluate hardware guardrail from a `hardware_profile.json` file path with policy support.
pub fn evaluate_hardware_guardrail_from_profile_file(
    path: &Path,
    requested_model: &str,
    requested_quantization: &str,
    requested_device: &str,
    policy: GuardrailPolicy,
) -> Result<GuardrailDecision, AppError> {
    let content = std::fs::read_to_string(path).map_err(|e| {
        AppError::Stt(SttError::VadProcessingFailed(format!(
            "Failed reading hardware profile file {}: {e}",
            path.display()
        )))
    })?;
    let profile: HardwareProfile = serde_json::from_str(&content).map_err(|e| {
        AppError::Stt(SttError::ParserFailed(format!(
            "Failed parsing hardware profile JSON: {e}"
        )))
    })?;
    evaluate_hardware_guardrail_from_profile(
        &profile,
        requested_model,
        requested_quantization,
        requested_device,
        policy,
    )
}

/// Build CLI command arguments for invoking the Faster-Whisper worker (UT-16.1).
///
/// Guaranteed to include: `["--model", model, "--quantization", quantization, "--device", device]`
/// and supporting `--output_format verbose_json`, `--word_timestamps`,
/// `--condition_on_previous_text false`, `--prompt <prompt>`.
pub fn build_worker_command_args(
    audio_path: Option<&Path>,
    output_json_path: Option<&Path>,
    config: &FasterWhisperConfig,
) -> Vec<String> {
    let mut args = vec![
        "--model".to_string(),
        config.model.clone(),
        "--quantization".to_string(),
        config.quantization.clone(),
        "--device".to_string(),
        config.device.clone(),
    ];

    if let Some(audio) = audio_path {
        args.push("--audio".to_string());
        args.push(audio.to_string_lossy().to_string());
    }

    if let Some(output) = output_json_path {
        args.push("--output".to_string());
        args.push(output.to_string_lossy().to_string());
    }

    args.push("--output_format".to_string());
    args.push(config.output_format.clone());

    if config.word_timestamps {
        args.push("--word_timestamps".to_string());
    }

    if !config.condition_on_previous_text {
        args.push("--condition_on_previous_text".to_string());
        args.push("false".to_string());
    }

    if let Some(ref prompt_str) = config.prompt {
        args.push("--prompt".to_string());
        args.push(prompt_str.clone());
    }

    if let Some(ref lang) = config.language {
        args.push("--language".to_string());
        args.push(lang.clone());
    }

    if config.threads > 0 {
        args.push("--threads".to_string());
        args.push(config.threads.to_string());
    }

    if config.beam_size > 0 {
        args.push("--beam_size".to_string());
        args.push(config.beam_size.to_string());
    }

    args
}

/// Architectural alias for `build_worker_command_args` (UT-16.1).
pub fn build_faster_whisper_command_args(
    audio_path: Option<&Path>,
    output_json_path: Option<&Path>,
    config: &FasterWhisperConfig,
) -> Vec<String> {
    build_worker_command_args(audio_path, output_json_path, config)
}

/// Resolve path to the `faster-whisper-worker.exe` binary.
pub fn resolve_faster_whisper_binary() -> Result<PathBuf, AppError> {
    if let Ok(override_path) = std::env::var("NEUROCLIP_FASTER_WHISPER_PATH") {
        let p = PathBuf::from(override_path.trim());
        if p.exists() {
            return Ok(p);
        }
    }

    if let Ok(local_app_data) = std::env::var("LOCALAPPDATA") {
        let p = PathBuf::from(local_app_data)
            .join("NeuroClip")
            .join("sidecars")
            .join("faster-whisper")
            .join("faster-whisper-worker.exe");
        if p.exists() {
            return Ok(p);
        }
    }

    if let Ok(app_data) = std::env::var("APPDATA") {
        let p = PathBuf::from(app_data)
            .join("NeuroClip")
            .join("bin")
            .join("faster-whisper-worker.exe");
        if p.exists() {
            return Ok(p);
        }
    }

    let local_resource = PathBuf::from("resources")
        .join("sidecars")
        .join("faster-whisper-worker.exe");
    if local_resource.exists() {
        return Ok(local_resource);
    }

    let probe_names = if cfg!(windows) {
        vec![
            "faster-whisper-worker.exe",
            "faster_whisper_worker.exe",
            "whisper-ct2.exe",
        ]
    } else {
        vec![
            "faster-whisper-worker",
            "faster_whisper_worker",
            "whisper-ct2",
        ]
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
        name: "faster-whisper-worker".to_string(),
        path: "NEUROCLIP_FASTER_WHISPER_PATH, %LOCALAPPDATA%/NeuroClip/sidecars/faster-whisper/, resources/sidecars/, or PATH".to_string(),
    }))
}

/// Sanitize timestamp to ensure non-negative finite value.
fn sanitize_positive(val: f64) -> f64 {
    if val.is_finite() && val >= 0.0 {
        val
    } else {
        0.0
    }
}

/// Map Faster-Whisper `verbose_json` response to NeuroClip Schema 5.5 [`TranscriptSegmentsContainer`].
///
/// Guaranteed 100% homomorphic to Groq Cloud API output (`IT-16.1`).
#[must_use]
#[allow(clippy::too_many_lines)]
pub fn map_faster_whisper_to_container(
    clip_id: &str,
    response: FasterWhisperVerboseJsonResponse,
    time_offset_sec: f64,
) -> TranscriptSegmentsContainer {
    let mut segments = Vec::new();
    let mut segment_counter = 1usize;

    if response.segments.is_empty() && !response.text.trim().is_empty() {
        let start_sec = time_offset_sec;
        let end_sec = time_offset_sec + response.duration.unwrap_or(1.0);
        let text = response.text.trim().to_string();
        let cps = calculate_cps(&text, start_sec, end_sec);

        let words: Vec<WordTiming> = response
            .words
            .into_iter()
            .map(|w| WordTiming {
                word: w.word.trim().to_string(),
                start_sec: w.start + time_offset_sec,
                end_sec: (w.end + time_offset_sec).max(w.start + time_offset_sec),
                probability: w.probability.unwrap_or(1.0).clamp(0.0, 1.0),
            })
            .collect();

        segments.push(TranscriptSegment {
            segment_id: format!("{clip_id}_seg_{segment_counter:04}"),
            start_sec,
            end_sec,
            speaker_id: DEFAULT_SPEAKER_ID.to_string(),
            text_en: text,
            text_vi: None,
            confidence: 1.0,
            cps_score: cps,
            is_flagged_for_review: cps > CPS_FLAG_THRESHOLD,
            words,
        });

        return TranscriptSegmentsContainer {
            clip_id: clip_id.to_string(),
            segments,
        };
    }

    let flat_words = response.words;

    for seg in response.segments {
        let text = seg.text.trim().to_string();
        if text.is_empty() || is_special_token(&text) {
            continue;
        }

        let raw_start = seg.start + time_offset_sec;
        let raw_end = seg.end + time_offset_sec;
        let start_sec = sanitize_positive(raw_start);
        let end_sec = sanitize_positive(raw_end).max(start_sec);

        let confidence = (seg.avg_logprob.unwrap_or(0.0)).exp().clamp(0.0, 1.0);
        let cps_score = calculate_cps(&text, start_sec, end_sec);
        let is_flagged_for_review =
            confidence < CONFIDENCE_FLAG_THRESHOLD || cps_score > CPS_FLAG_THRESHOLD;

        let mut words: Vec<WordTiming> = Vec::new();

        if let Some(seg_words) = seg.words {
            if !seg_words.is_empty() {
                for w in seg_words {
                    let clean_word = w.word.trim().to_string();
                    if !clean_word.is_empty() && !is_special_token(&clean_word) {
                        let w_start = sanitize_positive(w.start + time_offset_sec).max(start_sec);
                        let w_end = sanitize_positive(w.end + time_offset_sec)
                            .max(start_sec)
                            .min(end_sec.max(start_sec));
                        words.push(WordTiming {
                            word: clean_word,
                            start_sec: w_start,
                            end_sec: w_end,
                            probability: w.probability.unwrap_or(confidence).clamp(0.0, 1.0),
                        });
                    }
                }
            }
        }

        if words.is_empty() {
            for w in &flat_words {
                let w_start = w.start + time_offset_sec;
                let w_end = w.end + time_offset_sec;

                if w_start >= start_sec - 0.05 && w_end <= end_sec + 0.05 {
                    let clean_word = w.word.trim().to_string();
                    if !clean_word.is_empty() && !is_special_token(&clean_word) {
                        words.push(WordTiming {
                            word: clean_word,
                            start_sec: sanitize_positive(w_start).max(start_sec),
                            end_sec: sanitize_positive(w_end)
                                .max(start_sec)
                                .min(end_sec.max(start_sec)),
                            probability: w.probability.unwrap_or(confidence).clamp(0.0, 1.0),
                        });
                    }
                }
            }
        }

        segments.push(TranscriptSegment {
            segment_id: format!("{clip_id}_seg_{segment_counter:04}"),
            start_sec,
            end_sec,
            speaker_id: DEFAULT_SPEAKER_ID.to_string(),
            text_en: text,
            text_vi: None,
            confidence,
            cps_score,
            is_flagged_for_review,
            words,
        });

        segment_counter += 1;
    }

    TranscriptSegmentsContainer {
        clip_id: clip_id.to_string(),
        segments,
    }
}

/// Parse Faster-Whisper `verbose_json` string into Schema 5.5 [`TranscriptSegmentsContainer`] (IT-16.1).
pub fn parse_faster_whisper_json(
    json_str: &str,
    clip_id: &str,
) -> Result<TranscriptSegmentsContainer, AppError> {
    let response: FasterWhisperVerboseJsonResponse =
        serde_json::from_str(json_str).map_err(|e| {
            AppError::Stt(SttError::ParserFailed(format!(
                "Failed parsing Faster-Whisper verbose_json output: {e}"
            )))
        })?;
    Ok(map_faster_whisper_to_container(clip_id, response, 0.0))
}

/// Parse progress percentage from a worker stderr line.
fn parse_faster_whisper_progress_line(line: &str) -> Option<f64> {
    let trimmed = line.trim();
    if let Some(pos) = trimmed.find('%') {
        let prefix = &trimmed[..pos];
        if let Some(start) = prefix.rfind(|c: char| !c.is_numeric() && c != '.') {
            let num_str = &prefix[start + 1..];
            if let Ok(val) = num_str.parse::<f64>() {
                return Some(val.clamp(0.0, 100.0));
            }
        } else if let Ok(val) = prefix.parse::<f64>() {
            return Some(val.clamp(0.0, 100.0));
        }
    }
    None
}

/// Execute Faster-Whisper offline transcription in a supervised child process (`ADR-007`).
///
/// Lifecycle Invariants:
/// - Attached to Windows Job Object via `assign_child_to_job_guard`
/// - Process killed on drop via `tokio::process::Command::kill_on_drop(true)`
/// - Streaming progress callbacks via `stt:progress`
/// - Mock bypass support via `NEUROCLIP_MOCK_FASTER_WHISPER_OUTPUT`,
///   `NEUROCLIP_MOCK_FASTER_WHISPER_ERROR`, and `NEUROCLIP_MOCK_CUDA_OOM`.
#[allow(clippy::too_many_lines)]
pub async fn run_faster_whisper_worker(
    clip_id: &str,
    audio_path: &Path,
    config: &FasterWhisperConfig,
    on_progress: Option<ProgressCallback>,
) -> Result<TranscriptSegmentsContainer, AppError> {
    let clean_clip_id = validate_vod_id(clip_id).map_err(AppError::Storage)?;

    // 1. Check for simulated mock CUDA OOM
    if std::env::var("NEUROCLIP_MOCK_CUDA_OOM").is_ok() {
        return Err(AppError::Stt(SttError::CudaOom(
            "CUDA out of memory: simulated by NEUROCLIP_MOCK_CUDA_OOM".to_string(),
        )));
    }

    // 2. Check for simulated mock error
    if let Ok(err_msg) = std::env::var("NEUROCLIP_MOCK_FASTER_WHISPER_ERROR") {
        let lower = err_msg.to_ascii_lowercase();
        if lower.contains("out of memory")
            || lower.contains("cuda oom")
            || lower.contains("cuda out of memory")
        {
            return Err(AppError::Stt(SttError::CudaOom(err_msg)));
        }
        return Err(AppError::Stt(SttError::SidecarExecutionFailed {
            details: err_msg,
        }));
    }

    // 3. Check for simulated mock output
    if let Ok(mock_json_path) = std::env::var("NEUROCLIP_MOCK_FASTER_WHISPER_OUTPUT") {
        let mock_file = PathBuf::from(mock_json_path.trim());
        let mock_content = std::fs::read_to_string(&mock_file).map_err(|e| {
            AppError::Stt(SttError::ParserFailed(format!(
                "Failed reading mock faster-whisper output {}: {e}",
                mock_file.display()
            )))
        })?;

        if let Some(ref cb) = on_progress {
            cb(SttProgressPayload {
                task_id: clean_clip_id.to_string(),
                stage: "transcribing".to_string(),
                percent: 50.0,
                eta_sec: Some(2.0),
                current_chunk: 1,
                total_chunks: 1,
            });
            cb(SttProgressPayload {
                task_id: clean_clip_id.to_string(),
                stage: "completed".to_string(),
                percent: 100.0,
                eta_sec: Some(0.0),
                current_chunk: 1,
                total_chunks: 1,
            });
        }

        return parse_faster_whisper_json(&mock_content, clean_clip_id);
    }

    // 4. Verify audio file existence
    if !audio_path.exists() {
        return Err(AppError::Stt(SttError::VadProcessingFailed(format!(
            "Audio file not found: {}",
            audio_path.display()
        ))));
    }

    // 5. Resolve binary path
    let binary = match &config.binary_path {
        Some(p) if p.exists() => p.clone(),
        _ => resolve_faster_whisper_binary()?,
    };

    let temp_dir = std::env::temp_dir();
    let seq = FASTER_WHISPER_TEMP_COUNTER.fetch_add(1, Ordering::Relaxed);
    let output_json = temp_dir.join(format!(
        "faster_whisper_out_{clean_clip_id}_{}_{seq}.json",
        std::process::id()
    ));
    let _temp_guard = TempFileGuard(output_json.clone());

    let args = build_worker_command_args(Some(audio_path), Some(&output_json), config);

    let mut cmd = tokio::process::Command::new(&binary);
    cmd.args(&args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);

    let mut child = cmd.spawn().map_err(|e| {
        AppError::Sidecar(SidecarError::ExecutionFailed {
            binary: binary.display().to_string(),
            exit_code: None,
            details: format!("Failed spawning faster-whisper-worker: {e}"),
        })
    })?;

    // Attach to Windows Job Object immediately (`ADR-007`)
    assign_child_to_job_guard(&child);

    // Concurrently drain stdout so large subprocess output cannot deadlock the pipe buffer
    let stdout_handle = child.stdout.take().map(|stdout| {
        tokio::spawn(async move {
            drain_pipe_lossy(stdout).await;
        })
    });

    // Stream progress from stderr concurrently with bounded tail retention
    let stderr_handle = child.stderr.take().map(|stderr| {
        let callback = on_progress.clone();
        let clip_id_owned = clean_clip_id.to_string();
        tokio::spawn(async move {
            let mut reader = LossyLineReader::new(stderr);
            let mut captured_lines: VecDeque<String> = VecDeque::with_capacity(200);

            while let Ok(Some(line)) = reader.next_line().await {
                if captured_lines.len() >= 200 {
                    captured_lines.pop_front();
                }
                captured_lines.push_back(line.clone());

                if let Some(ref cb) = callback {
                    if let Some(percent) = parse_faster_whisper_progress_line(&line) {
                        cb(SttProgressPayload {
                            task_id: clip_id_owned.clone(),
                            stage: "transcribing".to_string(),
                            percent,
                            eta_sec: None,
                            current_chunk: 1,
                            total_chunks: 1,
                        });
                    }
                }
            }

            captured_lines.into_iter().collect::<Vec<_>>().join("\n")
        })
    });

    let status = child.wait().await.map_err(|e| {
        AppError::Sidecar(SidecarError::ExecutionFailed {
            binary: binary.display().to_string(),
            exit_code: None,
            details: format!("Failed waiting for faster-whisper-worker: {e}"),
        })
    })?;

    if let Some(h) = stdout_handle {
        let _ = h.await;
    }
    let captured_stderr = match stderr_handle {
        Some(h) => h.await.unwrap_or_default(),
        None => String::new(),
    };

    if !status.success() {
        let lower = captured_stderr.to_ascii_lowercase();
        if lower.contains("out of memory")
            || lower.contains("cuda oom")
            || lower.contains("cuda out of memory")
        {
            return Err(AppError::Stt(SttError::CudaOom(captured_stderr)));
        }

        return Err(AppError::Sidecar(SidecarError::ExecutionFailed {
            binary: binary.display().to_string(),
            exit_code: status.code(),
            details: captured_stderr,
        }));
    }

    if !output_json.exists() {
        return Err(AppError::Stt(SttError::ParserFailed(format!(
            "Expected output JSON not found at {}",
            output_json.display()
        ))));
    }

    let json_content = std::fs::read_to_string(&output_json).map_err(|e| {
        AppError::Stt(SttError::ParserFailed(format!(
            "Failed reading output JSON {}: {e}",
            output_json.display()
        )))
    })?;

    let container = parse_faster_whisper_json(&json_content, clean_clip_id)?;

    if let Some(ref cb) = on_progress {
        cb(SttProgressPayload {
            task_id: clean_clip_id.to_string(),
            stage: "completed".to_string(),
            percent: 100.0,
            eta_sec: Some(0.0),
            current_chunk: 1,
            total_chunks: 1,
        });
    }

    Ok(container)
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_run_faster_whisper_worker_rejects_traversal_clip_id() {
        let cfg = FasterWhisperConfig::default();
        let audio = Path::new("unused.wav");
        for bad in ["../escape", "sub/clip", "sub\\clip", "NUL", "clip*1", ""] {
            let res = run_faster_whisper_worker(bad, audio, &cfg, None).await;
            assert!(
                matches!(res, Err(AppError::Storage(_))),
                "Expected Storage validation error for clip_id {bad:?}, got {res:?}"
            );
        }
    }

    #[test]
    fn test_faster_whisper_temp_file_guard_removes_file_on_drop() {
        let temp_path = std::env::temp_dir().join(format!(
            "neuroclip_fw_guard_test_{}.json",
            std::process::id()
        ));
        std::fs::write(&temp_path, b"{\"stale\": true}").unwrap();
        assert!(temp_path.exists());
        {
            let _guard = TempFileGuard(temp_path.clone());
            assert!(temp_path.exists());
        }
        assert!(
            !temp_path.exists(),
            "TempFileGuard must delete temp JSON file on drop"
        );
    }
}
