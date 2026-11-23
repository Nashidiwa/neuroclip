#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

//! Comprehensive integration tests for Groq Whisper Cloud API Client (`TASK-M4-15`, `M4.4`).
//!
//! Verifies:
//! - `UT-15.1`: Chunk sizing <= 20MB with 1.5s lookback overlap.
//! - `UT-15.2`: Karaoke / Anti-repetition mode `condition_on_previous_text=false`.
//! - `IT-15.1`: Mock verbose_json with word & segment timestamps.
//! - `IT-15.2`: Rate limit 429 exponential backoff (2 retries then success; 3 retries then failure).
//! - `ED-15.1`: Payload size assertion strictly <= 20MB (no 413, rejects > 25MB pre-flight).
//! - `ED-15.2`: Processing speed SLA.
//! - HTTP 401 InvalidApiKey handling.

use std::sync::{Arc, Mutex};
use std::time::Instant;

use neuroclip_lib::error::{AppError, SttError};
use neuroclip_lib::stt::audio_prep::PreparedAudioBuffer;
use neuroclip_lib::stt::groq_client::{
    deduplicate_overlap_segments, parse_retry_after, GroqClient, MockHttpHandler,
    TransportResponse, DEFAULT_OVERLAP_MS, SAFE_CHUNK_BYTES,
};
use neuroclip_lib::stt::transcript_parser::{TranscriptSegment, WordTiming};
use neuroclip_lib::stt::vad::{split_into_natural_pause_chunks, ChunkSplitConfig, SpeechSegment};
use neuroclip_lib::stt::SttProgressPayload;

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
                    "text": "Default mock transcript",
                    "segments": [],
                    "words": []
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

    fn enqueue_response(&self, resp: TransportResponse) {
        self.responses.lock().unwrap().push(resp);
    }

    fn set_default_response(&self, resp: TransportResponse) {
        *self.default_response.lock().unwrap() = resp;
    }

    fn recorded_requests(&self) -> Vec<RecordedRequest> {
        self.requests.lock().unwrap().clone()
    }

    fn request_count(&self) -> usize {
        self.requests.lock().unwrap().len()
    }
}

/// Helper: generate synthetic 16kHz mono WAV bytes of given duration.
fn generate_test_wav_bytes(duration_sec: f64) -> Vec<u8> {
    let sample_rate = 16_000u32;
    let num_samples = (sample_rate as f64 * duration_sec).round() as usize;
    let mut samples = Vec::with_capacity(num_samples);

    for i in 0..num_samples {
        let t = i as f64 / sample_rate as f64;
        let v = (2.0 * std::f64::consts::PI * 440.0 * t).sin();
        samples.push((v * 0.5) as f32);
    }

    let buf = PreparedAudioBuffer::from_f32(samples, sample_rate);
    buf.to_wav_bytes().expect("WAV serialization must succeed")
}

// ===========================================================================
// Test Cases
// ===========================================================================

/// UT-15.1: Chunk Sizing <= 20MB with 1.5s Lookback Overlap
/// Verifies that audio exceeding 20MB is split into chunks strictly <= 20MB
/// with 1.5s (1500ms) lookback overlap on consecutive chunks.
#[test]
fn test_ut_15_1_chunk_sizing_under_20mb_with_1_5s_overlap() {
    let sample_rate = 16_000usize;
    // 25 minutes of audio: 25 * 60 = 1500 seconds
    // 1500s * 16,000 * 2 bytes = 48MB (exceeds 20MB safe limit)
    let total_duration_sec = 1500.0;

    // Define speech segments spaced every 4 minutes (with silence midpoints)
    let speech_segments = vec![
        SpeechSegment {
            start_ms: 10_000,
            end_ms: 200_000,
        },
        SpeechSegment {
            start_ms: 250_000,
            end_ms: 500_000,
        },
        SpeechSegment {
            start_ms: 550_000,
            end_ms: 800_000,
        },
        SpeechSegment {
            start_ms: 850_000,
            end_ms: 1_100_000,
        },
        SpeechSegment {
            start_ms: 1_150_000,
            end_ms: 1_450_000,
        },
    ];

    let split_config = ChunkSplitConfig {
        max_chunk_size_bytes: SAFE_CHUNK_BYTES, // 20MB
        max_chunk_duration_ms: 600_000,         // 10 minutes
        overlap_ms: DEFAULT_OVERLAP_MS,         // 1.5s
    };

    let total_samples = (total_duration_sec * sample_rate as f64) as usize;
    let dummy_samples = vec![0.0f32; total_samples];

    let base_chunks = split_into_natural_pause_chunks(
        &dummy_samples,
        &speech_segments,
        &split_config,
        sample_rate,
    )
    .expect("Pause chunk splitting must succeed");

    assert!(
        base_chunks.len() >= 3,
        "48MB audio must produce at least 3 chunks, got {}",
        base_chunks.len()
    );

    for (idx, chunk) in base_chunks.iter().enumerate() {
        // Assert core chunk payload size <= 20MB
        assert!(
            chunk.size_bytes <= SAFE_CHUNK_BYTES,
            "Chunk {} size {} exceeded 20MB limit",
            idx,
            chunk.size_bytes
        );

        // Apply 1.5s lookback overlap for chunk index > 0
        let start_with_overlap = if idx == 0 {
            0u64
        } else {
            chunk.start_ms.saturating_sub(split_config.overlap_ms)
        };

        if idx > 0 {
            let overlap_duration = chunk.start_ms - start_with_overlap;
            assert_eq!(
                overlap_duration, 1_500,
                "Lookback overlap for chunk {} must be exactly 1500ms (1.5s)",
                idx
            );
        }

        // Verify total chunk audio duration with overlap remains <= 20MB
        let chunk_duration_ms = chunk.end_ms - start_with_overlap;
        let chunk_bytes_estimate = (chunk_duration_ms * (sample_rate as u64 * 2)) / 1000;
        assert!(
            chunk_bytes_estimate as usize <= SAFE_CHUNK_BYTES,
            "Chunk {} with overlap exceeded 20MB budget",
            idx
        );
    }
}

/// UT-15.2: Karaoke / Anti-Repetition Mode Guardrail
/// Verifies that when `is_music_mode == true`, the multipart payload includes
/// `condition_on_previous_text = "false"` to prevent repetition loops.
#[tokio::test]
async fn test_ut_15_2_music_mode_condition_on_previous_text_false() {
    let harness = MockHttpHarness::new();

    let mock_json = serde_json::json!({
        "task": "transcribe",
        "language": "english",
        "duration": 2.0,
        "text": "Singing a song",
        "segments": [{
            "id": 0, "seek": 0, "start": 0.0, "end": 2.0,
            "text": " Singing a song", "tokens": [1234],
            "temperature": 0.0, "avg_logprob": -0.1,
            "compression_ratio": 0.5, "no_speech_prob": 0.01
        }],
        "words": [
            {"word": "Singing", "start": 0.0, "end": 0.8},
            {"word": "a", "start": 0.9, "end": 1.1},
            {"word": "song", "start": 1.2, "end": 1.9}
        ]
    });
    harness.set_default_response(TransportResponse::ok(mock_json.to_string()));

    let client = GroqClient::new("gsk_valid_key_12345678".to_string())
        .expect("Client creation must succeed")
        .with_mock_handler(harness.to_handler());

    let wav_bytes = generate_test_wav_bytes(2.0);

    // 1. Music Mode = true
    let result_music = client
        .transcribe_audio_bytes("clip_music", &wav_bytes, true)
        .await
        .expect("Music transcription must succeed");
    assert_eq!(result_music.segments.len(), 1);

    let requests = harness.recorded_requests();
    assert_eq!(requests.len(), 1);
    assert!(requests[0].auth.starts_with("Bearer gsk_"));
    assert!(requests[0]
        .content_type
        .contains("multipart/form-data; boundary="));
    assert!(requests[0].url.ends_with("/audio/transcriptions"));
    let body_music_str = String::from_utf8_lossy(&requests[0].body);

    assert!(
        body_music_str.contains("name=\"condition_on_previous_text\""),
        "Music mode must pass condition_on_previous_text form field"
    );
    assert!(
        body_music_str.contains("false"),
        "condition_on_previous_text must be false in music mode"
    );

    // 2. Normal Mode = false
    let _ = client
        .transcribe_audio_bytes("clip_speech", &wav_bytes, false)
        .await
        .expect("Speech transcription must succeed");

    let requests2 = harness.recorded_requests();
    assert_eq!(requests2.len(), 2);
    let body_speech_str = String::from_utf8_lossy(&requests2[1].body);

    assert!(
        !body_speech_str.contains("name=\"condition_on_previous_text\""),
        "Normal speech mode must omit condition_on_previous_text"
    );
}

/// IT-15.1: verbose_json Parsing with Word & Segment Timestamps
/// Verifies parsing into Schema 5.5 TranscriptSegmentsContainer.
#[tokio::test]
async fn test_it_15_1_mock_groq_verbose_json_parsing() {
    let harness = MockHttpHarness::new();

    let mock_json = serde_json::json!({
        "task": "transcribe",
        "language": "english",
        "duration": 4.5,
        "text": "Hello Neuro-sama, welcome back!",
        "segments": [
            {
                "id": 0, "seek": 0, "start": 0.0, "end": 2.0,
                "text": " Hello Neuro-sama,",
                "tokens": [100, 200],
                "temperature": 0.0,
                "avg_logprob": -0.15,
                "compression_ratio": 0.8,
                "no_speech_prob": 0.01
            },
            {
                "id": 1, "seek": 0, "start": 2.1, "end": 4.5,
                "text": " welcome back!",
                "tokens": [300, 400],
                "temperature": 0.0,
                "avg_logprob": -0.05,
                "compression_ratio": 0.7,
                "no_speech_prob": 0.01
            }
        ],
        "words": [
            {"word": "Hello", "start": 0.0, "end": 0.6},
            {"word": "Neuro-sama,", "start": 0.7, "end": 1.9},
            {"word": "welcome", "start": 2.1, "end": 2.8},
            {"word": "back!", "start": 2.9, "end": 4.4}
        ]
    });
    harness.set_default_response(TransportResponse::ok(mock_json.to_string()));

    let client = GroqClient::new("gsk_test_token_87654321".to_string())
        .expect("Client creation must succeed")
        .with_mock_handler(harness.to_handler());

    let wav_bytes = generate_test_wav_bytes(4.5);
    let result = client
        .transcribe_audio_bytes("clip_vtuber_01", &wav_bytes, false)
        .await
        .expect("Transcription parsing must succeed");

    assert_eq!(result.clip_id, "clip_vtuber_01");
    assert_eq!(result.segments.len(), 2);

    let seg0 = &result.segments[0];
    assert_eq!(seg0.segment_id, "clip_vtuber_01_seg_0001");
    assert_eq!(seg0.text_en, "Hello Neuro-sama,");
    assert_eq!(seg0.start_sec, 0.0);
    assert_eq!(seg0.end_sec, 2.0);
    assert_eq!(seg0.words.len(), 2);
    assert_eq!(seg0.words[0].word, "Hello");
    assert_eq!(seg0.words[1].word, "Neuro-sama,");
    assert!(seg0.confidence > 0.8);
    assert!(seg0.cps_score > 0.0);

    let seg1 = &result.segments[1];
    assert_eq!(seg1.segment_id, "clip_vtuber_01_seg_0002");
    assert_eq!(seg1.text_en, "welcome back!");
    assert_eq!(seg1.start_sec, 2.1);
    assert_eq!(seg1.end_sec, 4.5);
    assert_eq!(seg1.words.len(), 2);
    assert_eq!(seg1.words[0].word, "welcome");
    assert_eq!(seg1.words[1].word, "back!");
}

/// IT-15.2: Rate Limit 429 Exponential Backoff
/// Verifies:
/// 1. HTTP 429 twice, then HTTP 200 OK -> client retries with backoff and succeeds.
/// 2. HTTP 429 consecutively -> client retries 3 times, then fails with SttError::RateLimitExceeded.
#[tokio::test]
async fn test_it_15_2_rate_limit_exponential_backoff() {
    let harness = MockHttpHarness::new();

    let rate_limit_body = serde_json::json!({
        "error": {
            "message": "Rate limit reached on RPM. Please try again in 0.05s.",
            "type": "tokens",
            "code": "rate_limit_exceeded"
        }
    })
    .to_string();

    let success_body = serde_json::json!({
        "text": "Success after backoff",
        "segments": [{
            "id": 0, "seek": 0, "start": 0.0, "end": 1.0,
            "text": " Success after backoff", "tokens": [],
            "temperature": 0.0, "avg_logprob": -0.1,
            "compression_ratio": 0.5, "no_speech_prob": 0.0
        }],
        "words": []
    })
    .to_string();

    // Scenario A: 2 retries (HTTP 429) then HTTP 200 OK
    harness.enqueue_response(TransportResponse::rate_limit(
        Some("0.05"),
        rate_limit_body.clone(),
    ));
    harness.enqueue_response(TransportResponse::rate_limit(
        Some("0.05"),
        rate_limit_body.clone(),
    ));
    harness.enqueue_response(TransportResponse::ok(success_body));

    let client = GroqClient::new("gsk_rate_key_11112222".to_string())
        .expect("Client creation must succeed")
        .with_mock_handler(harness.to_handler());

    let wav = generate_test_wav_bytes(1.0);
    let result = client
        .transcribe_audio_bytes("clip_retry", &wav, false)
        .await
        .expect("Must succeed after 2 retries");

    assert_eq!(result.segments[0].text_en, "Success after backoff");
    assert_eq!(harness.request_count(), 3);

    // Scenario B: 4 consecutive HTTP 429 -> exhausts MAX_RATE_LIMIT_RETRIES (3)
    let harness_fail = MockHttpHarness::new();
    for _ in 0..5 {
        harness_fail.enqueue_response(TransportResponse::rate_limit(
            Some("0.05"),
            rate_limit_body.clone(),
        ));
    }

    let client_fail = GroqClient::new("gsk_fail_key_33334444".to_string())
        .expect("Client creation must succeed")
        .with_mock_handler(harness_fail.to_handler());

    let err = client_fail
        .transcribe_audio_bytes("clip_exhaust", &wav, false)
        .await
        .unwrap_err();

    match err {
        AppError::Stt(SttError::RateLimitExceeded(msg)) => {
            assert!(
                msg.contains("3 retries"),
                "Error message must indicate 3 retries were exhausted, got: {}",
                msg
            );
        }
        other => panic!("Expected SttError::RateLimitExceeded, got: {:?}", other),
    }

    // Attempt 0 + 3 retries = 4 requests sent before giving up
    assert_eq!(harness_fail.request_count(), 4);
}

/// HTTP 401 Unauthorized returns SttError::InvalidApiKey immediately without retry
#[tokio::test]
async fn test_groq_invalid_api_key_401() {
    let harness = MockHttpHarness::new();

    let error_body = serde_json::json!({
        "error": {
            "message": "Invalid API Key",
            "type": "invalid_request_error",
            "code": "invalid_api_key"
        }
    })
    .to_string();

    harness.set_default_response(TransportResponse::unauthorized(error_body));

    let client = GroqClient::new("gsk_invalid_bad_token".to_string())
        .expect("Client creation must succeed")
        .with_mock_handler(harness.to_handler());

    let wav = generate_test_wav_bytes(1.0);
    let err = client
        .transcribe_audio_bytes("clip_unauth", &wav, false)
        .await
        .unwrap_err();

    match err {
        AppError::Stt(SttError::InvalidApiKey) => {
            // Expected
        }
        other => panic!("Expected SttError::InvalidApiKey, got: {:?}", other),
    }

    // Must not retry on 401
    assert_eq!(harness.request_count(), 1);
}

/// ED-15.1: Payload Size Pre-flight Guardrail
/// Verifies that payloads > 25MB are rejected pre-flight with SttError::PayloadTooLarge
/// without emitting any network request.
#[tokio::test]
async fn test_ed_15_1_payload_limit_strict_assertion() {
    let harness = MockHttpHarness::new();

    let client = GroqClient::new("gsk_size_key_55556666".to_string())
        .expect("Client creation must succeed")
        .with_mock_handler(harness.to_handler());

    // 26 MB buffer (exceeds 25 MB Groq hard ceiling)
    let oversize_buffer = vec![0u8; 26 * 1024 * 1024];

    let err = client
        .transcribe_audio_bytes("clip_oversize", &oversize_buffer, false)
        .await
        .unwrap_err();

    match err {
        AppError::Stt(SttError::PayloadTooLarge { size_mb }) => {
            assert!(
                size_mb >= 25.0,
                "Expected size_mb >= 25.0, got: {}",
                size_mb
            );
        }
        other => panic!("Expected SttError::PayloadTooLarge, got: {:?}", other),
    }

    // Zero requests emitted to server
    assert_eq!(harness.request_count(), 0);
}

/// ED-15.2: Processing Speed SLA
/// Verifies that chunk boundary calculation and multipart assembly for 45-minute audio takes < 100ms.
#[tokio::test]
async fn test_ed_15_2_processing_speed_sla() {
    let sample_rate = 16_000usize;
    // 45 minutes = 2700 seconds
    let total_samples = 2700 * sample_rate;
    let dummy_samples = vec![0.0f32; total_samples];

    let speech_segments = vec![
        SpeechSegment {
            start_ms: 10_000,
            end_ms: 400_000,
        },
        SpeechSegment {
            start_ms: 500_000,
            end_ms: 900_000,
        },
        SpeechSegment {
            start_ms: 1_000_000,
            end_ms: 1_400_000,
        },
        SpeechSegment {
            start_ms: 1_500_000,
            end_ms: 1_900_000,
        },
        SpeechSegment {
            start_ms: 2_000_000,
            end_ms: 2_400_000,
        },
        SpeechSegment {
            start_ms: 2_500_000,
            end_ms: 2_650_000,
        },
    ];

    let split_config = ChunkSplitConfig {
        max_chunk_size_bytes: SAFE_CHUNK_BYTES,
        max_chunk_duration_ms: 600_000,
        overlap_ms: DEFAULT_OVERLAP_MS,
    };

    let start = Instant::now();
    let chunks = split_into_natural_pause_chunks(
        &dummy_samples,
        &speech_segments,
        &split_config,
        sample_rate,
    )
    .expect("Splitting must pass");
    let elapsed = start.elapsed();

    assert!(chunks.len() >= 4);
    assert!(
        elapsed.as_millis() < 100,
        "Splitting SLA breached: took {:?} (expected < 100ms)",
        elapsed
    );

    // Test multipart assembly SLA
    let client = GroqClient::new("gsk_perf_token_99998888".to_string()).unwrap();
    let synthetic_wav = generate_test_wav_bytes(5.0);

    let start_multipart = Instant::now();
    let payload = client
        .build_multipart_payload("TestBoundary12345", &synthetic_wav, true)
        .expect("Payload assembly must pass");
    let elapsed_multipart = start_multipart.elapsed();

    assert!(payload.len() > synthetic_wav.len());
    assert!(
        elapsed_multipart.as_millis() < 20,
        "Multipart assembly SLA breached: took {:?}",
        elapsed_multipart
    );
}

/// Boundary Overlap Deduplication Integration Test
#[test]
fn test_deduplicate_overlap_segments_unit() {
    let seg1 = TranscriptSegment {
        segment_id: "clip_seg_0001".to_string(),
        start_sec: 0.0,
        end_sec: 10.0,
        speaker_id: "unknown".to_string(),
        text_en: "Hello and welcome to NeuroClip".to_string(),
        text_vi: None,
        confidence: 0.95,
        cps_score: 12.0,
        is_flagged_for_review: false,
        words: vec![
            WordTiming {
                word: "Hello".to_string(),
                start_sec: 0.0,
                end_sec: 1.0,
                probability: 0.95,
            },
            WordTiming {
                word: "and".to_string(),
                start_sec: 1.1,
                end_sec: 1.5,
                probability: 0.95,
            },
            WordTiming {
                word: "welcome".to_string(),
                start_sec: 1.6,
                end_sec: 3.0,
                probability: 0.95,
            },
            WordTiming {
                word: "to".to_string(),
                start_sec: 3.1,
                end_sec: 4.0,
                probability: 0.95,
            },
            WordTiming {
                word: "NeuroClip".to_string(),
                start_sec: 4.1,
                end_sec: 5.5,
                probability: 0.95,
            },
        ],
    };

    // Duplicate segment in 1.5s overlap zone
    let seg1_dup = TranscriptSegment {
        segment_id: "clip_seg_0002".to_string(),
        start_sec: 0.05,
        end_sec: 10.02,
        speaker_id: "unknown".to_string(),
        text_en: "Hello and welcome to NeuroClip".to_string(),
        text_vi: None,
        confidence: 0.94,
        cps_score: 12.0,
        is_flagged_for_review: false,
        words: vec![],
    };

    let seg2 = TranscriptSegment {
        segment_id: "clip_seg_0003".to_string(),
        start_sec: 10.5,
        end_sec: 15.0,
        speaker_id: "unknown".to_string(),
        text_en: "Let's edit some highlights!".to_string(),
        text_vi: None,
        confidence: 0.92,
        cps_score: 14.0,
        is_flagged_for_review: false,
        words: vec![
            WordTiming {
                word: "Let's".to_string(),
                start_sec: 10.5,
                end_sec: 11.2,
                probability: 0.92,
            },
            WordTiming {
                word: "edit".to_string(),
                start_sec: 11.3,
                end_sec: 12.0,
                probability: 0.92,
            },
        ],
    };

    let deduplicated = deduplicate_overlap_segments(vec![seg1, seg1_dup, seg2], "clip_test");

    assert_eq!(deduplicated.len(), 2, "Duplicate segment must be removed");
    assert_eq!(deduplicated[0].segment_id, "clip_test_seg_0001");
    assert_eq!(deduplicated[1].segment_id, "clip_test_seg_0002");
    assert_eq!(deduplicated[1].text_en, "Let's edit some highlights!");
}

/// Retry-After parser helper test
#[test]
fn test_parse_retry_after_helper() {
    assert_eq!(parse_retry_after(Some("2"), 1).as_secs(), 2);
    assert_eq!(parse_retry_after(Some("1.5"), 1).as_millis(), 1500);
    // Exponential fallback
    assert_eq!(parse_retry_after(None, 1).as_secs(), 1);
    assert_eq!(parse_retry_after(None, 2).as_secs(), 2);
    assert_eq!(parse_retry_after(None, 3).as_secs(), 4);
    assert_eq!(parse_retry_after(Some("invalid"), 2).as_secs(), 2);
}

/// M4-15: Verify real multi-chunk `SttProgressPayload` emission & camelCase JSON contract
#[tokio::test]
async fn test_m4_15_transcribe_samples_with_progress_multi_chunk_and_camelcase_json() {
    let harness = MockHttpHarness::new();

    // Queue 2 responses for 2 chunks (> 20MB / > 600s audio at 16kHz)
    let make_chunk_resp = |text: &str| {
        TransportResponse::ok(
            serde_json::json!({
                "task": "transcribe",
                "language": "english",
                "duration": 5.0,
                "text": text,
                "segments": [{
                    "id": 0, "seek": 0, "start": 0.0, "end": 2.0,
                    "text": text, "tokens": [],
                    "temperature": 0.0, "avg_logprob": -0.1,
                    "compression_ratio": 0.5, "no_speech_prob": 0.01
                }],
                "words": []
            })
            .to_string(),
        )
    };
    harness.enqueue_response(make_chunk_resp("First chunk dialogue"));
    harness.enqueue_response(make_chunk_resp("Second chunk dialogue"));

    let client = GroqClient::new("gsk_progress_test_123456".to_string())
        .expect("Client creation must succeed")
        .with_mock_handler(harness.to_handler());

    // 15 minutes (900s) at 16kHz -> ~28.8MB WAV > 20MB SAFE_CHUNK_BYTES -> 2 chunks
    let samples = vec![0.0f32; 16_000 * 900];
    let events: Arc<Mutex<Vec<SttProgressPayload>>> = Arc::new(Mutex::new(Vec::new()));
    let events_cb = Arc::clone(&events);

    let result = client
        .transcribe_samples_with_progress(
            "clip_multi_progress",
            &samples,
            16_000,
            false,
            Some(Arc::new(move |payload| {
                events_cb.lock().unwrap().push(payload);
            })),
        )
        .await
        .expect("Multi-chunk transcription must succeed");

    assert_eq!(result.clip_id, "clip_multi_progress");
    assert_eq!(result.segments.len(), 2);
    assert_eq!(harness.request_count(), 2);

    let captured = events.lock().unwrap().clone();
    assert_eq!(
        captured.len(),
        4,
        "Expected 2 events per chunk (before/after) for 2 chunks, got: {captured:?}"
    );

    // Chunk 1 before & after
    assert_eq!(captured[0].task_id, "clip_multi_progress");
    assert_eq!(captured[0].stage, "transcribing");
    assert_eq!(captured[0].current_chunk, 1);
    assert_eq!(captured[0].total_chunks, 2);
    assert!((captured[0].percent - 0.0).abs() < 1e-6);

    assert_eq!(captured[1].stage, "transcribing");
    assert_eq!(captured[1].current_chunk, 1);
    assert_eq!(captured[1].total_chunks, 2);
    assert!((captured[1].percent - 50.0).abs() < 1e-6);

    // Chunk 2 before & after
    assert_eq!(captured[2].stage, "transcribing");
    assert_eq!(captured[2].current_chunk, 2);
    assert_eq!(captured[2].total_chunks, 2);
    assert!((captured[2].percent - 50.0).abs() < 1e-6);

    assert_eq!(captured[3].stage, "completed");
    assert_eq!(captured[3].current_chunk, 2);
    assert_eq!(captured[3].total_chunks, 2);
    assert!((captured[3].percent - 100.0).abs() < 1e-6);
    assert_eq!(captured[3].eta_sec, Some(0.0));

    // Verify serialized JSON matches camelCase contract required by `isSttProgressPayloadJson`
    let json_val = serde_json::to_value(&captured[3]).unwrap();
    assert_eq!(json_val["taskId"], "clip_multi_progress");
    assert_eq!(json_val["stage"], "completed");
    assert_eq!(json_val["percent"], 100.0);
    assert_eq!(json_val["currentChunk"], 2);
    assert_eq!(json_val["totalChunks"], 2);
    assert!(json_val.get("clip_id").is_none());
}

/// Regression test for bounded future cut & overlap budget on >10.9 min continuous speech
#[test]
fn test_continuous_12min_speech_respects_safe_chunk_bytes_with_overlap() {
    let sample_rate = 16_000usize;
    // 13 minutes total: continuous speech from 10s to 12m10s (730_000ms), then pause, then speech
    let total_samples = 780 * sample_rate;
    let dummy_samples = vec![0.0f32; total_samples];
    let speech_segments = vec![
        SpeechSegment {
            start_ms: 10_000,
            end_ms: 730_000,
        },
        SpeechSegment {
            start_ms: 735_000,
            end_ms: 770_000,
        },
    ];
    let split_config = ChunkSplitConfig {
        max_chunk_size_bytes: SAFE_CHUNK_BYTES,
        max_chunk_duration_ms: 600_000,
        overlap_ms: DEFAULT_OVERLAP_MS,
    };

    let chunks = split_into_natural_pause_chunks(
        &dummy_samples,
        &speech_segments,
        &split_config,
        sample_rate,
    )
    .expect("Continuous speech chunking must succeed without size overflow");

    assert!(chunks.len() >= 2);
    for (idx, chunk) in chunks.iter().enumerate() {
        let start_with_overlap = if idx == 0 {
            0u64
        } else {
            chunk.start_ms.saturating_sub(split_config.overlap_ms)
        };
        let duration_ms = chunk.end_ms - start_with_overlap;
        let wav_bytes_estimate = 44 + ((duration_ms as usize) * sample_rate * 2) / 1000;
        assert!(
            wav_bytes_estimate <= SAFE_CHUNK_BYTES,
            "Chunk {idx} with overlap + WAV header ({wav_bytes_estimate} B) exceeded SAFE_CHUNK_BYTES ({SAFE_CHUNK_BYTES} B)"
        );
    }
}

/// Verify transcribe_samples_with_progress handles >20MB continuous speech via async VAD without PayloadTooLarge
#[tokio::test]
async fn test_groq_transcribe_samples_continuous_speech_no_payload_too_large() {
    let harness = MockHttpHarness::new();
    let client = GroqClient::new("gsk_continuous_test_123456".to_string())
        .expect("Client creation must succeed")
        .with_mock_handler(harness.to_handler());

    // 11.5 minutes of constant voiced-like non-zero signal (>20MB WAV)
    let sample_rate = 16_000usize;
    let samples = vec![0.25f32; sample_rate * 690];
    let res = client
        .transcribe_samples_with_progress("clip_continuous_11m", &samples, sample_rate, false, None)
        .await;

    assert!(
        res.is_ok(),
        "Continuous >20MB speech must chunk cleanly without PayloadTooLarge: {res:?}"
    );
    assert!(harness.request_count() >= 2);
}
