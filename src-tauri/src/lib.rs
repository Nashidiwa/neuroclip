#![deny(clippy::all)]
#![warn(clippy::pedantic)]
#![deny(clippy::unwrap_used)]
#![deny(clippy::expect_used)]
#![deny(clippy::panic)]
#![deny(clippy::todo)]
#![deny(clippy::unimplemented)]
#![allow(clippy::module_name_repetitions)]
#![allow(clippy::must_use_candidate)]
#![allow(clippy::doc_markdown, clippy::missing_errors_doc)]
#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used, clippy::panic))]

pub mod commands;
pub mod config;
pub mod core;
pub mod error;
pub mod export;
pub mod highlight;
pub mod ingestion;
pub mod logging;
pub mod sidecar;
pub mod stt;
pub mod translation;

pub use commands::{
    cancel_chat_extraction, cancel_chat_task_token, cancel_slice_task_token,
    cancel_slice_video_segment, register_chat_task_token, register_slice_task_token,
    resolve_vod_project_dir, run_synthetic_e2e_pipeline_internal, slice_video_segment,
    slice_video_segment_internal, unregister_chat_task_token, unregister_slice_task_token,
    validate_schema_5_1_hardware_profile, validate_schema_5_2_vod_manifest,
    validate_schema_5_3_chat_events, validate_schema_5_4_highlight_clusters,
    validate_schema_5_5_transcript_segments, validate_schema_5_6_subtitle_project, AppState,
    SchemaValidationSummary, StageResult, SyntheticE2EOptions, SyntheticE2EReport,
};
pub use config::{
    AppConfig, CpuInfo, GpuInfo, GpuVendor, HardwareProfile, RamInfo, RecommendedMode, SidecarPaths,
};
pub use core::storage;
pub use error::{
    codes, AppError, ConfigError, ExportError, HardwareError, HighlightError, IngestError,
    IpcErrorResponse, SidecarError, StorageError, SttError, TranslationError,
};
pub use export::{
    canvas_config_for_layout, default_capcut_drafts_dir, detect_capcut_drafts_dir, escape_xml_text,
    export_ass_file, export_ass_project_file, export_capcut_draft_folder, export_premiere_xml_file,
    export_srt_file, export_srt_project_file, export_subtitle_project_json,
    generate_ass_script_from_project, generate_premiere_xml, generate_project_folder_name,
    generate_srt_script_from_project, generate_srt_script_from_segments, hex_to_ass_bgr,
    hex_to_normalized_rgb, load_subtitle_project_json, path_to_file_url, sanitize_project_name,
    sec_to_frames, sec_to_micros, write_file_with_bom, CanvasConfig, CapCutExportResult,
    DraftAudioMaterial, DraftContent, DraftMaterials, DraftMetaInfo, DraftMetaMaterialItem,
    DraftMetaMaterials, DraftSegment, DraftStoreItem, DraftTextMaterial, DraftTrack,
    DraftVideoMaterial, PremiereClipInput, PremiereExportResult, PremiereXmlExportOptions,
    RootMetaInfo, TimeRange, DEFAULT_PREMIERE_FPS, DEFAULT_SUBTITLE_TRANSFORM_X,
    DEFAULT_SUBTITLE_TRANSFORM_Y, MICROS_PER_SEC, MIN_CLIP_DURATION_MICROS,
    PREMIERE_AUDIO_BIT_DEPTH, PREMIERE_AUDIO_CHANNELS, PREMIERE_AUDIO_SAMPLE_RATE, UTF8_BOM,
    UTF8_BOM_BYTES,
};
pub use highlight::{
    analyze_audio_samples, analyze_audio_samples_default, analyze_wav_file, apply_silence_clamp,
    bin_chat_events, bin_chat_events_default, bin_chat_events_from_file, bucket_chat_events,
    calculate_adjusted_zscore, calculate_clip_bounds, calculate_emote_ratio,
    calculate_normalized_pcm_rms, calculate_pcm_rms, calculate_raw_rms,
    calculate_sine_normalized_rms, chunk_samples_to_blocks, chunk_samples_to_blocks_default,
    clamp_silence_floor, clean_word_token, cluster_and_deduplicate, compute_audio_only_score,
    compute_chat_velocity_series, compute_chat_velocity_series_default,
    compute_chat_velocity_series_from_events, compute_chat_velocity_series_from_file,
    compute_emote_density_series, compute_emote_density_series_default,
    compute_emote_density_series_from_file, compute_multimodal_score, compute_overlap_ratio,
    compute_zscore, dbfs_to_normalized_scale, dbfs_to_rms, default_swarm_dictionary,
    detect_energy_peaks, detect_energy_peaks_default, determine_primary_reason,
    extract_message_tokens, extract_swarm_emotes, fuse_multimodal_series,
    generate_clusters_from_fused_series, is_swarm_emote, normalize_z_score, process_pcm_reader,
    rms_to_dbfs, AudioBlockMetrics, AudioRmsPoint, AudioRmsSeriesResponse, ChatBucket,
    ChatVelocityPoint, EmoteDensityPoint, EnergyAnalyzerConfig, EnergyBaselineStats,
    FusedScorePoint, HighlightCluster, HighlightClustersContainer, SwarmDictionary,
    SwarmEmoteCategory, SwarmEmoteDefinition, CLUSTER_MERGE_DISTANCE_SEC,
    DEFAULT_AUDIO_BASELINE_WINDOW_SEC, DEFAULT_BASELINE_WINDOW_SEC, DEFAULT_BLOCK_DURATION_SEC,
    DEFAULT_BLOCK_SAMPLES, DEFAULT_BUCKET_WIDTH_SEC, DEFAULT_MAX_CLUSTERS,
    DEFAULT_PEAK_DB_SPIKE_THRESHOLD, DEFAULT_PEAK_SIGMA_THRESHOLD, DEFAULT_POST_ROLL_SEC,
    DEFAULT_PRE_ROLL_SEC, DEFAULT_SAMPLE_RATE, DEFAULT_TOTAL_PRE_OFFSET_SEC, EPSILON_RMS,
    I16_MAX_F64, MAX_OVERLAP_RATIO, MAX_PRE_ROLL_SEC, MAX_TOP_EMOTES_PER_BUCKET,
    MESSAGE_FLOOR_THRESHOLD, MIN_CLIP_DURATION_SEC, MIN_RMS_THRESHOLD, PEAK_DB_THRESHOLD,
    PEAK_SIGMA_THRESHOLD, SILENCE_FLOOR_AMPLITUDE, SILENCE_FLOOR_DBFS, SINE_1KHZ_EXPECTED_RMS,
    SINE_1KHZ_RMS_TOLERANCE, SINE_1KHZ_THEORETICAL_RMS, SMOOTHING_EPSILON,
    SPAM_BOT_MAX_MESSAGES_PER_AUTHOR, STREAM_LATENCY_COMPENSATION_SEC, WEIGHT_AUDIO_ONLY_RMS,
    WEIGHT_AUDIO_RMS, WEIGHT_CHAT_Z, WEIGHT_EMOTE_RATIO,
};
pub use ingestion::{
    build_ffmpeg_range_slice_args, build_slice_ffmpeg_args, evaluate_keyframe_alignment,
    execute_range_slice, extract_message_emotes, is_neuro_swarm_emote, parse_ffprobe_json,
    parse_raw_chat_line, probe_keyframe_alignment, resolve_ffmpeg_binary, resolve_ffprobe_binary,
    verify_stream_durations, AudioChunk, AudioEnergyWindow, AudioRingBuffer, AudioStreamValidator,
    BrowserType, ChatDensityPoint, ChatEvent, ChatEventsContainer, ChatHeatmapContainer,
    ChatHeatmapResponse, CookieSource, FfprobeFormat, FfprobeOutput, FfprobeStream, HeatmapWindow,
    IngestOptions, IngestProgressPayload, KeyframeAlignmentReport, RangeSliceRequest, SliceResult,
    StreamingChatNormalizer, VodManifest, VodMetadata, MAX_INITIAL_PTS_DELAY_SEC,
    MAX_KEYFRAME_DRIFT_MS, MIN_SLICING_DISK_SPACE_MB, NEURO_SWARM_EMOTES,
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
pub use translation::{
    strip_markdown_code_fence, DialogueChunk, GeminiCandidate, GeminiClient, GeminiClientConfig,
    GeminiContent, GeminiGenerationConfig, GeminiPart, GeminiRequest, GeminiResponse,
    GeminiSafetySetting, MockHttpHandler, PromptBuilder, TranslatedSegmentItem,
    TranslationProgressPayload, TransportResponse, BLOCK_NONE_THRESHOLD, CONTEXT_WINDOW_HEADER,
    DEFAULT_CHUNK_SIZE, DEFAULT_CONTEXT_WINDOW_SIZE, DEFAULT_GEMINI_API_URL, DEFAULT_GEMINI_MODEL,
    DEFAULT_RETRY_BASE_DELAY_MS, DIALOGUE_TARGET_HEADER, FORMAT_FALLBACK_PREFIX,
    MAX_RATE_LIMIT_RETRIES, SAFETY_CATEGORIES, SAFETY_FALLBACK_PREFIX, SUBTITLE_GUIDELINES,
    THEATRICAL_FRAMING_INSTRUCTION,
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
        .register_asynchronous_uri_scheme_protocol(
            "asset",
            core::custom_protocol::handle_asset_protocol,
        )
        .manage(AppState::default())
        .invoke_handler(tauri::generate_handler![
            commands::ping,
            commands::get_app_config,
            commands::save_app_config,
            commands::get_api_key,
            commands::set_api_key,
            commands::delete_api_key,
            commands::test_api_key_connection,
            commands::test_groq_connection,
            commands::complete_onboarding,
            commands::is_first_run,
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
            commands::import_local_video,
            commands::cancel_vod_ingestion,
            commands::probe_vod,
            commands::get_vod_manifest,
            commands::extract_pcm_stream,
            commands::start_chat_extraction,
            commands::cancel_chat_extraction,
            commands::get_chat_events,
            commands::get_chat_velocity_heatmap,
            commands::start_whisper_transcription,
            commands::start_faster_whisper_transcription,
            commands::transcribe_highlight_clip,
            commands::verify_stt_model,
            commands::export_subtitles_ass,
            commands::export_subtitles_srt,
            commands::export_subtitle_project,
            commands::export_subtitles_project_ass,
            commands::export_subtitles_project_srt,
            commands::export_capcut_draft,
            commands::export_premiere_xml,
            commands::detect_highlights,
            commands::get_highlight_clusters,
            commands::toggle_highlight_bookmark,
            commands::toggle_cluster_bookmark,
            commands::slice_video_segment,
            commands::cancel_slice_video_segment,
            commands::translate_vietsub,
            commands::test_gemini_connection,
            commands::update_speaker_role,
            commands::wrap_vietnamese_subtitles,
            commands::optimize_transcript_cps,
            commands::check_sidecar_updates,
            commands::download_and_apply_sidecar_update,
            commands::get_sidecar_status,
            commands::get_anti_bot_guidance,
            commands::trigger_bot_defense_update,
            commands::run_synthetic_e2e_pipeline
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
