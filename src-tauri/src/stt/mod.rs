//! Speech-to-Text (STT) and subtitle extraction pipeline for NeuroClip.
//!
//! Provides local Whisper.cpp process supervision, Q5_K quantized model management,
//! zero-panic transcript parsing adhering to Schema 5.5, VAD pause-alignment,
//! and dual `.ass` / `.srt` subtitle export engines (`ADR-001`, `ADR-005`, `ADR-007`).

pub mod model_manager;
pub mod subtitle_export;
pub mod transcript_parser;
pub mod vad_bridge;
pub mod whisper_sidecar;

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
pub use vad_bridge::{
    align_segments_with_vad, SpeechInterval, BOUNDARY_SNAP_TOLERANCE_SEC,
    WORD_GAP_SPLIT_THRESHOLD_SEC,
};
pub use whisper_sidecar::{
    resolve_whisper_binary, run_whisper_transcription, SttProgressPayload, WhisperSidecarConfig,
};
