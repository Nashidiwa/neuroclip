//! Groq Whisper Cloud Speech-to-Text (STT) Client (`TASK-M4-15`, `M4.4`).
//!
//! Provides high-throughput, OpenAI-compatible cloud transcription via Groq Cloud API
//! using `whisper-large-v3`. Implements:
//! - Auto-chunking <= 20MB with Silero VAD pause split and 1.5s lookback overlap (`AC-15.2`, `UT-15.1`, `ED-15.1`).
//! - Anti-repetition / Karaoke guardrail (`condition_on_previous_text=false`) for music streams (`AC-15.3`, `UT-15.2`).
//! - Exponential backoff rate limit retry (1s, 2s, 4s, max 3 retries) for HTTP 429 (`IT-15.2`).
//! - Zero-panic error handling mapped to Schema 5.5 [`TranscriptSegmentsContainer`] (`IT-15.1`).
//! - Secret-redacting diagnostics (`CONVENTIONS.md` §6.3).

use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::time::Duration;
use tracing::{debug, info, warn};

use crate::error::{AppError, SttError};
use crate::logging::redact_secret;
use crate::stt::audio_prep::PreparedAudioBuffer;
use crate::stt::transcript_parser::{
    calculate_cps, is_special_token, TranscriptSegment, TranscriptSegmentsContainer, WordTiming,
    CONFIDENCE_FLAG_THRESHOLD, CPS_FLAG_THRESHOLD, DEFAULT_SPEAKER_ID,
};
use crate::stt::vad::{
    detect_speech_segments_async, split_into_natural_pause_chunks, ChunkSplitConfig, VadConfig,
};
use crate::stt::whisper_sidecar::{ProgressCallback, SttProgressPayload};

/// Default Groq Cloud API OpenAI-compatible base URL.
pub const DEFAULT_GROQ_API_URL: &str = "https://api.groq.com/openai/v1";

/// Default high-accuracy Whisper model on Groq.
pub const DEFAULT_GROQ_MODEL: &str = "whisper-large-v3";

/// Hard payload ceiling enforced by Groq Cloud API (25 MB).
pub const MAX_PAYLOAD_BYTES: usize = 25 * 1024 * 1024;

/// Safe payload threshold before triggering Silero VAD auto-chunking (20 MB).
pub const SAFE_CHUNK_BYTES: usize = 20 * 1024 * 1024;

/// Default lookback overlap duration in milliseconds (1.5 seconds).
pub const DEFAULT_OVERLAP_MS: u64 = 1_500;

/// Maximum number of retry attempts on HTTP 429 rate limit.
pub const MAX_RATE_LIMIT_RETRIES: u32 = 3;

/// Simulated HTTP response from mock transport (`CONVENTIONS.md` §7.1).
#[derive(Debug, Clone, PartialEq)]
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
    pub fn payload_too_large(body: impl Into<String>) -> Self {
        Self {
            status_code: 413,
            retry_after: None,
            body: body.into(),
        }
    }
}

/// Closure type for mock transport testing without sockets.
pub type MockHttpHandler =
    Arc<dyn Fn(&str, &str, &str, &[u8]) -> Result<TransportResponse, AppError> + Send + Sync>;

/// Configuration options for [`GroqClient`].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GroqClientConfig {
    /// Groq API key (e.g. `gsk_...`).
    pub api_key: String,
    /// Base URL override (defaults to `https://api.groq.com/openai/v1` or `NEUROCLIP_GROQ_API_URL`).
    #[serde(default)]
    pub base_url: Option<String>,
    /// Whisper model identifier (defaults to `whisper-large-v3`).
    #[serde(default = "default_model")]
    pub model: String,
    /// Music / Karaoke mode: sets `condition_on_previous_text=false` to prevent loop hallucinations.
    #[serde(default)]
    pub is_music_mode: bool,
    /// Contextual prompt for vocabulary priming.
    #[serde(default)]
    pub prompt: Option<String>,
    /// Target language ISO code (e.g. `"en"`). Auto-detected if omitted.
    #[serde(default)]
    pub language: Option<String>,
    /// Sampling temperature (0.0 to 1.0).
    #[serde(default)]
    pub temperature: Option<f32>,
}

fn default_model() -> String {
    DEFAULT_GROQ_MODEL.to_string()
}

impl GroqClientConfig {
    /// Create a new configuration with the provided API key.
    pub fn new(api_key: impl Into<String>) -> Self {
        Self {
            api_key: api_key.into(),
            base_url: None,
            model: DEFAULT_GROQ_MODEL.to_string(),
            is_music_mode: false,
            prompt: None,
            language: None,
            temperature: None,
        }
    }

    /// Set an explicit base URL (useful for testing).
    #[must_use]
    pub fn with_base_url(mut self, base_url: impl Into<String>) -> Self {
        self.base_url = Some(base_url.into());
        self
    }

    /// Enable or disable music/karaoke anti-repetition mode.
    #[must_use]
    pub fn with_music_mode(mut self, is_music_mode: bool) -> Self {
        self.is_music_mode = is_music_mode;
        self
    }
}

/// Groq Cloud Whisper API HTTP client.
#[derive(Clone)]
pub struct GroqClient {
    client: reqwest::Client,
    config: GroqClientConfig,
    mock_handler: Option<MockHttpHandler>,
}

impl std::fmt::Debug for GroqClient {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GroqClient")
            .field("config", &self.config)
            .field("has_mock_handler", &self.mock_handler.is_some())
            .finish_non_exhaustive()
    }
}

/// High-level response container for Groq `verbose_json` transcription.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GroqVerboseJsonResponse {
    #[serde(default)]
    pub task: Option<String>,
    #[serde(default)]
    pub language: Option<String>,
    #[serde(default)]
    pub duration: Option<f64>,
    pub text: String,
    #[serde(default)]
    pub words: Vec<GroqWord>,
    #[serde(default)]
    pub segments: Vec<GroqSegment>,
}

/// Word-level timing entry returned by Groq Whisper.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GroqWord {
    pub word: String,
    pub start: f64,
    pub end: f64,
}

/// Segment-level metadata block returned by Groq Whisper.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GroqSegment {
    pub id: u32,
    #[serde(default)]
    pub seek: u32,
    pub start: f64,
    pub end: f64,
    pub text: String,
    #[serde(default)]
    pub tokens: Vec<u32>,
    #[serde(default)]
    pub temperature: f64,
    #[serde(default)]
    pub avg_logprob: f64,
    #[serde(default)]
    pub compression_ratio: f64,
    #[serde(default)]
    pub no_speech_prob: f64,
}

/// Groq standard error envelope.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GroqErrorEnvelope {
    pub error: GroqErrorBody,
}

/// Detailed error payload within Groq error envelope.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GroqErrorBody {
    pub message: String,
    #[serde(rename = "type", default)]
    pub error_type: Option<String>,
    #[serde(default)]
    pub code: Option<String>,
}

impl GroqClient {
    /// Initialize a new Groq client with the given API key.
    pub fn new(api_key: String) -> Result<Self, AppError> {
        Self::from_config(GroqClientConfig::new(api_key))
    }

    /// Initialize a new Groq client with custom base URL (for WireMock testing).
    pub fn new_with_base_url(api_key: String, base_url: String) -> Result<Self, AppError> {
        Self::from_config(GroqClientConfig::new(api_key).with_base_url(base_url))
    }

    /// Initialize from a full configuration object.
    pub fn from_config(config: GroqClientConfig) -> Result<Self, AppError> {
        if config.api_key.trim().is_empty() {
            return Err(AppError::Stt(SttError::InvalidApiKey));
        }

        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(120))
            .no_proxy()
            .build()
            .map_err(|e| {
                AppError::Stt(SttError::SidecarExecutionFailed {
                    details: format!("Failed to build HTTP client: {e}"),
                })
            })?;

        Ok(Self {
            client,
            config,
            mock_handler: None,
        })
    }

    /// Attach a mock HTTP transport handler for isolated testing without network sockets (`CONVENTIONS.md` §7.1).
    #[must_use]
    pub fn with_mock_handler(mut self, handler: MockHttpHandler) -> Self {
        self.mock_handler = Some(handler);
        self
    }

    /// Reference to client configuration.
    pub fn config(&self) -> &GroqClientConfig {
        &self.config
    }

    /// Resolve effective endpoint URL for audio transcription.
    pub fn endpoint_url(&self) -> String {
        let base = if let Some(ref url) = self.config.base_url {
            url.clone()
        } else if let Ok(env_url) = std::env::var("NEUROCLIP_GROQ_API_URL") {
            env_url
        } else {
            DEFAULT_GROQ_API_URL.to_string()
        };

        format!("{}/audio/transcriptions", base.trim_end_matches('/'))
    }

    /// Transcribe in-memory WAV audio bytes adhering to Schema 5.5.
    ///
    /// If payload exceeds 20MB, runs auto-chunking using Silero VAD pause points with 1.5s lookback overlap.
    /// Rejects payloads > 25MB before upload (`AC-15.2`, `ED-15.1`).
    #[allow(clippy::cast_precision_loss)]
    pub async fn transcribe_audio_bytes(
        &self,
        clip_id: &str,
        wav_bytes: &[u8],
        is_music_mode: bool,
    ) -> Result<TranscriptSegmentsContainer, AppError> {
        let size_bytes = wav_bytes.len();
        if size_bytes == 0 {
            return Ok(TranscriptSegmentsContainer {
                clip_id: clip_id.to_string(),
                segments: Vec::new(),
            });
        }

        // ED-15.1: Reject payloads > 25MB with SttError::PayloadTooLarge
        if size_bytes > MAX_PAYLOAD_BYTES {
            let size_mb = size_bytes as f32 / (1024.0 * 1024.0);
            return Err(AppError::Stt(SttError::PayloadTooLarge { size_mb }));
        }

        // AC-15.2 & UT-15.1: If payload exceeds 20MB safe threshold, auto-chunk with Silero VAD
        if size_bytes > SAFE_CHUNK_BYTES {
            let buffer = PreparedAudioBuffer::from_wav_bytes(wav_bytes)?;
            return self
                .transcribe_samples(
                    clip_id,
                    &buffer.samples_f32,
                    buffer.sample_rate as usize,
                    is_music_mode,
                )
                .await;
        }

        // Direct single-chunk transcription
        self.transcribe_single_chunk_with_retry(clip_id, wav_bytes, is_music_mode, 0.0)
            .await
    }

    /// Transcribe 16kHz float32 audio samples, applying Silero VAD natural pause chunking
    /// and 1.5s lookback overlap when total audio exceeds 20MB (`UT-15.1`, `AC-15.2`).
    ///
    /// # Errors
    /// Returns [`AppError::Stt`] if audio encoding, chunk sizing, rate limits, or API calls fail.
    pub async fn transcribe_samples(
        &self,
        clip_id: &str,
        samples: &[f32],
        sample_rate: usize,
        is_music_mode: bool,
    ) -> Result<TranscriptSegmentsContainer, AppError> {
        self.transcribe_samples_with_progress(clip_id, samples, sample_rate, is_music_mode, None)
            .await
    }

    /// Transcribe 16kHz float32 audio samples while emitting real per-chunk [`SttProgressPayload`] updates.
    ///
    /// # Errors
    /// Returns [`AppError::Stt`] if audio encoding, chunk sizing, rate limits, or API calls fail.
    #[allow(
        clippy::too_many_lines,
        clippy::cast_precision_loss,
        clippy::cast_possible_truncation
    )]
    pub async fn transcribe_samples_with_progress(
        &self,
        clip_id: &str,
        samples: &[f32],
        sample_rate: usize,
        is_music_mode: bool,
        on_progress: Option<ProgressCallback>,
    ) -> Result<TranscriptSegmentsContainer, AppError> {
        if samples.is_empty() {
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
            return Ok(TranscriptSegmentsContainer {
                clip_id: clip_id.to_string(),
                segments: Vec::new(),
            });
        }

        let sr = if sample_rate == 0 {
            16_000
        } else {
            sample_rate
        };
        let estimated_wav_bytes = 44 + samples.len() * 2;

        // Fits in single chunk safely
        if estimated_wav_bytes <= SAFE_CHUNK_BYTES {
            if let Some(ref cb) = on_progress {
                cb(SttProgressPayload {
                    task_id: clip_id.to_string(),
                    stage: "transcribing".to_string(),
                    percent: 0.0,
                    eta_sec: None,
                    current_chunk: 1,
                    total_chunks: 1,
                });
            }
            let buffer = PreparedAudioBuffer::from_f32(samples.to_vec(), sr as u32);
            let wav_bytes = buffer.to_wav_bytes()?;
            let result = self
                .transcribe_single_chunk_with_retry(clip_id, &wav_bytes, is_music_mode, 0.0)
                .await?;
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
            return Ok(result);
        }

        // Multi-chunk path: run Silero VAD on blocking threadpool to detect natural pause boundaries
        let vad_config = VadConfig::default();
        let speech_segments = detect_speech_segments_async(samples.to_vec(), vad_config).await?;

        let split_config = ChunkSplitConfig {
            max_chunk_size_bytes: SAFE_CHUNK_BYTES,
            max_chunk_duration_ms: 600_000, // 10 minutes
            overlap_ms: DEFAULT_OVERLAP_MS, // 1.5s
        };

        let base_chunks =
            split_into_natural_pause_chunks(samples, &speech_segments, &split_config, sr)?;
        if base_chunks.is_empty() {
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
            return Ok(TranscriptSegmentsContainer {
                clip_id: clip_id.to_string(),
                segments: Vec::new(),
            });
        }

        let total_chunks_usize = base_chunks.len();
        let total_chunks = u32::try_from(total_chunks_usize).unwrap_or(u32::MAX);
        info!(
            clip_id = %clip_id,
            chunks_count = total_chunks_usize,
            "Auto-chunking audio into {} VAD natural pause segments (< 20MB)",
            total_chunks_usize
        );

        let mut all_segments: Vec<TranscriptSegment> = Vec::new();
        let mut completed_emitted = false;

        for (idx, base_chunk) in base_chunks.iter().enumerate() {
            let chunk_num = u32::try_from(idx + 1).unwrap_or(total_chunks);
            let is_last_chunk = idx + 1 == total_chunks_usize;

            // Apply 1.5s lookback overlap for chunk index > 0
            let start_with_overlap_ms = if idx == 0 {
                0u64
            } else {
                base_chunk.start_ms.saturating_sub(split_config.overlap_ms)
            };
            let end_ms = base_chunk.end_ms;

            let start_sample = ((start_with_overlap_ms as usize) * sr) / 1000;
            let end_sample = (((end_ms as usize) * sr) / 1000).min(samples.len());

            if start_sample >= end_sample {
                continue;
            }

            if let Some(ref cb) = on_progress {
                cb(SttProgressPayload {
                    task_id: clip_id.to_string(),
                    stage: "transcribing".to_string(),
                    percent: (idx as f64 / f64::from(total_chunks)) * 100.0,
                    eta_sec: None,
                    current_chunk: chunk_num,
                    total_chunks,
                });
            }

            let chunk_slice = &samples[start_sample..end_sample];
            let chunk_buf = PreparedAudioBuffer::from_f32(chunk_slice.to_vec(), sr as u32);
            let chunk_wav = chunk_buf.to_wav_bytes()?;

            // Hard ceiling check at 25MB Groq limit; warn if above 20MB soft threshold
            if chunk_wav.len() > MAX_PAYLOAD_BYTES {
                let size_mb = chunk_wav.len() as f32 / (1024.0 * 1024.0);
                return Err(AppError::Stt(SttError::PayloadTooLarge { size_mb }));
            }
            if chunk_wav.len() > SAFE_CHUNK_BYTES {
                warn!(
                    clip_id = %clip_id,
                    chunk_index = idx,
                    size_bytes = chunk_wav.len(),
                    "Groq STT chunk exceeded 20MB soft threshold but remains under 25MB hard ceiling"
                );
            }

            let time_offset_sec = start_with_overlap_ms as f64 / 1000.0;
            let chunk_result = self
                .transcribe_single_chunk_with_retry(
                    clip_id,
                    &chunk_wav,
                    is_music_mode,
                    time_offset_sec,
                )
                .await?;

            // Deduplicate segments that fall inside the overlap zone of previous chunks
            let cutoff_sec = base_chunk.start_ms as f64 / 1000.0;
            for seg in chunk_result.segments {
                // For chunks > 0, discard segments completely before core chunk start
                if idx > 0 && seg.end_sec <= cutoff_sec {
                    continue;
                }
                all_segments.push(seg);
            }

            if let Some(ref cb) = on_progress {
                let stage = if is_last_chunk {
                    completed_emitted = true;
                    "completed"
                } else {
                    "transcribing"
                };
                cb(SttProgressPayload {
                    task_id: clip_id.to_string(),
                    stage: stage.to_string(),
                    percent: (f64::from(chunk_num) / f64::from(total_chunks)) * 100.0,
                    eta_sec: if is_last_chunk { Some(0.0) } else { None },
                    current_chunk: chunk_num,
                    total_chunks,
                });
            }
        }

        if !completed_emitted {
            if let Some(ref cb) = on_progress {
                cb(SttProgressPayload {
                    task_id: clip_id.to_string(),
                    stage: "completed".to_string(),
                    percent: 100.0,
                    eta_sec: Some(0.0),
                    current_chunk: total_chunks,
                    total_chunks,
                });
            }
        }

        // Deduplicate boundary words and sort monotonic
        let deduplicated = deduplicate_overlap_segments(all_segments, clip_id);

        Ok(TranscriptSegmentsContainer {
            clip_id: clip_id.to_string(),
            segments: deduplicated,
        })
    }

    /// Transcribe a single audio chunk with exponential backoff on HTTP 429 (`IT-15.2`).
    #[allow(clippy::too_many_lines, clippy::cast_precision_loss)]
    pub async fn transcribe_single_chunk_with_retry(
        &self,
        clip_id: &str,
        wav_bytes: &[u8],
        is_music_mode: bool,
        time_offset_sec: f64,
    ) -> Result<TranscriptSegmentsContainer, AppError> {
        let endpoint = self.endpoint_url();
        let redacted_key = redact_secret(&self.config.api_key);

        debug!(
            endpoint = %endpoint,
            key = %redacted_key,
            is_music_mode = is_music_mode,
            size_bytes = wav_bytes.len(),
            "Emitting Groq Whisper transcription request"
        );

        let mut attempts = 0u32;
        let auth_header = format!("Bearer {}", self.config.api_key);

        loop {
            // Build multipart payload for current attempt
            let boundary = format!("----NeuroClipBoundary{:016x}", fastrand_u64());
            let body_bytes = self.build_multipart_payload(&boundary, wav_bytes, is_music_mode)?;
            let content_type = format!("multipart/form-data; boundary={boundary}");

            let (status_code, retry_after_hdr, text) = if let Some(ref mock) = self.mock_handler {
                let resp = mock(&endpoint, &auth_header, &content_type, &body_bytes)?;
                (resp.status_code, resp.retry_after, resp.body)
            } else {
                let request = self
                    .client
                    .post(&endpoint)
                    .header("Authorization", &auth_header)
                    .header("Content-Type", &content_type)
                    .body(body_bytes);

                match request.send().await {
                    Ok(response) => {
                        let status = response.status().as_u16();
                        let retry_after = response
                            .headers()
                            .get("retry-after")
                            .and_then(|h| h.to_str().ok())
                            .map(ToString::to_string);
                        let body = response.text().await.map_err(|e| {
                            AppError::Stt(SttError::ParserFailed(format!(
                                "Failed reading Groq HTTP response body: {e}"
                            )))
                        })?;
                        (status, retry_after, body)
                    }
                    Err(e) => {
                        attempts += 1;
                        if attempts <= MAX_RATE_LIMIT_RETRIES {
                            let wait_duration = Duration::from_secs(1 << (attempts - 1));
                            warn!(
                                attempt = attempts,
                                error = %e,
                                "Network error connecting to Groq API, retrying"
                            );
                            tokio::time::sleep(wait_duration).await;
                            continue;
                        }

                        return Err(AppError::Stt(SttError::SidecarExecutionFailed {
                            details: format!("Network request to Groq API failed: {e:?}"),
                        }));
                    }
                }
            };

            if (200..300).contains(&status_code) {
                let parsed: GroqVerboseJsonResponse = serde_json::from_str(&text).map_err(|e| {
                    AppError::Stt(SttError::ParserFailed(format!(
                        "Failed parsing Groq verbose_json response: {e}. Body: {text}"
                    )))
                })?;

                return Ok(map_verbose_json_to_container(
                    clip_id,
                    parsed,
                    time_offset_sec,
                ));
            }

            if status_code == 401 {
                warn!(
                    status = %status_code,
                    key = %redacted_key,
                    "Groq API returned HTTP 401 Unauthorized"
                );
                return Err(AppError::Stt(SttError::InvalidApiKey));
            }

            if status_code == 413 {
                let size_mb = wav_bytes.len() as f32 / (1024.0 * 1024.0);
                return Err(AppError::Stt(SttError::PayloadTooLarge { size_mb }));
            }

            if status_code == 429 {
                // Rate limit handling with exponential backoff (IT-15.2)
                attempts += 1;
                let last_error_message = if text.is_empty() {
                    "Rate limit exceeded (HTTP 429)".to_string()
                } else {
                    text
                };

                if attempts > MAX_RATE_LIMIT_RETRIES {
                    warn!(attempts = attempts, "Groq API rate limit retries exhausted");
                    return Err(AppError::Stt(SttError::RateLimitExceeded(format!(
                        "Groq rate limit exceeded after {MAX_RATE_LIMIT_RETRIES} retries: {last_error_message}"
                    ))));
                }

                let wait_duration = parse_retry_after(retry_after_hdr.as_deref(), attempts);
                info!(
                    attempt = attempts,
                    wait_sec = wait_duration.as_secs_f64(),
                    "Groq HTTP 429 encountered, sleeping before retry"
                );
                tokio::time::sleep(wait_duration).await;
                continue;
            }

            // Other error statuses (5xx, etc.)
            attempts += 1;
            if attempts <= MAX_RATE_LIMIT_RETRIES && status_code >= 500 {
                let wait_duration = Duration::from_secs(1 << (attempts - 1));
                warn!(
                    status = %status_code,
                    attempt = attempts,
                    "Groq server error, retrying"
                );
                tokio::time::sleep(wait_duration).await;
                continue;
            }

            return Err(AppError::Stt(SttError::SidecarExecutionFailed {
                details: format!("Groq API returned HTTP {status_code}: {text}"),
            }));
        }
    }

    /// Construct RFC 7578 multipart/form-data byte payload.
    ///
    /// Includes `condition_on_previous_text=false` when `is_music_mode == true` (`UT-15.2`).
    pub fn build_multipart_payload(
        &self,
        boundary: &str,
        wav_bytes: &[u8],
        is_music_mode: bool,
    ) -> Result<Vec<u8>, AppError> {
        let mut fields: Vec<(&str, String)> = vec![
            ("model", self.config.model.clone()),
            ("response_format", "verbose_json".to_string()),
            ("timestamp_granularities[]", "word".to_string()),
            ("timestamp_granularities[]", "segment".to_string()),
        ];

        // UT-15.2 & AC-15.3: Karaoke / Anti-repetition mode guardrail
        if is_music_mode {
            fields.push(("condition_on_previous_text", "false".to_string()));
        }

        if let Some(ref lang) = self.config.language {
            fields.push(("language", lang.clone()));
        }

        // Suppress prompt in music mode to prevent loop seeding
        if !is_music_mode {
            if let Some(ref p) = self.config.prompt {
                fields.push(("prompt", p.clone()));
            }
        }

        if let Some(temp) = self.config.temperature {
            fields.push(("temperature", format!("{temp:.2}")));
        }

        let mut body = Vec::with_capacity(wav_bytes.len() + 2048);

        for (name, val) in &fields {
            let part_header = format!(
                "--{boundary}\r\nContent-Disposition: form-data; name=\"{name}\"\r\n\r\n{val}\r\n"
            );
            body.extend_from_slice(part_header.as_bytes());
        }

        // Binary audio file part
        let file_header = format!(
            "--{boundary}\r\nContent-Disposition: form-data; name=\"file\"; filename=\"audio.wav\"\r\nContent-Type: audio/wav\r\n\r\n"
        );
        body.extend_from_slice(file_header.as_bytes());
        body.extend_from_slice(wav_bytes);
        body.extend_from_slice(format!("\r\n--{boundary}--\r\n").as_bytes());

        Ok(body)
    }
}

/// Parse `Retry-After` header or calculate exponential backoff delay (1s, 2s, 4s).
pub fn parse_retry_after(header_val: Option<&str>, attempt: u32) -> Duration {
    if let Some(hdr) = header_val {
        if let Ok(secs) = hdr.trim().parse::<f64>() {
            if secs > 0.0 && secs.is_finite() {
                return Duration::from_secs_f64(secs.min(30.0));
            }
        }
    }

    // Default exponential steps: attempt 1 -> 1s, attempt 2 -> 2s, attempt 3 -> 4s
    let exp_sec = 1u64 << (attempt.saturating_sub(1).min(5));
    Duration::from_secs(exp_sec)
}

/// Map Groq `verbose_json` response to NeuroClip Schema 5.5 [`TranscriptSegmentsContainer`].
pub fn map_verbose_json_to_container(
    clip_id: &str,
    response: GroqVerboseJsonResponse,
    time_offset_sec: f64,
) -> TranscriptSegmentsContainer {
    let mut segments = Vec::new();
    let mut segment_counter = 1usize;

    if response.segments.is_empty() && !response.text.trim().is_empty() {
        // Fallback for single segment without detailed segments array
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
                probability: 1.0,
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

    // Associate flat words with segments based on time boundaries
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

        // Calculate confidence from avg_logprob
        let confidence = (seg.avg_logprob).exp().clamp(0.0, 1.0);
        let cps_score = calculate_cps(&text, start_sec, end_sec);
        let is_flagged_for_review =
            confidence < CONFIDENCE_FLAG_THRESHOLD || cps_score > CPS_FLAG_THRESHOLD;

        // Extract words falling within segment boundary [start - 0.05, end + 0.05]
        let mut words: Vec<WordTiming> = Vec::new();
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
                        probability: confidence,
                    });
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

/// Deduplicate overlapping segments and words across multi-chunk transcription.
pub fn deduplicate_overlap_segments(
    mut segments: Vec<TranscriptSegment>,
    clip_id: &str,
) -> Vec<TranscriptSegment> {
    if segments.len() <= 1 {
        return segments;
    }

    // Sort by start_sec monotonically
    segments.sort_by(|a, b| {
        a.start_sec
            .partial_cmp(&b.start_sec)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    let mut result: Vec<TranscriptSegment> = Vec::with_capacity(segments.len());

    for seg in segments {
        if let Some(prev) = result.last_mut() {
            // Check for exact duplicate segment text overlapping
            if (seg.start_sec - prev.start_sec).abs() < 0.35
                && seg.text_en.trim().eq_ignore_ascii_case(prev.text_en.trim())
            {
                continue;
            }

            // Word-level deduplication on boundaries
            if !seg.words.is_empty() && !prev.words.is_empty() {
                if let (Some(last_prev_word), Some(first_cur_word)) =
                    (prev.words.last(), seg.words.first())
                {
                    if (last_prev_word.start_sec - first_cur_word.start_sec).abs() < 0.25
                        && last_prev_word
                            .word
                            .eq_ignore_ascii_case(&first_cur_word.word)
                    {
                        // Duplicate token on boundary; skip from current segment
                        let mut trimmed_words = seg.words.clone();
                        trimmed_words.remove(0);
                        let mut modified = seg;
                        modified.words = trimmed_words;
                        result.push(modified);
                        continue;
                    }
                }
            }
        }

        result.push(seg);
    }

    // Re-assign clean sequential segment IDs
    for (idx, seg) in result.iter_mut().enumerate() {
        seg.segment_id = format!("{clip_id}_seg_{:04}", idx + 1);
    }

    result
}

/// Sanitize float timestamp ensuring non-negative finite value.
fn sanitize_positive(val: f64) -> f64 {
    if val.is_finite() && val >= 0.0 {
        val
    } else {
        0.0
    }
}

/// Simple deterministic pseudo-random u64 generator for boundaries without external random crate.
fn fastrand_u64() -> u64 {
    use std::sync::atomic::{AtomicU64, Ordering};
    static COUNTER: AtomicU64 = AtomicU64::new(0x4d59_5f53_4545_4431);
    let prev = COUNTER.fetch_add(0x9e37_79b9_7f4a_7c15, Ordering::Relaxed);
    let mut z = prev.wrapping_add(0x9e37_79b9_7f4a_7c15);
    z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    z ^ (z >> 31)
}
