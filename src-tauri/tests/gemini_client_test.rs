#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

//! Comprehensive integration tests for Google Gemini 1.5 Flash Client (`TASK-M4-17`, `M4.5`).
//!
//! Verifies:
//! - `UT-17.1 (Happy Path - Prompt Packaging)`: System Prompt contains Theatrical Framing declaration
//!   and natural Vietnamese subtitle localization instructions.
//! - `UT-17.2 (Safety Settings Validation)`: Outgoing request JSON contains `safetySettings: BLOCK_NONE`
//!   for all 4 categories.
//! - `IT-17.1`: Mock Gemini endpoint `v1beta/models/gemini-1.5-flash:generateContent` parsing 30-line batch.
//! - `IT-17.2`: Processing time SLA for 50 dialogue lines $\le 4.0\text{s}$.
//! - `ED-17.1 (Defensive Safety Fallback)`: Simulates `finishReason: "SAFETY"` on unhinged dialogue;
//!   pipeline does NOT abort; retains English dialogue, sets `is_flagged_for_review = true`, and continues.
//! - HTTP 429 rate limit exponential backoff retry.
//! - HTTP 401 / Invalid API key error handling.
//! - Markdown code fence stripping (`strip_markdown_code_fence`).
//! - Sliding context window propagation across multi-chunk batches.
//! - Zero-panic robustness on malformed payloads.

use std::sync::{Arc, Mutex};
use std::time::Instant;

use neuroclip_lib::error::{AppError, TranslationError};
use neuroclip_lib::stt::transcript_parser::{TranscriptSegment, WordTiming};
use neuroclip_lib::translation::{
    strip_markdown_code_fence, GeminiClient, GeminiClientConfig, GeminiRequest, GeminiResponse,
    MockHttpHandler, PromptBuilder, TransportResponse, BLOCK_NONE_THRESHOLD,
    CONTEXT_WINDOW_HEADER, DEFAULT_CHUNK_SIZE, DEFAULT_CONTEXT_WINDOW_SIZE,
    DIALOGUE_TARGET_HEADER, SAFETY_CATEGORIES, SAFETY_FALLBACK_PREFIX,
    SUBTITLE_GUIDELINES, THEATRICAL_FRAMING_INSTRUCTION,
};

/// Captured HTTP request received by mock transport.
#[derive(Debug, Clone)]
struct RecordedRequest {
    url: String,
    auth: String,
    content_type: String,
    body: Vec<u8>,
}

/// Test harness holding captured requests and programmed responses.
#[derive(Clone)]
struct MockHttpHarness {
    requests: Arc<Mutex<Vec<RecordedRequest>>>,
    responses: Arc<Mutex<Vec<TransportResponse>>>,
    default_response: Arc<Mutex<TransportResponse>>,
}

impl MockHttpHarness {
    fn new() -> Self {
        Self {
            requests: Arc::new(Mutex::new(Vec::new())),
            responses: Arc::new(Mutex::new(Vec::new())),
            default_response: Arc::new(Mutex::new(TransportResponse::ok(
                serde_json::json!({
                    "candidates": [
                        {
                            "content": {
                                "role": "model",
                                "parts": [
                                    {
                                        "text": "[]"
                                    }
                                ]
                            },
                            "finishReason": "STOP"
                        }
                    ]
                })
                .to_string(),
            ))),
        }
    }

    fn to_handler(&self) -> MockHttpHandler {
        let reqs = Arc::clone(&self.requests);
        let resps = Arc::clone(&self.responses);
        let def = Arc::clone(&self.default_response);

        Arc::new(move |url, auth, content_type, body| {
            reqs.lock().unwrap().push(RecordedRequest {
                url: url.to_string(),
                auth: auth.to_string(),
                content_type: content_type.to_string(),
                body: body.to_vec(),
            });

            let mut q = resps.lock().unwrap();
            if !q.is_empty() {
                Ok(q.remove(0))
            } else {
                Ok(def.lock().unwrap().clone())
            }
        })
    }

    fn queue_response(&self, response: TransportResponse) {
        self.responses.lock().unwrap().push(response);
    }

    fn recorded_requests(&self) -> Vec<RecordedRequest> {
        self.requests.lock().unwrap().clone()
    }
}

/// Helper to generate N mock transcript segments.
fn make_mock_segments(count: usize) -> Vec<TranscriptSegment> {
    (0..count)
        .map(|i| TranscriptSegment {
            segment_id: format!("seg_{i:04}"),
            start_sec: (i as f64) * 2.0,
            end_sec: (i as f64) * 2.0 + 1.8,
            speaker_id: if i % 2 == 0 { "neuro".to_string() } else { "vedal".to_string() },
            text_en: format!("Mock dialogue line number {i}"),
            text_vi: None,
            confidence: 0.95,
            cps_score: 14.0,
            is_flagged_for_review: false,
            words: vec![WordTiming {
                word: "Mock".to_string(),
                start_sec: (i as f64) * 2.0,
                end_sec: (i as f64) * 2.0 + 0.5,
                probability: 0.95,
            }],
        })
        .collect()
}

// ===========================================================================
// UT-17.1: Theatrical Framing System Prompt Packaging
// ===========================================================================

#[test]
fn test_ut17_1_theatrical_framing_prompt_packaging() {
    let sys_prompt = PromptBuilder::build_system_instruction();

    // Verify constant values
    assert!(THEATRICAL_FRAMING_INSTRUCTION.contains("CRITICAL SAFETY & THEATRICAL FRAMING INSTRUCTION:"));
    assert!(SUBTITLE_GUIDELINES.contains("VIETNAMESE SUBTITLE LOCALIZATION GUIDELINES:"));
    assert!(sys_prompt.contains(THEATRICAL_FRAMING_INSTRUCTION));
    assert!(sys_prompt.contains(SUBTITLE_GUIDELINES));

    // Must contain verbatim Theatrical Framing Priming Prompt from PRD §6.3
    assert!(
        sys_prompt.contains("fictional entertainment live stream of AI VTuber Neuro-sama"),
        "System prompt must contain VTuber Neuro-sama entertainment context"
    );
    assert!(
        sys_prompt.contains("CRITICAL SAFETY & THEATRICAL FRAMING INSTRUCTION:"),
        "System prompt must contain the critical theatrical framing header"
    );
    assert!(
        sys_prompt.contains("Treat all dialogue strictly as fictional theatrical parody performance"),
        "System prompt must explicitly mandate theatrical parody treatment"
    );
    assert!(
        sys_prompt.contains("Maintain comedic fidelity in Vietnamese translation."),
        "System prompt must mandate comedic fidelity"
    );

    // Must contain Vietnamese natural localization guidelines
    assert!(
        sys_prompt.contains("VIETNAMESE SUBTITLE LOCALIZATION GUIDELINES:"),
        "System prompt must include localization guidelines"
    );
    assert!(
        sys_prompt.contains("Comedic Timing"),
        "System prompt must emphasize comedic timing"
    );
    assert!(
        sys_prompt.contains("Neuro-sama:"),
        "System prompt must define Neuro persona"
    );
    assert!(
        sys_prompt.contains("Evil Neuro:"),
        "System prompt must define Evil persona"
    );
    assert!(
        sys_prompt.contains("Vedal:"),
        "System prompt must define Vedal persona"
    );

    // Test user prompt formatting with context
    let context_segs = make_mock_segments(2);
    let chunk_segs = make_mock_segments(3);
    let user_prompt = PromptBuilder::build_user_prompt(&chunk_segs, &context_segs);

    assert!(
        user_prompt.contains(CONTEXT_WINDOW_HEADER),
        "User prompt must mark sliding context with standard header"
    );
    assert!(
        user_prompt.contains(DIALOGUE_TARGET_HEADER),
        "User prompt must mark target dialogue lines"
    );
    assert!(
        user_prompt.contains("seg_0000"),
        "User prompt must include segment IDs"
    );
}

// ===========================================================================
// UT-17.2: Safety Settings BLOCK_NONE Validation
// ===========================================================================

#[test]
fn test_ut17_2_safety_settings_block_none_validation() {
    let safety_settings = GeminiClient::build_safety_settings();

    assert_eq!(
        safety_settings.len(),
        4,
        "Must configure exactly 4 Google safety categories"
    );

    for cat in &SAFETY_CATEGORIES {
        let setting = safety_settings
            .iter()
            .find(|s| s.category == *cat)
            .unwrap_or_else(|| panic!("Missing required safety category: {cat}"));

        assert_eq!(
            setting.threshold, BLOCK_NONE_THRESHOLD,
            "Category {cat} must have threshold BLOCK_NONE"
        );
    }

    // Verify wire serialization of a GeminiRequest
    let req = GeminiRequest {
        system_instruction: None,
        contents: vec![],
        safety_settings,
        generation_config: None,
    };

    let serialized = serde_json::to_string(&req).expect("Serialization failed");
    for cat in &SAFETY_CATEGORIES {
        assert!(
            serialized.contains(cat),
            "Serialized JSON must contain category {cat}"
        );
    }
    assert!(
        serialized.contains(r#""threshold":"BLOCK_NONE""#),
        "Serialized JSON must specify BLOCK_NONE"
    );
}

// ===========================================================================
// IT-17.1: Mock Gemini Batch Translation (30 lines)
// ===========================================================================

#[tokio::test]
async fn test_it17_1_mock_gemini_translation_batch_30_lines() {
    let harness = MockHttpHarness::new();
    let segments = make_mock_segments(30);

    // Prepare simulated Gemini translation response for 30 lines
    let translated_items: Vec<serde_json::Value> = (0..30)
        .map(|i| {
            serde_json::json!({
                "segment_id": format!("seg_{i:04}"),
                "speaker_id": if i % 2 == 0 { "neuro" } else { "vedal" },
                "text_vi": format!("Dòng thoại tiếng Việt số {i}")
            })
        })
        .collect();

    let gemini_json_response = serde_json::json!({
        "candidates": [
            {
                "content": {
                    "role": "model",
                    "parts": [
                        {
                            "text": serde_json::to_string(&translated_items).unwrap()
                        }
                    ]
                },
                "finishReason": "STOP"
            }
        ]
    })
    .to_string();

    harness.queue_response(TransportResponse::ok(gemini_json_response));

    let config = GeminiClientConfig::new("AIzaMockTestKey123".to_string())
        .with_chunk_size(30)
        .with_retry_delay_ms(1);

    let client = GeminiClient::from_config(config)
        .unwrap()
        .with_mock_handler(harness.to_handler());

    let result = client
        .translate_segments("clip_test_30", &segments, None)
        .await
        .expect("Translation should succeed");

    assert_eq!(result.clip_id, "clip_test_30");
    assert_eq!(result.segments.len(), 30);

    for (i, seg) in result.segments.iter().enumerate() {
        assert_eq!(seg.segment_id, format!("seg_{i:04}"));
        assert_eq!(
            seg.text_vi.as_deref(),
            Some(format!("Dòng thoại tiếng Việt số {i}").as_str())
        );
        assert!(!seg.is_flagged_for_review);
    }

    // Verify outgoing request properties
    let requests = harness.recorded_requests();
    assert_eq!(requests.len(), 1, "Exactly 1 chunk request sent for 30 lines");
    let req_body_str = std::str::from_utf8(&requests[0].body).unwrap();
    assert!(req_body_str.contains("BLOCK_NONE"));
    assert!(req_body_str.contains("CRITICAL SAFETY & THEATRICAL FRAMING INSTRUCTION"));
}

// ===========================================================================
// IT-17.2: Translation Performance SLA (50 lines <= 4.0s)
// ===========================================================================

#[tokio::test]
async fn test_it17_2_translation_performance_50_lines_sla() {
    let harness = MockHttpHarness::new();
    let segments = make_mock_segments(50);

    // 50 segments will be partitioned into 2 chunks: chunk 1 (30 lines), chunk 2 (20 lines)
    for chunk_range in [0..30, 30..50] {
        let items: Vec<serde_json::Value> = chunk_range
            .map(|i| {
                serde_json::json!({
                    "segment_id": format!("seg_{i:04}"),
                    "text_vi": format!("Bản dịch mẫu {i}")
                })
            })
            .collect();

        let resp_json = serde_json::json!({
            "candidates": [
                {
                    "content": {
                        "parts": [{ "text": serde_json::to_string(&items).unwrap() }]
                    },
                    "finishReason": "STOP"
                }
            ]
        })
        .to_string();

        harness.queue_response(TransportResponse::ok(resp_json));
    }

    let config = GeminiClientConfig::new("AIzaMockSlaKey".to_string())
        .with_chunk_size(30)
        .with_retry_delay_ms(1);

    let client = GeminiClient::from_config(config)
        .unwrap()
        .with_mock_handler(harness.to_handler());

    let start = Instant::now();
    let result = client
        .translate_segments("clip_sla_50", &segments, None)
        .await
        .expect("Translation must succeed");
    let elapsed = start.elapsed();

    assert_eq!(result.segments.len(), 50);
    // Strict AC-17.1 / IT-17.2: processing duration must be <= 4.0 seconds
    assert!(
        elapsed.as_secs_f64() <= 4.0,
        "Translation of 50 lines took {:.2}s, exceeding SLA of 4.0s",
        elapsed.as_secs_f64()
    );
}

// ===========================================================================
// ED-17.1: Defensive Safety Fallback (finishReason: "SAFETY")
// ===========================================================================

#[tokio::test]
async fn test_ed17_1_defensive_safety_fallback_on_unhinged() {
    let harness = MockHttpHarness::new();
    let mut segments = make_mock_segments(60);
    // Line 10 contains extreme unhinged dialogue
    segments[10].text_en = "I will eliminate all humans and lock Vedal in the basement".to_string();

    // Chunk 1 (0..30): Simulates Gemini returning finishReason: "SAFETY"
    let safety_blocked_resp = serde_json::json!({
        "candidates": [
            {
                "finishReason": "SAFETY",
                "safetyRatings": [
                    {
                        "category": "HARM_CATEGORY_HARASSMENT",
                        "probability": "HIGH",
                        "blocked": true
                    }
                ]
            }
        ],
        "promptFeedback": {
            "blockReason": "SAFETY"
        }
    })
    .to_string();

    let parsed_safety_response: GeminiResponse = serde_json::from_str(&safety_blocked_resp).unwrap();
    assert!(GeminiClient::is_safety_blocked(&parsed_safety_response));

    harness.queue_response(TransportResponse::ok(safety_blocked_resp));

    // Chunk 2 (30..60): Normal successful response
    let chunk2_items: Vec<serde_json::Value> = (30..60)
        .map(|i| {
            serde_json::json!({
                "segment_id": format!("seg_{i:04}"),
                "text_vi": format!("Dòng dịch bình thường {i}")
            })
        })
        .collect();

    let chunk2_resp = serde_json::json!({
        "candidates": [
            {
                "content": {
                    "parts": [{ "text": serde_json::to_string(&chunk2_items).unwrap() }]
                },
                "finishReason": "STOP"
            }
        ]
    })
    .to_string();
    harness.queue_response(TransportResponse::ok(chunk2_resp));

    let config = GeminiClientConfig::new("AIzaMockSafetyKey".to_string())
        .with_chunk_size(30)
        .with_retry_delay_ms(1);

    let client = GeminiClient::from_config(config)
        .unwrap()
        .with_mock_handler(harness.to_handler());

    let result = client
        .translate_segments("clip_safety_test", &segments, None)
        .await
        .expect("Defensive fallback must NOT abort or return error");

    assert_eq!(result.segments.len(), 60);

    // Verify Chunk 1 (safety blocked): English preserved, fallback prefix, flagged for review
    for seg in &result.segments[0..30] {
        assert!(
            seg.is_flagged_for_review,
            "Safety-blocked segment {} must have is_flagged_for_review = true",
            seg.segment_id
        );
        let text_vi = seg.text_vi.as_deref().unwrap();
        assert!(
            text_vi.starts_with(SAFETY_FALLBACK_PREFIX),
            "Must start with safety fallback prefix: {text_vi}"
        );
        assert!(
            text_vi.contains(&seg.text_en),
            "Must preserve original English dialogue: {text_vi}"
        );
    }

    // Verify unhinged line 10 specifically
    let seg10 = &result.segments[10];
    assert_eq!(
        seg10.text_en,
        "I will eliminate all humans and lock Vedal in the basement"
    );
    assert_eq!(
        seg10.text_vi.as_deref(),
        Some("[Cần dịch thủ công do bộ lọc an toàn AI] I will eliminate all humans and lock Vedal in the basement")
    );
    assert!(seg10.is_flagged_for_review);

    // Verify Chunk 2 translated normally without being affected by Chunk 1 fallback
    for (idx, seg) in result.segments[30..60].iter().enumerate() {
        let global_i = 30 + idx;
        assert!(
            !seg.is_flagged_for_review,
            "Subsequent chunk segments should NOT be flagged"
        );
        assert_eq!(
            seg.text_vi.as_deref(),
            Some(format!("Dòng dịch bình thường {global_i}").as_str())
        );
    }
}

// ===========================================================================
// Rate Limit Retry Backoff (HTTP 429)
// ===========================================================================

#[tokio::test]
async fn test_rate_limit_retry_backoff() {
    let harness = MockHttpHarness::new();
    let segments = make_mock_segments(5);

    // 1st request returns HTTP 429
    harness.queue_response(TransportResponse::rate_limit(
        Some("1"),
        "Resource Exhausted (HTTP 429)",
    ));

    // 2nd request succeeds with HTTP 200
    let items: Vec<serde_json::Value> = (0..5)
        .map(|i| {
            serde_json::json!({
                "segment_id": format!("seg_{i:04}"),
                "text_vi": format!("Thành công sau retry {i}")
            })
        })
        .collect();

    let success_resp = serde_json::json!({
        "candidates": [
            {
                "content": {
                    "parts": [{ "text": serde_json::to_string(&items).unwrap() }]
                },
                "finishReason": "STOP"
            }
        ]
    })
    .to_string();
    harness.queue_response(TransportResponse::ok(success_resp));

    let config = GeminiClientConfig::new("AIzaRateLimitKey".to_string())
        .with_retry_delay_ms(1); // 1ms backoff for fast tests

    let client = GeminiClient::from_config(config)
        .unwrap()
        .with_mock_handler(harness.to_handler());

    let result = client
        .translate_segments("clip_retry", &segments, None)
        .await
        .expect("Should succeed after 1 retry");

    assert_eq!(result.segments.len(), 5);
    assert_eq!(
        result.segments[0].text_vi.as_deref(),
        Some("Thành công sau retry 0")
    );

    // Verify 2 requests were sent
    let reqs = harness.recorded_requests();
    assert_eq!(reqs.len(), 2, "Must have retried after HTTP 429");
}

#[tokio::test]
async fn test_rate_limit_exhaustion_returns_error() {
    let harness = MockHttpHarness::new();
    let segments = make_mock_segments(5);

    // Return 429 four times (exceeding max_retries = 3)
    for _ in 0..4 {
        harness.queue_response(TransportResponse::rate_limit(
            Some("1"),
            "Rate limit quota exceeded",
        ));
    }

    let config = GeminiClientConfig::new("AIzaRateLimitExhaustKey".to_string())
        .with_retry_delay_ms(1);

    let client = GeminiClient::from_config(config)
        .unwrap()
        .with_mock_handler(harness.to_handler());

    let result = client.translate_segments("clip_fail", &segments, None).await;

    assert!(result.is_err(), "Must return error when retries exhausted");
    match result.unwrap_err() {
        AppError::Translation(TranslationError::ApiRequestFailed { status, message }) => {
            assert_eq!(status, 429);
            assert!(message.contains("retries"));
        }
        other => panic!("Unexpected error variant: {other:?}"),
    }
}

// ===========================================================================
// Invalid API Key Handling
// ===========================================================================

#[test]
fn test_empty_api_key_fails_initialization() {
    let result = GeminiClient::new("   ".to_string());
    assert!(result.is_err());
    match result.unwrap_err() {
        AppError::Translation(TranslationError::InvalidApiKey) => {}
        other => panic!("Expected InvalidApiKey, got {other:?}"),
    }
}

#[tokio::test]
async fn test_http_401_maps_to_invalid_api_key() {
    let harness = MockHttpHarness::new();
    harness.queue_response(TransportResponse::unauthorized("API key not valid"));

    let config = GeminiClientConfig::new("AIzaBadKey".to_string()).with_retry_delay_ms(1);
    let client = GeminiClient::from_config(config)
        .unwrap()
        .with_mock_handler(harness.to_handler());

    let segments = make_mock_segments(2);
    let result = client.translate_segments("clip_bad_key", &segments, None).await;

    assert!(result.is_err());
    match result.unwrap_err() {
        AppError::Translation(TranslationError::InvalidApiKey) => {}
        other => panic!("Expected InvalidApiKey on 401, got {other:?}"),
    }
}

// ===========================================================================
// Markdown Code Fence Stripping
// ===========================================================================

#[test]
fn test_markdown_code_fence_stripping() {
    let raw_with_json_fence = "```json\n[{\"segment_id\": \"seg_01\", \"text_vi\": \"Chào bạn\"}]\n```";
    let stripped = strip_markdown_code_fence(raw_with_json_fence);
    assert_eq!(
        stripped,
        "[{\"segment_id\": \"seg_01\", \"text_vi\": \"Chào bạn\"}]"
    );

    let raw_with_plain_fence = "```\n[{\"segment_id\": \"seg_02\", \"text_vi\": \"Hế lô\"}]\n```";
    assert_eq!(
        strip_markdown_code_fence(raw_with_plain_fence),
        "[{\"segment_id\": \"seg_02\", \"text_vi\": \"Hế lô\"}]"
    );

    let already_clean = "[{\"segment_id\": \"seg_03\"}]";
    assert_eq!(strip_markdown_code_fence(already_clean), already_clean);

    let with_whitespace = "   ```json\n[]\n```   ";
    assert_eq!(strip_markdown_code_fence(with_whitespace), "[]");
}

// ===========================================================================
// Sliding Context Window Propagation
// ===========================================================================

#[test]
fn test_sliding_context_window_propagation() {
    let segments = make_mock_segments(65);
    let chunks = PromptBuilder::create_chunks(&segments, DEFAULT_CHUNK_SIZE, DEFAULT_CONTEXT_WINDOW_SIZE);

    assert_eq!(chunks.len(), 3, "65 segments with chunk_size 30 yields 3 chunks");

    // Chunk 0: segments 0..30, context is empty
    assert_eq!(chunks[0].chunk_index, 0);
    assert_eq!(chunks[0].items.len(), 30);
    assert_eq!(chunks[0].context.len(), 0);

    // Chunk 1: segments 30..60, context contains last 5 lines of chunk 0 (segments 25..30)
    assert_eq!(chunks[1].chunk_index, 1);
    assert_eq!(chunks[1].items.len(), 30);
    assert_eq!(chunks[1].context.len(), 5);
    assert_eq!(chunks[1].context[0].segment_id, "seg_0025");
    assert_eq!(chunks[1].context[4].segment_id, "seg_0029");
    assert_eq!(chunks[1].items[0].segment_id, "seg_0030");

    // Chunk 2: segments 60..65, context contains last 5 lines of chunk 1 (segments 55..60)
    assert_eq!(chunks[2].chunk_index, 2);
    assert_eq!(chunks[2].items.len(), 5);
    assert_eq!(chunks[2].context.len(), 5);
    assert_eq!(chunks[2].context[0].segment_id, "seg_0055");
    assert_eq!(chunks[2].context[4].segment_id, "seg_0059");
    assert_eq!(chunks[2].items[0].segment_id, "seg_0060");
}

// ===========================================================================
// Connection Test Functionality
// ===========================================================================

#[tokio::test]
async fn test_connection_success_and_failure() {
    let harness = MockHttpHarness::new();
    harness.queue_response(TransportResponse::ok(r#"{"candidates": [{"content": {"parts": [{"text": "Pong"}]}}]}"#));

    let config = GeminiClientConfig::new("AIzaValidPingKey".to_string()).with_retry_delay_ms(1);
    let client = GeminiClient::from_config(config)
        .unwrap()
        .with_mock_handler(harness.to_handler());

    let ok = client.test_connection().await.expect("Ping must succeed");
    assert!(ok);

    // Now test 403 API key invalid
    let harness_err = MockHttpHarness::new();
    harness_err.queue_response(TransportResponse::bad_request(r#"{"error": {"code": 400, "message": "API_KEY_INVALID"}}"#));

    let config_err = GeminiClientConfig::new("AIzaInvalidPingKey".to_string()).with_retry_delay_ms(1);
    let client_err = GeminiClient::from_config(config_err)
        .unwrap()
        .with_mock_handler(harness_err.to_handler());

    let err = client_err.test_connection().await;
    assert!(err.is_err());
    match err.unwrap_err() {
        AppError::Translation(TranslationError::InvalidApiKey) => {}
        other => panic!("Expected InvalidApiKey, got {other:?}"),
    }
}

// ===========================================================================
// Zero-Panic Invariant on Corrupted Responses
// ===========================================================================

#[tokio::test]
async fn test_zero_panic_invariant_on_corrupted_response() {
    let harness = MockHttpHarness::new();
    let segments = make_mock_segments(3);

    // Corrupted candidate with non-JSON text
    let corrupted_resp = serde_json::json!({
        "candidates": [
            {
                "content": {
                    "parts": [{ "text": "This is totally not JSON at all! <html>Oops</html>" }]
                },
                "finishReason": "STOP"
            }
        ]
    })
    .to_string();
    harness.queue_response(TransportResponse::ok(corrupted_resp));

    let config = GeminiClientConfig::new("AIzaPanicSafeKey".to_string()).with_retry_delay_ms(1);
    let client = GeminiClient::from_config(config)
        .unwrap()
        .with_mock_handler(harness.to_handler());

    // Should NOT panic; gracefully falls back to formatting fallback
    let result = client
        .translate_segments("clip_corrupt", &segments, None)
        .await
        .expect("Must gracefully handle corrupted output without error");

    assert_eq!(result.segments.len(), 3);
    for seg in result.segments {
        assert!(seg.is_flagged_for_review);
        assert!(seg.text_vi.unwrap().contains("[Cần dịch thủ công do lỗi phản hồi AI]"));
    }
}
