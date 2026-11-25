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
        .or_else(|| {
            crate::core::keyring::KeyringManager::new(crate::core::keyring::SERVICE_NAME)
                .get_api_key("gemini")
                .ok()
                .flatten()
                .filter(|k| !k.trim().is_empty())
        })
        .or_else(|| std::env::var("GEMINI_API_KEY").ok())
        .or_else(|| std::env::var("NEUROCLIP_GEMINI_API_KEY").ok())
        .ok_or(AppError::Translation(TranslationError::InvalidApiKey))?;

    let mut config = GeminiClientConfig::new(resolved_key);
    if let Some(m) = model.filter(|m| !m.trim().is_empty()) {
        if m.trim() == "gemini-1.5-flash" {
            config.model = DEFAULT_GEMINI_MODEL.to_string();
        } else {
            config.model = m;
        }
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

/// Wrap Vietnamese subtitle text across all segments in a container, preserving compound words.
#[command]
pub async fn wrap_vietnamese_subtitles(
    mut container: TranscriptSegmentsContainer,
    max_chars_per_line: Option<usize>,
    max_lines: Option<usize>,
) -> Result<TranscriptSegmentsContainer, AppError> {
    let max_chars = max_chars_per_line
        .filter(|&c| c > 0)
        .unwrap_or(crate::translation::line_wrapper::DEFAULT_MAX_CHARS_PER_LINE);
    let max_l = max_lines
        .filter(|&l| l > 0)
        .unwrap_or(crate::translation::line_wrapper::DEFAULT_MAX_LINES);

    for seg in &mut container.segments {
        if let Some(ref text_vi) = seg.text_vi {
            if !text_vi.trim().is_empty()
                && !text_vi.starts_with(crate::translation::SAFETY_FALLBACK_PREFIX)
                && !text_vi.starts_with(crate::translation::FORMAT_FALLBACK_PREFIX)
            {
                seg.text_vi = Some(
                    crate::translation::line_wrapper::wrap_vietnamese_text_to_string(
                        text_vi, max_chars, max_l,
                    ),
                );
            }
        }
    }

    Ok(container)
}

/// Optimize reading speed (CPS) across transcript segments using Time Borrowing.
#[command]
pub async fn optimize_transcript_cps(
    mut container: TranscriptSegmentsContainer,
    clip_duration: Option<f64>,
    max_borrow_ms: Option<u64>,
    min_gap_ms: Option<u64>,
    target_cps: Option<f64>,
) -> Result<TranscriptSegmentsContainer, AppError> {
    let config = crate::translation::cps_optimizer::TimeBorrowingConfig {
        max_borrow_ms: max_borrow_ms
            .unwrap_or(crate::translation::cps_optimizer::DEFAULT_MAX_BORROW_MS),
        min_gap_ms: min_gap_ms.unwrap_or(crate::translation::cps_optimizer::DEFAULT_MIN_GAP_MS),
        target_cps: target_cps.unwrap_or(crate::translation::cps_optimizer::DEFAULT_TARGET_CPS),
        trigger_cps: crate::translation::cps_optimizer::CPS_RED_THRESHOLD,
        max_clip_duration_sec: clip_duration,
    };

    crate::translation::cps_optimizer::optimize_container_cps(&mut container, &config);

    Ok(container)
}
