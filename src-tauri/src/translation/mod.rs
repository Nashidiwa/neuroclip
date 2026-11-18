//! Translation and Subtitle Localization Subsystem (`TASK-M4-17`, `M4.5`).
//!
//! Provides Google Gemini 1.5 Flash translation client with Theatrical Framing,
//! `BLOCK_NONE` safety settings, defensive safety fallback (`ERR-07`), chunking with
//! sliding context windows, and Tauri IPC command bindings.

pub mod gemini_client;
pub mod prompt_builder;

pub use gemini_client::{
    strip_markdown_code_fence, GeminiCandidate, GeminiClient, GeminiClientConfig, GeminiContent,
    GeminiGenerationConfig, GeminiPart, GeminiRequest, GeminiResponse, GeminiSafetySetting,
    MockHttpHandler, TranslatedSegmentItem, TransportResponse, BLOCK_NONE_THRESHOLD,
    DEFAULT_GEMINI_API_URL, DEFAULT_GEMINI_MODEL, DEFAULT_RETRY_BASE_DELAY_MS,
    FORMAT_FALLBACK_PREFIX, MAX_RATE_LIMIT_RETRIES, SAFETY_CATEGORIES, SAFETY_FALLBACK_PREFIX,
};

pub use prompt_builder::{
    DialogueChunk, PromptBuilder, CONTEXT_WINDOW_HEADER, DEFAULT_CHUNK_SIZE,
    DEFAULT_CONTEXT_WINDOW_SIZE, DIALOGUE_TARGET_HEADER, SUBTITLE_GUIDELINES,
    THEATRICAL_FRAMING_INSTRUCTION,
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
