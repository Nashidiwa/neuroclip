//! Tauri IPC command handlers for subtitle translation and localization (`TASK-M4-17`, `M4.5`).

use std::sync::Arc;
use tauri::{command, Emitter};

use crate::error::{AppError, TranslationError};
use crate::stt::transcript_parser::TranscriptSegmentsContainer;
use crate::translation::{
    GeminiClient, GeminiClientConfig, TranslationProgressPayload, DEFAULT_GEMINI_MODEL,
};

/// Translate English transcript segments into natural Vietnamese subtitles using Google Gemini 1.5 Flash.
///
/// Features:
/// - Theatrical Framing System Prompt packaging (`PRD.md` §6.3).
/// - Outgoing `BLOCK_NONE` safety threshold settings across all 4 categories.
/// - Defensive Safety Fallback (`ERR-07` / `ED-17.1`): unhinged lines are preserved in English
///   with `is_flagged_for_review: true` and prefixed with warning label.
/// - Sliding context window (5 lines) across 25–35 line chunks.
/// - Periodic progress emission to frontend via `translation:progress` event.
#[command]
pub async fn translate_vietsub(
    clip_id: String,
    container: TranscriptSegmentsContainer,
    api_key: Option<String>,
    model: Option<String>,
    style_preset: Option<String>,
    cast_members: Option<Vec<String>>,
    app: tauri::AppHandle,
) -> Result<TranscriptSegmentsContainer, AppError> {
    let resolved_key = api_key
        .filter(|k| !k.trim().is_empty())
        .or_else(|| std::env::var("GEMINI_API_KEY").ok())
        .or_else(|| std::env::var("NEUROCLIP_GEMINI_API_KEY").ok())
        .ok_or(AppError::Translation(TranslationError::InvalidApiKey))?;

    let mut config = GeminiClientConfig::new(resolved_key);
    if let Some(m) = model.filter(|m| !m.trim().is_empty()) {
        config.model = m;
    } else {
        config.model = DEFAULT_GEMINI_MODEL.to_string();
    }
    if let Some(s) = style_preset.filter(|s| !s.trim().is_empty()) {
        config.style_preset = Some(s);
    }
    if let Some(cast) = cast_members {
        config.cast_members = Some(cast);
    }

    let client = GeminiClient::from_config(config)?;

    let app_handle = app.clone();
    let clip_id_owned = clip_id.clone();
    let on_progress = Some(
        Arc::new(move |translated_count: usize, total_count: usize| {
            let payload = TranslationProgressPayload {
                clip_id: clip_id_owned.clone(),
                translated_count,
                total_count,
            };
            let _ = app_handle.emit("translation:progress", payload);
        }) as Arc<dyn Fn(usize, usize) + Send + Sync>,
    );

    client
        .translate_segments(&clip_id, &container.segments, on_progress)
        .await
}

/// Verify Google Gemini API credentials with a lightweight ping prompt.
#[command]
pub async fn test_gemini_connection(api_key: String) -> Result<bool, AppError> {
    if api_key.trim().is_empty() {
        return Err(AppError::Translation(TranslationError::InvalidApiKey));
    }

    let client = GeminiClient::new(api_key.trim().to_string())?;
    client.test_connection().await
}

/// Update speaker role for a specific transcript segment (`CONVENTIONS.md` §2.5).
#[command]
pub async fn update_speaker_role(
    mut container: TranscriptSegmentsContainer,
    segment_id: String,
    new_speaker: String,
) -> Result<TranscriptSegmentsContainer, AppError> {
    let normalized = crate::translation::diarization::normalize_speaker_id(&new_speaker, &[]);
    for seg in &mut container.segments {
        if seg.segment_id == segment_id {
            seg.speaker_id = normalized.to_string();
        }
    }
    Ok(container)
}
