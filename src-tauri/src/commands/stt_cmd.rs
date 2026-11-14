//! Tauri IPC command handlers for STT transcription and subtitle generation.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use tauri::{command, Emitter};

use crate::error::{AppError, ExportError};
use crate::stt::{
    generate_ass_script, generate_srt_script, run_whisper_transcription, verify_model, ModelInfo,
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
