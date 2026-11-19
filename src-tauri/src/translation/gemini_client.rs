//! Google Gemini 1.5 Flash Translation Client (`TASK-M4-17`, `M4.5`).
//!
//! Provides high-throughput subtitle localization via Google Gemini 1.5 Flash API with:
//! - Theatrical Framing System Prompt packaging (`ADR-001`, `PRD.md` §6.3).
//! - `BLOCK_NONE` safety threshold configuration across all 4 categories (`UT-17.2`).
//! - Defensive Safety Fallback (`ERR-07`, `ED-17.1`): preserves original English dialogue,
//!   flags with `is_flagged_for_review: true`, and never crashes or aborts the batch.
//! - Sliding context window (5 previous lines) across 25–35 line dialogue chunks (`AC-17.1`).
//! - Exponential backoff retry for HTTP 429 rate limiting (`IT-15.2` / `TASK-M4-17`).
//! - In-memory mock transport for hermetic offline tests (`CONVENTIONS.md` §7.1).
//! - Panic-free error handling with zero unwrap or expect calls (`CONVENTIONS.md` §2.1).
//! - Clean error propagation using `Result` and `AppError`.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tracing::{debug, info, warn};

use crate::error::{AppError, TranslationError};
use crate::logging::redact_secret;
use crate::stt::transcript_parser::{TranscriptSegment, TranscriptSegmentsContainer};
use crate::translation::prompt_builder::{
    DialogueChunk, PromptBuilder, DEFAULT_CHUNK_SIZE, DEFAULT_CONTEXT_WINDOW_SIZE,
};

/// Default Google AI Studio base URL.
pub const DEFAULT_GEMINI_API_URL: &str = "https://generativelanguage.googleapis.com";

/// Default fast translation model.
pub const DEFAULT_GEMINI_MODEL: &str = "gemini-1.5-flash";

/// Maximum rate limit retry attempts on HTTP 429.
pub const MAX_RATE_LIMIT_RETRIES: u32 = 3;

/// Default base retry backoff in milliseconds (1 second).
pub const DEFAULT_RETRY_BASE_DELAY_MS: u64 = 1_000;

/// Fallback text prefix when Gemini safety filter is triggered (`PRD.md` §6.3, `ERR-07`).
pub const SAFETY_FALLBACK_PREFIX: &str = "[Cần dịch thủ công do bộ lọc an toàn AI]";

/// Fallback text prefix when LLM output format is corrupted.
pub const FORMAT_FALLBACK_PREFIX: &str = "[Cần dịch thủ công do lỗi phản hồi AI]";

/// Four Google safety categories required to be configured with `BLOCK_NONE` (`UT-17.2`).
pub const SAFETY_CATEGORIES: [&str; 4] = [
    "HARM_CATEGORY_HARASSMENT",
    "HARM_CATEGORY_HATE_SPEECH",
    "HARM_CATEGORY_SEXUALLY_EXPLICIT",
    "HARM_CATEGORY_DANGEROUS_CONTENT",
];

/// Constant for minimal blocking threshold (`UT-17.2`).
pub const BLOCK_NONE_THRESHOLD: &str = "BLOCK_NONE";

/// Simulated HTTP response for mock transport testing without sockets (`CONVENTIONS.md` §7.1).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TransportResponse {
    pub status_code: u16,
    pub retry_after: Option<String>,
    pub body: String,
}

impl TransportResponse {
    #[must_use]
    pub fn ok(json_body: impl Into<String>) -> Self {
        Self {
            status_code: 200,
            retry_after: None,
            body: json_body.into(),
        }
    }

    #[must_use]
    pub fn rate_limit(retry_after_secs: Option<&str>, body: impl Into<String>) -> Self {
        Self {
            status_code: 429,
            retry_after: retry_after_secs.map(ToString::to_string),
            body: body.into(),
        }
    }

    #[must_use]
    pub fn unauthorized(body: impl Into<String>) -> Self {
        Self {
            status_code: 401,
            retry_after: None,
            body: body.into(),
        }
    }

    #[must_use]
    pub fn forbidden(body: impl Into<String>) -> Self {
        Self {
            status_code: 403,
            retry_after: None,
            body: body.into(),
        }
    }

    #[must_use]
    pub fn bad_request(body: impl Into<String>) -> Self {
        Self {
            status_code: 400,
            retry_after: None,
            body: body.into(),
        }
    }
}

/// Closure type for mock transport testing.
pub type MockHttpHandler =
    Arc<dyn Fn(&str, &str, &str, &[u8]) -> Result<TransportResponse, AppError> + Send + Sync>;

/// Configuration options for [`GeminiClient`].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GeminiClientConfig {
    /// Gemini API key (e.g. `AIza...`).
    pub api_key: String,
    /// Base URL override (defaults to `https://generativelanguage.googleapis.com`).
    #[serde(default)]
    pub base_url: Option<String>,
    /// Model identifier (defaults to `gemini-1.5-flash`).
    #[serde(default = "default_model")]
    pub model: String,
    /// Sampling temperature (default 0.3 for consistent translation).
    #[serde(default = "default_temperature")]
    pub temperature: Option<f32>,
    /// Chunk size in dialogue lines (default 30, range 25–35).
    #[serde(default = "default_chunk_size")]
    pub chunk_size: usize,
    /// Sliding context window size (default 5 lines).
    #[serde(default = "default_context_size")]
    pub context_window_size: usize,
    /// Maximum retry attempts on HTTP 429.
    #[serde(default = "default_max_retries")]
    pub max_retries: u32,
    /// Base backoff delay in milliseconds.
    #[serde(default = "default_retry_delay_ms")]
    pub retry_base_delay_ms: u64,
    /// Style preset override (e.g. "TikTok Gen Z Trend" vs "Formal Subber").
    #[serde(default)]
    pub style_preset: Option<String>,
    /// Participating collab cast members (e.g. `["Anny", "Camila", "Filian"]`).
    #[serde(default)]
    pub cast_members: Option<Vec<String>>,
}

fn default_model() -> String {
    DEFAULT_GEMINI_MODEL.to_string()
}

// Serde default callback for `Option<f32>` field `GeminiClientConfig::temperature`.
#[allow(clippy::unnecessary_wraps)]
fn default_temperature() -> Option<f32> {
    Some(0.3)
}

const fn default_chunk_size() -> usize {
    DEFAULT_CHUNK_SIZE
}

const fn default_context_size() -> usize {
    DEFAULT_CONTEXT_WINDOW_SIZE
}

const fn default_max_retries() -> u32 {
    MAX_RATE_LIMIT_RETRIES
}

const fn default_retry_delay_ms() -> u64 {
    DEFAULT_RETRY_BASE_DELAY_MS
}

impl GeminiClientConfig {
    #[must_use]
    pub fn new(api_key: String) -> Self {
        Self {
            api_key,
            base_url: None,
            model: default_model(),
            temperature: default_temperature(),
            chunk_size: default_chunk_size(),
            context_window_size: default_context_size(),
            max_retries: default_max_retries(),
            retry_base_delay_ms: default_retry_delay_ms(),
            style_preset: None,
            cast_members: None,
        }
    }

    #[must_use]
    pub fn with_style_preset(mut self, style_preset: String) -> Self {
        self.style_preset = Some(style_preset);
        self
    }

    #[must_use]
    pub fn with_cast_members(mut self, cast_members: Vec<String>) -> Self {
        self.cast_members = Some(cast_members);
        self
    }

    #[must_use]
    pub fn with_base_url(mut self, base_url: String) -> Self {
        self.base_url = Some(base_url);
        self
    }

    #[must_use]
    pub fn with_model(mut self, model: String) -> Self {
        self.model = model;
        self
    }

    #[must_use]
    pub fn with_chunk_size(mut self, chunk_size: usize) -> Self {
        self.chunk_size = chunk_size;
        self
    }

    #[must_use]
    pub fn with_context_window_size(mut self, context_size: usize) -> Self {
        self.context_window_size = context_size;
        self
    }

    #[must_use]
    pub fn with_retry_delay_ms(mut self, delay_ms: u64) -> Self {
        self.retry_base_delay_ms = delay_ms;
        self
    }
}

// ---------------------------------------------------------------------------
// Gemini Wire Request Schemas
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GeminiRequest {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub system_instruction: Option<GeminiSystemInstruction>,
    pub contents: Vec<GeminiContent>,
    pub safety_settings: Vec<GeminiSafetySetting>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub generation_config: Option<GeminiGenerationConfig>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GeminiSystemInstruction {
    pub parts: Vec<GeminiPart>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GeminiContent {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub role: Option<String>,
    pub parts: Vec<GeminiPart>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GeminiPart {
    pub text: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GeminiSafetySetting {
    pub category: String,
    pub threshold: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GeminiGenerationConfig {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub response_mime_type: Option<String>,
}

// ---------------------------------------------------------------------------
// Gemini Wire Response Schemas
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GeminiResponse {
    #[serde(default)]
    pub candidates: Vec<GeminiCandidate>,
    #[serde(default)]
    pub prompt_feedback: Option<GeminiPromptFeedback>,
    #[serde(default)]
    pub error: Option<GeminiErrorBody>,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GeminiCandidate {
    #[serde(default)]
    pub content: Option<GeminiCandidateContent>,
    #[serde(default)]
    pub finish_reason: Option<String>,
    #[serde(default)]
    pub safety_ratings: Vec<GeminiSafetyRating>,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GeminiCandidateContent {
    #[serde(default)]
    pub role: Option<String>,
    #[serde(default)]
    pub parts: Vec<GeminiPart>,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GeminiPromptFeedback {
    #[serde(default)]
    pub block_reason: Option<String>,
    #[serde(default)]
    pub safety_ratings: Vec<GeminiSafetyRating>,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GeminiSafetyRating {
    pub category: String,
    pub probability: Option<String>,
    #[serde(default)]
    pub blocked: Option<bool>,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct GeminiErrorBody {
    pub code: Option<u16>,
    pub message: Option<String>,
    pub status: Option<String>,
}

/// Structured item parsed from model's JSON response array.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TranslatedSegmentItem {
    pub segment_id: String,
    #[serde(default)]
    pub speaker_id: Option<String>,
    pub text_vi: String,
}

// ---------------------------------------------------------------------------
// GeminiClient
// ---------------------------------------------------------------------------

/// Google Gemini 1.5 Flash translation client.
#[derive(Clone)]
pub struct GeminiClient {
    client: reqwest::Client,
    config: GeminiClientConfig,
    mock_handler: Option<MockHttpHandler>,
}

impl std::fmt::Debug for GeminiClient {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GeminiClient")
            .field("config", &self.config)
            .field("has_mock_handler", &self.mock_handler.is_some())
            .finish_non_exhaustive()
    }
}

impl GeminiClient {
    /// Initialize a new Gemini client with an API key.
    ///
    /// # Errors
    /// Returns [`AppError::Translation`] if the API key is empty or HTTP client initialization fails.
    pub fn new(api_key: String) -> Result<Self, AppError> {
        Self::from_config(GeminiClientConfig::new(api_key))
    }

    /// Initialize a new Gemini client from full configuration.
    ///
    /// # Errors
    /// Returns [`AppError::Translation`] if the API key is empty or HTTP client initialization fails.
    pub fn from_config(config: GeminiClientConfig) -> Result<Self, AppError> {
        if config.api_key.trim().is_empty() {
            return Err(AppError::Translation(TranslationError::InvalidApiKey));
        }

        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(60))
            .no_proxy()
            .build()
            .map_err(|e| {
                AppError::Translation(TranslationError::ApiRequestFailed {
                    status: 0,
                    message: format!("Failed to build HTTP client: {e}"),
                })
            })?;

        Ok(Self {
            client,
            config,
            mock_handler: None,
        })
    }

    /// Attach an in-memory mock HTTP transport handler for hermetic testing (`CONVENTIONS.md` §7.1).
    #[must_use]
    pub fn with_mock_handler(mut self, handler: MockHttpHandler) -> Self {
        self.mock_handler = Some(handler);
        self
    }

    /// Reference to client configuration.
    #[must_use]
    pub fn config(&self) -> &GeminiClientConfig {
        &self.config
    }

    /// Resolve effective generateContent endpoint URL.
    #[must_use]
    pub fn endpoint_url(&self) -> String {
        let base = if let Some(ref url) = self.config.base_url {
            url.clone()
        } else if let Ok(env_url) = std::env::var("NEUROCLIP_GEMINI_API_URL") {
            env_url
        } else {
            DEFAULT_GEMINI_API_URL.to_string()
        };

        format!(
            "{}/v1beta/models/{}:generateContent",
            base.trim_end_matches('/'),
            self.config.model
        )
    }

    /// Helper to construct the 4 standard `BLOCK_NONE` safety settings (`UT-17.2`).
    #[must_use]
    pub fn build_safety_settings() -> Vec<GeminiSafetySetting> {
        SAFETY_CATEGORIES
            .iter()
            .map(|cat| GeminiSafetySetting {
                category: (*cat).to_string(),
                threshold: BLOCK_NONE_THRESHOLD.to_string(),
            })
            .collect()
    }

    /// Test API key validity with a lightweight 1-token request.
    ///
    /// # Errors
    /// Returns [`AppError::Translation`] if authentication or network transport fails.
    pub async fn test_connection(&self) -> Result<bool, AppError> {
        let endpoint = self.endpoint_url();
        let payload = GeminiRequest {
            system_instruction: None,
            contents: vec![GeminiContent {
                role: Some("user".to_string()),
                parts: vec![GeminiPart {
                    text: "Ping".to_string(),
                }],
            }],
            safety_settings: Self::build_safety_settings(),
            generation_config: Some(GeminiGenerationConfig {
                temperature: Some(0.0),
                response_mime_type: None,
            }),
        };

        let body_bytes = serde_json::to_vec(&payload).map_err(|e| {
            AppError::Translation(TranslationError::ApiRequestFailed {
                status: 0,
                message: format!("Failed serializing test request: {e}"),
            })
        })?;

        let (status, _retry_after, body_str) =
            self.send_http_with_retry(&endpoint, &body_bytes).await?;

        if (200..300).contains(&status) {
            Ok(true)
        } else if status == 400 || status == 401 || status == 403 {
            warn!(
                status = status,
                key = %redact_secret(&self.config.api_key),
                "Gemini API authentication failed during connection test"
            );
            Err(AppError::Translation(TranslationError::InvalidApiKey))
        } else {
            Err(AppError::Translation(TranslationError::ApiRequestFailed {
                status,
                message: body_str,
            }))
        }
    }

    /// Translate a list of transcript segments adhering to Schema 5.5.
    ///
    /// Manages dialogue chunking (25–35 lines, default 30), 5-line sliding context window,
    /// defensive safety fallback (`ERR-07`), rate-limiting exponential backoff, and progress emission.
    ///
    /// # Errors
    /// Returns [`AppError::Translation`] if any chunk fails authentication, transport, or retry limits.
    pub async fn translate_segments(
        &self,
        clip_id: &str,
        segments: &[TranscriptSegment],
        on_progress: Option<Arc<dyn Fn(usize, usize) + Send + Sync>>,
    ) -> Result<TranscriptSegmentsContainer, AppError> {
        if segments.is_empty() {
            return Ok(TranscriptSegmentsContainer {
                clip_id: clip_id.to_string(),
                segments: Vec::new(),
            });
        }

        let chunks = PromptBuilder::create_chunks(
            segments,
            self.config.chunk_size,
            self.config.context_window_size,
        );

        let total_count = segments.len();
        let mut translated_segments: Vec<TranscriptSegment> = Vec::with_capacity(total_count);

        info!(
            clip_id = %clip_id,
            total_lines = total_count,
            chunks_count = chunks.len(),
            "Starting Gemini batch translation with Theatrical Framing"
        );

        for chunk in chunks {
            let chunk_result = self.translate_chunk(&chunk).await?;
            translated_segments.extend(chunk_result);

            if let Some(ref cb) = on_progress {
                cb(translated_segments.len(), total_count);
            }
        }

        // Apply smart Vietnamese line wrapping to translated segments
        for seg in &mut translated_segments {
            if let Some(ref vi) = seg.text_vi {
                // Do not line-wrap defensive fallback notices (safety filter or formatting errors)
                // because they contain verbatim original English dialogue awaiting manual review.
                if !vi.trim().is_empty()
                    && !vi.starts_with(SAFETY_FALLBACK_PREFIX)
                    && !vi.starts_with(FORMAT_FALLBACK_PREFIX)
                {
                    let wrapped = crate::translation::line_wrapper::wrap_vietnamese_text_to_string(
                        vi,
                        crate::translation::line_wrapper::DEFAULT_MAX_CHARS_PER_LINE,
                        crate::translation::line_wrapper::DEFAULT_MAX_LINES,
                    );
                    seg.text_vi = Some(wrapped);
                }
            }
        }

        // Run CPS time borrowing optimization
        crate::translation::cps_optimizer::optimize_cps_time_borrowing_default(
            &mut translated_segments,
        );

        // Defensive Invariant (ERR-07): segments with safety or format fallback notices
        // MUST remain flagged for manual human review, regardless of CPS duration borrowing.
        for seg in &mut translated_segments {
            if let Some(ref vi) = seg.text_vi {
                if vi.starts_with(SAFETY_FALLBACK_PREFIX) || vi.starts_with(FORMAT_FALLBACK_PREFIX) {
                    seg.is_flagged_for_review = true;
                }
            }
        }

        Ok(TranscriptSegmentsContainer {
            clip_id: clip_id.to_string(),
            segments: translated_segments,
        })
    }

    /// Translate a single dialogue chunk with sliding context.
    ///
    /// # Errors
    /// Returns [`AppError::Translation`] if serialization, authentication, or HTTP transport fails.
    #[allow(clippy::too_many_lines)]
    pub async fn translate_chunk(
        &self,
        chunk: &DialogueChunk,
    ) -> Result<Vec<TranscriptSegment>, AppError> {
        if chunk.items.is_empty() {
            return Ok(Vec::new());
        }

        let endpoint = self.endpoint_url();
        let style = self
            .config
            .style_preset
            .as_deref()
            .map(crate::translation::pronoun_matrix::StylePreset::from_str_loose);
        let cast = self.config.cast_members.as_deref();
        let system_instruction_text =
            PromptBuilder::build_system_instruction_with_options(style, cast);
        let user_prompt_text = PromptBuilder::build_user_prompt(&chunk.items, &chunk.context);

        let request_payload = GeminiRequest {
            system_instruction: Some(GeminiSystemInstruction {
                parts: vec![GeminiPart {
                    text: system_instruction_text,
                }],
            }),
            contents: vec![GeminiContent {
                role: Some("user".to_string()),
                parts: vec![GeminiPart {
                    text: user_prompt_text,
                }],
            }],
            safety_settings: Self::build_safety_settings(),
            generation_config: Some(GeminiGenerationConfig {
                temperature: self.config.temperature,
                response_mime_type: Some("application/json".to_string()),
            }),
        };

        let body_bytes = serde_json::to_vec(&request_payload).map_err(|e| {
            AppError::Translation(TranslationError::ApiRequestFailed {
                status: 0,
                message: format!("Failed serializing Gemini request: {e}"),
            })
        })?;

        debug!(
            chunk_index = chunk.chunk_index,
            items_count = chunk.items.len(),
            context_count = chunk.context.len(),
            "Emitting Gemini translation request"
        );

        let (status, _retry_after, body_str) =
            self.send_http_with_retry(&endpoint, &body_bytes).await?;

        if status == 401 || status == 403 || (status == 400 && body_str.contains("API_KEY_INVALID"))
        {
            warn!(
                status = status,
                key = %redact_secret(&self.config.api_key),
                "Gemini API authentication failed during chunk translation"
            );
            return Err(AppError::Translation(TranslationError::InvalidApiKey));
        }

        if !(200..300).contains(&status) {
            return Err(AppError::Translation(TranslationError::ApiRequestFailed {
                status,
                message: body_str,
            }));
        }

        let response: GeminiResponse = serde_json::from_str(&body_str).map_err(|e| {
            AppError::Translation(TranslationError::ApiRequestFailed {
                status,
                message: format!("Failed parsing Gemini JSON envelope: {e}. Body: {body_str}"),
            })
        })?;

        // Defensive Safety Fallback Check (ERR-07 / ED-17.1)
        if Self::is_safety_blocked(&response) {
            warn!(
                chunk_index = chunk.chunk_index,
                "Gemini safety filter triggered on chunk. Activating defensive fallback."
            );
            return Ok(Self::apply_safety_fallback(&chunk.items));
        }

        // Extract raw JSON content from first candidate
        let Some(candidate_text) = response
            .candidates
            .first()
            .and_then(|c| c.content.as_ref())
            .and_then(|content| content.parts.first())
            .map(|p| p.text.as_str())
        else {
            warn!(
                chunk_index = chunk.chunk_index,
                "Gemini candidate returned empty text without safety reason. Activating fallback."
            );
            return Ok(Self::apply_safety_fallback(&chunk.items));
        };

        // Strip markdown code fences if present (```json ... ```)
        let cleaned_json = strip_markdown_code_fence(candidate_text);

        let parsed_items: Vec<TranslatedSegmentItem> = match serde_json::from_str(cleaned_json) {
            Ok(items) => items,
            Err(e) => {
                warn!(
                    chunk_index = chunk.chunk_index,
                    error = %e,
                    "Failed parsing translation JSON items from Gemini response. Activating formatting fallback."
                );
                return Ok(Self::apply_formatting_fallback(&chunk.items));
            }
        };

        let empty_cast: [String; 0] = [];
        let cast_slice = self.config.cast_members.as_deref().unwrap_or(&empty_cast);
        Ok(Self::merge_translations_with_cast(
            &chunk.items,
            &parsed_items,
            cast_slice,
        ))
    }

    /// Check whether a response was rejected by safety filtering (`finishReason: "SAFETY"` or `blockReason: "SAFETY"`).
    #[must_use]
    pub fn is_safety_blocked(response: &GeminiResponse) -> bool {
        if let Some(ref feedback) = response.prompt_feedback {
            if let Some(ref reason) = feedback.block_reason {
                let r = reason.trim().to_uppercase();
                if r == "SAFETY" || r == "BLOCKED" || r == "PROHIBITED_CONTENT" {
                    return true;
                }
            }
        }

        if response.candidates.is_empty() {
            return true;
        }

        if let Some(candidate) = response.candidates.first() {
            if let Some(ref reason) = candidate.finish_reason {
                let r = reason.trim().to_uppercase();
                if r == "SAFETY" || r == "RECITATION" || r == "BLOCKLIST" {
                    return true;
                }
            }

            if let Some(ref content) = candidate.content {
                if content.parts.is_empty() {
                    return true;
                }
            } else {
                return true;
            }
        }

        false
    }

    /// Transform segments with Defensive Safety Fallback (`ERR-07` / `ED-17.1`).
    ///
    /// Preserves original English dialogue, prefixes `[Cần dịch thủ công do bộ lọc an toàn AI]`,
    /// and marks `is_flagged_for_review: true`.
    #[must_use]
    pub fn apply_safety_fallback(segments: &[TranscriptSegment]) -> Vec<TranscriptSegment> {
        segments
            .iter()
            .map(|seg| {
                let mut fallback = seg.clone();
                fallback.text_vi =
                    Some(format!("{} {}", SAFETY_FALLBACK_PREFIX, seg.text_en.trim()));
                fallback.is_flagged_for_review = true;
                fallback
            })
            .collect()
    }

    /// Transform segments when model response has invalid JSON structure.
    #[must_use]
    pub fn apply_formatting_fallback(segments: &[TranscriptSegment]) -> Vec<TranscriptSegment> {
        segments
            .iter()
            .map(|seg| {
                let mut fallback = seg.clone();
                fallback.text_vi =
                    Some(format!("{} {}", FORMAT_FALLBACK_PREFIX, seg.text_en.trim()));
                fallback.is_flagged_for_review = true;
                fallback
            })
            .collect()
    }

    fn apply_translated_item(
        updated: &mut TranscriptSegment,
        original: &TranscriptSegment,
        item: &TranslatedSegmentItem,
        cast_members: &[String],
    ) {
        if let Some(ref raw_speaker) = item.speaker_id {
            let trimmed = raw_speaker.trim();
            if !trimmed.is_empty() {
                let normalized =
                    crate::translation::diarization::normalize_speaker_id(trimmed, cast_members);
                if normalized != "unknown" {
                    updated.speaker_id = normalized.to_string();
                } else if updated.speaker_id.is_empty() || updated.speaker_id == "unknown" {
                    let fallback = crate::translation::diarization::infer_speaker_heuristic(
                        &original.text_en,
                        None,
                        cast_members,
                    );
                    updated.speaker_id = fallback.speaker_id.to_string();
                }
            } else if updated.speaker_id.is_empty() || updated.speaker_id == "unknown" {
                let fallback = crate::translation::diarization::infer_speaker_heuristic(
                    &original.text_en,
                    None,
                    cast_members,
                );
                updated.speaker_id = fallback.speaker_id.to_string();
            }
        } else if updated.speaker_id.is_empty() || updated.speaker_id == "unknown" {
            let fallback = crate::translation::diarization::infer_speaker_heuristic(
                &original.text_en,
                None,
                cast_members,
            );
            updated.speaker_id = fallback.speaker_id.to_string();
        }

        let text = item.text_vi.trim().to_string();
        if text.is_empty() {
            updated.text_vi = Some(format!(
                "{} {}",
                SAFETY_FALLBACK_PREFIX,
                original.text_en.trim()
            ));
            updated.is_flagged_for_review = true;
        } else {
            updated.text_vi = Some(text);
        }
    }

    /// Merge translated items back into the original transcript segments.
    #[must_use]
    pub fn merge_translations(
        original: &[TranscriptSegment],
        translated: &[TranslatedSegmentItem],
    ) -> Vec<TranscriptSegment> {
        Self::merge_translations_with_cast(original, translated, &[])
    }

    /// Merge translated items back into the original transcript segments with guest cast support.
    #[must_use]
    pub fn merge_translations_with_cast(
        original: &[TranscriptSegment],
        translated: &[TranslatedSegmentItem],
        cast_members: &[String],
    ) -> Vec<TranscriptSegment> {
        let mut lookup: HashMap<&str, &TranslatedSegmentItem> =
            HashMap::with_capacity(translated.len());
        for item in translated {
            lookup.insert(item.segment_id.as_str(), item);
        }

        original
            .iter()
            .enumerate()
            .map(|(idx, seg)| {
                let mut updated = seg.clone();
                if let Some(item) = lookup.get(seg.segment_id.as_str()) {
                    Self::apply_translated_item(&mut updated, seg, item, cast_members);
                } else if let Some(item) = translated.get(idx) {
                    // Fallback to sequential index if segment_id was slightly mutated
                    Self::apply_translated_item(&mut updated, seg, item, cast_members);
                } else {
                    // Segment missing from response
                    let fallback_speaker = crate::translation::diarization::resolve_speaker_id(
                        None,
                        &seg.text_en,
                        None,
                        cast_members,
                    );
                    updated.speaker_id = fallback_speaker.to_string();
                    updated.text_vi =
                        Some(format!("{} {}", SAFETY_FALLBACK_PREFIX, seg.text_en.trim()));
                    updated.is_flagged_for_review = true;
                }
                updated
            })
            .collect()
    }

    /// Internal HTTP transport with rate limit exponential backoff retry.
    async fn send_http_with_retry(
        &self,
        endpoint: &str,
        body_bytes: &[u8],
    ) -> Result<(u16, Option<String>, String), AppError> {
        let mut attempts = 0u32;
        let auth_header = format!("Bearer {}", self.config.api_key);
        let content_type = "application/json";

        loop {
            let (status_code, retry_after, body) = if let Some(ref mock) = self.mock_handler {
                let resp = mock(endpoint, &auth_header, content_type, body_bytes)?;
                (resp.status_code, resp.retry_after, resp.body)
            } else {
                let req = self
                    .client
                    .post(endpoint)
                    .header("x-goog-api-key", &self.config.api_key)
                    .header("Content-Type", content_type)
                    .body(body_bytes.to_vec());

                match req.send().await {
                    Ok(response) => {
                        let status = response.status().as_u16();
                        let retry_after = response
                            .headers()
                            .get("retry-after")
                            .and_then(|h| h.to_str().ok())
                            .map(ToString::to_string);
                        let body = response.text().await.map_err(|e| {
                            AppError::Translation(TranslationError::ApiRequestFailed {
                                status,
                                message: format!("Failed reading Gemini response body: {e}"),
                            })
                        })?;
                        (status, retry_after, body)
                    }
                    Err(e) => {
                        attempts += 1;
                        if attempts <= self.config.max_retries {
                            let delay_ms = self
                                .config
                                .retry_base_delay_ms
                                .saturating_mul(1 << (attempts - 1));
                            warn!(
                                attempt = attempts,
                                error = %e,
                                "Network connection error connecting to Gemini, retrying"
                            );
                            tokio::time::sleep(Duration::from_millis(delay_ms)).await;
                            continue;
                        }

                        return Err(AppError::Translation(TranslationError::ApiRequestFailed {
                            status: 0,
                            message: format!("Gemini network request failed after retries: {e}"),
                        }));
                    }
                }
            };

            // Handle HTTP 429 Rate Limiting with exponential backoff
            if status_code == 429 {
                attempts += 1;
                if attempts > self.config.max_retries {
                    warn!(
                        attempts = attempts,
                        "Gemini API rate limit retries exhausted"
                    );
                    return Err(AppError::Translation(TranslationError::ApiRequestFailed {
                        status: 429,
                        message: format!(
                            "Gemini rate limit exceeded after {} retries: {}",
                            self.config.max_retries, body
                        ),
                    }));
                }

                let delay_ms = self
                    .config
                    .retry_base_delay_ms
                    .saturating_mul(1 << (attempts - 1));
                warn!(
                    attempt = attempts,
                    delay_ms = delay_ms,
                    "Gemini API returned HTTP 429 Too Many Requests, backing off"
                );
                tokio::time::sleep(Duration::from_millis(delay_ms)).await;
                continue;
            }

            return Ok((status_code, retry_after, body));
        }
    }
}

/// Strip markdown code fences (e.g. ````json\n[...]\n```` or ````\n[...]\n````).
#[must_use]
pub fn strip_markdown_code_fence(input: &str) -> &str {
    let trimmed = input.trim();
    if let Some(stripped) = trimmed.strip_prefix("```") {
        let rest = if let Some(newline_idx) = stripped.find('\n') {
            &stripped[newline_idx + 1..]
        } else {
            stripped
        };

        if let Some(end_idx) = rest.rfind("```") {
            rest[..end_idx].trim()
        } else {
            rest.trim()
        }
    } else {
        trimmed
    }
}
