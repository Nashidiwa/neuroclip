//! Speech-to-Text (STT) and subtitle extraction pipeline for NeuroClip.
//!
//! Provides local Whisper.cpp process supervision, Q5_K quantized model management,
//! zero-panic transcript parsing adhering to Schema 5.5, VAD pause-alignment,
//! and dual `.ass` / `.srt` subtitle export engines (`ADR-001`, `ADR-005`, `ADR-007`).

pub mod audio_prep;
pub mod faster_whisper_worker;
pub mod groq_client;
pub mod model_manager;
pub mod subtitle_export;
pub mod transcript_parser;
pub mod vad;
pub mod vad_bridge;
pub mod whisper_sidecar;

pub use audio_prep::{
    apply_screaming_limiter, build_loudnorm_ffmpeg_args, build_loudnorm_pipe_args,
    calculate_peak_dbfs_f32, calculate_peak_dbfs_i16, compute_true_peak_dbfs,
    compute_true_peak_f32, generate_synthetic_normalized_buffer, is_mock_audio_prep_enabled,
    prepare_audio_file, prepare_audio_to_buffer, resolve_ffmpeg_binary, verify_screaming_defense,
    AudioPrepConfig, AudioPrepResult, PreparedAudioBuffer, DEFAULT_LOUDNESS_RANGE_LU,
    DEFAULT_TARGET_LUFS, DEFAULT_TRUE_PEAK_DBFS, MAX_I16_PEAK_AMPLITUDE, TARGET_CHANNELS,
    TARGET_SAMPLE_RATE,
};
pub use faster_whisper_worker::{
    build_faster_whisper_command_args, build_worker_command_args, check_hardware_guardrail,
    check_hardware_guardrail_from_profile, check_hardware_guardrail_from_profile_file,
    enforce_hardware_guardrail, estimate_vram_usage_mb, evaluate_hardware_guardrail,
    evaluate_hardware_guardrail_from_profile, evaluate_hardware_guardrail_from_profile_file,
    map_faster_whisper_to_container, parse_faster_whisper_json, resolve_faster_whisper_binary,
    resolve_faster_whisper_binary as resolve_faster_whisper_worker_binary,
    run_faster_whisper_worker, FasterWhisperConfig, FasterWhisperSegment,
    FasterWhisperVerboseJsonResponse, FasterWhisperWord, GuardrailDecision, GuardrailPolicy,
    DEFAULT_FASTER_WHISPER_DEVICE, DEFAULT_FASTER_WHISPER_MODEL,
    DEFAULT_FASTER_WHISPER_QUANTIZATION, MAX_INT8_VRAM_CAP_MB, MIN_LARGE_VRAM_MB,
};
pub use groq_client::{
    deduplicate_overlap_segments, map_verbose_json_to_container, parse_retry_after, GroqClient,
    GroqClientConfig, GroqErrorBody, GroqErrorEnvelope, GroqSegment, GroqVerboseJsonResponse,
    GroqWord, DEFAULT_GROQ_API_URL, DEFAULT_GROQ_MODEL, DEFAULT_OVERLAP_MS, MAX_PAYLOAD_BYTES,
    MAX_RATE_LIMIT_RETRIES, SAFE_CHUNK_BYTES,
};
pub use model_manager::is_whisper_large;
pub use model_manager::{
    compute_file_sha256, find_catalog_entry, resolve_models_dir, verify_model, ModelCatalogEntry,
    ModelInfo, MODEL_CATALOG, SHA256_CHUNK_SIZE,
};
pub use subtitle_export::{
    create_subtitle_project, default_speaker_styles, escape_ass_text, format_ass_timecode,
    format_srt_timecode, generate_ass_script, generate_srt_script, map_speaker_to_ass_style,
    SpeakerStyle, SubtitleProject,
};
pub use transcript_parser::{
    calculate_cps, is_special_token, parse_timestamp_to_seconds, parse_whisper_json_full,
    parse_whisper_stdout_line, TranscriptSegment, TranscriptSegmentsContainer, WordTiming,
    CONFIDENCE_FLAG_THRESHOLD, CPS_FLAG_THRESHOLD, DEFAULT_SPEAKER_ID, MIN_SEGMENT_DURATION_SEC,
};
pub use vad::{
    detect_speech_segments, generate_synthetic_speech_signal, process_vad_pipeline_async,
    prune_silence, split_into_natural_pause_chunks, AudioChunk, ChunkSplitConfig, SpeechSegment,
    VadConfig, VadPruneResult, DEFAULT_MAX_SILENCE_GAP_MS, DEFAULT_MIN_SILENCE_DURATION_MS,
    DEFAULT_MIN_SPEECH_DURATION_MS, DEFAULT_NEG_THRESHOLD, DEFAULT_SPEECH_PAD_MS,
    DEFAULT_THRESHOLD, DEFAULT_VAD_SAMPLE_RATE, DEFAULT_WINDOW_SIZE_SAMPLES,
};
pub use vad_bridge::{
    align_segments_with_vad, SpeechInterval, BOUNDARY_SNAP_TOLERANCE_SEC,
    WORD_GAP_SPLIT_THRESHOLD_SEC,
};
pub use whisper_sidecar::{
    resolve_whisper_binary, run_whisper_transcription, SttProgressPayload, WhisperSidecarConfig,
};
