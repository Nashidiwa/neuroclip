//! Zero-panic transcript parser and Schema 5.5 normalizer.
//!
//! Parses Whisper.cpp `--output-json-full` output and streaming stdout lines into
//! strongly typed, monotonic, validated [`TranscriptSegmentsContainer`] models.

use serde::{Deserialize, Serialize};
use crate::error::{AppError, SttError};

/// Speaker identifier for VTuber highlight streams.
pub const DEFAULT_SPEAKER_ID: &str = "unknown";

/// Characters-per-second warning threshold for subtitle readability.
pub const CPS_FLAG_THRESHOLD: f64 = 20.0;

/// Minimum confidence score threshold below which a segment is flagged for review.
pub const CONFIDENCE_FLAG_THRESHOLD: f64 = 0.6;

/// Minimum segment duration in seconds to avoid divide-by-zero in CPS calculations.
pub const MIN_SEGMENT_DURATION_SEC: f64 = 0.05;

/// Word-level timing entry adhering to Schema 5.5 (`transcript_segments.json`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct WordTiming {
    pub word: String,
    pub start_sec: f64,
    pub end_sec: f64,
    #[serde(alias = "confidence")]
    pub probability: f64,
}

/// Normalized transcript segment adhering to Schema 5.5.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct TranscriptSegment {
    pub segment_id: String,
    pub start_sec: f64,
    pub end_sec: f64,
    pub speaker_id: String,
    pub text_en: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub text_vi: Option<String>,
    pub confidence: f64,
    pub cps_score: f64,
    #[serde(default)]
    pub is_flagged_for_review: bool,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub words: Vec<WordTiming>,
}

/// Container object adhering to Schema 5.5 (`transcript_segments.json`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct TranscriptSegmentsContainer {
    pub clip_id: String,
    pub segments: Vec<TranscriptSegment>,
}

// Internal raw JSON structures emitted by Whisper.cpp `--output-json-full`
#[derive(Debug, Deserialize)]
struct WhisperOutputFull {
    #[serde(default)]
    transcription: Vec<WhisperTranscriptionItem>,
    #[serde(default)]
    segments: Vec<WhisperTranscriptionItem>,
}

#[derive(Debug, Deserialize)]
struct WhisperTranscriptionItem {
    #[serde(default)]
    timestamps: Option<WhisperTimestamps>,
    #[serde(default)]
    offsets: Option<WhisperOffsets>,
    #[serde(default)]
    text: Option<String>,
    #[serde(default)]
    tokens: Vec<WhisperTokenItem>,
}

#[derive(Debug, Deserialize)]
struct WhisperTimestamps {
    #[serde(default)]
    from: Option<String>,
    #[serde(default)]
    to: Option<String>,
}

#[derive(Debug, Deserialize)]
struct WhisperOffsets {
    #[serde(default)]
    from: Option<i64>,
    #[serde(default)]
    to: Option<i64>,
}

#[derive(Debug, Deserialize)]
struct WhisperTokenItem {
    #[serde(default)]
    text: Option<String>,
    #[serde(default)]
    timestamps: Option<WhisperTimestamps>,
    #[serde(default)]
    offsets: Option<WhisperOffsets>,
    #[serde(default)]
    p: Option<f64>,
}

/// Parse timestamp string in format `HH:MM:SS,mmm`, `HH:MM:SS.mmm`, `MM:SS,mmm`, or `MM:SS.mmm` into seconds.
pub fn parse_timestamp_to_seconds(ts: &str) -> Option<f64> {
    let clean = ts.trim();
    if clean.is_empty() {
        return None;
    }

    let parts: Vec<&str> = clean.split(':').collect();
    let (hours, mins, secs_str) = match parts.len() {
        3 => {
            let h = parts.first()?.parse::<f64>().ok()?;
            let m = parts.get(1)?.parse::<f64>().ok()?;
            let s = *parts.get(2)?;
            (h, m, s)
        }
        2 => {
            let m = parts.first()?.parse::<f64>().ok()?;
            let s = *parts.get(1)?;
            (0.0, m, s)
        }
        1 => {
            let s = *parts.first()?;
            (0.0, 0.0, s)
        }
        _ => return None,
    };

    let normalized_secs = secs_str.replace(',', ".");
    let seconds_fraction = normalized_secs.parse::<f64>().ok()?;

    let total = hours * 3600.0 + mins * 60.0 + seconds_fraction;
    if total.is_finite() && total >= 0.0 {
        Some(total)
    } else {
        Some(0.0)
    }
}

/// Sanitize floating point number: ensure it is finite and non-negative.
fn sanitize_float(val: f64) -> f64 {
    if val.is_finite() && val >= 0.0 {
        val
    } else {
        0.0
    }
}

/// Compute characters per second (CPS) metric.
pub fn calculate_cps(text: &str, start_sec: f64, end_sec: f64) -> f64 {
    let clean_len = text.trim().chars().count() as f64;
    let duration = (end_sec - start_sec).max(MIN_SEGMENT_DURATION_SEC);
    let cps = clean_len / duration;
    if cps.is_finite() && cps >= 0.0 {
        cps
    } else {
        0.0
    }
}

/// Determine whether a token represents an internal Whisper special token.
pub fn is_special_token(token: &str) -> bool {
    let trimmed = token.trim();
    if trimmed.is_empty() {
        return false;
    }
    (trimmed.starts_with("<|") && trimmed.ends_with("|>"))
        || (trimmed.starts_with('[') && trimmed.ends_with(']'))
        || trimmed == "[_BEG_]"
        || trimmed == "[BLANK_AUDIO]"
}

/// Parse Whisper.cpp `--output-json-full` output into a validated [`TranscriptSegmentsContainer`].
///
/// Handles missing fields, NaN timestamps, subword token aggregation, and flags lines
/// with high CPS (> 20.0) or low confidence (< 0.6).
pub fn parse_whisper_json_full(
    json_str: &str,
    clip_id: &str,
) -> Result<TranscriptSegmentsContainer, AppError> {
    let trimmed = json_str.trim();
    if trimmed.is_empty() {
        return Ok(TranscriptSegmentsContainer {
            clip_id: clip_id.to_string(),
            segments: Vec::new(),
        });
    }

    // Direct deserialization check if already in Schema 5.5 container format
    if let Ok(container) = serde_json::from_str::<TranscriptSegmentsContainer>(trimmed) {
        return Ok(container);
    }

    // Attempt parsing Whisper full output
    let parsed: WhisperOutputFull = serde_json::from_str(trimmed).map_err(|e| {
        AppError::Stt(SttError::ParserFailed(format!(
            "Invalid Whisper JSON structure: {e}"
        )))
    })?;

    let items = if !parsed.transcription.is_empty() {
        parsed.transcription
    } else {
        parsed.segments
    };

    let mut segments = Vec::new();
    let mut segment_counter = 1;

    for item in items {
        let text_raw = item.text.unwrap_or_default();
        let clean_text = text_raw.trim().to_string();

        if clean_text.is_empty() || is_special_token(&clean_text) {
            continue;
        }

        // Resolve start/end timestamps from offsets (ms) or string timestamps
        let (raw_start, raw_end) = match (&item.offsets, &item.timestamps) {
            (Some(offsets), _) if offsets.from.is_some() || offsets.to.is_some() => {
                let s = offsets.from.map_or(0.0, |f| f as f64 / 1000.0);
                let e = offsets.to.map_or(s, |t| t as f64 / 1000.0);
                (s, e)
            }
            (_, Some(ts)) => {
                let s = ts.from.as_deref().and_then(parse_timestamp_to_seconds).unwrap_or(0.0);
                let e = ts.to.as_deref().and_then(parse_timestamp_to_seconds).unwrap_or(s);
                (s, e)
            }
            _ => (0.0, 0.0),
        };

        let start_sec = sanitize_float(raw_start);
        let end_sec = sanitize_float(raw_end).max(start_sec);

        // Process token level timings
        let mut words: Vec<WordTiming> = Vec::new();
        let mut token_probabilities = Vec::new();

        for token in item.tokens {
            let token_text = token.text.unwrap_or_default();
            if token_text.trim().is_empty() || is_special_token(&token_text) {
                continue;
            }

            let prob = token.p.unwrap_or(1.0);
            token_probabilities.push(prob);

            let (t_start, t_end) = match (&token.offsets, &token.timestamps) {
                (Some(offsets), _) if offsets.from.is_some() || offsets.to.is_some() => {
                    let s = offsets.from.map_or(start_sec, |f| f as f64 / 1000.0);
                    let e = offsets.to.map_or(s, |t| t as f64 / 1000.0);
                    (s, e)
                }
                (_, Some(ts)) => {
                    let s = ts.from.as_deref().and_then(parse_timestamp_to_seconds).unwrap_or(start_sec);
                    let e = ts.to.as_deref().and_then(parse_timestamp_to_seconds).unwrap_or(s);
                    (s, e)
                }
                _ => (start_sec, end_sec),
            };

            let safe_token_start = sanitize_float(t_start).max(start_sec);
            let safe_token_end = sanitize_float(t_end).max(safe_token_start).min(end_sec.max(safe_token_start));

            // Token aggregation into words: if token has leading whitespace or words is empty, start new word
            if token_text.starts_with(' ') || words.is_empty() {
                words.push(WordTiming {
                    word: token_text.trim().to_string(),
                    start_sec: safe_token_start,
                    end_sec: safe_token_end,
                    probability: prob.clamp(0.0, 1.0),
                });
            } else if let Some(last_word) = words.last_mut() {
                // Subword continuation
                last_word.word.push_str(token_text.trim());
                last_word.end_sec = safe_token_end.max(last_word.end_sec);
                last_word.probability = (last_word.probability + prob.clamp(0.0, 1.0)) / 2.0;
            }
        }

        // If no word tokens were provided by whisper, synthesize words by splitting text
        if words.is_empty() && !clean_text.is_empty() {
            let split_words: Vec<&str> = clean_text.split_whitespace().collect();
            let word_count = split_words.len();
            if word_count > 0 {
                let duration = (end_sec - start_sec).max(MIN_SEGMENT_DURATION_SEC);
                let step = duration / word_count as f64;
                for (idx, word_str) in split_words.into_iter().enumerate() {
                    let w_start = start_sec + idx as f64 * step;
                    let w_end = if idx == word_count - 1 {
                        end_sec
                    } else {
                        w_start + step
                    };
                    words.push(WordTiming {
                        word: word_str.to_string(),
                        start_sec: w_start,
                        end_sec: w_end.max(w_start),
                        probability: 1.0,
                    });
                }
            }
        }

        // Compute overall segment confidence
        let confidence = if !token_probabilities.is_empty() {
            let sum: f64 = token_probabilities.iter().sum();
            (sum / token_probabilities.len() as f64).clamp(0.0, 1.0)
        } else {
            1.0
        };

        let cps_score = calculate_cps(&clean_text, start_sec, end_sec);
        let is_flagged_for_review = confidence < CONFIDENCE_FLAG_THRESHOLD || cps_score > CPS_FLAG_THRESHOLD;

        segments.push(TranscriptSegment {
            segment_id: format!("seg_{segment_counter:04}"),
            start_sec,
            end_sec,
            speaker_id: DEFAULT_SPEAKER_ID.to_string(),
            text_en: clean_text,
            text_vi: None,
            confidence,
            cps_score,
            is_flagged_for_review,
            words,
        });

        segment_counter += 1;
    }

    Ok(TranscriptSegmentsContainer {
        clip_id: clip_id.to_string(),
        segments,
    })
}

/// Fallback parser for single Whisper stdout lines.
///
/// Matches patterns like `[00:00:01.000 --> 00:00:04.500]   Hello world`
/// and safely returns `Some((start_sec, end_sec, text))`.
pub fn parse_whisper_stdout_line(line: &str) -> Option<(f64, f64, String)> {
    let trimmed = line.trim();
    if !trimmed.starts_with('[') {
        return None;
    }

    let closing_bracket = trimmed.find(']')?;
    let timestamp_part = &trimmed[1..closing_bracket];
    let text_part = trimmed[closing_bracket + 1..].trim();

    let arrow_pos = timestamp_part.find("-->")?;
    let from_str = timestamp_part[..arrow_pos].trim();
    let to_str = timestamp_part[arrow_pos + 3..].trim();

    let start_sec = parse_timestamp_to_seconds(from_str)?;
    let end_sec = parse_timestamp_to_seconds(to_str)?.max(start_sec);

    if text_part.is_empty() || is_special_token(text_part) {
        return None;
    }

    Some((start_sec, end_sec, text_part.to_string()))
}
