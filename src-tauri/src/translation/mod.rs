//! Translation and Subtitle Localization Subsystem (`TASK-M4-17`, `TASK-M4-18`, `M4.5`).
//!
//! Provides Google Gemini 1.5 Flash translation client with Theatrical Framing,
//! `BLOCK_NONE` safety settings, defensive safety fallback (`ERR-07`), chunking with
//! sliding context windows, Community Lore Dictionary (12 terms per PRD §6.2),
//! Unified Pronoun Matrix & Style Presets (PRD §6.1, ADR-006), and Hybrid Speaker Diarization.

pub mod diarization;
pub mod gemini_client;
pub mod lore_dictionary;
pub mod prompt_builder;
pub mod pronoun_matrix;

pub use diarization::{
    infer_speaker_heuristic, normalize_speaker_id, resolve_speaker_id, HybridDiarizer,
    SpeakerInferenceResult, VALID_SPEAKER_IDS,
};

pub use gemini_client::{
    strip_markdown_code_fence, GeminiCandidate, GeminiClient, GeminiClientConfig, GeminiContent,
    GeminiGenerationConfig, GeminiPart, GeminiRequest, GeminiResponse, GeminiSafetySetting,
    MockHttpHandler, TranslatedSegmentItem, TransportResponse, BLOCK_NONE_THRESHOLD,
    DEFAULT_GEMINI_API_URL, DEFAULT_GEMINI_MODEL, DEFAULT_RETRY_BASE_DELAY_MS,
    FORMAT_FALLBACK_PREFIX, MAX_RATE_LIMIT_RETRIES, SAFETY_CATEGORIES, SAFETY_FALLBACK_PREFIX,
};

pub use lore_dictionary::{
    contains_approved_translation, detect_forbidden_translations, find_lore_term,
    format_lore_prompt_section, get_lore_terms, LoreTerm, LORE_TERMS,
};

pub use prompt_builder::{
    DialogueChunk, PromptBuilder, CONTEXT_WINDOW_HEADER, DEFAULT_CHUNK_SIZE,
    DEFAULT_CONTEXT_WINDOW_SIZE, DIALOGUE_TARGET_HEADER, SUBTITLE_GUIDELINES,
    THEATRICAL_FRAMING_INSTRUCTION,
};

pub use pronoun_matrix::{
    format_pronoun_matrix_prompt_section, get_character_pronoun_rule, get_standard_guest_profile,
    CanonicalSpeaker, CharacterPronounRule, GuestProfile, StylePreset, CHARACTER_PRONOUN_RULES,
};

use serde::{Deserialize, Serialize};

/// Progress payload emitted across the Tauri IPC bridge (`translation:progress`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TranslationProgressPayload {
    /// Clip identifier being translated.
    pub clip_id: String,
    /// Number of segments translated so far.
    pub translated_count: usize,
    /// Total number of segments in the clip.
    pub total_count: usize,
}
