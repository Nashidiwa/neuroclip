//! Tauri IPC command handlers for STT transcription and subtitle generation.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use tauri::{command, Emitter};

use crate::error::{AppError, ExportError, SttError};
use crate::stt::{
    generate_ass_script, generate_srt_script, prepare_audio_to_buffer, run_whisper_transcription,
    verify_model, AudioPrepConfig, GroqClient, GroqClientConfig, ModelInfo, PreparedAudioBuffer,
    TranscriptSegmentsContainer, WhisperSidecarConfig,
};

/// Trigger local Whisper.cpp speech-to-text transcription on an audio file.
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
    }) as Arc<dyn Fn(crate::stt::SttProgressPayload) + Send + Sync>);

    run_whisper_transcription(&clip_id, &audio_file_path, &config, on_progress).await
}

/// Verify the cryptographic integrity of a local Whisper Q5_K model.
#[command]
pub async fn verify_stt_model(
    model_name: Option<String>,
) -> Result<ModelInfo, AppError> {
    let target = model_name.unwrap_or_else(|| "base.en-q5_k".to_string());
    verify_model(&target, None)
}

/// Export transcript segments into an Advanced SubStation Alpha (.ass) subtitle file.
#[command]
pub async fn export_subtitles_ass(
    container: TranscriptSegmentsContainer,
    output_path: String,
) -> Result<String, AppError> {
    let ass_content = generate_ass_script(&container);
    let path = Path::new(&output_path);

    if let Some(parent) = path.parent() {
        if !parent.exists() {
            std::fs::create_dir_all(parent).map_err(|e| {
                AppError::Export(ExportError::FileWriteFailed {
                    path: parent.display().to_string(),
                    details: e.to_string(),
                })
            })?;
        }
    }

    std::fs::write(path, ass_content).map_err(|e| {
        AppError::Export(ExportError::FileWriteFailed {
            path: output_path.clone(),
            details: e.to_string(),
        })
    })?;

    Ok(output_path)
}

/// Export transcript segments into a standard SubRip (.srt) subtitle file.
#[command]
pub async fn export_subtitles_srt(
    container: TranscriptSegmentsContainer,
    output_path: String,
) -> Result<String, AppError> {
    let srt_content = generate_srt_script(&container);
    let path = Path::new(&output_path);

    if let Some(parent) = path.parent() {
        if !parent.exists() {
            std::fs::create_dir_all(parent).map_err(|e| {
                AppError::Export(ExportError::FileWriteFailed {
                    path: parent.display().to_string(),
                    details: e.to_string(),
                })
            })?;
        }
    }

    std::fs::write(path, srt_content).map_err(|e| {
        AppError::Export(ExportError::FileWriteFailed {
            path: output_path.clone(),
            details: e.to_string(),
        })
    })?;

    Ok(output_path)
}

/// Transcribe a highlight clip using Groq Whisper Cloud API (`TASK-M4-15`).
///
/// Supports automatic chunking <= 20MB with Silero VAD pause split and 1.5s lookback overlap,
/// Karaoke anti-repetition guardrail, and exponential backoff retry on HTTP 429.
/// Emits `stt:progress` and `stt:chunk` events across the Tauri event bridge.
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

    // Emit initial progress
    let _ = app.emit(
        "stt:progress",
        serde_json::json!({
            "clip_id": clip_id,
            "current_chunk": 0,
            "total_chunks": 1,
            "percent": 0.0,
        }),
    );

    // Perform transcription with chunking and overlap deduplication
    let result = client
        .transcribe_samples(
            &clip_id,
            &audio_buf.samples_f32,
            audio_buf.sample_rate as usize,
            is_music_mode,
        )
        .await?;

    // Emit per-segment events and final progress
    for (idx, seg) in result.segments.iter().enumerate() {
        let _ = app.emit(
            "stt:chunk",
            serde_json::json!({
                "clip_id": clip_id,
                "chunk_index": idx,
                "segment": seg,
            }),
        );
    }

    let _ = app.emit(
        "stt:progress",
        serde_json::json!({
            "clip_id": clip_id,
            "current_chunk": 1,
            "total_chunks": 1,
            "percent": 100.0,
        }),
    );

    Ok(result)
}
