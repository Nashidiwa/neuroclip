#![deny(clippy::all)]
#![warn(clippy::pedantic)]
#![deny(clippy::unwrap_used)]
#![deny(clippy::expect_used)]
#![deny(clippy::panic)]
#![deny(clippy::todo)]
#![deny(clippy::unimplemented)]
#![allow(clippy::module_name_repetitions)]
#![allow(clippy::must_use_candidate)]
#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used, clippy::panic))]

pub mod commands;
pub mod config;
pub mod error;
pub mod ingestion;
pub mod logging;
pub mod stt;

/// Backward-compatibility and architectural alias for core logging subsystem.
pub mod core {
    pub use crate::logging;
    #[path = "storage.rs"]
    pub mod storage;
}

pub use commands::AppState;
pub use config::{
    AppConfig, CpuInfo, GpuInfo, GpuVendor, HardwareProfile, RamInfo, RecommendedMode, SidecarPaths,
};
pub use core::storage;
pub use error::{
    codes, AppError, ConfigError, ExportError, HardwareError, HighlightError, IngestError,
    IpcErrorResponse, SidecarError, StorageError, SttError, TranslationError,
};
pub use ingestion::{
    extract_message_emotes, is_neuro_swarm_emote, parse_raw_chat_line, AudioChunk,
    AudioEnergyWindow, AudioRingBuffer, AudioStreamValidator, BrowserType, ChatDensityPoint,
    ChatEvent, ChatEventsContainer, ChatHeatmapContainer, ChatHeatmapResponse, CookieSource,
    HeatmapWindow, IngestOptions, IngestProgressPayload, StreamingChatNormalizer, VodManifest,
    VodMetadata, NEURO_SWARM_EMOTES,
};
pub use logging::{
    is_neuroclip_log_file, prune_logs, redact_secret, redact_secrets_in_text, resolve_log_dir,
    LogEntry, PruneReport, RedactingWriter, RetentionPolicy,
};
pub use stt::{
    align_segments_with_vad, calculate_cps, compute_file_sha256, create_subtitle_project,
    default_speaker_styles, escape_ass_text, find_catalog_entry, format_ass_timecode,
    format_srt_timecode, generate_ass_script, generate_srt_script, is_special_token,
    map_speaker_to_ass_style, parse_timestamp_to_seconds, parse_whisper_json_full,
    parse_whisper_stdout_line, resolve_models_dir, resolve_whisper_binary,
    run_whisper_transcription, verify_model, ModelCatalogEntry, ModelInfo, SpeakerStyle,
    SpeechInterval, SttProgressPayload, SubtitleProject, TranscriptSegment,
    TranscriptSegmentsContainer, WhisperSidecarConfig, WordTiming, BOUNDARY_SNAP_TOLERANCE_SEC,
    CONFIDENCE_FLAG_THRESHOLD, CPS_FLAG_THRESHOLD, DEFAULT_SPEAKER_ID, MIN_SEGMENT_DURATION_SEC,
    MODEL_CATALOG, SHA256_CHUNK_SIZE, WORD_GAP_SPLIT_THRESHOLD_SEC,
};

/// Health check ping/pong command adhering to Result<T, AppError> contract
pub fn ping() -> Result<String, AppError> {
    commands::ping()
}

pub const DEFAULT_WINDOW_WIDTH: f64 = 1280.0;
pub const DEFAULT_WINDOW_HEIGHT: f64 = 800.0;
pub const MIN_WINDOW_WIDTH: f64 = 1024.0;
pub const MIN_WINDOW_HEIGHT: f64 = 640.0;
pub const SOLID_BACKGROUND_FALLBACK: &str = "#12131C";

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    if let Err(err) = logging::init_logging() {
        eprintln!("Failed to initialize logging: {err}");
    }

    let builder = tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .manage(AppState::default())
        .invoke_handler(tauri::generate_handler![
            commands::ping,
            commands::get_app_config,
            commands::save_app_config,
            commands::get_hardware_profile,
            commands::detect_hardware,
            commands::get_recent_logs,
            commands::clear_logs,
            commands::log_frontend_event,
            commands::check_preflight_storage,
            commands::initialize_storage,
            commands::initialize_project,
            commands::clean_expired_cache,
            commands::start_vod_ingestion,
            commands::cancel_vod_ingestion,
            commands::probe_vod,
            commands::get_vod_manifest,
            commands::extract_pcm_stream,
            commands::start_chat_extraction,
            commands::get_chat_events,
            commands::get_chat_velocity_heatmap,
            commands::start_whisper_transcription,
            commands::verify_stt_model,
            commands::export_subtitles_ass,
            commands::export_subtitles_srt
        ]);

    if let Err(err) = builder.run(tauri::generate_context!()) {
        eprintln!("error while running tauri application: {err}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ping_pong_response() {
        let result = ping();
        assert!(result.is_ok());
        if let Ok(value) = result {
            assert_eq!(value, "pong");
        }
    }

    #[test]
    fn test_window_geometry_constants() {
        assert!((DEFAULT_WINDOW_WIDTH - 1280.0).abs() < f64::EPSILON);
        assert!((DEFAULT_WINDOW_HEIGHT - 800.0).abs() < f64::EPSILON);
        assert!((MIN_WINDOW_WIDTH - 1024.0).abs() < f64::EPSILON);
        assert!((MIN_WINDOW_HEIGHT - 640.0).abs() < f64::EPSILON);
        assert_eq!(SOLID_BACKGROUND_FALLBACK, "#12131C");
    }
}
