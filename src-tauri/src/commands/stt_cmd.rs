//! Tauri IPC command handlers for STT transcription and subtitle generation.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use tauri::{command, Emitter};

use crate::commands::config_cmd::AppState;
use crate::config::hardware::{probe_system_hardware, HardwareProfile};
use crate::error::{AppError, SttError};
use crate::stt::whisper_sidecar::ProgressCallback;
use crate::stt::{
    evaluate_hardware_guardrail_from_profile, prepare_audio_to_buffer, run_faster_whisper_worker,
    run_whisper_transcription, verify_model, AudioPrepConfig, FasterWhisperConfig, GroqClient,
    GroqClientConfig, GuardrailPolicy, ModelInfo, PreparedAudioBuffer, SttProgressPayload,
    TranscriptSegment, TranscriptSegmentsContainer, WhisperSidecarConfig,
    DEFAULT_FASTER_WHISPER_DEVICE, DEFAULT_FASTER_WHISPER_MODEL,
    DEFAULT_FASTER_WHISPER_QUANTIZATION,
};

/// Trigger local Whisper.cpp speech-to-text transcription on an audio file.
///
/// # Errors
/// Returns [`AppError::Stt`] or [`AppError::Sidecar`] if model verification or worker execution fails.
#[command]
pub async fn start_whisper_transcription(
    clip_id: String,
    audio_path: String,
    model_name: Option<String>,
    app: tauri::AppHandle,
) -> Result<TranscriptSegmentsContainer, AppError> {
    let chosen_model = model_name.unwrap_or_else(|| "base.en-q5_k".to_string());
    let audio_file_path = PathBuf::from(audio_path);

    // Verify model existence and integrity unless mock mode is active
    let model_path = if std::env::var("NEUROCLIP_MOCK_WHISPER_OUTPUT").is_ok() {
        PathBuf::from("mock_model.bin")
    } else {
        let model_info = verify_model(&chosen_model, None)?;
        PathBuf::from(model_info.path)
    };

    let config = WhisperSidecarConfig {
        binary_path: None,
        model_path,
        threads: 4,
        language: "en".to_string(),
        prompt: Some("Vedal, Neuro-sama, Evil Neuro, Tutel, Gymbag".to_string()),
    };

    let app_handle = app.clone();
    let on_progress = Some(Arc::new(move |payload| {
        let _ = app_handle.emit("stt:progress", payload);
    })
        as Arc<dyn Fn(crate::stt::SttProgressPayload) + Send + Sync>);

    run_whisper_transcription(&clip_id, &audio_file_path, &config, on_progress).await
}

/// Verify the cryptographic integrity of a local Whisper `Q5_K` or `CTranslate2` model.
///
/// # Errors
/// Returns [`AppError::Stt`] if the model is missing, empty, or fails SHA-256 verification.
#[command]
pub async fn verify_stt_model(model_name: Option<String>) -> Result<ModelInfo, AppError> {
    let target = model_name.unwrap_or_else(|| "base.en-q5_k".to_string());
    verify_model(&target, None)
}

pub use crate::commands::export_cmd::{
    __cmd__export_subtitles_ass, __cmd__export_subtitles_srt,
    __tauri_command_name_export_subtitles_ass, __tauri_command_name_export_subtitles_srt,
    export_subtitles_ass, export_subtitles_srt,
};

/// Callback invoked for each transcribed segment before returning the full container.
pub type ChunkCallback<'a> = &'a (dyn Fn(usize, &TranscriptSegment) + Send + Sync);

/// Helper executing Groq sample transcription with typed `SttProgressPayload` and per-segment callbacks.
///
/// # Errors
/// Returns [`AppError::Stt`] if transcription fails.
pub async fn transcribe_highlight_clip_with_client(
    clip_id: &str,
    samples: &[f32],
    sample_rate: usize,
    is_music_mode: bool,
    client: &GroqClient,
    on_progress: Option<ProgressCallback>,
    on_chunk: Option<ChunkCallback<'_>>,
) -> Result<TranscriptSegmentsContainer, AppError> {
    if let Some(ref cb) = on_progress {
        cb(SttProgressPayload {
            task_id: clip_id.to_string(),
            stage: "preparing".to_string(),
            percent: 0.0,
            eta_sec: None,
            current_chunk: 0,
            total_chunks: 1,
        });
    }

    let result = client
        .transcribe_samples_with_progress(clip_id, samples, sample_rate, is_music_mode, on_progress)
        .await?;

    if let Some(chunk_cb) = on_chunk {
        for (idx, seg) in result.segments.iter().enumerate() {
            chunk_cb(idx, seg);
        }
    }

    Ok(result)
}

/// Transcribe a highlight clip using Groq Whisper Cloud API (`TASK-M4-15`).
///
/// Supports automatic chunking <= 20MB with Silero VAD pause split and 1.5s lookback overlap,
/// Karaoke anti-repetition guardrail, and exponential backoff retry on HTTP 429.
/// Emits `stt:progress` (`SttProgressPayload`) and `stt:chunk` events across the Tauri event bridge.
///
/// # Errors
/// Returns [`AppError::Stt`] or [`AppError::Sidecar`] if the API key is missing, audio prep fails, or Groq API fails.
#[command]
pub async fn transcribe_highlight_clip(
    clip_id: String,
    audio_path: String,
    api_key: Option<String>,
    is_music_mode: bool,
    app: tauri::AppHandle,
) -> Result<TranscriptSegmentsContainer, AppError> {
    // Resolve Groq API key (passed parameter or env fallback)
    let resolved_key = api_key
        .filter(|k| !k.trim().is_empty())
        .or_else(|| std::env::var("GROQ_API_KEY").ok())
        .or_else(|| std::env::var("NEUROCLIP_GROQ_API_KEY").ok())
        .ok_or(AppError::Stt(SttError::InvalidApiKey))?;

    let path = Path::new(&audio_path);
    if !path.exists() {
        return Err(AppError::Stt(SttError::SidecarExecutionFailed {
            details: format!("Audio file not found at path: {audio_path}"),
        }));
    }

    // Prepare audio to 16kHz mono buffer
    let prep_config = AudioPrepConfig::default();
    let audio_buf = match prepare_audio_to_buffer(path, &prep_config, None).await {
        Ok(buf) => buf,
        Err(AppError::Sidecar(crate::error::SidecarError::BinaryNotFound { .. })) => {
            let file_bytes = std::fs::read(path).map_err(|e| {
                AppError::Stt(SttError::SidecarExecutionFailed {
                    details: format!("Failed reading audio file: {e}"),
                })
            })?;
            PreparedAudioBuffer::from_wav_bytes(&file_bytes)?
        }
        Err(e) => return Err(e),
    };

    let mut client_config = GroqClientConfig::new(resolved_key).with_music_mode(is_music_mode);
    if let Ok(base_url) = std::env::var("NEUROCLIP_GROQ_API_URL") {
        client_config = client_config.with_base_url(base_url);
    }
    let client = GroqClient::from_config(client_config)?;

    let app_for_progress = app.clone();
    let on_progress: ProgressCallback = Arc::new(move |payload| {
        let _ = app_for_progress.emit("stt:progress", payload);
    });

    let app_for_chunk = app.clone();
    let clip_id_for_chunk = clip_id.clone();
    let emit_chunk = move |idx: usize, seg: &TranscriptSegment| {
        let _ = app_for_chunk.emit(
            "stt:chunk",
            serde_json::json!({
                "clip_id": clip_id_for_chunk,
                "chunk_index": idx,
                "segment": seg,
            }),
        );
    };

    transcribe_highlight_clip_with_client(
        &clip_id,
        &audio_buf.samples_f32,
        audio_buf.sample_rate as usize,
        is_music_mode,
        &client,
        Some(on_progress),
        Some(&emit_chunk),
    )
    .await
}

/// Execute local Faster-Whisper transcription with hardware guardrail enforcement and progress/chunk callbacks.
///
/// # Errors
/// Returns [`AppError::Stt`] or [`AppError::Sidecar`] if guardrail rejects the request or the worker fails.
#[allow(clippy::too_many_arguments)]
pub async fn start_faster_whisper_transcription_with_profile(
    clip_id: &str,
    audio_path: &Path,
    model: Option<&str>,
    quantization: Option<&str>,
    device: Option<&str>,
    guardrail_policy: Option<GuardrailPolicy>,
    hardware_profile: &HardwareProfile,
    on_progress: Option<ProgressCallback>,
    on_chunk: Option<ChunkCallback<'_>>,
) -> Result<TranscriptSegmentsContainer, AppError> {
    let req_model = model
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .unwrap_or(DEFAULT_FASTER_WHISPER_MODEL);
    let req_quant = quantization
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .unwrap_or(DEFAULT_FASTER_WHISPER_QUANTIZATION);
    let req_device = device
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .unwrap_or(DEFAULT_FASTER_WHISPER_DEVICE);
    let policy = guardrail_policy.unwrap_or(GuardrailPolicy::AutoDowngrade);

    let decision = evaluate_hardware_guardrail_from_profile(
        hardware_profile,
        req_model,
        req_quant,
        req_device,
        policy,
    )?;

    if decision.was_downgraded {
        tracing::warn!(
            clip_id = %clip_id,
            requested_model = %req_model,
            requested_quantization = %req_quant,
            requested_device = %req_device,
            effective_model = %decision.effective_model,
            effective_quantization = %decision.effective_quantization,
            effective_device = %decision.effective_device,
            reason = ?decision.reason,
            "Faster-Whisper configuration downgraded by hardware guardrail"
        );
    }

    let config = FasterWhisperConfig {
        model: decision.effective_model,
        quantization: decision.effective_quantization,
        device: decision.effective_device,
        ..FasterWhisperConfig::default()
    };

    if let Some(ref cb) = on_progress {
        cb(SttProgressPayload {
            task_id: clip_id.to_string(),
            stage: "starting".to_string(),
            percent: 0.0,
            eta_sec: None,
            current_chunk: 1,
            total_chunks: 1,
        });
    }

    let container = run_faster_whisper_worker(clip_id, audio_path, &config, on_progress).await?;

    if let Some(chunk_cb) = on_chunk {
        for (idx, seg) in container.segments.iter().enumerate() {
            chunk_cb(idx, seg);
        }
    }

    Ok(container)
}

/// Trigger local Faster-Whisper (`CTranslate2`) speech-to-text transcription with hardware guardrail (`TASK-M4-16`).
///
/// Reads cached [`HardwareProfile`] from [`AppState`] (probing and caching if absent),
/// evaluates VRAM/CUDA guardrails via `evaluate_hardware_guardrail_from_profile`, runs
/// `run_faster_whisper_worker`, and emits `stt:progress` and `stt:chunk` events.
///
/// # Errors
/// Returns [`AppError::Internal`] on state lock poisoning, [`AppError::Hardware`] if hardware probe fails,
/// or [`AppError::Stt`] / [`AppError::Sidecar`] if guardrail rejects or worker execution fails.
#[command]
#[allow(clippy::too_many_arguments)]
pub async fn start_faster_whisper_transcription(
    clip_id: String,
    audio_path: String,
    model: Option<String>,
    quantization: Option<String>,
    device: Option<String>,
    guardrail_policy: Option<GuardrailPolicy>,
    state: tauri::State<'_, AppState>,
    app: tauri::AppHandle,
) -> Result<TranscriptSegmentsContainer, AppError> {
    let cached_profile = {
        let guard = state.hardware_profile.read().map_err(|e| {
            AppError::Internal(format!("Failed to acquire hardware profile read lock: {e}"))
        })?;
        guard.clone()
    };

    let hardware_profile = if let Some(profile) = cached_profile {
        profile
    } else {
        let probed = probe_system_hardware()?;
        let mut guard = state.hardware_profile.write().map_err(|e| {
            AppError::Internal(format!(
                "Failed to acquire hardware profile write lock: {e}"
            ))
        })?;
        *guard = Some(probed.clone());
        probed
    };

    let app_for_progress = app.clone();
    let on_progress: ProgressCallback = Arc::new(move |payload| {
        let _ = app_for_progress.emit("stt:progress", payload);
    });

    let app_for_chunk = app.clone();
    let clip_id_for_chunk = clip_id.clone();
    let emit_chunk = move |idx: usize, seg: &TranscriptSegment| {
        let _ = app_for_chunk.emit(
            "stt:chunk",
            serde_json::json!({
                "clip_id": clip_id_for_chunk,
                "chunk_index": idx,
                "segment": seg,
            }),
        );
    };

    start_faster_whisper_transcription_with_profile(
        &clip_id,
        Path::new(&audio_path),
        model.as_deref(),
        quantization.as_deref(),
        device.as_deref(),
        guardrail_policy,
        &hardware_profile,
        Some(on_progress),
        Some(&emit_chunk),
    )
    .await
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{CpuInfo, GpuInfo, GpuVendor, RamInfo, RecommendedMode};
    use crate::core::storage::test_env::{ScopedEnvVar, ENV_LOCK};
    use crate::stt::groq_client::{MockHttpHandler, TransportResponse};
    use std::sync::Mutex;

    fn mock_4gb_hardware_profile() -> HardwareProfile {
        HardwareProfile {
            cpu_info: CpuInfo {
                model: "Mock CPU".to_string(),
                cores: 8,
                threads: 16,
            },
            ram_info: RamInfo {
                total_mb: 16384,
                available_mb: 8192,
            },
            gpu_info: GpuInfo {
                vendor: GpuVendor::Nvidia,
                model: "GTX 1650".to_string(),
                vram_mb: 4096,
                cuda_supported: true,
            },
            recommended_mode: RecommendedMode::Hybrid,
            recommendation_reason: "Mock 4GB GPU".to_string(),
            timestamp: "2026-11-18T00:00:00Z".to_string(),
        }
    }

    #[tokio::test]
    async fn test_transcribe_highlight_clip_with_client_emits_typed_progress_and_chunks() {
        let mock_json = serde_json::json!({
            "task": "transcribe",
            "language": "english",
            "duration": 2.0,
            "text": "Hello chat",
            "segments": [{
                "id": 0, "seek": 0, "start": 0.0, "end": 2.0,
                "text": " Hello chat", "tokens": [],
                "temperature": 0.0, "avg_logprob": -0.1,
                "compression_ratio": 0.5, "no_speech_prob": 0.01
            }],
            "words": []
        })
        .to_string();

        let handler: MockHttpHandler =
            Arc::new(move |_url, _auth, _ct, _body| Ok(TransportResponse::ok(mock_json.clone())));
        let client = GroqClient::new("gsk_stt_cmd_test_12345".to_string())
            .unwrap()
            .with_mock_handler(handler);

        let progress_events: Arc<Mutex<Vec<SttProgressPayload>>> = Arc::new(Mutex::new(Vec::new()));
        let progress_cb_ref = Arc::clone(&progress_events);
        let on_progress: ProgressCallback = Arc::new(move |payload| {
            progress_cb_ref.lock().unwrap().push(payload);
        });

        let chunks: Arc<Mutex<Vec<(usize, String)>>> = Arc::new(Mutex::new(Vec::new()));
        let chunks_ref = Arc::clone(&chunks);
        let on_chunk = move |idx: usize, seg: &TranscriptSegment| {
            chunks_ref.lock().unwrap().push((idx, seg.text_en.clone()));
        };

        let samples = vec![0.01f32; 16_000];
        let res = transcribe_highlight_clip_with_client(
            "clip_cmd_01",
            &samples,
            16_000,
            false,
            &client,
            Some(on_progress),
            Some(&on_chunk),
        )
        .await
        .unwrap();

        assert_eq!(res.clip_id, "clip_cmd_01");
        assert_eq!(res.segments.len(), 1);

        let captured_progress = progress_events.lock().unwrap().clone();
        assert_eq!(captured_progress.len(), 3);
        assert_eq!(captured_progress[0].task_id, "clip_cmd_01");
        assert_eq!(captured_progress[0].stage, "preparing");
        assert_eq!(captured_progress[1].stage, "transcribing");
        assert_eq!(captured_progress[2].stage, "completed");
        assert_eq!(captured_progress[2].current_chunk, 1);
        assert_eq!(captured_progress[2].total_chunks, 1);
        assert!((captured_progress[2].percent - 100.0).abs() < 1e-6);

        let serialized = serde_json::to_value(&captured_progress[0]).unwrap();
        assert_eq!(serialized["taskId"], "clip_cmd_01");
        assert_eq!(serialized["stage"], "preparing");
        assert!(serialized.get("clip_id").is_none());

        let captured_chunks = chunks.lock().unwrap().clone();
        assert_eq!(captured_chunks, vec![(0, "Hello chat".to_string())]);
    }

    #[tokio::test]
    async fn test_start_faster_whisper_with_profile_guardrail_and_events() {
        let _env_guard = ENV_LOCK.lock().await;
        let low_vram_profile = HardwareProfile {
            gpu_info: GpuInfo {
                vendor: GpuVendor::Nvidia,
                model: "GTX 1650 Low".to_string(),
                vram_mb: 2048,
                cuda_supported: true,
            },
            ..mock_4gb_hardware_profile()
        };

        // 1. StrictReject on Large with 2048MB VRAM must fail before running worker
        let reject_res = start_faster_whisper_transcription_with_profile(
            "clip_fw_reject",
            Path::new("nonexistent.wav"),
            Some("large-v3"),
            Some("int8"),
            Some("cuda"),
            Some(GuardrailPolicy::StrictReject),
            &low_vram_profile,
            None,
            None,
        )
        .await;
        assert!(matches!(
            reject_res,
            Err(AppError::Stt(SttError::InsufficientVram {
                available_mb: 2048
            }))
        ));

        // 2. AutoDowngrade with mock worker output emits starting -> transcribing -> completed & chunks
        let temp_json =
            std::env::temp_dir().join(format!("fw_cmd_mock_{}.json", std::process::id()));
        let mock_payload = serde_json::json!({
            "text": "Vedal stop coding",
            "segments": [{
                "id": 0,
                "start": 0.0,
                "end": 1.8,
                "text": " Vedal stop coding",
                "avg_logprob": -0.1,
                "words": []
            }],
            "words": []
        })
        .to_string();
        std::fs::write(&temp_json, mock_payload).unwrap();
        let _mock_env = ScopedEnvVar::set(
            "NEUROCLIP_MOCK_FASTER_WHISPER_OUTPUT",
            &temp_json.to_string_lossy(),
        );

        let progress_events: Arc<Mutex<Vec<SttProgressPayload>>> = Arc::new(Mutex::new(Vec::new()));
        let progress_ref = Arc::clone(&progress_events);
        let on_progress: ProgressCallback = Arc::new(move |payload| {
            progress_ref.lock().unwrap().push(payload);
        });

        let chunks: Arc<Mutex<Vec<(usize, String)>>> = Arc::new(Mutex::new(Vec::new()));
        let chunks_ref = Arc::clone(&chunks);
        let on_chunk = move |idx: usize, seg: &TranscriptSegment| {
            chunks_ref.lock().unwrap().push((idx, seg.text_en.clone()));
        };

        let ok_res = start_faster_whisper_transcription_with_profile(
            "clip_fw_ok",
            Path::new("unused_in_mock.wav"),
            Some("large-v3"),
            Some("float16"),
            Some("auto"),
            Some(GuardrailPolicy::AutoDowngrade),
            &low_vram_profile,
            Some(on_progress),
            Some(&on_chunk),
        )
        .await;

        let _ = std::fs::remove_file(&temp_json);
        let container = ok_res.expect("AutoDowngrade + mock worker must succeed");
        assert_eq!(container.clip_id, "clip_fw_ok");
        assert_eq!(container.segments.len(), 1);

        let captured_progress = progress_events.lock().unwrap().clone();
        assert_eq!(captured_progress.len(), 3);
        assert_eq!(captured_progress[0].stage, "starting");
        assert_eq!(captured_progress[1].stage, "transcribing");
        assert_eq!(captured_progress[2].stage, "completed");
        assert!((captured_progress[2].percent - 100.0).abs() < 1e-6);

        let captured_chunks = chunks.lock().unwrap().clone();
        assert_eq!(captured_chunks, vec![(0, "Vedal stop coding".to_string())]);
    }
}
