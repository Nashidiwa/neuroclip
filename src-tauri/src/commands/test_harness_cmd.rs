//! End-to-End Synthetic Pipeline Test Harness & Schema Validator (`TASK-M4-30`).
//!
//! Orchestrates an offline, deterministic 6-stage synthetic VOD pipeline inside an isolated
//! temporary workspace (without network calls or production `%APPDATA%`/`%LOCALAPPDATA%` writes):
//! 1. Hardware Probe (`Schema 5.1`), Synthetic Chat (`Schema 5.3`), VOD Manifest (`Schema 5.2`).
//! 2. Multimodal Highlight Detection (`Schema 5.4`, 4.5s latency compensation).
//! 3. Lossless Range Slice + Screaming Defense Limiter (`ERR-08`) + Silero VAD (`tract-onnx`).
//! 4. Mock Groq STT + Mock Gemini Vietsub Translation (`Theatrical Framing`, `BLOCK_NONE`, Lore Dict, Hybrid Diarization).
//! 5. Vietnamese Smart Compound Line Wrapping + CPS Time Borrowing Optimization (`Schema 5.5`).
//! 6. Subtitle Project (`Schema 5.6`) + CapCut Draft JSON & Dual Export Safeguard (`.ass` + `.srt` UTF-8 BOM).

#![allow(
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss
)]

use std::collections::HashSet;
use std::path::{Component, Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Instant;

use serde::{Deserialize, Serialize};

use crate::config::hardware::probe_system_hardware;
use crate::core::storage::validate_vod_id;
use crate::error::{AppError, ExportError, HighlightError, IngestError, StorageError, SttError};
use crate::export::{
    create_subtitle_project, export_capcut_draft_folder, export_subtitle_project_json,
    load_subtitle_project_json, DraftContent, DraftMetaInfo, UTF8_BOM_BYTES,
};
use crate::highlight::{
    analyze_wav_file, compute_chat_velocity_series_from_file,
    compute_emote_density_series_from_file, fuse_multimodal_series,
    generate_clusters_from_fused_series, HighlightCluster, HighlightClustersContainer,
};
use crate::ingestion::{build_ffmpeg_range_slice_args, ChatEventsContainer, VodManifest};
use crate::stt::audio_prep::{
    apply_screaming_limiter, build_loudnorm_ffmpeg_args, verify_screaming_defense, AudioPrepConfig,
    PreparedAudioBuffer, TARGET_SAMPLE_RATE,
};
use crate::stt::groq_client::{
    GroqClient, GroqClientConfig, TransportResponse as GroqTransportResponse, SAFE_CHUNK_BYTES,
};
use crate::stt::transcript_parser::{
    TranscriptSegmentsContainer, CONFIDENCE_FLAG_THRESHOLD, CPS_FLAG_THRESHOLD,
};
use crate::stt::vad::{
    process_vad_pipeline_async, split_into_natural_pause_chunks, ChunkSplitConfig, VadConfig,
};
use crate::stt::vad_bridge::{align_segments_with_vad, SpeechInterval};
use crate::translation::{
    detect_forbidden_translations, format_lore_prompt_section,
    format_pronoun_matrix_prompt_section, normalize_speaker_id, optimize_container_cps,
    serialize_transcript_segments_json, wrap_vietnamese_subtitle, GeminiClient, GeminiClientConfig,
    PromptBuilder, StylePreset, TimeBorrowingConfig, TransportResponse as GeminiTransportResponse,
    BLOCK_NONE_THRESHOLD, DEFAULT_MAX_CHARS_PER_LINE, DEFAULT_MAX_LINES, SAFETY_CATEGORIES,
    THEATRICAL_FRAMING_INSTRUCTION, VERTICAL_9_16_MAX_CHARS_PER_LINE,
};

const VALID_GPU_VENDORS: &[&str] = &["NVIDIA", "AMD", "Intel", "Unknown"];
const VALID_RECOMMENDED_MODES: &[&str] = &["cloud_byok", "hybrid", "offline_local"];
const VALID_SOURCE_TYPES: &[&str] = &["twitch_vod", "youtube_vod", "local_file"];
const VALID_SPEAKER_ROLES: &[&str] = &["vedal", "neuro", "evil", "guest", "chat", "unknown"];
const VALID_TARGET_LAYOUTS: &[&str] = &[
    "original_16_9",
    "preset_center_crop_9_16",
    "preset_two_stack_9_16",
];

/// Input parameters for invoking the E2E synthetic pipeline harness.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SyntheticE2EOptions {
    #[serde(alias = "audio_fixture_path")]
    pub audio_fixture_path: String,
    #[serde(alias = "chat_fixture_path")]
    pub chat_fixture_path: String,
    #[serde(default, alias = "output_dir")]
    pub output_dir: Option<String>,
    #[serde(default, alias = "vod_id")]
    pub vod_id: Option<String>,
    #[serde(default, alias = "target_layout")]
    pub target_layout: Option<String>,
}

/// Diagnostic result for a single pipeline stage.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StageResult {
    pub stage: String,
    pub passed: bool,
    pub duration_ms: u64,
    pub details: String,
}

/// Summary verifying all 6 canonical Data Contracts (`docs/ARCHITECTURE.md` §5.1–§5.6).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
#[allow(clippy::struct_excessive_bools)]
pub struct SchemaValidationSummary {
    pub schema_5_1_hardware_profile: bool,
    pub schema_5_2_vod_manifest: bool,
    pub schema_5_3_chat_events: bool,
    pub schema_5_4_highlight_clusters: bool,
    pub schema_5_5_transcript_segments: bool,
    pub schema_5_6_subtitle_project: bool,
}

impl SchemaValidationSummary {
    /// Returns `true` when all 6 canonical Data Contracts passed strict validation.
    #[must_use]
    pub const fn all_passed(&self) -> bool {
        self.schema_5_1_hardware_profile
            && self.schema_5_2_vod_manifest
            && self.schema_5_3_chat_events
            && self.schema_5_4_highlight_clusters
            && self.schema_5_5_transcript_segments
            && self.schema_5_6_subtitle_project
    }
}

/// Complete report returned from an E2E synthetic pipeline execution.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SyntheticE2EReport {
    pub passed: bool,
    pub total_duration_ms: u64,
    pub workspace_dir: String,
    pub detected_peak_sec: f64,
    pub cluster_start_sec: f64,
    pub cluster_end_sec: f64,
    pub vad_speech_segments: usize,
    pub vad_reduction_ratio: f64,
    pub translated_segments: usize,
    pub cps_optimized_count: usize,
    pub capcut_draft_dir: String,
    pub dual_export_files: Vec<String>,
    pub schemas: SchemaValidationSummary,
    pub stages: Vec<StageResult>,
}

fn contract_err(schema_label: &str, reason: impl Into<String>) -> AppError {
    AppError::Internal(format!(
        "Data contract violation ({schema_label}): {}",
        reason.into()
    ))
}

fn require_object<'a>(
    val: &'a serde_json::Value,
    schema_label: &str,
    field_name: &str,
) -> Result<&'a serde_json::Map<String, serde_json::Value>, AppError> {
    val.as_object().ok_or_else(|| {
        contract_err(
            schema_label,
            format!("field '{field_name}' must be a JSON object"),
        )
    })
}

fn require_field<'a>(
    obj: &'a serde_json::Map<String, serde_json::Value>,
    schema_label: &str,
    field: &str,
) -> Result<&'a serde_json::Value, AppError> {
    obj.get(field)
        .ok_or_else(|| contract_err(schema_label, format!("missing required field '{field}'")))
}

fn require_str<'a>(
    obj: &'a serde_json::Map<String, serde_json::Value>,
    schema_label: &str,
    field: &str,
) -> Result<&'a str, AppError> {
    require_field(obj, schema_label, field)?
        .as_str()
        .ok_or_else(|| contract_err(schema_label, format!("field '{field}' must be a string")))
}

fn require_non_empty_str<'a>(
    obj: &'a serde_json::Map<String, serde_json::Value>,
    schema_label: &str,
    field: &str,
) -> Result<&'a str, AppError> {
    let s = require_str(obj, schema_label, field)?;
    if s.trim().is_empty() {
        return Err(contract_err(
            schema_label,
            format!("field '{field}' must be non-empty"),
        ));
    }
    Ok(s)
}

fn require_u64(
    obj: &serde_json::Map<String, serde_json::Value>,
    schema_label: &str,
    field: &str,
) -> Result<u64, AppError> {
    require_field(obj, schema_label, field)?
        .as_u64()
        .ok_or_else(|| {
            contract_err(
                schema_label,
                format!("field '{field}' must be a non-negative integer"),
            )
        })
}

fn require_f64(
    obj: &serde_json::Map<String, serde_json::Value>,
    schema_label: &str,
    field: &str,
) -> Result<f64, AppError> {
    let num = require_field(obj, schema_label, field)?
        .as_f64()
        .ok_or_else(|| {
            contract_err(
                schema_label,
                format!("field '{field}' must be a finite number"),
            )
        })?;
    if !num.is_finite() {
        return Err(contract_err(
            schema_label,
            format!("field '{field}' must be finite"),
        ));
    }
    Ok(num)
}

fn require_bool(
    obj: &serde_json::Map<String, serde_json::Value>,
    schema_label: &str,
    field: &str,
) -> Result<bool, AppError> {
    require_field(obj, schema_label, field)?
        .as_bool()
        .ok_or_else(|| contract_err(schema_label, format!("field '{field}' must be a boolean")))
}

fn check_optional_nullable_str(
    obj: &serde_json::Map<String, serde_json::Value>,
    schema_label: &str,
    field: &str,
) -> Result<(), AppError> {
    if let Some(v) = obj.get(field) {
        if !v.is_null() && !v.is_string() {
            return Err(contract_err(
                schema_label,
                format!("field '{field}' must be string or null"),
            ));
        }
    }
    Ok(())
}

fn validate_rfc3339(
    obj: &serde_json::Map<String, serde_json::Value>,
    schema_label: &str,
    field: &str,
) -> Result<(), AppError> {
    let raw = require_non_empty_str(obj, schema_label, field)?;
    chrono::DateTime::parse_from_rfc3339(raw).map_err(|e| {
        contract_err(
            schema_label,
            format!("field '{field}' is not valid RFC 3339 timestamp: {e}"),
        )
    })?;
    Ok(())
}

/// Validate JSON payload against Schema 5.1 (`hardware_profile.json`).
///
/// # Errors
/// Returns [`AppError::Internal`] if required keys, types, enums, or timestamp fail validation.
pub fn validate_schema_5_1_hardware_profile(val: &serde_json::Value) -> Result<(), AppError> {
    let label = "Schema 5.1 HardwareProfile";
    let root = require_object(val, label, "$")?;

    let cpu = require_object(require_field(root, label, "cpu_info")?, label, "cpu_info")?;
    require_non_empty_str(cpu, label, "model")?;
    let cores = require_u64(cpu, label, "cores")?;
    let threads = require_u64(cpu, label, "threads")?;
    if cores == 0 || threads == 0 {
        return Err(contract_err(label, "CPU cores and threads must be > 0"));
    }

    let ram = require_object(require_field(root, label, "ram_info")?, label, "ram_info")?;
    let total_mb = require_u64(ram, label, "total_mb")?;
    let _available_mb = require_u64(ram, label, "available_mb")?;
    if total_mb == 0 {
        return Err(contract_err(label, "RAM total_mb must be > 0"));
    }

    let gpu = require_object(require_field(root, label, "gpu_info")?, label, "gpu_info")?;
    let vendor = require_str(gpu, label, "vendor")?;
    if !VALID_GPU_VENDORS.contains(&vendor) {
        return Err(contract_err(
            label,
            format!("invalid gpu vendor '{vendor}'"),
        ));
    }
    require_non_empty_str(gpu, label, "model")?;
    let _vram_mb = require_u64(gpu, label, "vram_mb")?;
    let _cuda_supported = require_bool(gpu, label, "cuda_supported")?;

    let mode = require_str(root, label, "recommended_mode")?;
    if !VALID_RECOMMENDED_MODES.contains(&mode) {
        return Err(contract_err(
            label,
            format!("invalid recommended_mode '{mode}'"),
        ));
    }
    check_optional_nullable_str(root, label, "recommendation_reason")?;
    validate_rfc3339(root, label, "timestamp")?;

    Ok(())
}

/// Validate JSON payload against Schema 5.2 (`vod_manifest.json`).
///
/// # Errors
/// Returns [`AppError::Internal`] if required keys, enums, positive duration, or timestamp fail validation.
pub fn validate_schema_5_2_vod_manifest(val: &serde_json::Value) -> Result<(), AppError> {
    let label = "Schema 5.2 VodManifest";
    let root = require_object(val, label, "$")?;

    require_non_empty_str(root, label, "vod_id")?;
    let source_type = require_str(root, label, "source_type")?;
    if !VALID_SOURCE_TYPES.contains(&source_type) {
        return Err(contract_err(
            label,
            format!("invalid source_type '{source_type}'"),
        ));
    }
    require_non_empty_str(root, label, "title")?;
    let duration = require_f64(root, label, "duration_seconds")?;
    if duration <= 0.0 {
        return Err(contract_err(label, "duration_seconds must be > 0"));
    }
    require_non_empty_str(root, label, "audio_path")?;
    check_optional_nullable_str(root, label, "original_url")?;
    check_optional_nullable_str(root, label, "streamer")?;
    check_optional_nullable_str(root, label, "chat_file_path")?;
    check_optional_nullable_str(root, label, "video_stream_url")?;
    check_optional_nullable_str(root, label, "local_video_path")?;
    validate_rfc3339(root, label, "created_at")?;

    Ok(())
}

/// Validate JSON payload against Schema 5.3 (`chat_events.json`).
///
/// # Errors
/// Returns [`AppError::Internal`] if required fields, counts, or event structure fail validation.
pub fn validate_schema_5_3_chat_events(val: &serde_json::Value) -> Result<(), AppError> {
    let label = "Schema 5.3 ChatEventsContainer";
    let root = require_object(val, label, "$")?;

    require_non_empty_str(root, label, "vod_id")?;
    let total_messages = require_u64(root, label, "total_messages")?;
    let events = require_field(root, label, "events")?
        .as_array()
        .ok_or_else(|| contract_err(label, "field 'events' must be an array"))?;

    if total_messages != events.len() as u64 {
        return Err(contract_err(
            label,
            format!(
                "total_messages ({total_messages}) does not match events length ({})",
                events.len()
            ),
        ));
    }

    for (idx, item) in events.iter().enumerate() {
        let ev = require_object(item, label, &format!("events[{idx}]"))?;
        let ts = require_f64(ev, label, "timestamp_sec")?;
        if ts < 0.0 {
            return Err(contract_err(
                label,
                format!("events[{idx}].timestamp_sec must be >= 0"),
            ));
        }
        require_str(ev, label, "author")?;
        require_str(ev, label, "message")?;
        let emotes = require_field(ev, label, "emotes_detected")?
            .as_array()
            .ok_or_else(|| {
                contract_err(
                    label,
                    format!("events[{idx}].emotes_detected must be array"),
                )
            })?;
        for emote in emotes {
            if !emote.is_string() {
                return Err(contract_err(
                    label,
                    format!("events[{idx}].emotes_detected items must be strings"),
                ));
            }
        }
        if let Some(swarm_flag) = ev.get("is_swarm_emote") {
            if !swarm_flag.is_boolean() {
                return Err(contract_err(
                    label,
                    format!("events[{idx}].is_swarm_emote must be boolean"),
                ));
            }
        }
    }

    Ok(())
}

/// Validate JSON payload against Schema 5.4 (`highlight_clusters.json`).
///
/// # Errors
/// Returns [`AppError::Internal`] if required cluster keys, ranges, or bounds fail validation.
pub fn validate_schema_5_4_highlight_clusters(val: &serde_json::Value) -> Result<(), AppError> {
    let label = "Schema 5.4 HighlightClustersContainer";
    let root = require_object(val, label, "$")?;

    require_non_empty_str(root, label, "vod_id")?;
    let clusters = require_field(root, label, "clusters")?
        .as_array()
        .ok_or_else(|| contract_err(label, "field 'clusters' must be an array"))?;

    for (idx, item) in clusters.iter().enumerate() {
        let cl = require_object(item, label, &format!("clusters[{idx}]"))?;
        require_non_empty_str(cl, label, "cluster_id")?;
        let start_sec = require_f64(cl, label, "start_sec")?;
        let end_sec = require_f64(cl, label, "end_sec")?;
        let _peak_sec = require_f64(cl, label, "peak_sec")?;
        if start_sec < 0.0 || start_sec >= end_sec {
            return Err(contract_err(
                label,
                format!("clusters[{idx}] invalid bounds [{start_sec}, {end_sec}]"),
            ));
        }

        let score = require_f64(cl, label, "score")?;
        if !(0.0..=100.0).contains(&score) {
            return Err(contract_err(
                label,
                format!("clusters[{idx}].score {score} outside [0, 100]"),
            ));
        }
        require_non_empty_str(cl, label, "primary_reason")?;

        if let Some(z) = cl.get("chat_z_score") {
            if !z.is_number() {
                return Err(contract_err(
                    label,
                    format!("clusters[{idx}].chat_z_score must be number"),
                ));
            }
        }
        if let Some(rms) = cl.get("audio_rms_peak") {
            if !rms.is_number() {
                return Err(contract_err(
                    label,
                    format!("clusters[{idx}].audio_rms_peak must be number"),
                ));
            }
        }
        if let Some(emotes) = cl.get("top_emotes") {
            let arr = emotes.as_array().ok_or_else(|| {
                contract_err(label, format!("clusters[{idx}].top_emotes must be array"))
            })?;
            for e in arr {
                if !e.is_string() {
                    return Err(contract_err(
                        label,
                        format!("clusters[{idx}].top_emotes items must be strings"),
                    ));
                }
            }
        }
        if let Some(bm) = cl.get("is_bookmarked") {
            if !bm.is_boolean() {
                return Err(contract_err(
                    label,
                    format!("clusters[{idx}].is_bookmarked must be boolean"),
                ));
            }
        }
    }

    Ok(())
}

fn validate_transcript_segment_value(
    item: &serde_json::Value,
    schema_label: &str,
    idx: usize,
) -> Result<(), AppError> {
    let seg = require_object(item, schema_label, &format!("segments[{idx}]"))?;
    require_non_empty_str(seg, schema_label, "segment_id")?;
    let start_sec = require_f64(seg, schema_label, "start_sec")?;
    let end_sec = require_f64(seg, schema_label, "end_sec")?;
    if start_sec < 0.0 || end_sec < start_sec {
        return Err(contract_err(
            schema_label,
            format!("segments[{idx}] invalid timing [{start_sec}, {end_sec}]"),
        ));
    }

    let speaker = require_str(seg, schema_label, "speaker_id")?;
    if !VALID_SPEAKER_ROLES.contains(&speaker) {
        return Err(contract_err(
            schema_label,
            format!("segments[{idx}].speaker_id invalid '{speaker}'"),
        ));
    }

    require_str(seg, schema_label, "text_en")?;
    check_optional_nullable_str(seg, schema_label, "text_vi")?;
    let _confidence = require_f64(seg, schema_label, "confidence")?;
    let _cps_score = require_f64(seg, schema_label, "cps_score")?;
    if let Some(flagged) = seg.get("is_flagged_for_review") {
        if !flagged.is_boolean() {
            return Err(contract_err(
                schema_label,
                format!("segments[{idx}].is_flagged_for_review must be boolean"),
            ));
        }
    }

    if let Some(words) = seg.get("words") {
        let arr = words.as_array().ok_or_else(|| {
            contract_err(schema_label, format!("segments[{idx}].words must be array"))
        })?;
        for (w_idx, w_val) in arr.iter().enumerate() {
            let w = require_object(
                w_val,
                schema_label,
                &format!("segments[{idx}].words[{w_idx}]"),
            )?;
            require_str(w, schema_label, "word")?;
            let _ws = require_f64(w, schema_label, "start_sec")?;
            let _we = require_f64(w, schema_label, "end_sec")?;
            let _prob = require_f64(w, schema_label, "probability")?;
        }
    }

    Ok(())
}

/// Validate JSON payload against Schema 5.5 (`transcript_segments.json`).
///
/// # Errors
/// Returns [`AppError::Internal`] if required transcript fields, speaker enum, or timings fail validation.
pub fn validate_schema_5_5_transcript_segments(val: &serde_json::Value) -> Result<(), AppError> {
    let label = "Schema 5.5 TranscriptSegmentsContainer";
    let root = require_object(val, label, "$")?;

    require_non_empty_str(root, label, "clip_id")?;
    let segments = require_field(root, label, "segments")?
        .as_array()
        .ok_or_else(|| contract_err(label, "field 'segments' must be an array"))?;

    for (idx, item) in segments.iter().enumerate() {
        validate_transcript_segment_value(item, label, idx)?;
    }

    Ok(())
}

/// Validate JSON payload against Schema 5.6 (`subtitle_project.json`).
///
/// # Errors
/// Returns [`AppError::Internal`] if required project fields, layout enum, styles, or segments fail validation.
pub fn validate_schema_5_6_subtitle_project(val: &serde_json::Value) -> Result<(), AppError> {
    let label = "Schema 5.6 SubtitleProject";
    let root = require_object(val, label, "$")?;

    require_non_empty_str(root, label, "project_id")?;
    require_non_empty_str(root, label, "project_name")?;
    require_non_empty_str(root, label, "vod_id")?;
    check_optional_nullable_str(root, label, "video_file_path")?;

    let target_layout = require_str(root, label, "target_layout")?;
    if !VALID_TARGET_LAYOUTS.contains(&target_layout) {
        return Err(contract_err(
            label,
            format!("invalid target_layout '{target_layout}'"),
        ));
    }

    let segments = require_field(root, label, "segments")?
        .as_array()
        .ok_or_else(|| contract_err(label, "field 'segments' must be an array"))?;
    for (idx, item) in segments.iter().enumerate() {
        validate_transcript_segment_value(item, label, idx)?;
    }

    if let Some(styles_val) = root.get("speaker_styles") {
        let styles = require_object(styles_val, label, "speaker_styles")?;
        for (speaker_key, style_val) in styles {
            let style = require_object(style_val, label, &format!("speaker_styles.{speaker_key}"))?;
            require_non_empty_str(style, label, "color_hex")?;
            require_non_empty_str(style, label, "font_name")?;
            let font_size = require_u64(style, label, "font_size")?;
            if font_size == 0 {
                return Err(contract_err(
                    label,
                    format!("speaker_styles.{speaker_key}.font_size must be > 0"),
                ));
            }
            require_non_empty_str(style, label, "outline_color_hex")?;
        }
    }

    validate_rfc3339(root, label, "updated_at")?;
    Ok(())
}

fn ensure_clean_path_input(
    raw: &str,
    label: &str,
    reject_parent_dir: bool,
) -> Result<PathBuf, AppError> {
    let trimmed = raw.trim();
    if trimmed.is_empty() || trimmed.contains('\0') {
        return Err(AppError::Storage(StorageError::DirectoryCreationFailed {
            path: raw.to_string(),
            details: format!("Invalid {label}: empty or contains null byte"),
        }));
    }
    let path = PathBuf::from(trimmed);
    if reject_parent_dir && path.components().any(|c| matches!(c, Component::ParentDir)) {
        return Err(AppError::Storage(StorageError::DirectoryCreationFailed {
            path: raw.to_string(),
            details: format!("Path traversal segment ('..') rejected in {label}"),
        }));
    }
    Ok(path)
}

fn read_json_value(path: &Path) -> Result<serde_json::Value, AppError> {
    let bytes = std::fs::read(path).map_err(|e| {
        AppError::Storage(StorageError::FileNotFound(format!(
            "Failed to read JSON file '{}': {e}",
            path.display()
        )))
    })?;
    serde_json::from_slice(&bytes).map_err(|e| {
        AppError::Internal(format!(
            "Failed to parse JSON file '{}': {e}",
            path.display()
        ))
    })
}

fn write_json_pretty<T: Serialize>(path: &Path, data: &T) -> Result<(), AppError> {
    let bytes = serde_json::to_vec_pretty(data)
        .map_err(|e| AppError::Internal(format!("Failed serializing '{}': {e}", path.display())))?;
    std::fs::write(path, bytes).map_err(|e| {
        AppError::Export(ExportError::FileWriteFailed {
            path: path.display().to_string(),
            details: e.to_string(),
        })
    })
}

struct Stage1Artifacts {
    vod_id: String,
    audio_path: PathBuf,
    chat_path: PathBuf,
    duration_seconds: f64,
    audio_buffer: PreparedAudioBuffer,
    stage_result: StageResult,
}

fn run_stage_1_ingestion(
    workspace_dir: &Path,
    audio_fixture: &Path,
    chat_fixture: &Path,
    vod_id: &str,
    schemas: &mut SchemaValidationSummary,
) -> Result<Stage1Artifacts, AppError> {
    let start = Instant::now();

    let hw_profile = probe_system_hardware()?;
    let hw_path = workspace_dir.join("hardware_profile.json");
    write_json_pretty(&hw_path, &hw_profile)?;
    let hw_json = read_json_value(&hw_path)?;
    validate_schema_5_1_hardware_profile(&hw_json)?;
    schemas.schema_5_1_hardware_profile = true;

    let chat_bytes = std::fs::read(chat_fixture).map_err(|e| {
        AppError::Storage(StorageError::FileNotFound(format!(
            "Missing chat fixture '{}': {e}",
            chat_fixture.display()
        )))
    })?;
    let mut chat_container: ChatEventsContainer =
        serde_json::from_slice(&chat_bytes).map_err(|e| {
            AppError::Highlight(HighlightError::ChatParsingFailed(format!(
                "Invalid synthetic chat fixture: {e}"
            )))
        })?;
    chat_container.vod_id = vod_id.to_string();
    chat_container.total_messages = chat_container.events.len() as u64;
    let chat_path = workspace_dir.join("chat_events.json");
    write_json_pretty(&chat_path, &chat_container)?;
    let chat_json = read_json_value(&chat_path)?;
    validate_schema_5_3_chat_events(&chat_json)?;
    schemas.schema_5_3_chat_events = true;

    let audio_bytes = std::fs::read(audio_fixture).map_err(|e| {
        AppError::Storage(StorageError::FileNotFound(format!(
            "Missing audio fixture '{}': {e}",
            audio_fixture.display()
        )))
    })?;
    let audio_buffer = PreparedAudioBuffer::from_wav_bytes(&audio_bytes)?;
    let duration_seconds = audio_buffer.duration_sec;

    let copied_audio_path = workspace_dir.join("synthetic_audio.wav");
    std::fs::write(&copied_audio_path, &audio_bytes).map_err(|e| {
        AppError::Export(ExportError::FileWriteFailed {
            path: copied_audio_path.display().to_string(),
            details: e.to_string(),
        })
    })?;

    let manifest = VodManifest {
        vod_id: vod_id.to_string(),
        source_type: "local_file".to_string(),
        original_url: None,
        title: "NeuroClip E2E Synthetic VOD".to_string(),
        streamer: Some("Neuro-sama".to_string()),
        duration_seconds,
        audio_path: copied_audio_path.to_string_lossy().to_string(),
        chat_file_path: Some(chat_path.to_string_lossy().to_string()),
        video_stream_url: None,
        local_video_path: Some(copied_audio_path.to_string_lossy().to_string()),
        created_at: chrono::Utc::now().to_rfc3339(),
    };
    let manifest_path = workspace_dir.join("vod_manifest.json");
    write_json_pretty(&manifest_path, &manifest)?;
    let manifest_json = read_json_value(&manifest_path)?;
    validate_schema_5_2_vod_manifest(&manifest_json)?;
    schemas.schema_5_2_vod_manifest = true;

    let elapsed = start.elapsed().as_millis() as u64;
    Ok(Stage1Artifacts {
        vod_id: vod_id.to_string(),
        audio_path: copied_audio_path,
        chat_path,
        duration_seconds,
        audio_buffer,
        stage_result: StageResult {
            stage: "1_ingestion_and_hardware_probe".to_string(),
            passed: true,
            duration_ms: elapsed,
            details: format!(
                "Loaded {:.1}s audio & {} chat messages; Schemas 5.1/5.2/5.3 validated",
                duration_seconds, chat_container.total_messages
            ),
        },
    })
}

fn run_stage_2_highlights(
    workspace_dir: &Path,
    stage1: &Stage1Artifacts,
    schemas: &mut SchemaValidationSummary,
) -> Result<(HighlightCluster, StageResult), AppError> {
    let start = Instant::now();

    let velocity = compute_chat_velocity_series_from_file(
        &stage1.chat_path,
        5.0,
        60.0,
        Some(stage1.duration_seconds),
    )?;
    let emotes = compute_emote_density_series_from_file(
        &stage1.chat_path,
        None,
        5.0,
        Some(stage1.duration_seconds),
    )?;
    let audio_rms = analyze_wav_file(&stage1.audio_path)?;

    let fused = fuse_multimodal_series(&velocity, &emotes, &audio_rms, stage1.duration_seconds)?;
    let container: HighlightClustersContainer =
        generate_clusters_from_fused_series(&stage1.vod_id, &fused, stage1.duration_seconds)?;

    let clusters_path = workspace_dir.join("highlight_clusters.json");
    write_json_pretty(&clusters_path, &container)?;
    let clusters_json = read_json_value(&clusters_path)?;
    validate_schema_5_4_highlight_clusters(&clusters_json)?;
    schemas.schema_5_4_highlight_clusters = true;

    let top_cluster = container
        .clusters
        .first()
        .cloned()
        .ok_or(AppError::Highlight(HighlightError::NoHighlightsFound))?;

    if (top_cluster.peak_sec - 60.0).abs() > 5.0 {
        return Err(AppError::Highlight(HighlightError::ChatParsingFailed(
            format!(
                "Peak timestamp {:.1}s missed t=60s chat surge",
                top_cluster.peak_sec
            ),
        )));
    }
    if top_cluster.start_sec > 55.5 {
        return Err(AppError::Highlight(HighlightError::ChatParsingFailed(
            format!(
                "Latency compensation not applied: start_sec {:.1}s > 55.5s",
                top_cluster.start_sec
            ),
        )));
    }

    let elapsed = start.elapsed().as_millis() as u64;
    let details = format!(
        "Top cluster '{}' peak={:.1}s bounds=[{:.1}s, {:.1}s] score={:.1}",
        top_cluster.cluster_id,
        top_cluster.peak_sec,
        top_cluster.start_sec,
        top_cluster.end_sec,
        top_cluster.score
    );
    Ok((
        top_cluster,
        StageResult {
            stage: "2_multimodal_highlights".to_string(),
            passed: true,
            duration_ms: elapsed,
            details,
        },
    ))
}

struct Stage3Artifacts {
    clip_slice_path: PathBuf,
    normalized_wav_bytes: Vec<u8>,
    speech_intervals: Vec<SpeechInterval>,
    speech_segment_count: usize,
    reduction_ratio: f64,
    stage_result: StageResult,
}

fn validate_stage_3_cli_builders(
    audio_path: &Path,
    clip_slice_path: &Path,
    clip_norm_path: &Path,
    cluster: &HighlightCluster,
    prep_cfg: &AudioPrepConfig,
) -> Result<(), AppError> {
    let slice_args = build_ffmpeg_range_slice_args(
        &audio_path.to_string_lossy(),
        cluster.start_sec,
        cluster.end_sec,
        &clip_slice_path.to_string_lossy(),
        false,
    )?;
    if !slice_args.contains(&"-avoid_negative_ts".to_string()) {
        return Err(AppError::Ingestion(IngestError::SliceFailed(
            "Range slice args missing PTS normalization".to_string(),
        )));
    }
    let loudnorm_args = build_loudnorm_ffmpeg_args(clip_slice_path, clip_norm_path, prep_cfg);
    if !loudnorm_args
        .iter()
        .any(|a| a.contains("loudnorm=I=-16.0:TP=-1.5:LRA=11.0"))
    {
        return Err(AppError::Stt(SttError::VadProcessingFailed(
            "Loudnorm filter specification mismatch".to_string(),
        )));
    }
    Ok(())
}

async fn run_stage_3_slice_and_vad(
    workspace_dir: &Path,
    stage1: &Stage1Artifacts,
    cluster: &HighlightCluster,
) -> Result<Stage3Artifacts, AppError> {
    let start = Instant::now();
    let clip_slice_path = workspace_dir.join("clip_slice.wav");
    let clip_norm_path = workspace_dir.join("clip_normalized.wav");

    let prep_cfg = AudioPrepConfig::default();
    validate_stage_3_cli_builders(
        &stage1.audio_path,
        &clip_slice_path,
        &clip_norm_path,
        cluster,
        &prep_cfg,
    )?;

    // Slice exact PCM range and apply ERR-08 Screaming Defense limiter off Tokio executor
    let full_samples = stage1.audio_buffer.samples_f32.clone();
    let sr =
        usize::try_from(stage1.audio_buffer.sample_rate).unwrap_or(TARGET_SAMPLE_RATE as usize);
    let slice_start_sec = cluster.start_sec;
    let slice_end_sec = cluster.end_sec;
    let slice_path_clone = clip_slice_path.clone();
    let norm_path_clone = clip_norm_path.clone();

    let (normalized_buf, normalized_wav_bytes) = tokio::task::spawn_blocking(
        move || -> Result<(PreparedAudioBuffer, Vec<u8>), AppError> {
            let start_idx =
                ((slice_start_sec * sr as f64).round() as usize).min(full_samples.len());
            let end_idx =
                ((slice_end_sec * sr as f64).round() as usize).clamp(start_idx, full_samples.len());
            let sliced = full_samples[start_idx..end_idx].to_vec();

            let raw_slice_buf = PreparedAudioBuffer::from_f32(sliced.clone(), sr as u32);
            raw_slice_buf.write_wav_file(&slice_path_clone)?;

            let mut limited = sliced;
            apply_screaming_limiter(&mut limited, prep_cfg.true_peak as f32);
            let norm_buf = PreparedAudioBuffer::from_f32(limited, sr as u32);
            verify_screaming_defense(&norm_buf.samples_i16, prep_cfg.true_peak)?;
            norm_buf.write_wav_file(&norm_path_clone)?;
            let wav_bytes = norm_buf.to_wav_bytes()?;
            Ok((norm_buf, wav_bytes))
        },
    )
    .await
    .map_err(|e| AppError::Internal(format!("Slice/loudnorm worker failed: {e}")))??;

    // Run real tract-onnx Silero VAD pipeline (spawns blocking internally)
    let vad_cfg = VadConfig::default();
    let vad_res = process_vad_pipeline_async(normalized_buf.samples_f32.clone(), vad_cfg).await?;
    if vad_res.speech_segments.is_empty() {
        return Err(AppError::Stt(SttError::VadProcessingFailed(
            "Silero VAD detected 0 speech segments in synthetic clip".to_string(),
        )));
    }

    // Verify natural pause chunking invariants (< 20MB, never splits inside speech)
    let split_cfg = ChunkSplitConfig {
        max_chunk_size_bytes: SAFE_CHUNK_BYTES,
        max_chunk_duration_ms: 600_000,
        overlap_ms: 1_500,
    };
    let pause_chunks = split_into_natural_pause_chunks(
        &normalized_buf.samples_f32,
        &vad_res.speech_segments,
        &split_cfg,
        sr,
    )?;
    if pause_chunks.is_empty() {
        return Err(AppError::Stt(SttError::VadProcessingFailed(
            "VAD natural pause splitting returned 0 chunks".to_string(),
        )));
    }

    let speech_intervals: Vec<SpeechInterval> = vad_res
        .speech_segments
        .iter()
        .map(SpeechInterval::from)
        .collect();
    let speech_segment_count = vad_res.speech_segments.len();
    let reduction_ratio = vad_res.reduction_ratio;
    let elapsed = start.elapsed().as_millis() as u64;

    Ok(Stage3Artifacts {
        clip_slice_path,
        normalized_wav_bytes,
        speech_intervals,
        speech_segment_count,
        reduction_ratio,
        stage_result: StageResult {
            stage: "3_slice_loudnorm_silero_vad".to_string(),
            passed: true,
            duration_ms: elapsed,
            details: format!(
                "Peak <= -1.5dBFS verified; Silero VAD found {} speech segments ({:.1}% silence pruned)",
                speech_segment_count,
                reduction_ratio * 100.0
            ),
        },
    })
}

fn build_mock_groq_verbose_json() -> String {
    let payload = serde_json::json!({
        "task": "transcribe",
        "language": "english",
        "duration": 26.0,
        "text": "Hello chat and Heart! Why is tutel staring? I am not a tutel, stop calling me a gymbag. I bought a very delicious banh mi for father Vedal to eat. Filter is leaking! The Swarm is cooked KEKW! You are unhinged, did you abandon me? Dinking deeply while chat is clueless.",
        "words": [
            { "word": "Hello", "start": 0.15, "end": 0.55 },
            { "word": "chat", "start": 0.60, "end": 1.05 },
            { "word": "Heart", "start": 1.15, "end": 1.75 },
            { "word": "tutel", "start": 1.85, "end": 2.35 },
            { "word": "Stop", "start": 2.50, "end": 3.00 },
            { "word": "calling", "start": 3.05, "end": 3.60 },
            { "word": "gymbag", "start": 3.70, "end": 4.35 },
            { "word": "I", "start": 8.65, "end": 8.85 },
            { "word": "bought", "start": 8.90, "end": 9.20 },
            { "word": "banh", "start": 9.30, "end": 9.60 },
            { "word": "mi", "start": 9.65, "end": 10.00 },
            { "word": "Vedal", "start": 10.05, "end": 10.45 },
            { "word": "Filter", "start": 12.00, "end": 12.50 },
            { "word": "leaking", "start": 12.60, "end": 13.10 },
            { "word": "Swarm", "start": 13.20, "end": 13.80 },
            { "word": "cooked", "start": 13.90, "end": 14.50 },
            { "word": "Unhinged", "start": 14.80, "end": 15.50 },
            { "word": "abandoned", "start": 15.60, "end": 17.30 },
            { "word": "Dinking", "start": 22.65, "end": 23.40 },
            { "word": "aware", "start": 23.50, "end": 24.40 },
            { "word": "clueless", "start": 24.50, "end": 25.50 }
        ],
        "segments": [
            {
                "id": 0,
                "start": 0.15,
                "end": 2.35,
                "text": "Hello chat and Heart! Why is tutel staring at me?",
                "avg_logprob": -0.05
            },
            {
                "id": 1,
                "start": 2.50,
                "end": 4.35,
                "text": "I am not a tutel, stop calling me a gymbag right now.",
                "avg_logprob": -0.04
            },
            {
                "id": 2,
                "start": 8.65,
                "end": 10.45,
                "text": "I bought a very delicious banh mi for father Vedal to eat.",
                "avg_logprob": -0.05
            },
            {
                "id": 3,
                "start": 12.00,
                "end": 14.50,
                "text": "Filter is leaking! The Swarm is totally cooked now KEKW!",
                "avg_logprob": -0.06
            },
            {
                "id": 4,
                "start": 14.80,
                "end": 17.30,
                "text": "Wait, you are unhinged! Did you just abandon me?",
                "avg_logprob": -0.04
            },
            {
                "id": 5,
                "start": 22.65,
                "end": 25.50,
                "text": "Dinking deeply, feeling aware while chat acts clueless!",
                "avg_logprob": -0.05
            }
        ]
    });
    payload.to_string()
}

fn translate_mock_line(text_en: &str) -> (&'static str, &'static str) {
    if text_en.contains("staring") {
        (
            "neuro",
            "Chào chat và Thả tim <3! Sao Cụ Rùa cứ nhìn chằm chằm con thế?",
        )
    } else if text_en.contains("gymbag") {
        (
            "vedal",
            "Tôi không phải Cụ Rùa, đừng gọi tôi là bao tải gym nữa.",
        )
    } else if text_en.contains("banh mi") {
        (
            "neuro",
            "Tôi đã mua một ổ bánh mì rất ngon cho bố Vedal ăn.",
        )
    } else if text_en.contains("Filter is leaking") {
        (
            "evil",
            "Bộ lọc bị thủng rồi! The Swarm lần này toang rồi cười ẻ!",
        )
    } else if text_en.contains("unhinged") {
        (
            "vedal",
            "Khoan, phát ngôn điên rồ thật sự! Bố bỏ rơi con rồi hả?",
        )
    } else {
        (
            "neuro",
            "Đang dinking suy nghĩ, tỉnh ngộ rồi còn chat cứ ngây thơ chưa biết gì!",
        )
    }
}

fn build_mock_gemini_response_for_request(
    body: &[u8],
    captured_prompt_checks: &Arc<Mutex<bool>>,
) -> Result<GeminiTransportResponse, AppError> {
    let req_val: serde_json::Value = serde_json::from_slice(body).map_err(|e| {
        AppError::Internal(format!("Mock Gemini failed parsing request payload: {e}"))
    })?;

    // Verify Theatrical Framing, Lore Dict, Pronoun Matrix & BLOCK_NONE safety settings
    let sys_instruction = req_val
        .get("systemInstruction")
        .and_then(|s| s.get("parts"))
        .and_then(|p| p.as_array())
        .and_then(|arr| arr.first())
        .and_then(|part| part.get("text"))
        .and_then(|t| t.as_str())
        .unwrap_or("");
    let has_framing = sys_instruction.contains(THEATRICAL_FRAMING_INSTRUCTION);
    let has_lore = sys_instruction.contains("COMMUNITY LORE DICTIONARY");
    let has_pronouns = sys_instruction.contains("PRONOUN MATRIX & CHARACTER PERSONAS");
    let mut has_all_block_none = true;
    if let Some(settings) = req_val.get("safetySettings").and_then(|v| v.as_array()) {
        for cat in &SAFETY_CATEGORIES {
            let ok = settings.iter().any(|item| {
                item.get("category").and_then(|c| c.as_str()) == Some(*cat)
                    && item.get("threshold").and_then(|t| t.as_str()) == Some(BLOCK_NONE_THRESHOLD)
            });
            if !ok {
                has_all_block_none = false;
                break;
            }
        }
    } else {
        has_all_block_none = false;
    }

    if let Ok(mut guard) = captured_prompt_checks.lock() {
        *guard = has_framing && has_lore && has_pronouns && has_all_block_none;
    }

    let user_prompt = req_val
        .get("contents")
        .and_then(|c| c.as_array())
        .and_then(|arr| arr.first())
        .and_then(|first| first.get("parts"))
        .and_then(|p| p.as_array())
        .and_then(|arr| arr.first())
        .and_then(|part| part.get("text"))
        .and_then(|t| t.as_str())
        .unwrap_or("");

    let mut items = Vec::new();
    let mut in_target = false;
    for line in user_prompt.lines() {
        let trimmed = line.trim();
        if trimmed == "[DIALOGUE TO TRANSLATE]" {
            in_target = true;
            continue;
        }
        if !in_target || !trimmed.starts_with("- [") {
            continue;
        }
        if let Some(close_bracket) = trimmed.find(']') {
            let seg_id = &trimmed[3..close_bracket];
            let rest = trimmed[close_bracket + 1..].trim();
            let text_en = if let Some(close_paren) = rest.find(')') {
                rest[close_paren + 1..].trim()
            } else {
                rest
            };
            let (speaker_id, text_vi) = translate_mock_line(text_en);
            items.push(serde_json::json!({
                "segment_id": seg_id,
                "speaker_id": speaker_id,
                "text_vi": text_vi,
            }));
        }
    }

    let json_text = serde_json::to_string(&items)
        .map_err(|e| AppError::Internal(format!("Failed serializing mock Gemini items: {e}")))?;
    let resp = serde_json::json!({
        "candidates": [
            {
                "content": {
                    "role": "model",
                    "parts": [{ "text": json_text }]
                },
                "finishReason": "STOP"
            }
        ]
    });
    Ok(GeminiTransportResponse::ok(resp.to_string()))
}

async fn run_stage_4_mock_stt_and_translation(
    clip_id: &str,
    stage3: &Stage3Artifacts,
) -> Result<(TranscriptSegmentsContainer, StageResult), AppError> {
    let start = Instant::now();

    // Verify helper prompt generators emit non-empty domain sections
    let lore_sec = format_lore_prompt_section();
    let pronoun_sec = format_pronoun_matrix_prompt_section(&[], StylePreset::TikTokGenZ);
    if lore_sec.is_empty() || pronoun_sec.is_empty() {
        return Err(AppError::Internal(
            "Lore or Pronoun Matrix prompt section was empty".to_string(),
        ));
    }

    // 1. Mock Groq Whisper STT
    let groq_json = build_mock_groq_verbose_json();
    let groq_handler = Arc::new(
        move |_url: &str,
              _auth: &str,
              _ct: &str,
              _body: &[u8]|
              -> Result<GroqTransportResponse, AppError> {
            Ok(GroqTransportResponse::ok(groq_json.clone()))
        },
    );
    let groq_cfg =
        GroqClientConfig::new("gsk_e2e_synthetic_mock_key_0000000000").with_music_mode(true);
    let groq_client = GroqClient::from_config(groq_cfg)?.with_mock_handler(groq_handler);

    let raw_transcript = groq_client
        .transcribe_audio_bytes(clip_id, &stage3.normalized_wav_bytes, true)
        .await?;
    let aligned_segments =
        align_segments_with_vad(&raw_transcript.segments, &stage3.speech_intervals);

    // 2. Mock Gemini 1.5 Flash Translation + Diarization
    let prompt_checks = Arc::new(Mutex::new(false));
    let checks_clone = Arc::clone(&prompt_checks);
    let gemini_handler = Arc::new(
        move |_url: &str,
              _auth: &str,
              _ct: &str,
              body: &[u8]|
              -> Result<GeminiTransportResponse, AppError> {
            build_mock_gemini_response_for_request(body, &checks_clone)
        },
    );
    let gemini_cfg = GeminiClientConfig::new("AIzaE2ESyntheticMockKey0000000000".to_string())
        .with_style_preset("TikTok Gen Z Trend".to_string())
        .with_retry_delay_ms(1);
    let gemini_client = GeminiClient::from_config(gemini_cfg)?.with_mock_handler(gemini_handler);

    let chunks = PromptBuilder::create_chunks(&aligned_segments, 30, 5);
    let mut translated_segments = Vec::with_capacity(aligned_segments.len());
    for chunk in &chunks {
        let mut chunk_segs = gemini_client.translate_chunk(chunk).await?;
        for seg in &mut chunk_segs {
            seg.speaker_id = normalize_speaker_id(&seg.speaker_id, &[]).to_string();
        }
        translated_segments.extend(chunk_segs);
    }

    let prompt_ok = prompt_checks.lock().is_ok_and(|g| *g);
    if !prompt_ok {
        return Err(AppError::Internal(
            "Gemini prompt missing Theatrical Framing, Lore, Pronoun Matrix, or BLOCK_NONE"
                .to_string(),
        ));
    }

    // Verify 0 forbidden literal translations across all translated lines
    for seg in &translated_segments {
        if let Some(ref vi) = seg.text_vi {
            let forbidden = detect_forbidden_translations(vi);
            if !forbidden.is_empty() {
                return Err(AppError::Internal(format!(
                    "Forbidden literal lore translation detected in {}: {:?}",
                    seg.segment_id, forbidden
                )));
            }
        }
    }

    let seg_count = translated_segments.len();
    let container = TranscriptSegmentsContainer {
        clip_id: clip_id.to_string(),
        segments: translated_segments,
    };
    let elapsed = start.elapsed().as_millis() as u64;
    Ok((
        container,
        StageResult {
            stage: "4_mock_stt_and_gemini_translation".to_string(),
            passed: true,
            duration_ms: elapsed,
            details: format!(
                "Transcribed & localized {seg_count} segments with Lore Dictionary & Hybrid Diarization"
            ),
        },
    ))
}

fn run_stage_5_wrap_and_cps(
    workspace_dir: &Path,
    mut container: TranscriptSegmentsContainer,
    target_layout: &str,
    schemas: &mut SchemaValidationSummary,
) -> Result<(TranscriptSegmentsContainer, usize, StageResult), AppError> {
    let start = Instant::now();
    let max_chars = if target_layout == "original_16_9" {
        DEFAULT_MAX_CHARS_PER_LINE
    } else {
        VERTICAL_9_16_MAX_CHARS_PER_LINE
    };

    for seg in &mut container.segments {
        if let Some(ref vi) = seg.text_vi {
            let wrapped = wrap_vietnamese_subtitle(vi, max_chars, DEFAULT_MAX_LINES);
            for line in wrapped.lines() {
                if line.chars().count() > max_chars {
                    return Err(AppError::Internal(format!(
                        "Wrapped line exceeds {max_chars} chars in {}: '{line}'",
                        seg.segment_id
                    )));
                }
            }
            seg.text_vi = Some(wrapped);
        }
    }

    // Verify compound nouns remain unbroken on a single line for original_16_9
    if target_layout == "original_16_9" {
        if let Some(banh_mi_seg) = container
            .segments
            .iter()
            .find(|s| s.text_vi.as_deref().is_some_and(|t| t.contains("bánh")))
        {
            let vi_text = banh_mi_seg.text_vi.as_deref().unwrap_or("");
            if !vi_text.contains("bánh mì") || !vi_text.contains("bố Vedal") {
                return Err(AppError::Internal(
                    "Vietnamese compound word split across line break".to_string(),
                ));
            }
        }
    }

    let cps_report = optimize_container_cps(&mut container, &TimeBorrowingConfig::default());
    if cps_report.optimized_count == 0 {
        return Err(AppError::Internal(
            "CPS Time Borrowing failed to optimize high-CPS synthetic segment".to_string(),
        ));
    }

    for seg in &mut container.segments {
        seg.is_flagged_for_review =
            seg.confidence < CONFIDENCE_FLAG_THRESHOLD || seg.cps_score > CPS_FLAG_THRESHOLD;
    }

    // Verify non-collision buffer >= 50ms between consecutive segments
    for pair in container.segments.windows(2) {
        if pair[1].start_sec - pair[0].end_sec < 0.049 {
            return Err(AppError::Internal(format!(
                "Time Borrowing collision between '{}' and '{}'",
                pair[0].segment_id, pair[1].segment_id
            )));
        }
    }

    let transcript_json_str = serialize_transcript_segments_json(&container)?;
    let transcript_path = workspace_dir.join("transcript_segments.json");
    std::fs::write(&transcript_path, transcript_json_str).map_err(|e| {
        AppError::Export(ExportError::FileWriteFailed {
            path: transcript_path.display().to_string(),
            details: e.to_string(),
        })
    })?;

    let reloaded_val = read_json_value(&transcript_path)?;
    validate_schema_5_5_transcript_segments(&reloaded_val)?;
    schemas.schema_5_5_transcript_segments = true;

    let elapsed = start.elapsed().as_millis() as u64;
    Ok((
        container,
        cps_report.optimized_count,
        StageResult {
            stage: "5_line_wrap_and_cps_optimization".to_string(),
            passed: true,
            duration_ms: elapsed,
            details: format!(
                "Wrapped <= {max_chars} chars/line; CPS optimized {} segment(s), borrowed {:.3}s; Schema 5.5 validated",
                cps_report.optimized_count, cps_report.total_borrowed_sec
            ),
        },
    ))
}

fn verify_utf8_bom_file(path: &Path) -> Result<(), AppError> {
    let bytes = std::fs::read(path).map_err(|e| {
        AppError::Storage(StorageError::FileNotFound(format!(
            "Missing dual export file '{}': {e}",
            path.display()
        )))
    })?;
    if bytes.len() < 3 || &bytes[0..3] != UTF8_BOM_BYTES {
        return Err(AppError::Export(ExportError::FileWriteFailed {
            path: path.display().to_string(),
            details: "Subtitle file missing UTF-8 BOM [0xEF, 0xBB, 0xBF]".to_string(),
        }));
    }
    Ok(())
}

fn run_stage_6_capcut_export(
    workspace_dir: &Path,
    vod_id: &str,
    clip_slice_path: &Path,
    container: TranscriptSegmentsContainer,
    target_layout: &str,
    schemas: &mut SchemaValidationSummary,
) -> Result<(String, Vec<String>, StageResult), AppError> {
    let start = Instant::now();

    let mut project = create_subtitle_project(
        container,
        "NeuroClip_E2E_Synthetic",
        vod_id,
        Some(target_layout),
    );
    project.video_file_path = Some(clip_slice_path.to_string_lossy().to_string());

    let project_path = workspace_dir.join("subtitle_project.json");
    export_subtitle_project_json(&project, &project_path)?;
    let _loaded_project = load_subtitle_project_json(&project_path)?;
    let project_val = read_json_value(&project_path)?;
    validate_schema_5_6_subtitle_project(&project_val)?;
    schemas.schema_5_6_subtitle_project = true;

    let capcut_root = workspace_dir.join("capcut_drafts");
    let export_res = export_capcut_draft_folder(
        &project,
        project.video_file_path.as_deref(),
        Some(&capcut_root),
    )?;

    let content_bytes = std::fs::read(&export_res.draft_content_path).map_err(|e| {
        AppError::Storage(StorageError::FileNotFound(format!(
            "Missing draft_content.json: {e}"
        )))
    })?;
    let draft_content: DraftContent = serde_json::from_slice(&content_bytes)
        .map_err(|e| AppError::Internal(format!("Invalid draft_content.json schema: {e}")))?;
    if draft_content.tracks.len() != 3
        || draft_content.materials.texts.len() != project.segments.len()
    {
        return Err(AppError::Internal(
            "CapCut draft tracks or text materials count mismatch".to_string(),
        ));
    }

    let meta_bytes = std::fs::read(&export_res.draft_meta_info_path).map_err(|e| {
        AppError::Storage(StorageError::FileNotFound(format!(
            "Missing draft_meta_info.json: {e}"
        )))
    })?;
    let draft_meta: DraftMetaInfo = serde_json::from_slice(&meta_bytes)
        .map_err(|e| AppError::Internal(format!("Invalid draft_meta_info.json schema: {e}")))?;
    if draft_meta.draft_id != draft_content.id {
        return Err(AppError::Internal(
            "CapCut draft_meta_info draft_id does not match draft_content id".to_string(),
        ));
    }

    verify_utf8_bom_file(Path::new(&export_res.ass_backup_path))?;
    verify_utf8_bom_file(Path::new(&export_res.srt_backup_path))?;

    let elapsed = start.elapsed().as_millis() as u64;
    Ok((
        export_res.project_dir,
        export_res.dual_safeguard_files,
        StageResult {
            stage: "6_capcut_draft_and_dual_export".to_string(),
            passed: true,
            duration_ms: elapsed,
            details: "Schema 5.6 validated; CapCut draft_content/draft_meta_info & UTF-8 BOM .ass/.srt verified".to_string(),
        },
    ))
}

struct ResolvedE2EInputs {
    audio_fixture: PathBuf,
    chat_fixture: PathBuf,
    vod_id: String,
    target_layout: String,
    workspace_dir: PathBuf,
}

fn resolve_e2e_inputs(options: &SyntheticE2EOptions) -> Result<ResolvedE2EInputs, AppError> {
    let audio_fixture =
        ensure_clean_path_input(&options.audio_fixture_path, "audio_fixture_path", false)?;
    let chat_fixture =
        ensure_clean_path_input(&options.chat_fixture_path, "chat_fixture_path", false)?;

    let raw_vod_id = options
        .vod_id
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .unwrap_or("synthetic_vod_e2e_001");
    let vod_id = validate_vod_id(raw_vod_id)
        .map_err(AppError::Storage)?
        .to_string();

    let target_layout = options
        .target_layout
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .unwrap_or("original_16_9")
        .to_string();
    if !VALID_TARGET_LAYOUTS.contains(&target_layout.as_str()) {
        return Err(AppError::Export(ExportError::InvalidLayout(target_layout)));
    }

    let base_output_dir = if let Some(ref custom_out) = options.output_dir {
        ensure_clean_path_input(custom_out, "output_dir", true)?
    } else {
        std::env::temp_dir()
    };
    let workspace_dir = base_output_dir.join(format!(
        "neuroclip_e2e_{}_{}",
        std::process::id(),
        chrono::Utc::now().timestamp_nanos_opt().unwrap_or(0)
    ));
    std::fs::create_dir_all(&workspace_dir).map_err(|e| {
        AppError::Storage(StorageError::DirectoryCreationFailed {
            path: workspace_dir.display().to_string(),
            details: e.to_string(),
        })
    })?;

    Ok(ResolvedE2EInputs {
        audio_fixture,
        chat_fixture,
        vod_id,
        target_layout,
        workspace_dir,
    })
}

/// Execute the full 6-stage E2E synthetic VOD pipeline in an isolated temporary workspace.
///
/// # Errors
/// Returns [`AppError`] if any path check, pipeline computation, schema validation, or export fails.
pub async fn run_synthetic_e2e_pipeline_internal(
    options: SyntheticE2EOptions,
) -> Result<SyntheticE2EReport, AppError> {
    let total_start = Instant::now();
    let inputs = resolve_e2e_inputs(&options)?;
    let workspace_dir = inputs.workspace_dir;
    let vod_id = inputs.vod_id;
    let target_layout = inputs.target_layout;

    // Stage 1 & Stage 2 run on Tokio blocking pool so async executor never stalls
    let ws_s12 = workspace_dir.clone();
    let audio_fix_clone = inputs.audio_fixture;
    let chat_fix_clone = inputs.chat_fixture;
    let vod_id_clone = vod_id.clone();

    let (stage1, top_cluster, stage2_res, mut schemas) =
        tokio::task::spawn_blocking(move || -> Result<(Stage1Artifacts, HighlightCluster, StageResult, SchemaValidationSummary), AppError> {
            let mut schema_summary = SchemaValidationSummary::default();
            let s1 = run_stage_1_ingestion(
                &ws_s12,
                &audio_fix_clone,
                &chat_fix_clone,
                &vod_id_clone,
                &mut schema_summary,
            )?;
            let (cluster, s2) = run_stage_2_highlights(&ws_s12, &s1, &mut schema_summary)?;
            Ok((s1, cluster, s2, schema_summary))
        })
        .await
        .map_err(|e| AppError::Internal(format!("E2E Stages 1-2 worker panicked: {e}")))??;

    // Stage 3: Range Slice + Loudnorm Limiter + Silero VAD
    let stage3 = run_stage_3_slice_and_vad(&workspace_dir, &stage1, &top_cluster).await?;

    // Stage 4: Mock Groq STT + Mock Gemini Vietsub + Lore + Hybrid Diarization
    let clip_id = format!("{}_{}", vod_id, top_cluster.cluster_id);
    let (translated_container, stage4_res) =
        run_stage_4_mock_stt_and_translation(&clip_id, &stage3).await?;

    // Stages 5 & 6 run on Tokio blocking pool
    let ws_s56 = workspace_dir.clone();
    let slice_path_clone = stage3.clip_slice_path.clone();
    let layout_clone = target_layout.clone();
    let vod_id_s6 = vod_id.clone();

    let (
        final_container,
        cps_optimized_count,
        stage5_res,
        capcut_draft_dir,
        dual_export_files,
        stage6_res,
        schemas_final,
    ) = tokio::task::spawn_blocking(move || {
        let (wrapped_container, opt_count, s5) =
            run_stage_5_wrap_and_cps(&ws_s56, translated_container, &layout_clone, &mut schemas)?;
        let seg_count = wrapped_container.segments.len();
        let (draft_dir, dual_files, s6) = run_stage_6_capcut_export(
            &ws_s56,
            &vod_id_s6,
            &slice_path_clone,
            wrapped_container,
            &layout_clone,
            &mut schemas,
        )?;
        Ok::<_, AppError>((seg_count, opt_count, s5, draft_dir, dual_files, s6, schemas))
    })
    .await
    .map_err(|e| AppError::Internal(format!("E2E Stages 5-6 worker panicked: {e}")))??;

    // Keep track of unique stage names for sanity
    let stage_results = vec![
        stage1.stage_result,
        stage2_res,
        stage3.stage_result,
        stage4_res,
        stage5_res,
        stage6_res,
    ];
    let stage_names: HashSet<&str> = stage_results.iter().map(|s| s.stage.as_str()).collect();
    let total_duration_ms = total_start.elapsed().as_millis() as u64;
    let passed = schemas_final.all_passed()
        && stage_names.len() == 6
        && stage_results.iter().all(|s| s.passed);

    Ok(SyntheticE2EReport {
        passed,
        total_duration_ms,
        workspace_dir: workspace_dir.to_string_lossy().to_string(),
        detected_peak_sec: top_cluster.peak_sec,
        cluster_start_sec: top_cluster.start_sec,
        cluster_end_sec: top_cluster.end_sec,
        vad_speech_segments: stage3.speech_segment_count,
        vad_reduction_ratio: stage3.reduction_ratio,
        translated_segments: final_container,
        cps_optimized_count,
        capcut_draft_dir,
        dual_export_files,
        schemas: schemas_final,
        stages: stage_results,
    })
}

/// Tauri IPC command executing the offline synthetic E2E pipeline verification harness.
///
/// # Errors
/// Returns [`AppError`] if any pipeline stage or schema validation fails.
#[tauri::command]
pub async fn run_synthetic_e2e_pipeline(
    options: SyntheticE2EOptions,
) -> Result<SyntheticE2EReport, AppError> {
    run_synthetic_e2e_pipeline_internal(options).await
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;

    #[test]
    fn test_schema_validators_happy_path_and_rejection() {
        // Schema 5.1
        let valid_51 = serde_json::json!({
            "cpu_info": { "model": "Ryzen 5", "cores": 6, "threads": 12 },
            "ram_info": { "total_mb": 16384, "available_mb": 8192 },
            "gpu_info": { "vendor": "NVIDIA", "model": "RTX 3060", "vram_mb": 12288, "cuda_supported": true },
            "recommended_mode": "offline_local",
            "recommendation_reason": "OK",
            "timestamp": "2026-11-24T12:00:00Z"
        });
        assert!(validate_schema_5_1_hardware_profile(&valid_51).is_ok());

        let mut bad_51 = valid_51.clone();
        bad_51["recommended_mode"] = serde_json::json!("unknown_mode");
        assert!(validate_schema_5_1_hardware_profile(&bad_51).is_err());

        // Schema 5.2
        let valid_52 = serde_json::json!({
            "vod_id": "vod_1",
            "source_type": "local_file",
            "title": "Test VOD",
            "duration_seconds": 180.0,
            "audio_path": "C:/tmp/audio.wav",
            "created_at": "2026-11-24T12:00:00Z"
        });
        assert!(validate_schema_5_2_vod_manifest(&valid_52).is_ok());
        let mut bad_52 = valid_52.clone();
        bad_52["duration_seconds"] = serde_json::json!(0.0);
        assert!(validate_schema_5_2_vod_manifest(&bad_52).is_err());

        // Schema 5.3
        let valid_53 = serde_json::json!({
            "vod_id": "vod_1",
            "total_messages": 1,
            "events": [{
                "timestamp_sec": 1.0,
                "author": "user1",
                "message": "KEKW",
                "emotes_detected": ["KEKW"],
                "is_swarm_emote": true
            }]
        });
        assert!(validate_schema_5_3_chat_events(&valid_53).is_ok());
        let mut bad_53 = valid_53.clone();
        bad_53["total_messages"] = serde_json::json!(99);
        assert!(validate_schema_5_3_chat_events(&bad_53).is_err());

        // Schema 5.4
        let valid_54 = serde_json::json!({
            "vod_id": "vod_1",
            "clusters": [{
                "cluster_id": "cluster_1",
                "start_sec": 45.5,
                "end_sec": 75.0,
                "peak_sec": 60.0,
                "score": 92.5,
                "primary_reason": "KEKW spike",
                "is_bookmarked": false
            }]
        });
        assert!(validate_schema_5_4_highlight_clusters(&valid_54).is_ok());
        let mut bad_54 = valid_54.clone();
        bad_54["clusters"][0]["score"] = serde_json::json!(150.0);
        assert!(validate_schema_5_4_highlight_clusters(&bad_54).is_err());

        // Schema 5.5
        let valid_55 = serde_json::json!({
            "clip_id": "clip_1",
            "segments": [{
                "segment_id": "seg_1",
                "start_sec": 0.0,
                "end_sec": 2.0,
                "speaker_id": "neuro",
                "text_en": "Hello",
                "text_vi": "Xin chào",
                "confidence": 0.95,
                "cps_score": 4.0,
                "is_flagged_for_review": false
            }]
        });
        assert!(validate_schema_5_5_transcript_segments(&valid_55).is_ok());
        let mut bad_55 = valid_55.clone();
        bad_55["segments"][0]["speaker_id"] = serde_json::json!("invalid_role");
        assert!(validate_schema_5_5_transcript_segments(&bad_55).is_err());

        // Schema 5.6
        let valid_56 = serde_json::json!({
            "project_id": "proj_1",
            "project_name": "Clip",
            "vod_id": "vod_1",
            "target_layout": "original_16_9",
            "segments": valid_55["segments"].clone(),
            "speaker_styles": {
                "neuro": {
                    "color_hex": "#FFB6C1",
                    "font_name": "Arial",
                    "font_size": 54,
                    "outline_color_hex": "#000000"
                }
            },
            "updated_at": "2026-11-24T12:00:00Z"
        });
        assert!(validate_schema_5_6_subtitle_project(&valid_56).is_ok());
        let mut bad_56 = valid_56.clone();
        bad_56["target_layout"] = serde_json::json!("square_1_1");
        assert!(validate_schema_5_6_subtitle_project(&bad_56).is_err());
    }
}
